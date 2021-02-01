mod particle;
mod universe;

use std::rc::Rc;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;

use futures::lock::Mutex;
use universe::Settings;
use universe::Universe;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::CanvasRenderingContext2d;
use web_sys::HtmlCanvasElement;
use web_sys::KeyboardEvent;

fn register_keydown_handler(
    universe: Rc<Mutex<Universe>>,
    steps_per_frame: Rc<AtomicU8>,
) -> Result<(), JsValue> {
    let callback = move |ev: KeyboardEvent| {
        let universe = universe.clone();
        let steps_per_frame = steps_per_frame.clone();
        spawn_local(async move {
            let mut universe = universe.lock().await;
            match ev.code().as_str() {
                "KeyB" => universe.seed(9, 400, &Settings::BALANCED).await.unwrap(),
                "KeyC" => universe.seed(6, 400, &Settings::CHAOS).await.unwrap(),
                "KeyD" => universe.seed(12, 400, &Settings::DIVERSITY).await.unwrap(),
                "KeyF" => universe.seed(6, 300, &Settings::FRICTIONLESS).await.unwrap(),
                "KeyG" => universe.seed(6, 400, &Settings::GLIDERS).await.unwrap(),
                "KeyH" => universe.seed(4, 400, &Settings::HOMOGENEITY).await.unwrap(),
                "KeyL" => universe.seed(6, 400, &Settings::LARGE_CLUSTERS).await.unwrap(),
                "KeyM" => universe.seed(6, 400, &Settings::MEDIUM_CLUSTERS).await.unwrap(),
                "KeyQ" => universe.seed(6, 300, &Settings::QUIESCENCE).await.unwrap(),
                "KeyS" => universe.seed(6, 600, &Settings::SMALL_CLUSTERS).await.unwrap(),

                "KeyW" => universe.wrap = !universe.wrap,
                "Space" => steps_per_frame.store(1, Ordering::Relaxed),
                "Enter" => universe.randomize_particles(),

                _ => {}
            }
        })
    };

    let closure = Closure::wrap(Box::new(callback) as Box<dyn Fn(_)>);

    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;

    closure.forget();

    Ok(())
}

fn register_keyup_handler(steps_per_frame: Rc<AtomicU8>) -> Result<(), JsValue> {
    let callback = move |ev: KeyboardEvent| {
        if ev.code().as_str() == "Space" {
            steps_per_frame.store(10, Ordering::Relaxed)
        }
    };

    let closure = Closure::wrap(Box::new(callback) as Box<dyn Fn(_)>);

    web_sys::window()
        .ok_or(JsValue::UNDEFINED)?
        .add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())?;

    closure.forget();

    Ok(())
}

async fn register_update(
    universe: Rc<Mutex<Universe>>,
    canvas: HtmlCanvasElement,
    steps_per_frame: Rc<AtomicU8>,
) -> Result<(), JsValue> {
    let closure: Rc<Mutex<Option<Closure<dyn FnMut() -> Result<(), JsValue>>>>> =
        Rc::new(Mutex::new(None));
    let clone = closure.clone();

    let callback = move || -> Result<(), JsValue> {
        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")?
            .ok_or(JsValue::UNDEFINED)?
            .dyn_into()?;
        ctx.set_fill_style(&JsValue::from_str("black"));
        ctx.fill_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);

        let universe = universe.clone();
        let closure = closure.clone();
        let steps_per_frame = steps_per_frame.clone();
        spawn_local(async move {
            let mut universe = universe.lock().await;

            for opacity in 1..=steps_per_frame.load(Ordering::Relaxed) {
                universe.step();
                universe
                    .draw(
                        &ctx,
                        opacity as f64 / steps_per_frame.load(Ordering::Relaxed) as f64,
                    )
                    .unwrap();
            }

            web_sys::window()
                .unwrap()
                .request_animation_frame(
                    closure
                        .lock()
                        .await
                        .as_ref()
                        .unwrap()
                        .as_ref()
                        .unchecked_ref(),
                )
                .unwrap();
        });

        Ok(())
    };

    *clone.lock().await = Some(Closure::wrap(
        Box::new(callback) as Box<dyn FnMut() -> Result<(), JsValue>>
    ));

    web_sys::window()
        .ok_or(JsValue::UNDEFINED)?
        .request_animation_frame(
            clone
                .lock()
                .await
                .as_ref()
                .ok_or(JsValue::UNDEFINED)?
                .as_ref()
                .unchecked_ref(),
        )?;

    Ok(())
}

#[wasm_bindgen(start)]
pub async fn main() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let window = web_sys::window().ok_or(JsValue::UNDEFINED)?;

    let canvas: HtmlCanvasElement = window
        .document()
        .ok_or(JsValue::UNDEFINED)?
        .get_element_by_id("canvas")
        .ok_or(JsValue::UNDEFINED)?
        .dyn_into()?;

    canvas.set_width(canvas.offset_width() as u32);
    canvas.set_height(canvas.offset_height() as u32);

    let mut universe = Universe::new(canvas.width() as f64, canvas.height() as f64);

    universe.wrap = true;
    universe.seed(9, 400, &Settings::BALANCED).await?;

    let universe = Rc::new(Mutex::new(universe));

    let steps_per_frame = Rc::new(AtomicU8::new(10));

    register_keydown_handler(universe.clone(), steps_per_frame.clone())?;

    register_keyup_handler(steps_per_frame.clone())?;

    register_update(universe.clone(), canvas, steps_per_frame.clone()).await?;

    Ok(())
}
