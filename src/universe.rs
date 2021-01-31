use std::f64::consts::TAU;

use rand::rngs::OsRng;
use rand_distr::Distribution;
use rand_distr::Normal;
use rand_distr::Uniform;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::CanvasRenderingContext2d;
use web_sys::HtmlCanvasElement;
use web_sys::ImageBitmap;

use crate::particle::Particle;

const RADIUS: f64 = 5.0;
const DIAMETER: f64 = RADIUS * 2.0;
const R_SMOOTH: f64 = 2.0;

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
    width: f64,
    height: f64,

    pub wrap: bool,
    flat_force: bool,
    friction: f64,

    sprites: Vec<ImageBitmap>,
    attractions: Vec<Vec<f64>>,
    min_radii: Vec<Vec<f64>>,
    max_radii: Vec<Vec<f64>>,

    particles: Vec<Particle>,
}

impl Universe {
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width,
            height,

            wrap: false,
            flat_force: false,
            friction: 0.05,

            sprites: Vec::new(),
            attractions: Vec::new(),
            min_radii: Vec::new(),
            max_radii: Vec::new(),

            particles: Vec::new(),
        }
    }

    pub async fn seed(&mut self, types: usize, particles: usize, settings: &Settings) -> () {
        self.friction = settings.friction;
        self.flat_force = settings.flat_force;

        let type_dist = Uniform::new(0, types - 1);
        let (x_dist, y_dist) = if self.wrap {
            (
                Uniform::new(0.0, self.width),
                Uniform::new(0.0, self.height),
            )
        } else {
            (
                Uniform::new(self.width * 0.25, self.width * 0.75),
                Uniform::new(self.height * 0.25, self.height * 0.75),
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

        self.seed_types(types, settings).await.unwrap();
    }

    async fn seed_types(&mut self, num: usize, settings: &Settings) -> Result<(), JsValue> {
        let attr_dist = Normal::new(settings.attract_mean, settings.attract_std).unwrap();
        let minr_dist = Uniform::new(settings.minr_lower, settings.minr_upper);
        let maxr_dist = Uniform::new(settings.maxr_lower, settings.maxr_upper);

        self.attractions = Vec::with_capacity(num);
        self.min_radii = Vec::with_capacity(num);
        self.max_radii = Vec::with_capacity(num);

        let window = web_sys::window().unwrap();
        let canvas: HtmlCanvasElement = window
            .document()
            .unwrap()
            .create_element("canvas")
            .unwrap()
            .dyn_into()
            .unwrap();
        // Make the canvas slightly larger than what we're drawing to prevent weird clipping.
        canvas.set_width(DIAMETER as u32 + 2);
        canvas.set_height(DIAMETER as u32 + 2);
        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into()
            .unwrap();

        for i in 0..num {
            // Draw the ellipse once now, and then just draw it back onto the canvas every time to save performance.
            ctx.clear_rect(0.0, 0.0, DIAMETER + 2.0, DIAMETER + 2.0);
            ctx.set_fill_style(&JsValue::from_str(&format!(
                "hsl({}, 100%, {}%)",
                i as f64 / num as f64 * 360.0,
                (i % 2 + 1) * 25
            )));
            ctx.begin_path();
            ctx.ellipse(RADIUS + 1.0, RADIUS + 1.0, RADIUS, RADIUS, 0.0, 0.0, TAU)
                .unwrap();
            ctx.fill();
            self.sprites.push(
                JsFuture::from(
                    window
                        .create_image_bitmap_with_html_canvas_element(&canvas)
                        .unwrap(),
                )
                .await?
                .dyn_into()?,
            );
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

        Ok(())
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

    pub fn draw(&self, ctx: &CanvasRenderingContext2d, opacity: f64) -> Result<(), JsValue> {
        ctx.set_global_alpha(opacity);
        for p in self.particles.iter() {
            ctx.draw_image_with_image_bitmap(&self.sprites[p.r#type], p.x, p.y)?;

            if self.wrap {
                if p.x > self.width - DIAMETER - 2.0 {
                    if p.y > self.height - DIAMETER - 2.0 {
                        ctx.draw_image_with_image_bitmap(
                            &self.sprites[p.r#type],
                            p.x - self.width,
                            p.y - self.height,
                        )?;
                    }
                    ctx.draw_image_with_image_bitmap(&self.sprites[p.r#type], p.x - self.width, p.y)?;
                }
                if p.y > self.height - DIAMETER - 2.0 {
                    ctx.draw_image_with_image_bitmap(&self.sprites[p.r#type], p.x, p.y - self.height)?;
                }
            }
        }
        Ok(())
    }
}
