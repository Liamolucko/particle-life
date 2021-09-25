use particle_life::settings::Settings;
use particle_life::State;
use rand::rngs::OsRng;
use wgpu::Maintain;
use winit::event::ElementState;
use winit::event::Event;
use winit::event::VirtualKeyCode;
use winit::event::WindowEvent;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::window::Window;

fn main() {
    let event_loop = EventLoop::new();
    let window = Window::new(&event_loop).unwrap();

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

    // It's easier to just keep track of this externally than read the current value from the GPU buffer.
    let mut wrap = false;

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
                                VirtualKeyCode::W => {
                                    wrap = !wrap;
                                    state.set_wrap(wrap);
                                }

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
                                    // wrap gets reset to false when the settings are replaced, so manually fix it.
                                    state.set_wrap(wrap);
                                }

                                VirtualKeyCode::Return => state.regenerate_particles(&mut rng),
                                VirtualKeyCode::Space => state.step_rate = 30,

                                _ => {}
                            }
                        } else if code == VirtualKeyCode::Space {
                            // Space was lifted, set the step rate back to normal.
                            state.step_rate = 300;
                        }
                    }
                }
                _ => {}
            },
            Event::RedrawRequested(_) => state.render(),
            Event::MainEventsCleared => {
                state.device.poll(Maintain::Wait);

                window.request_redraw();
            }
            _ => {}
        }
    });
}
