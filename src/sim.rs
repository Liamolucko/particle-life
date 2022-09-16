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

/// The properties between a pair of particle kinds.
/// Everything apart from `attraction` is the same in both directions.
#[derive(Clone, Copy, Debug)]
pub struct PairProps {
    /// The peak attraction between two particles.
    pub attraction: f32,
    /// The distance below which particles begin to unconditionally repel each
    /// other.
    pub repel_distance: f32,
    /// The distance above which particles have no influence on each other.
    ///
    /// This isn't actually used by `step` but is useful during construction.
    pub influence_radius: f32,

    // Stuff which is just computed ahead-of-time to improve performance.
    /// The distance above which particles have no influence on each other,
    /// squared.
    pub influence_radius_sq: f32,
    /// The point of maximum force, halfway between `repel_distance` and
    /// `influence_radius`.
    pub peak: f32,
    /// The reciprocal of the distance between
    /// `repel_distance`/`influence_radius` and `peak`.
    pub inv_base: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Particle {
    // This is stored in clip space, so that we can just send it directly to the GPU and it doesn't
    // require any extra work on resize.
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
    pub pair_props: Vec<PairProps>,

    pub particles: Vec<Particle>,
}

impl Sim {
    pub fn new<R: Rng>(settings: Settings, rng: &mut R) -> Self {
        let mut colors = Vec::with_capacity(settings.kinds);
        let mut pair_props: Vec<PairProps> = Vec::with_capacity(settings.kinds * settings.kinds);

        // The angle between each color's hue.
        let angle = 360.0 / settings.kinds as f32;

        for i in 0..settings.kinds {
            let value = if i % 2 == 0 { 0.5 } else { 1.0 };
            let color = Hsv::with_wp(angle * i as f32, 1.0, value);
            colors.push(LinSrgb::from_color(color));

            for j in 0..settings.kinds {
                let attraction = if i == j {
                    -f32::abs(settings.attraction_distr.sample(rng))
                } else {
                    settings.attraction_distr.sample(rng)
                };

                let (repel_distance, influence_radius) = if j < i {
                    // We've already generated this one (apart from attraction),
                    // so re-use that to make it symmetrical.
                    let props = pair_props[j * settings.kinds + i];
                    (props.repel_distance, props.influence_radius)
                } else {
                    let repel_distance = if i == j {
                        DIAMETER
                    } else {
                        f32::max(settings.repel_distance_distr.sample(rng), DIAMETER)
                    };

                    let mut influence_radius = settings.influence_radius_distr.sample(rng);
                    if influence_radius < repel_distance {
                        influence_radius = repel_distance;
                    }

                    (repel_distance, influence_radius)
                };
                pair_props.push(PairProps {
                    attraction,
                    repel_distance,
                    influence_radius,

                    influence_radius_sq: influence_radius * influence_radius,
                    peak: 0.5 * (repel_distance + influence_radius),
                    inv_base: 2.0 / (influence_radius - repel_distance),
                });
            }
        }

        let mut particles: Vec<_> = (0..settings.particles)
            .map(|_| Particle::generate(settings.kinds, rng))
            .collect();
        // Sort the particles by kind so that we're advancing linearly through
        // the particle kinds, which is better for cache.
        particles.sort_unstable_by_key(|particle| particle.kind);

        Self {
            wrap: false,
            flat_force: settings.flat_force,
            friction: settings.friction,

            colors,
            pair_props,

            particles,
        }
    }

    pub fn regenerate_particles<R: Rng>(&mut self, rng: &mut R) {
        for particle in self.particles.iter_mut() {
            *particle = Particle::generate(self.colors.len(), rng);
        }
        // Sort the particles by kind so that we're advancing linearly through
        // the particle kinds, which is better for cache.
        self.particles
            .sort_unstable_by_key(|particle| particle.kind);
    }

    pub fn step(&mut self, width: f32, height: f32) {
        let size = vec2(width, height);

        // The amount we want to scale up clip space by to get to pixel space.
        // This isn't just width because clip space ranges from -1 to 1, so it's
        // actually 2x2.
        let scale = 0.5 * size;

        // The inverse of `x_scale` and `y_scale`, to go from pixel space to clip space.
        let inv_scale = 2.0 / size;

        // Figure out the width/height of the particles in clip space.
        let clip_size = RADIUS * inv_scale;

        for i in 0..self.particles.len() {
            let p = self.particles[i];
            for j in i + 1..self.particles.len() {
                let q = self.particles[j];

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

                // The positions are in clip space, but velocities are in pixel space, so we
                // need to scale these up.
                delta *= scale;

                let dist2 = delta.length_squared();

                let PairProps {
                    attraction: p_attr,
                    repel_distance,
                    influence_radius_sq,
                    peak,
                    inv_base,
                    ..
                } = self.pair_props[p.kind * self.colors.len() + q.kind];

                // Disallow small distances to avoid division by zero, since we divide by this
                // to normalize the vector later on.
                if dist2 > influence_radius_sq || dist2 < 0.01 {
                    continue;
                }

                let dist = dist2.sqrt();

                let (f1, f2) = if dist < repel_distance {
                    let f = R_SMOOTH
                        * repel_distance
                        * (1.0 / (repel_distance + R_SMOOTH) - 1.0 / (dist + R_SMOOTH));
                    (f, f)
                } else {
                    let mut f1 = p_attr;
                    let mut f2 = self.pair_props[q.kind * self.colors.len() + p.kind].attraction;

                    if !self.flat_force {
                        let coefficient = 1.0 - (f32::abs(dist - peak) * inv_base);

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

        for p in self.particles.iter_mut() {
            let mut pos = p.pos;
            let mut vel = p.vel;

            pos += vel * inv_scale;
            vel *= 1.0 - self.friction;

            if self.wrap {
                if pos.x > 1.0 {
                    pos.x -= 2.0;
                } else if pos.x < -1.0 {
                    pos.x += 2.0;
                }

                if pos.y > 1.0 {
                    pos.y -= 2.0;
                } else if pos.y < -1.0 {
                    pos.y += 2.0;
                }
            } else {
                if pos.x + clip_size.x > 1.0 {
                    pos.x = 1.0 - clip_size.x;
                    vel.x *= -1.0;
                } else if pos.x - clip_size.x < -1.0 {
                    pos.x = -1.0 + clip_size.x;
                    vel.x *= -1.0;
                }

                if pos.y + clip_size.y > 1.0 {
                    pos.y = 1.0 - clip_size.y;
                    vel.y *= -1.0;
                } else if pos.y - clip_size.y < -1.0 {
                    pos.y = -1.0 + clip_size.y;
                    vel.y *= -1.0;
                }
            }

            p.pos = pos;
            p.vel = vel;
        }
    }

    /// Convert the current state of the particles into the representation used
    /// by the GPU.
    pub fn export_particles(&self, buffer: &mut [GpuParticle; MAX_PARTICLES]) {
        for (i, particle) in self.particles.iter().enumerate() {
            buffer[i] = GpuParticle {
                pos: particle.pos,
                color: self.colors[particle.kind],
            };
        }
    }
}
