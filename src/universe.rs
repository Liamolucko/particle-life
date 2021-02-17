use std::cmp::Ordering;

use quicksilver::geom::Vector;
use rand::rngs::OsRng;
use rand_distr::Distribution;
use rand_distr::Normal;
use rand_distr::Uniform;
use serde::{Deserialize, Serialize};

use crate::particle::Particle;

pub const RADIUS: f32 = 5.0;
pub const DIAMETER: f32 = RADIUS * 2.0;
pub const R_SMOOTH: f32 = 2.0;

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Settings {
    pub particles: usize,
    pub types: usize,
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
        types: 9,
        particles: 400,
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
        types: 6,
        particles: 400,
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
        types: 12,
        particles: 400,
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
        types: 6,
        particles: 300,
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
        types: 6,
        particles: 400,
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
        types: 4,
        particles: 400,
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
        types: 6,
        particles: 400,
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
        types: 6,
        particles: 400,
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
        types: 6,
        particles: 300,
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
        types: 6,
        particles: 600,
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
    pub size: Vector,

    pub wrap: bool,
    flat_force: bool,
    friction: f32,

    attractions: Vec<Vec<f32>>,
    min_radii: Vec<Vec<f32>>,
    max_radii: Vec<Vec<f32>>,

    pub particles: Vec<Particle>,
}

impl Universe {
    pub fn new(size: Vector) -> Self {
        Self {
            size,

            wrap: false,
            flat_force: false,
            friction: 0.05,

            attractions: Vec::new(),
            min_radii: Vec::new(),
            max_radii: Vec::new(),

            particles: Vec::new(),
        }
    }

    pub fn seed(&mut self, settings: &Settings) {
        self.friction = settings.friction;
        self.flat_force = settings.flat_force;

        self.seed_types(settings.types, settings);
        self.randomize_particles_inner(settings.particles);
    }

    pub fn randomize_particles(&mut self) {
        self.randomize_particles_inner(self.particles.len());
    }

    fn randomize_particles_inner(&mut self, num: usize) {
        let type_dist = Uniform::new(0, self.attractions.len());
        let (x_dist, y_dist) = if self.wrap {
            (
                Uniform::new_inclusive(0.0, self.size.x),
                Uniform::new_inclusive(0.0, self.size.y),
            )
        } else {
            (
                Uniform::new_inclusive(self.size.x * 0.25, self.size.x * 0.75),
                Uniform::new_inclusive(self.size.y * 0.25, self.size.y * 0.75),
            )
        };
        let vel_dist = Normal::new(0.0, 0.2).unwrap();

        self.particles.clear();
        self.particles.reserve(num);
        for _ in 0..num {
            self.particles.push(Particle {
                r#type: type_dist.sample(&mut OsRng),
                pos: Vector {
                    x: x_dist.sample(&mut OsRng),
                    y: y_dist.sample(&mut OsRng),
                },
                vel: Vector {
                    x: vel_dist.sample(&mut OsRng),
                    y: vel_dist.sample(&mut OsRng),
                },
            })
        }
    }

    fn seed_types(&mut self, num: usize, settings: &Settings) {
        let attr_dist = Normal::new(settings.attract_mean, settings.attract_std).unwrap();
        let minr_dist = Uniform::new_inclusive(settings.minr_lower, settings.minr_upper);
        let maxr_dist = Uniform::new_inclusive(settings.maxr_lower, settings.maxr_upper);

        self.attractions.clear();
        self.attractions.reserve(num);
        self.min_radii.clear();
        self.min_radii.reserve(num);
        self.max_radii.clear();
        self.max_radii.reserve(num);

        for i in 0..num {
            self.attractions.push(Vec::with_capacity(num));
            self.min_radii.push(Vec::with_capacity(num));
            self.max_radii.push(Vec::with_capacity(num));
            for j in 0..num {
                self.attractions[i].push(if i == j {
                    -f32::abs(attr_dist.sample(&mut OsRng))
                } else {
                    attr_dist.sample(&mut OsRng)
                });

                // Have the type with the lower index choose their shared radii rather than having it be overridden later
                let min_radius = match i.cmp(&j) {
                    Ordering::Greater => self.min_radii[j][i],
                    Ordering::Equal => DIAMETER,
                    Ordering::Less => f32::max(minr_dist.sample(&mut OsRng), DIAMETER),
                };
                self.min_radii[i].push(min_radius);

                let max_radius = if i <= j {
                    f32::max(maxr_dist.sample(&mut OsRng), self.min_radii[i][j])
                } else {
                    self.max_radii[j][i]
                };
                self.max_radii[i].push(max_radius);
            }
        }
    }

    pub fn step(&mut self) {
        // rust-analyzer couldn't figure out it's type (same for all the other times)
        let center: Vector = self.size * 0.5;

        for i in 0..self.particles.len() {
            // Only iterate over all the particles after i, and then calculate new velocities to both.
            // This is more efficient because one of the most expensive calculations is the distance calculation,
            // but since distance is symmetric we can use it for both.
            for j in i + 1..self.particles.len() {
                let p = &self.particles[i];
                let q = &self.particles[j];

                let mut delta: Vector = q.pos - p.pos;
                if self.wrap {
                    if delta.x > center.x {
                        delta.x -= self.size.x;
                    } else if delta.x < -center.x {
                        delta.x += self.size.x;
                    }
                    if delta.y > center.y {
                        delta.y -= self.size.y;
                    } else if delta.y < -center.y {
                        delta.y += self.size.y
                    }
                }

                let r2 = delta.len2();
                let max_r = self.max_radii[p.r#type][q.r#type];

                if r2 < 0.01 || r2 > max_r * max_r {
                    continue;
                }

                let min_r = self.min_radii[p.r#type][q.r#type];

                let r = f32::sqrt(r2);
                delta /= r;

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

                self.particles[i].vel += delta * f1;

                self.particles[j].vel += -delta * f2;
            }
        }

        for p in self.particles.iter_mut() {
            p.pos += p.vel;
            p.vel *= 1.0 - self.friction;

            if self.wrap {
                if p.pos.x < RADIUS {
                    p.pos.x += self.size.x;
                } else if p.pos.x >= self.size.x {
                    p.pos.x -= self.size.x;
                }
                if p.pos.y < RADIUS {
                    p.pos.y += self.size.y;
                } else if p.pos.y >= self.size.y {
                    p.pos.y -= self.size.y;
                }
            } else {
                if p.pos.x <= RADIUS {
                    p.vel.x *= -1.0;
                    p.pos.x = RADIUS;
                } else if p.pos.x >= self.size.x - RADIUS {
                    p.vel.x *= -1.0;
                    p.pos.x = self.size.x - RADIUS;
                }

                if p.pos.y <= RADIUS {
                    p.vel.y *= -1.0;
                    p.pos.y = RADIUS;
                } else if p.pos.y >= self.size.y - RADIUS {
                    p.vel.y *= -1.0;
                    p.pos.y = self.size.y - RADIUS;
                }
            }
        }
    }

    pub fn resize(&mut self, size: Vector) {
        let x_mult = size.x / self.size.x;
        let y_mult = size.y / self.size.y;

        for p in self.particles.iter_mut() {
            p.pos.x *= x_mult;
            p.pos.y *= y_mult;
        }

        self.size = size;
    }
}
