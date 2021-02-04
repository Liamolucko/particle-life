// macroquad has a `rand` module in it's prelude which conflicts.
use ::rand::rngs::SmallRng;
use ::rand::SeedableRng;
use macroquad::prelude::*;
use palette::encoding::Linear;
use palette::encoding::Srgb;
use palette::rgb::Rgb;
use palette::Hsv;
use palette::IntoColor;
use rand_distr::Distribution;
use rand_distr::Normal;
use rand_distr::Uniform;

use crate::particle::Particle;

const RADIUS: f32 = 5.0;
const DIAMETER: f32 = RADIUS * 2.0;
const R_SMOOTH: f32 = 2.0;

pub struct Settings {
    pub attract_mean: f32,
    pub attract_std: f32,
    pub minr_lower: f32,
    pub minr_upper: f32,
    pub maxr_lower: f32,
    pub maxr_upper: f32,
    pub friction: f32,
    pub flat_force: bool,
}

impl Settings {
    pub const BALANCED: Settings = Settings {
        attract_mean: -0.02,
        attract_std: 0.06,
        minr_lower: 0.0,
        minr_upper: 20.0,
        maxr_lower: 20.0,
        maxr_upper: 70.0,
        friction: 0.05,
        flat_force: false,
    };

    pub const CHAOS: Settings = Settings {
        attract_mean: 0.02,
        attract_std: 0.04,
        minr_lower: 0.0,
        minr_upper: 30.0,
        maxr_lower: 30.0,
        maxr_upper: 100.0,
        friction: 0.01,
        flat_force: false,
    };

    pub const DIVERSITY: Settings = Settings {
        attract_mean: -0.01,
        attract_std: 0.04,
        minr_lower: 0.0,
        minr_upper: 20.0,
        maxr_lower: 10.0,
        maxr_upper: 60.0,
        friction: 0.05,
        flat_force: true,
    };

    pub const FRICTIONLESS: Settings = Settings {
        attract_mean: 0.01,
        attract_std: 0.005,
        minr_lower: 10.0,
        minr_upper: 10.0,
        maxr_lower: 10.0,
        maxr_upper: 60.0,
        friction: 0.0,
        flat_force: true,
    };

    pub const GLIDERS: Settings = Settings {
        attract_mean: 0.0,
        attract_std: 0.06,
        minr_lower: 0.0,
        minr_upper: 20.0,
        maxr_lower: 10.0,
        maxr_upper: 50.0,
        friction: 0.01,
        flat_force: true,
    };

    pub const HOMOGENEITY: Settings = Settings {
        attract_mean: 0.0,
        attract_std: 0.04,
        minr_lower: 10.0,
        minr_upper: 10.0,
        maxr_lower: 10.0,
        maxr_upper: 80.0,
        friction: 0.05,
        flat_force: true,
    };

    pub const LARGE_CLUSTERS: Settings = Settings {
        attract_mean: 0.025,
        attract_std: 0.02,
        minr_lower: 0.0,
        minr_upper: 30.0,
        maxr_lower: 30.0,
        maxr_upper: 100.0,
        friction: 0.2,
        flat_force: false,
    };

    pub const MEDIUM_CLUSTERS: Settings = Settings {
        attract_mean: 0.02,
        attract_std: 0.05,
        minr_lower: 0.0,
        minr_upper: 20.0,
        maxr_lower: 20.0,
        maxr_upper: 50.0,
        friction: 0.05,
        flat_force: false,
    };

    pub const QUIESCENCE: Settings = Settings {
        attract_mean: -0.02,
        attract_std: 0.1,
        minr_lower: 10.0,
        minr_upper: 20.0,
        maxr_lower: 20.0,
        maxr_upper: 60.0,
        friction: 0.2,
        flat_force: false,
    };

    pub const SMALL_CLUSTERS: Settings = Settings {
        attract_mean: -0.005,
        attract_std: 0.01,
        minr_lower: 10.0,
        minr_upper: 10.0,
        maxr_lower: 20.0,
        maxr_upper: 50.0,
        friction: 0.01,
        flat_force: false,
    };
}

