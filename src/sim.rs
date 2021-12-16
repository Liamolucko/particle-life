use glam::vec2;
use glam::Vec2;
use palette::FromColor;
use palette::Hsv;
use palette::LinSrgb;
use rand::Rng;
use rand_distr::Distribution;
use rand_distr::Normal;
use rand_distr::Uniform;

use crate::settings::Settings;
use crate::GpuParticle;
use crate::MAX_PARTICLES;

pub const RADIUS: f32 = 5.0;
pub const DIAMETER: f32 = RADIUS * 2.0;
pub const R_SMOOTH: f32 = 2.0;

#[derive(Clone, Copy, Debug)]
pub struct SymmetricProperties {
    /// The distance below which particles begin to unconditionally repel each other.
    pub repel_distance: f32,
    /// The distance above which particles have no influence on each other.
    pub influence_radius: f32,
    /// The distance above which particles have no influence on each other, squared.
    pub influence_radius_sq: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Particle {
    // This is stored in clip space, so that we can just send it directly to the GPU and it doesn't require any extra work on resize.
    pub pos: Vec2,
    pub vel: Vec2,
    pub kind: usize,
}

impl Particle {
    pub fn generate<R: Rng>(num_kinds: usize, rng: &mut R) -> Self {
        let kinds = Uniform::new(0, num_kinds);
        // This is in clip space, so it ranges from -1 to 1.
        let pos_dist = Uniform::new(-0.5, 0.5);
        let vel_dist = Normal::new(0.0, 0.2).unwrap();

        Self {
            kind: kinds.sample(rng),
            pos: vec2(pos_dist.sample(rng), pos_dist.sample(rng)),
            vel: vec2(vel_dist.sample(rng), vel_dist.sample(rng)),
        }
    }
}

/// The state required for the simulation of the particles.
pub struct Sim {
    pub wrap: bool,
    pub flat_force: bool,
    pub friction: f32,

    pub colors: Vec<LinSrgb>,
    pub symmetric_props: Vec<SymmetricProperties>,
    pub attractions: Vec<f32>,

    pub particles: Vec<Particle>,
}

impl Sim {
    pub fn new<R: Rng>(settings: Settings, rng: &mut R) -> Self {
        let mut colors = Vec::with_capacity(settings.kinds);
        let mut symmetric_props = Vec::with_capacity(settings.kinds * (settings.kinds + 1) / 2);
        let mut attractions = Vec::with_capacity(settings.kinds * settings.kinds);

        // The angle between each color's hue.
        let angle = 360.0 / settings.kinds as f32;

        for i in 0..settings.kinds {
            let value = if i % 2 == 0 { 0.5 } else { 1.0 };
            let color = Hsv::with_wp(angle * i as f32, 1.0, value);
            colors.push(LinSrgb::from_color(color));

            for j in 0..settings.kinds {
                attractions.push(if i == j {
                    -f32::abs(settings.attraction_distr.sample(rng))
                } else {
                    settings.attraction_distr.sample(rng)
                });

                if j <= i {
                    let repel_distance = if i == j {
                        DIAMETER
                    } else {
                        f32::max(settings.repel_distance_distr.sample(rng), DIAMETER)
                    };

                    let mut influence_radius = settings.influence_radius_distr.sample(rng);
                    if influence_radius < repel_distance {
                        influence_radius = repel_distance;
                    }

                    symmetric_props.push(SymmetricProperties {
                        repel_distance,
                        influence_radius,
                        influence_radius_sq: influence_radius * influence_radius,
                    });
                }
            }
        }

        Self {
            wrap: false,
            flat_force: settings.flat_force,
            friction: settings.friction,

            colors,
            symmetric_props,
            attractions,

            particles: (0..settings.particles)
                .map(|_| Particle::generate(settings.kinds, rng))
                .collect(),
        }
    }

    pub fn regenerate_particles<R: Rng>(&mut self, rng: &mut R) {
        for particle in self.particles.iter_mut() {
            *particle = Particle::generate(self.colors.len(), rng);
        }
    }

