use glam::vec2;
use particle_life::settings::Settings;
use particle_life::State;
use rand::rngs::OsRng;
use winit::event::ElementState;
use winit::event::Event;
use winit::event::MouseScrollDelta;
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;
use winit::event_loop::EventLoopWindowTarget;
use winit::keyboard::Key;
use winit::keyboard::NamedKey;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::EventLoopExtWebSys;
use winit::window::Fullscreen;
use winit::window::Window;
use winit::window::WindowBuilder;

fn main() {
    #[cfg(target_arch = "wasm32")]
    // Do this as early as physically possible.
    console_error_panic_hook::set_once();

    let event_loop = EventLoop::new().unwrap();
    #[allow(unused_mut)]
    let mut builder = WindowBuilder::new().with_title("Particle Life");

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowBuilderExtWebSys;

        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document.query_selector("canvas").unwrap().unwrap();

        builder = builder.with_canvas(Some(canvas.dyn_into().unwrap()));
    }

    let window = builder.build(&event_loop).unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();

        pollster::block_on(run(event_loop, window));
    }
    #[cfg(target_arch = "wasm32")]
    {
        console_log::init().expect("could not initialize logger");

        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}

async fn run(event_loop: EventLoop<()>, window: Window) {
    let mut state = State::new(&window).await;

    // The offset from the center of the window in clip space.
    let mut mouse_pos = vec2(0.0, 0.0);
    let mut drag_cause = None;

    let mut rng = OsRng;

    let event_handler = move |event, elwt: &EventLoopWindowTarget<()>| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::Resized(size) => state.resize(size, window.scale_factor()),
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state == ElementState::Pressed {
                        match event.logical_key {
                            Key::Character(char) => match char.as_str() {
                                "w" => state.toggle_wrap(),

                                "b" | "c" | "d" | "f" | "g" | "h" | "l" | "m" | "q" | "s" => {
                                    let settings = match char.as_str() {
                                        "b" => Settings::balanced(),
                                        "c" => Settings::chaos(),
                                        "d" => Settings::diversity(),
                                        "f" => Settings::frictionless(),
                                        "g" => Settings::gliders(),
                                        "h" => Settings::homogeneity(),
                                        "l" => Settings::large_clusters(),
                                        "m" => Settings::medium_clusters(),
                                        "q" => Settings::quiescence(),
                                        "s" => Settings::small_clusters(),
                                        _ => unreachable!(),
                                    };

                                    state.replace_settings(settings, &mut rng);
                                }

                                _ => {}
                            },

                            Key::Named(NamedKey::Enter) => state.regenerate_particles(&mut rng),
                            Key::Named(NamedKey::Space) => state.step_rate = 30,

                            Key::Named(NamedKey::F11) => {
                                if window.fullscreen().is_some() {
                                    window.set_fullscreen(None);
                                } else {
                                    window.set_fullscreen(Some(Fullscreen::Borderless(None)))
                                }
                            }

                            _ => {}
                        }
                    } else if event.logical_key == Key::Named(NamedKey::Space) {
                        // Space was lifted, set the step rate back to normal.
                        state.step_rate = 300;
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

                    let center = 0.5 * vec2(size.width, size.height);
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
                WindowEvent::RedrawRequested => {
                    let size = window.inner_size().to_logical(window.scale_factor());
                    state.render(size.width, size.height);
                    window.request_redraw();
                }
                _ => {}
            }
        }
    };

    #[cfg(not(target_arch = "wasm32"))]
    event_loop.run(event_handler).unwrap();

    #[cfg(target_arch = "wasm32")]
    event_loop.spawn(event_handler);
}