pub struct Universe {
    width: f32,
    height: f32,

    pub wrap: bool,
    flat_force: bool,
    friction: f32,

    rng: SmallRng,

    colors: Vec<Color>,
    attractions: Vec<Vec<f32>>,
    min_radii: Vec<Vec<f32>>,
    max_radii: Vec<Vec<f32>>,

    particles: Vec<Particle>,
}

impl Universe {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,

            wrap: false,
            flat_force: false,
            friction: 0.05,

            // I'm only using the time for random data, so the actual value doesn't matter.
            rng: SmallRng::seed_from_u64(u64::from_be_bytes(miniquad::date::now().to_be_bytes())),

            colors: Vec::new(),
            attractions: Vec::new(),
            min_radii: Vec::new(),
            max_radii: Vec::new(),

            particles: Vec::new(),
        }
    }

    pub fn seed(&mut self, types: usize, particles: usize, settings: &Settings) {
        self.friction = settings.friction;
        self.flat_force = settings.flat_force;

        self.seed_types(types, settings);
        self.randomize_particles_inner(particles);
    }

    pub fn randomize_particles(&mut self) {
        self.randomize_particles_inner(self.particles.len());
    }

    fn randomize_particles_inner(&mut self, num: usize) {
        let type_dist = Uniform::new(0, self.colors.len());
        let (x_dist, y_dist) = if self.wrap {
            (
                Uniform::new_inclusive(0.0, self.width),
                Uniform::new_inclusive(0.0, self.height),
            )
        } else {
            (
                Uniform::new_inclusive(self.width * 0.25, self.width * 0.75),
                Uniform::new_inclusive(self.height * 0.25, self.height * 0.75),
            )
        };
        let vel_dist = Normal::new(0.0, 0.2).unwrap();

        self.particles = Vec::with_capacity(num);
        for _ in 0..num {
            self.particles.push(Particle {
                r#type: type_dist.sample(&mut self.rng),
                x: x_dist.sample(&mut self.rng),
                y: y_dist.sample(&mut self.rng),
                vx: vel_dist.sample(&mut self.rng),
                vy: vel_dist.sample(&mut self.rng),
            })
        }
    }

    fn seed_types(&mut self, num: usize, settings: &Settings) {
        let attr_dist = Normal::new(settings.attract_mean, settings.attract_std).unwrap();
        let minr_dist = Uniform::new_inclusive(settings.minr_lower, settings.minr_upper);
        let maxr_dist = Uniform::new_inclusive(settings.maxr_lower, settings.maxr_upper);

        self.colors = Vec::with_capacity(num);
        self.attractions = Vec::with_capacity(num);
        self.min_radii = Vec::with_capacity(num);
        self.max_radii = Vec::with_capacity(num);

        for i in 0..num {
            let color: Rgb<Linear<Srgb>> =
                Hsv::new(i as f32 / num as f32 * 360.0, 1.0, (i % 2 + 1) as f32 / 2.0).into_rgb();
            self.colors
                .push(Color::new(color.red, color.green, color.blue, 1.0));
            self.attractions.push(Vec::with_capacity(num));
            self.min_radii.push(Vec::with_capacity(num));
            self.max_radii.push(Vec::with_capacity(num));
            for j in 0..num {
                self.attractions[i].push(if i == j {
                    -f32::abs(attr_dist.sample(&mut self.rng))
                } else {
                    attr_dist.sample(&mut self.rng)
                });

                // Have the type with the lower index choose their shared radii rather than having it be overridden later
                let min_radius = if i < j {
                    f32::max(minr_dist.sample(&mut self.rng), DIAMETER)
                } else if i == j {
                    DIAMETER
                } else {
                    self.min_radii[j][i]
                };
                self.min_radii[i].push(min_radius);

                let max_radius = if i <= j {
                    f32::max(maxr_dist.sample(&mut self.rng), self.min_radii[i][j])
                } else {
                    self.max_radii[j][i]
                };
                self.max_radii[i].push(max_radius);
            }
        }
    }

    pub fn step(&mut self) {
        for i in 0..self.particles.len() {
            // Only iterate over all the particles after i, and then calculate new velocities to both.
            // This is more efficient because one of the most expensive calculations is the distance calculation,
            // but since distance is symmetric we can use it for both.
            for j in i + 1..self.particles.len() {
                let p = &self.particles[i];
                let q = &self.particles[j];

                let mut dx = q.x - p.x;
                let mut dy = q.y - p.y;
                if self.wrap {
                    if dx > self.width * 0.5 {
                        dx -= self.width;
                    } else if dx < -self.width * 0.5 {
                        dx += self.width;
                    }
                    if dy > self.width * 0.5 {
                        dy -= self.height;
                    } else if dy < -self.height * 0.5 {
                        dy += self.height
                    }
                }

                let r2 = dx * dx + dy * dy;
                let min_r = self.min_radii[p.r#type][q.r#type];
                let max_r = self.max_radii[p.r#type][q.r#type];

                if r2 > max_r * max_r || r2 < 0.01 {
                    continue;
                }

                let r = f32::sqrt(r2);
                dx /= r;
                dy /= r;

                let f1 = if r > min_r {
                    if self.flat_force {
                        self.attractions[p.r#type][q.r#type]
                    } else {
                        let numer = 2.0 * f32::abs(r - 0.5 * (max_r + min_r));
                        let denom = max_r - min_r;
                        self.attractions[p.r#type][q.r#type] * (1.0 - numer / denom)
                    }
                } else {
                    R_SMOOTH * min_r * (1.0 / (min_r + R_SMOOTH) - 1.0 / (r + R_SMOOTH))
                };

                let f2 = if r > min_r {
                    if self.flat_force {
                        self.attractions[q.r#type][p.r#type]
                    } else {
                        let numer = 2.0 * f32::abs(r - 0.5 * (max_r + min_r));
                        let denom = max_r - min_r;
                        self.attractions[q.r#type][p.r#type] * (1.0 - numer / denom)
                    }
                } else {
                    R_SMOOTH * min_r * (1.0 / (min_r + R_SMOOTH) - 1.0 / (r + R_SMOOTH))
                };

                self.particles[i].vx += f1 * dx;
                self.particles[i].vy += f1 * dy;

                self.particles[j].vx += f2 * -dx;
                self.particles[j].vy += f2 * -dy;
            }
        }

        for p in self.particles.iter_mut() {
            p.x += p.vx;
            p.y += p.vy;
            p.vx *= 1.0 - self.friction;
            p.vy *= 1.0 - self.friction;

            if self.wrap {
                if p.x < 0.0 {
                    p.x += self.width;
                } else if p.x >= self.width {
                    p.x -= self.width;
                }
                if p.y < 0.0 {
                    p.y += self.height;
                } else if p.y >= self.height {
                    p.y -= self.height;
                }
            } else {
                if p.x <= 0.0 {
                    p.vx *= -1.0;
                    p.x = 0.0;
                } else if p.x >= self.width - DIAMETER - 2.0 {
                    p.vx *= -1.0;
                    p.x = self.width - DIAMETER - 2.0;
                }

                if p.y <= 0.0 {
                    p.vy *= -1.0;
                    p.y = 0.0;
                } else if p.y >= self.height - DIAMETER - 2.0 {
                    p.vy *= -1.0;
                    p.y = self.height - DIAMETER - 2.0;
                }
            }
        }
    }

    pub fn draw(&self, opacity: f32) {
        for p in self.particles.iter() {
            let color = Color {
                a: opacity,
                ..self.colors[p.r#type]
            };

            draw_circle(p.x, p.y, RADIUS, color);

            if self.wrap {
                if p.x > self.width - DIAMETER - 2.0 {
                    if p.y > self.height - DIAMETER - 2.0 {
                        draw_circle(p.x - self.width, p.y - self.height, RADIUS, color);
                    }
                    draw_circle(p.x - self.width, p.y, RADIUS, color);
                }
                if p.y > self.height - DIAMETER - 2.0 {
                    draw_circle(p.x, p.y - self.height, RADIUS, color);
                }
            }
        }
    }
}
