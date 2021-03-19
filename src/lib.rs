mod channel;
mod particle;
mod universe;
#[cfg(target_arch = "wasm32")]
mod wasm;

use std::collections::VecDeque;
use std::time::Duration;

use channel::Command;
use channel::StepChannel;
use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use palette::encoding::Linear;
use palette::encoding::Srgb;
use palette::rgb::Rgb;
use palette::Hsv;
use palette::IntoColor;
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
use universe::RADIUS;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;

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

fn gen_colors(n: usize) -> Vec<Color> {
    let mut colors = Vec::with_capacity(n);
    for i in 0..n {
        let color: Rgb<Linear<Srgb>> =
            Hsv::new(i as f32 / n as f32 * 360.0, 1.0, (i % 2 + 1) as f32 * 0.5).into_rgb();
        colors.push(Color {
            r: color.red,
            g: color.green,
            b: color.blue,
            a: 1.0,
        });
    }
    colors
}

fn particle_at(particles: &[Particle], pos: Vector) -> Option<usize> {
    for (i, p) in particles.iter().enumerate() {
        let delta: Vector = p.pos - pos;
        if delta.len2() < RADIUS * RADIUS {
            return Some(i);
        }
    }
    None
}

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

pub async fn app(window: Window, mut gfx: Graphics, mut input: Input) -> Result<()> {
    let mut chan = StepChannel::new();
    let mut wrap = false;

    let mut colors = gen_colors(9);

    let mut zoom = 1.0;
    let mut zoom_dest = zoom;

    let mut cam_pos = window.size() * 0.5;
    let mut cam_dest = cam_pos;

    let mut scroll_timer = Timer::with_duration(Duration::from_millis(300));
    let mut step_timer = Timer::time_per_second(300.0);

    let mut tracking: Option<usize> = None;

    // Store the last 10 particle positions so we can maintain the nice trails even when doing less than 10 steps per frame.
    let mut particle_hist: VecDeque<Vec<Particle>> = VecDeque::with_capacity(10);

    gfx.set_resize_handler(ResizeHandler::Stretch);

    #[cfg(target_arch = "wasm32")]
    let root_element = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .document_element()
        .unwrap();

    chan.send(Command::Resize(window.size())).await.unwrap();
    chan.send(Command::Seed(Settings::BALANCED)).await.unwrap();

    loop {
        gfx.clear(Color::BLACK);

        // winit doesn't currently trigger resize events on the web, so we have to do it this way
        #[cfg(target_arch = "wasm32")]
        if window.size().x as i32 != root_element.client_width()
            || window.size().y as i32 != root_element.client_height()
        {
            let win_size = Vector {
                x: root_element.client_width() as f32,
                y: root_element.client_height() as f32,
            };

            window.set_size(win_size);

            gfx.set_camera_size(win_size);
            chan.send(Command::Resize(win_size)).await.unwrap();
            particle_hist.clear();
        }

        while let Some(ev) = input.next_event().await {
            match ev {
                Event::KeyboardInput(ev) => {
                    if ev.is_down() {
                        match ev.key() {
                            Key::B
                            | Key::C
                            | Key::D
                            | Key::F
                            | Key::G
                            | Key::H
                            | Key::L
                            | Key::M
                            | Key::Q
                            | Key::S => {
                                let settings = match ev.key() {
                                    Key::B => Settings::BALANCED,
                                    Key::C => Settings::CHAOS,
                                    Key::D => Settings::DIVERSITY,
                                    Key::F => Settings::FRICTIONLESS,
                                    Key::G => Settings::GLIDERS,
                                    Key::H => Settings::HOMOGENEITY,
                                    Key::L => Settings::LARGE_CLUSTERS,
                                    Key::M => Settings::MEDIUM_CLUSTERS,
                                    Key::Q => Settings::QUIESCENCE,
                                    Key::S => Settings::SMALL_CLUSTERS,
                                    _ => unreachable!(),
                                };

                                tracking = None;
                                particle_hist.clear();
                                colors = gen_colors(settings.types);

                                chan.send(Command::Seed(settings)).await.unwrap();
                            }

                            Key::W => {
                                chan.send(Command::ToggleWrap).await.unwrap();
                                wrap = !wrap;
                            }

                            Key::Return => {
                                chan.send(Command::RandomizeParticles).await.unwrap();
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
                        let center = window.size() * 0.5;
                        cam_dest = cam_pos + (input.mouse().location() - center) / zoom;
                    }
                    scroll_timer.reset();
                }
                Event::PointerInput(ev) => {
                    if ev.button() == MouseButton::Left && ev.is_down() {
                        let center = window.size() * 0.5;
                        cam_dest = cam_pos + (input.mouse().location() - center) / zoom;

                        tracking = particle_at(particle_hist.front().unwrap(), cam_dest);
                    }
                }
                Event::Resized(ev) => {
                    gfx.set_camera_size(ev.size());
                    chan.send(Command::Resize(ev.size())).await.unwrap();
                    particle_hist.clear();
                }
                _ => {}
            }
        }

        // Browsers only fire animation frames when the tab is selected,
        // so cap the steps to 10 when it's been a long time since the last frame.
        let count = usize::min(step_timer.exhaust().map_or(0, Into::into), 10);
        for _ in 0..count {
            // Continue even if not enough particles are ready
            if let Some(particles) = chan.next().now_or_never().flatten() {
                if particle_hist.len() == 10 {
                    particle_hist.pop_front();
                }
                particle_hist.push_back(particles);
            } else if particle_hist.len() == 0 {
                #[cfg(target_arch = "wasm32")]
                chan.req();
                // It's better to delay the frame than display a black screen
                particle_hist.push_back(chan.next().await.unwrap());
            }
        }

        #[cfg(target_arch = "wasm32")]
        chan.req();

        if let Some(index) = tracking {
            let p = &particle_hist.front().unwrap()[index];
            cam_dest = p.pos;

            if wrap {
                if cam_dest.x - cam_pos.x > window.size().x * 0.5 {
                    cam_dest.x -= window.size().x;
                } else if cam_dest.x - cam_pos.x < window.size().x * -0.5 {
                    cam_dest.x += window.size().x;
                }

                if cam_dest.y - cam_pos.y > window.size().y * 0.5 {
                    cam_dest.y -= window.size().y;
                } else if cam_dest.y - cam_pos.y < window.size().y * -0.5 {
                    cam_dest.y += window.size().y;
                }
            }
        }

        cam_pos = cam_pos * 0.9 + cam_dest * 0.1;
        zoom = zoom * 0.8 + zoom_dest * 0.2;

        if wrap {
            if cam_pos.x > window.size().x {
                cam_pos.x -= window.size().x;
                cam_dest.x -= window.size().x;
            } else if cam_pos.x < 0.0 {
                cam_pos.x += window.size().x;
                cam_dest.x += window.size().x;
            }

            if cam_pos.y > window.size().y {
                cam_pos.y -= window.size().y;
                cam_dest.y -= window.size().y;
            } else if cam_pos.y < 0.0 {
                cam_pos.y += window.size().y;
                cam_dest.y += window.size().y;
            }
        } else {
            cam_pos = cam_pos
                .min(window.size() * (1.0 - 0.5 / zoom))
                .max(window.size() * (0.5 / zoom));
        }

        for (opacity, particles) in particle_hist.iter().enumerate() {
            draw(
                &mut gfx,
                particles,
                &colors,
                window.size(),
                wrap,
                zoom,
                cam_pos,
                (opacity + 1) as f32 / particle_hist.len() as f32,
            );
        }

        gfx.present(&window)?;
    }
}