    pub fn step(&mut self, width: f32, height: f32) {
        for i in 0..self.particles.len() {
            for j in 0..i {
                // We can't just use `iter` for these because the iterator would borrow `particles`, preventing us from mutating anything.
                let p = &self.particles[i];
                let q = &self.particles[j];

                let mut delta = q.pos - p.pos;

                if self.wrap {
                    if delta.x > 1.0 {
                        delta.x -= 2.0;
                    } else if delta.x < -1.0 {
                        delta.x += 2.0;
                    }

                    if delta.y > 1.0 {
                        delta.y -= 2.0;
                    } else if delta.y < -1.0 {
                        delta.y += 2.0;
                    }
                }

                // The positions are in clip space, but velocities are in pixel space, so we need to scale these up.
                delta.x *= 0.5 * width;
                delta.y *= 0.5 * height;

                let dist2 = delta.length_squared();

                let index = p.kind * (p.kind + 1) / 2 + q.kind;
                let SymmetricProperties {
                    repel_distance,
                    influence_radius,
                    influence_radius_sq,
                } = self.symmetric_props[index];

                // Disallow small distances to avoid division by zero, since we divide by this to normalize the vector later on.
                if dist2 < 0.01 || dist2 > influence_radius_sq {
                    continue;
                }

                let dist = dist2.sqrt();

                let (f1, f2) = if dist < repel_distance {
                    let f = R_SMOOTH
                        * repel_distance
                        * (1.0 / (repel_distance + R_SMOOTH) - 1.0 / (dist + R_SMOOTH));
                    (f, f)
                } else {
                    let mut f1 = self.attractions[p.kind * self.colors.len() + q.kind];
                    let mut f2 = self.attractions[q.kind * self.colors.len() + p.kind];

                    if !self.flat_force {
                        let peak = 0.5 * (repel_distance + influence_radius);
                        let base = 0.5 * (influence_radius - repel_distance);
                        let coefficient = 1.0 - (f32::abs(dist - peak) / base);

                        f1 *= coefficient;
                        f2 *= coefficient;
                    }

                    (f1, f2)
                };

                let direction = delta / dist;

                self.particles[i].vel += f1 * direction;
                self.particles[j].vel += f2 * -direction;
            }
        }

        // Figure out the width/height of the particles in clip space.
        let clip_width = 2.0 * RADIUS / width;
        let clip_height = 2.0 * RADIUS / height;

        for p in self.particles.iter_mut() {
            p.pos += vec2(2.0 * p.vel.x / width, 2.0 * p.vel.y / height);
            p.vel *= 1.0 - self.friction;

            if self.wrap {
                if p.pos.x > 1.0 {
                    p.pos.x -= 2.0;
                } else if p.pos.x < -1.0 {
                    p.pos.x += 2.0;
                }

                if p.pos.y > 1.0 {
                    p.pos.y -= 2.0;
                } else if p.pos.y < -1.0 {
                    p.pos.y += 2.0;
                }
            } else {
                if p.pos.x + clip_width > 1.0 {
                    p.pos.x = 1.0 - clip_width;
                    p.vel.x *= -1.0;
                } else if p.pos.x - clip_width < -1.0 {
                    p.pos.x = -1.0 + clip_width;
                    p.vel.x *= -1.0;
                }

                if p.pos.y + clip_height > 1.0 {
                    p.pos.y = 1.0 - clip_height;
                    p.vel.y *= -1.0;
                } else if p.pos.y - clip_height < -1.0 {
                    p.pos.y = -1.0 + clip_height;
                    p.vel.y *= -1.0;
                }
            }
        }
    }

    /// Convert the current state of the particles into the representation used by the GPU.
    pub fn export_particles(&self) -> [GpuParticle; MAX_PARTICLES] {
        let mut out = [GpuParticle::default(); MAX_PARTICLES];
        for (i, particle) in self.particles.iter().enumerate() {
            out[i] = GpuParticle {
                pos: particle.pos,
                color: self.colors[particle.kind],
            };
        }
        out
    }
}
