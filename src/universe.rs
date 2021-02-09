use std::cmp::Ordering;

use palette::encoding::Linear;
use palette::encoding::Srgb;
use palette::rgb::Rgb;
use palette::Hsv;
use palette::IntoColor;
use quicksilver::geom::Circle;
use quicksilver::geom::Shape;
use quicksilver::geom::Vector;
use quicksilver::graphics::Color;
use quicksilver::Graphics;
use rand::rngs::OsRng;
use rand_distr::Distribution;
use rand_distr::Normal;
use rand_distr::Uniform;

use crate::particle::Particle;

pub const RADIUS: f32 = 5.0;
pub const DIAMETER: f32 = RADIUS * 2.0;
pub const R_SMOOTH: f32 = 2.0;

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
    pub size: Vector,

    pub wrap: bool,
    flat_force: bool,
    friction: f32,

    pub colors: Vec<Color>,
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

        self.colors.clear();
        self.colors.reserve(num);
        self.attractions.clear();
        self.attractions.reserve(num);
        self.min_radii.clear();
        self.min_radii.reserve(num);
        self.max_radii.clear();
        self.max_radii.reserve(num);

        for i in 0..num {
            let color: Rgb<Linear<Srgb>> =
                Hsv::new(i as f32 / num as f32 * 360.0, 1.0, (i % 2 + 1) as f32 * 0.5).into_rgb();
            self.colors.push(Color {
                r: color.red,
                g: color.green,
                b: color.blue,
                a: 1.0,
            });
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

    pub fn step(&mut self, dt: f32) {
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

                self.particles[i].vel += delta * f1 * dt;

                self.particles[j].vel += -delta * f2 * dt;
            }
        }

        for p in self.particles.iter_mut() {
            p.pos += p.vel * dt;
            p.vel *= f32::powf(1.0 - self.friction, dt);

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

    pub fn draw(&self, gfx: &mut Graphics, target: Vector, zoom: f32) {
        let center: Vector = self.size * 0.5;

        for p in self.particles.iter() {
            let mut color = self.colors[p.r#type];

            let mut rel: Vector = p.pos - target;

            for _ in 0..10 {
                // Wrapping render position
                if self.wrap {
                    if rel.x > center.x {
                        rel.x -= self.size.x;
                    } else if rel.x < -center.x {
                        rel.x += self.size.x;
                    }
                    if rel.y > center.y {
                        rel.y -= self.size.y;
                    } else if rel.y < -center.y {
                        rel.y += self.size.y;
                    }
                }

                let pos = rel * zoom + center;

                if pos.x - RADIUS * zoom < self.size.x
                    && pos.x + RADIUS * zoom > 0.0
                    && pos.y - RADIUS * zoom < self.size.y
                    && pos.y + RADIUS * zoom > 0.0
                {
                    let mut circle = Circle::new(pos, RADIUS * zoom);

                    gfx.fill_polygon(&circle_points(&circle), color);

                    let mut y_wrapped = false;
                    if self.wrap {
                        if rel.y > center.y - RADIUS && pos.y < self.size.y + RADIUS {
                            circle.pos.y -= self.size.y;

                            gfx.fill_polygon(&circle_points(&circle), color);

                            y_wrapped = true;
                        } else if rel.y < -center.y + RADIUS && pos.y > -RADIUS {
                            circle.pos.y += self.size.y;

                            gfx.fill_polygon(&circle_points(&circle), color);

                            y_wrapped = true;
                        }

                        if rel.x > center.x - RADIUS && pos.x < self.size.x + RADIUS {
                            circle.pos.x -= self.size.x;

                            gfx.fill_polygon(&circle_points(&circle), color);

                            if y_wrapped {
                                circle.pos.y = pos.y;

                                gfx.fill_polygon(&circle_points(&circle), color);
                            }
                        } else if rel.x < -center.x + RADIUS && pos.x > -RADIUS {
                            circle.pos.x += self.size.x;

                            gfx.fill_polygon(&circle_points(&circle), color);

                            if y_wrapped {
                                circle.pos.y = pos.y;

                                gfx.fill_polygon(&circle_points(&circle), color);
                            }
                        }
                    }
                }
                rel -= p.vel;
                color.a -= 0.1;
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

    pub fn particle_at(&mut self, pos: Vector) -> Option<usize> {
        for (i, p) in self.particles.iter().enumerate() {
            let delta: Vector = p.pos - pos;
            if delta.len2() < RADIUS * RADIUS {
                return Some(i);
            }
        }
        None
    }
}

const CIRCLE_POINTS: [Vector; 20] = [
    Vector { x: 1.0, y: 0.0 },
    Vector {
        x: 0.945_817_23,
        y: 0.324_699_46,
    },
    Vector {
        x: 0.789_140_5,
        y: 0.614_212_7,
    },
    Vector {
        x: 0.546_948_13,
        y: 0.837_166_5,
    },
    Vector {
        x: 0.245_485_48,
        y: 0.969_400_3,
    },
    Vector {
        x: -0.082_579_345,
        y: 0.996_584_5,
    },
    Vector {
        x: -0.401_695_43,
        y: 0.915_773_33,
    },
    Vector {
        x: -0.677_281_56,
        y: 0.735_723_9,
    },
    Vector {
        x: -0.879_473_75,
        y: 0.475_947_38,
    },
    Vector {
        x: -0.986_361_3,
        y: 0.164_594_59,
    },
    Vector {
        x: -0.986_361_3,
        y: -0.164_594_59,
    },
    Vector {
        x: -0.879_473_75,
        y: -0.475_947_38,
    },
    Vector {
        x: -0.677_281_56,
        y: -0.735_723_9,
    },
    Vector {
        x: -0.401_695_43,
        y: -0.915_773_33,
    },
    Vector {
        x: -0.082_579_345,
        y: -0.996_584_5,
    },
    Vector {
        x: 0.245_485_48,
        y: -0.969_400_3,
    },
    Vector {
        x: 0.546_948_13,
        y: -0.837_166_5,
    },
    Vector {
        x: 0.789_140_5,
        y: -0.614_212_7,
    },
    Vector {
        x: 0.945_817_23,
        y: -0.324_699_46,
    },
    Vector { x: 1.0, y: 0.0 },
];

fn circle_points(circle: &Circle) -> [Vector; 20] {
    let mut points = CIRCLE_POINTS;
    for point in points.iter_mut() {
        *point = circle.center() + (*point * circle.radius);
    }
    points
}
