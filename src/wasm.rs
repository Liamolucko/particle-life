use crate::app;
use crate::channel::Command;
use crate::universe::Universe;
use js_sys::Uint8Array;
use quicksilver::geom::Vector;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::DedicatedWorkerGlobalScope;
use web_sys::MessageEvent;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn run() {
    console_error_panic_hook::set_once();

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

#[wasm_bindgen]
pub fn run_worker() {
    console_error_panic_hook::set_once();

    let global: DedicatedWorkerGlobalScope = js_sys::global().dyn_into().unwrap();
    let global2 = global.clone();

    // This'll be immediately resized to the actual size
    let mut universe = Universe::new(Vector::ZERO);
    let mut round = 0;

    let closure = Closure::wrap(Box::new(move |msg: MessageEvent| {
        let buf: Uint8Array = msg.data().dyn_into().unwrap();
        let cmd: Command = serde_cbor::from_slice(&buf.to_vec()).unwrap();

        match cmd {
            Command::Resize(size) => universe.resize(size),
            Command::Seed(settings) => {
                universe.seed(&settings);
                round += 1;
            }
            Command::ToggleWrap => universe.wrap = !universe.wrap,
            Command::RandomizeParticles => {
                universe.randomize_particles();
                round += 1;
            }
            Command::Step => {
                universe.step();
                global
                    .post_message(&Uint8Array::from(
                        serde_cbor::to_vec(&(round, universe.particles.clone()))
                            .unwrap()
                            .as_slice(),
                    ))
                    .unwrap();
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    global2.set_onmessage(Some(closure.as_ref().unchecked_ref()));

    // Let the main thread know we're ready to start recieving messages
    global2.post_message(&JsValue::TRUE).unwrap();

    closure.forget();
}
