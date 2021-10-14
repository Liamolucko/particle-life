use glam::vec2;
use particle_life::settings::Settings;
use particle_life::State;
use rand::rngs::OsRng;
use wgpu::Maintain;
use winit::event::ElementState;
use winit::event::Event;
use winit::event::MouseScrollDelta;
use winit::event::VirtualKeyCode;
use winit::event::WindowEvent;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::window::Fullscreen;
use winit::window::Window;

fn main() {
    let event_loop = EventLoop::new();
    let window = Window::new(&event_loop).unwrap();

    window.set_title("Particle Life");

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();

        pollster::block_on(run(event_loop, window));
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowExtWebSys;

        console_error_panic_hook::set_once();
        console_log::init().expect("could not initialize logger");

        // On wasm, append the canvas to the document body
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");

        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}

async fn run(event_loop: EventLoop<()>, window: Window) {
    let mut state = State::new(&window).await;

    // The offset from the center of the window in clip space.
    let mut mouse_pos = vec2(0.0, 0.0);
    let mut drag_cause = None;

    let mut rng = OsRng;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(size) => state.resize(size.width, size.height),
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(code) = input.virtual_keycode {
                        if input.state == ElementState::Pressed {
                            match code {
                                VirtualKeyCode::W => state.toggle_wrap(),

                                VirtualKeyCode::B
                                | VirtualKeyCode::C
                                | VirtualKeyCode::D
                                | VirtualKeyCode::F
                                | VirtualKeyCode::G
                                | VirtualKeyCode::H
                                | VirtualKeyCode::L
                                | VirtualKeyCode::M
                                | VirtualKeyCode::Q
                                | VirtualKeyCode::S => {
                                    let settings = match code {
                                        VirtualKeyCode::B => Settings::balanced(),
                                        VirtualKeyCode::C => Settings::chaos(),
                                        VirtualKeyCode::D => Settings::diversity(),
                                        VirtualKeyCode::F => Settings::frictionless(),
                                        VirtualKeyCode::G => Settings::gliders(),
                                        VirtualKeyCode::H => Settings::homogeneity(),
                                        VirtualKeyCode::L => Settings::large_clusters(),
                                        VirtualKeyCode::M => Settings::medium_clusters(),
                                        VirtualKeyCode::Q => Settings::quiescence(),
                                        VirtualKeyCode::S => Settings::small_clusters(),
                                        _ => unreachable!(),
                                    };

                                    state.replace_settings(settings, &mut rng);
                                }

                                VirtualKeyCode::Return => state.regenerate_particles(&mut rng),
                                VirtualKeyCode::Space => state.step_rate = 30,

                                VirtualKeyCode::F11 => {
                                    if window.fullscreen().is_some() {
                                        window.set_fullscreen(None);
                                    } else {
                                        window.set_fullscreen(Some(Fullscreen::Borderless(None)))
                                    }
                                }

                                _ => {}
                            }
                        } else if code == VirtualKeyCode::Space {
                            // Space was lifted, set the step rate back to normal.
                            state.step_rate = 300;
                        }
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let scrolled = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 60.0,
                    };

                    let old_pos = mouse_pos / state.zoom - state.camera;

                    state.zoom *= 1.1f32.powf(scrolled);
                    state.zoom = state.zoom.clamp(1.0, 10.0);

                    let new_pos = mouse_pos / state.zoom - state.camera;

                    let delta = new_pos - old_pos;

                    state.camera += delta;

                    state.set_camera();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let position = position.to_logical(window.scale_factor());
                    let size = window.inner_size().to_logical(window.scale_factor());

                    let old_pos = mouse_pos;

                    let center = vec2(size.width, size.height) / 2.0;
                    let offset = vec2(position.x, position.y) - center;
                    mouse_pos = vec2(offset.x, -offset.y) / center;

                    if drag_cause.is_some() {
                        let delta = (mouse_pos - old_pos) / state.zoom;

                        // Drag the camera by however much the mouse position has changed.
                        state.camera += delta;

                        state.set_camera();
                    }
                }
                WindowEvent::MouseInput { button, state, .. } => {
                    if state == ElementState::Pressed && drag_cause.is_none() {
                        drag_cause = Some(button);
                    } else if state == ElementState::Released && drag_cause == Some(button) {
                        drag_cause = None;
                    }
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                let size = window.inner_size().to_logical(window.scale_factor());
                state.render(size.width, size.height);
            }
            Event::MainEventsCleared => {
                state.device.poll(Maintain::Wait);

                // Note: we don't need to handle vsync here because surface.get_current_texture() blocks until the last frame is done.
                // TODO: figure out what the deal is on the web
                window.request_redraw();
            }
            _ => {}
        }
    });
}
