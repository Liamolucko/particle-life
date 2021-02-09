mod particle;
mod universe;

use std::time::Duration;

use instant::Instant;
use quicksilver::blinds::Key;
use quicksilver::blinds::MouseButton;
#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

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

    let mut tracking: Option<usize> = None;
    let mut slow_mo = false;

    gfx.set_resize_handler(ResizeHandler::Stretch);

    #[cfg(target_arch = "wasm32")]
    let root_element = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .document_element()
        .unwrap();

    let mut prev_frame = Instant::now();

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
            universe.resize(win_size);
        }

        while let Some(ev) = input.next_event().await {
            match ev {
                Event::KeyboardInput(ev) => {
                    if ev.is_down() {
                        match ev.key() {
                            Key::B => {
                                universe.seed(9, 400, &Settings::BALANCED);
                                tracking = None;
                            }
                            Key::C => {
                                universe.seed(6, 400, &Settings::CHAOS);
                                tracking = None;
                            }
                            Key::D => {
                                universe.seed(12, 400, &Settings::DIVERSITY);
                                tracking = None;
                            }
                            Key::F => {
                                universe.seed(6, 300, &Settings::FRICTIONLESS);
                                tracking = None;
                            }
                            Key::G => {
                                universe.seed(6, 400, &Settings::GLIDERS);
                                tracking = None;
                            }
                            Key::H => {
                                universe.seed(4, 400, &Settings::HOMOGENEITY);
                                tracking = None;
                            }
                            Key::L => {
                                universe.seed(6, 400, &Settings::LARGE_CLUSTERS);
                                tracking = None;
                            }
                            Key::M => {
                                universe.seed(6, 400, &Settings::MEDIUM_CLUSTERS);
                                tracking = None;
                            }
                            Key::Q => {
                                universe.seed(6, 300, &Settings::QUIESCENCE);
                                tracking = None;
                            }
                            Key::S => {
                                universe.seed(6, 600, &Settings::SMALL_CLUSTERS);
                                tracking = None;
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
                        slow_mo = ev.is_down();
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

        universe.step(prev_frame.elapsed().as_secs_f32() * if slow_mo { 30.0 } else { 300.0 });
        universe.draw(&mut gfx, cam_pos, zoom);
        prev_frame = Instant::now();

        gfx.present(&window)?;
    }
}
