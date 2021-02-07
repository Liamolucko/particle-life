mod particle;
mod universe;

use std::collections::VecDeque;
use std::time::Duration;

use particle::Particle;
use quicksilver::blinds::Key;
use quicksilver::blinds::MouseButton;
use quicksilver::geom::Circle;
use quicksilver::geom::Shape;
use quicksilver::geom::Vector;
use quicksilver::graphics::Color;
use quicksilver::graphics::ResizeHandler;
use quicksilver::input::Event;
use quicksilver::input::ScrollDelta;
use quicksilver::Graphics;
use quicksilver::Input;
use quicksilver::Result;
use quicksilver::Timer;
use quicksilver::Window;
use universe::Settings;
use universe::Universe;
use universe::RADIUS;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

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

fn draw(
    gfx: &mut Graphics,
    particles: &[Particle],
    colors: &[Color],
    size: Vector,
    wrap: bool,
    zoom: f32,
    target: Vector,
    opacity: f32,
) {
    let center: Vector = size * 0.5;

    for p in particles.iter() {
        let color = colors[p.r#type].with_alpha(opacity);

        let mut rel: Vector = p.pos - target;

        // Wrapping render position
        if wrap {
            if rel.x > center.x {
                rel.x -= size.x;
            } else if rel.x < -center.x {
                rel.x += size.x;
            }
            if rel.y > center.y {
                rel.y -= size.y;
            } else if rel.y < -center.y {
                rel.y += size.y;
            }
        }

        let pos = rel * zoom + center;

        let mut circle = Circle::new(pos, RADIUS * zoom);

        if pos.x - RADIUS * zoom < size.x
            && pos.x + RADIUS * zoom > 0.0
            && pos.y - RADIUS * zoom < size.y
            && pos.y + RADIUS * zoom > 0.0
        {
            gfx.fill_polygon(&circle_points(&circle), color);

            let mut y_wrapped = false;
            if wrap {
                if rel.y > center.y - RADIUS && pos.y < size.y + RADIUS {
                    circle.pos.y -= size.y;

                    gfx.fill_polygon(&circle_points(&circle), color);

                    y_wrapped = true;
                } else if rel.y < -center.y + RADIUS && pos.y > -RADIUS {
                    circle.pos.y += size.y;

                    gfx.fill_polygon(&circle_points(&circle), color);

                    y_wrapped = true;
                }

                if rel.x > center.x - RADIUS && pos.x < size.x + RADIUS {
                    circle.pos.x -= size.x;

                    gfx.fill_polygon(&circle_points(&circle), color);

                    if y_wrapped {
                        circle.pos.y = pos.y;

                        gfx.fill_polygon(&circle_points(&circle), color);
                    }
                } else if rel.x < -center.x + RADIUS && pos.x > -RADIUS {
                    circle.pos.x += size.x;

                    gfx.fill_polygon(&circle_points(&circle), color);

                    if y_wrapped {
                        circle.pos.y = pos.y;

                        gfx.fill_polygon(&circle_points(&circle), color);
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    quicksilver::run(
        quicksilver::Settings {
            title: "Particle Life",
            size: Vector {
                x: 1600.0,
                y: 900.0,
            },
            multisampling: Some(4),
            resizable: true,
            vsync: true,
            ..Default::default()
        },
        app,
    );
}

pub async fn app(window: Window, mut gfx: Graphics, mut input: Input) -> Result<()> {
    let mut universe = Universe::new(window.size());

    universe.wrap = true;
    universe.seed(9, 400, &Settings::BALANCED);

    let mut zoom = 1.0;
    let mut zoom_dest = zoom;

    let mut cam_pos = universe.size * 0.5;
    let mut cam_dest = cam_pos;

    let mut scroll_timer = Timer::with_duration(Duration::from_millis(300));
    let mut step_timer = Timer::time_per_second(300.0);

    let mut tracking: Option<usize> = None;

    // Store the last 10 particle positions so we can maintain the nice trails even when doing less than 10 steps per frame.
    let mut particle_hist = VecDeque::with_capacity(10);

    gfx.set_resize_handler(ResizeHandler::Stretch);

    loop {
        gfx.clear(Color::BLACK);

        while let Some(ev) = input.next_event().await {
            match ev {
                Event::KeyboardInput(ev) => {
                    if ev.is_down() {
                        match ev.key() {
                            Key::B => {
                                universe.seed(9, 400, &Settings::BALANCED);
                                tracking = None;
                                particle_hist.clear();
                            }
                            Key::C => {
                                universe.seed(6, 400, &Settings::CHAOS);
                                tracking = None;
                                particle_hist.clear();
                            }
                            Key::D => {
                                universe.seed(12, 400, &Settings::DIVERSITY);
                                tracking = None;
                                particle_hist.clear();
                            }
                            Key::F => {
                                universe.seed(6, 300, &Settings::FRICTIONLESS);
                                tracking = None;
                                particle_hist.clear();
                            }
                            Key::G => {
                                universe.seed(6, 400, &Settings::GLIDERS);
                                tracking = None;
                                particle_hist.clear();
                            }
                            Key::H => {
                                universe.seed(4, 400, &Settings::HOMOGENEITY);
                                tracking = None;
                                particle_hist.clear();
                            }
                            Key::L => {
                                universe.seed(6, 400, &Settings::LARGE_CLUSTERS);
                                tracking = None;
                                particle_hist.clear();
                            }
                            Key::M => {
                                universe.seed(6, 400, &Settings::MEDIUM_CLUSTERS);
                                tracking = None;
                                particle_hist.clear();
                            }
                            Key::Q => {
                                universe.seed(6, 300, &Settings::QUIESCENCE);
                                tracking = None;
                                particle_hist.clear();
                            }
                            Key::S => {
                                universe.seed(6, 600, &Settings::SMALL_CLUSTERS);
                                tracking = None;
                                particle_hist.clear();
                            }

                            Key::W => {
                                universe.wrap = !universe.wrap;
                            }

                            Key::Return => {
                                universe.randomize_particles();
                            }

                            _ => {}
                        }
                    }

                    if ev.key() == Key::Space {
                        let period =
                            Duration::from_secs_f32(1.0 / if ev.is_down() { 30.0 } else { 300.0 });

                        // This event is called by key repeats, so don't reset the timer every time it's called if it already has the correct period.
                        if step_timer.period() != period {
                            step_timer = Timer::with_duration(period);
                        }
                    }
                }
                Event::ScrollInput(delta) => {
                    let scrolled = match delta {
                        ScrollDelta::Lines(lines) => {
                            // Firefox's lines are the other way round from native lines, and it marks more lines as being scrolled.
                            // I need to check other browsers though. (chromium uses pixels)
                            if cfg!(target_arch = "wasm32") {
                                lines.y / -3.0
                            } else {
                                lines.y
                            }
                        }
                        ScrollDelta::Pixels(pixels) => pixels.y / -60.0,
                    };

                    zoom_dest *= f32::powf(1.1, scrolled);
                    zoom_dest = zoom_dest.min(10.0).max(1.0);

                    if scroll_timer.exhaust().is_some() {
                        let center = universe.size * 0.5;
                        cam_dest = cam_pos + (input.mouse().location() - center) / zoom;
                    }
                    scroll_timer.reset();
                }
                Event::PointerInput(ev) => {
                    if ev.button() == MouseButton::Left && ev.is_down() {
                        let center = universe.size * 0.5;
                        cam_dest = cam_pos + (input.mouse().location() - center) / zoom;

                        tracking = universe.particle_at(cam_dest);
                    }
                }
                Event::Resized(ev) => {
                    gfx.set_camera_size(ev.size());
                    universe.resize(ev.size());
                }
                _ => {}
            }
        }

        if let Some(index) = tracking {
            let p = &universe.particles[index];
            cam_dest = p.pos;

            if universe.wrap {
                if cam_dest.x - cam_pos.x > universe.size.x * 0.5 {
                    cam_dest.x -= universe.size.x;
                } else if cam_dest.x - cam_pos.x < universe.size.x * -0.5 {
                    cam_dest.x += universe.size.x;
                }

                if cam_dest.y - cam_pos.y > universe.size.y * 0.5 {
                    cam_dest.y -= universe.size.y;
                } else if cam_dest.y - cam_pos.y < universe.size.y * -0.5 {
                    cam_dest.y += universe.size.y;
                }
            }
        }

        cam_pos = cam_pos * 0.9 + cam_dest * 0.1;
        zoom = zoom * 0.8 + zoom_dest * 0.2;

        if universe.wrap {
            if cam_pos.x > universe.size.x {
                cam_pos.x -= universe.size.x;
                cam_dest.x -= universe.size.x;
            } else if cam_pos.x < 0.0 {
                cam_pos.x += universe.size.x;
                cam_dest.x += universe.size.x;
            }

            if cam_pos.y > universe.size.y {
                cam_pos.y -= universe.size.y;
                cam_dest.y -= universe.size.y;
            } else if cam_pos.y < 0.0 {
                cam_pos.y += universe.size.y;
                cam_dest.y += universe.size.y;
            }
        } else {
            cam_pos = cam_pos
                .min(universe.size * (1.0 - 0.5 / zoom))
                .max(universe.size * (0.5 / zoom));
        }

        let mut count = 0;
        while step_timer.tick() {
            universe.step();
            if particle_hist.len() == 10 {
                particle_hist.pop_front();
            }
            particle_hist.push_back(universe.particles.clone());

            // Browsers only fire animation frames when the tab is selected,
            // so cap the steps to 10 when it's been a long time since the last frame.
            count += 1;
            if count == 10 {
                step_timer.reset();
            }
        }

        for (opacity, particles) in particle_hist.iter().enumerate() {
            draw(
                &mut gfx,
                particles,
                &universe.colors,
                universe.size,
                universe.wrap,
                zoom,
                cam_pos,
                opacity as f32 / particle_hist.len() as f32,
            );
        }

        gfx.present(&window)?;
    }
}
