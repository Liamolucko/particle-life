use std::f64::consts::TAU;

use rand::rngs::OsRng;
use rand_distr::Distribution;
use rand_distr::Normal;
use rand_distr::Uniform;
use wasm_bindgen::prelude::*;
use web_sys::CanvasRenderingContext2d;

use crate::particle::Particle;
use crate::utils::set_panic_hook;

const RADIUS: f64 = 5.0;
const DIAMETER: f64 = RADIUS * 2.0;
const R_SMOOTH: f64 = 2.0;

#[wasm_bindgen]
pub struct Settings {
    pub attract_mean: f64,
    pub attract_std: f64,
    pub minr_lower: f64,
    pub minr_upper: f64,
    pub maxr_lower: f64,
    pub maxr_upper: f64,
    pub friction: f64,
    pub flat_force: bool,
}

#[wasm_bindgen]
impl Settings {
    pub fn balanced() -> Settings {
        Settings {
            attract_mean: -0.02,
            attract_std: 0.06,
            minr_lower: 0.0,
            minr_upper: 20.0,
            maxr_lower: 20.0,
            maxr_upper: 70.0,
            friction: 0.05,
            flat_force: false,
        }
    }
}

#[wasm_bindgen]
pub struct Universe {
    width: f64,
    height: f64,

    pub wrap: bool,
    flat_force: bool,
    friction: f64,

    colors: Vec<JsValue>,
    attractions: Vec<Vec<f64>>,
    min_radii: Vec<Vec<f64>>,
    max_radii: Vec<Vec<f64>>,

    particles: Vec<Particle>,
}

#[wasm_bindgen]
impl Universe {
    pub fn new(width: f64, height: f64) -> Self {
        set_panic_hook();

        Self {
            width,
            height,

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

        let type_dist = Uniform::new(0, types - 1);
        let (x_dist, y_dist) = if self.wrap {
            (
                Uniform::new(0.25, self.width as f64 * 0.75),
                Uniform::new(0.25, self.height as f64 * 0.75),
            )
        } else {
            (
                Uniform::new(0.0, self.width as f64),
                Uniform::new(0.0, self.height as f64),
            )
        };
        let vel_dist = Normal::new(0.0, 0.2).unwrap();

        self.particles = Vec::with_capacity(particles);
        for _ in 0..particles {
            self.particles.push(Particle {
                r#type: type_dist.sample(&mut OsRng),
                x: x_dist.sample(&mut OsRng),
                y: y_dist.sample(&mut OsRng),
                vx: vel_dist.sample(&mut OsRng),
                vy: vel_dist.sample(&mut OsRng),
            })
        }
    }

    pub fn seed_types(&mut self, num: usize, settings: &Settings) {
        let attr_dist = Normal::new(settings.attract_mean, settings.attract_std).unwrap();
        let minr_dist = Uniform::new(settings.minr_lower, settings.minr_upper);
        let maxr_dist = Uniform::new(settings.maxr_lower, settings.maxr_upper);
        self.colors = Vec::with_capacity(num);
        self.attractions = Vec::with_capacity(num);
        self.min_radii = Vec::with_capacity(num);
        self.max_radii = Vec::with_capacity(num);
        for i in 0..num {
            self.colors.push(JsValue::from_str(&format!(
                "hsl({}, 100%, {}%)",
                i as f64 / num as f64 * 360.0,
                (i % 2 + 1) * 25
            )));
            self.attractions.push(Vec::with_capacity(num));
            self.min_radii.push(Vec::with_capacity(num));
            self.max_radii.push(Vec::with_capacity(num));
            for j in 0..num {
                self.attractions[i].push(if i == j {
                    -f64::abs(attr_dist.sample(&mut OsRng))
                } else {
                    attr_dist.sample(&mut OsRng)
                });

                // Have the type with the lower index choose their shared radii rather than having it be overridden later
                let min_radius = if i < j {
                    f64::max(minr_dist.sample(&mut OsRng), DIAMETER)
                } else if i == j {
                    DIAMETER
                } else {
                    self.min_radii[j][i]
                };
                self.min_radii[i].push(min_radius);

                let max_radius = if i <= j {
                    f64::max(maxr_dist.sample(&mut OsRng), self.min_radii[i][j])
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

                let r = f64::sqrt(r2);
                dx /= r;
                dy /= r;

                let f1 = if r > min_r {
                    if self.flat_force {
                        self.attractions[p.r#type][q.r#type]
                    } else {
                        let numer = 2.0 * f64::abs(r - 0.5 * (max_r + min_r));
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
                        let numer = 2.0 * f64::abs(r - 0.5 * (max_r + min_r));
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
                if p.x <= DIAMETER {
                    p.vx *= -1.0;
                    p.x = DIAMETER;
                } else if p.x >= self.width - DIAMETER {
                    p.vx *= -1.0;
                    p.x = self.width - DIAMETER;
                }

                if p.y <= DIAMETER {
                    p.vy *= -1.0;
                    p.y = DIAMETER;
                } else if p.y >= self.height - DIAMETER {
                    p.vy *= -1.0;
                    p.y = self.height - DIAMETER;
                }
            }
        }
    }

    pub fn draw(&self, ctx: CanvasRenderingContext2d) {
        ctx.set_fill_style(&JsValue::from_str("black"));
        ctx.fill_rect(0.0, 0.0, self.width, self.height);
        for p in self.particles.iter() {
            ctx.set_fill_style(&self.colors[p.r#type]);
            ctx.begin_path();
            ctx.ellipse(p.x, p.y, RADIUS, RADIUS, 0.0, 0.0, TAU)
                .unwrap();
            ctx.fill();
        }
    }
}
