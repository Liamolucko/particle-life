mod particle;
mod universe;

use macroquad::prelude::*;
use universe::Settings;
use universe::Universe;

// fn register_keydown_handler(
//     universe: Rc<Mutex<Universe>>,
//     steps_per_frame: Rc<AtomicU8>,
// ) -> Result<(), JsValue> {
//     let callback = move |ev: KeyboardEvent| {
//         let universe = universe.clone();
//         let steps_per_frame = steps_per_frame.clone();
//         spawn_local(async move {
//             let mut universe = universe.lock().await;
//             match ev.code().as_str() {
//                 "KeyB" => universe.seed(9, 400, &Settings::BALANCED).await.unwrap(),
//                 "KeyC" => universe.seed(6, 400, &Settings::CHAOS).await.unwrap(),
//                 "KeyD" => universe.seed(12, 400, &Settings::DIVERSITY).await.unwrap(),
//                 "KeyF" => universe.seed(6, 300, &Settings::FRICTIONLESS).await.unwrap(),
//                 "KeyG" => universe.seed(6, 400, &Settings::GLIDERS).await.unwrap(),
//                 "KeyH" => universe.seed(4, 400, &Settings::HOMOGENEITY).await.unwrap(),
//                 "KeyL" => universe.seed(6, 400, &Settings::LARGE_CLUSTERS).await.unwrap(),
//                 "KeyM" => universe.seed(6, 400, &Settings::MEDIUM_CLUSTERS).await.unwrap(),
//                 "KeyQ" => universe.seed(6, 300, &Settings::QUIESCENCE).await.unwrap(),
//                 "KeyS" => universe.seed(6, 600, &Settings::SMALL_CLUSTERS).await.unwrap(),

//                 "KeyW" => universe.wrap = !universe.wrap,
//                 "Space" => steps_per_frame.store(1, Ordering::Relaxed),
//                 "Enter" => universe.randomize_particles(),

//                 _ => {}
//             }
//         })
//     };

//     let closure = Closure::wrap(Box::new(callback) as Box<dyn Fn(_)>);

//     web_sys::window()
//         .unwrap()
//         .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;

//     closure.forget();

//     Ok(())
// }

// fn register_keyup_handler(steps_per_frame: Rc<AtomicU8>) -> Result<(), JsValue> {
//     let callback = move |ev: KeyboardEvent| {
//         if ev.code().as_str() == "Space" {
//             steps_per_frame.store(10, Ordering::Relaxed)
//         }
//     };

//     let closure = Closure::wrap(Box::new(callback) as Box<dyn Fn(_)>);

//     web_sys::window()
//         .ok_or(JsValue::UNDEFINED)?
//         .add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())?;

//     closure.forget();

//     Ok(())
// }

// async fn register_update(
//     universe: Rc<Mutex<Universe>>,
//     canvas: HtmlCanvasElement,
//     steps_per_frame: Rc<AtomicU8>,
// ) -> Result<(), JsValue> {
//     let closure: Rc<Mutex<Option<Closure<dyn FnMut() -> Result<(), JsValue>>>>> =
//         Rc::new(Mutex::new(None));
//     let clone = closure.clone();

//     let callback = move || -> Result<(), JsValue> {
//         let ctx: CanvasRenderingContext2d = canvas
//             .get_context("2d")?
//             .ok_or(JsValue::UNDEFINED)?
//             .dyn_into()?;
//         ctx.set_fill_style(&JsValue::from_str("black"));
//         ctx.fill_rect(0.0, 0.0, canvas.width() as f32, canvas.height() as f32);

//         let universe = universe.clone();
//         let closure = closure.clone();
//         let steps_per_frame = steps_per_frame.clone();
//         spawn_local(async move {
//             let mut universe = universe.lock().await;

//             for opacity in 1..=steps_per_frame.load(Ordering::Relaxed) {
//                 universe.step();
//                 universe
//                     .draw(
//                         &ctx,
//                         opacity as f32 / steps_per_frame.load(Ordering::Relaxed) as f32,
//                     )
//                     .unwrap();
//             }

//             web_sys::window()
//                 .unwrap()
//                 .request_animation_frame(
//                     closure
//                         .lock()
//                         .await
//                         .as_ref()
//                         .unwrap()
//                         .as_ref()
//                         .unchecked_ref(),
//                 )
//                 .unwrap();
//         });

//         Ok(())
//     };

//     *clone.lock().await = Some(Closure::wrap(
//         Box::new(callback) as Box<dyn FnMut() -> Result<(), JsValue>>
//     ));

//     web_sys::window()
//         .ok_or(JsValue::UNDEFINED)?
//         .request_animation_frame(
//             clone
//                 .lock()
//                 .await
//                 .as_ref()
//                 .ok_or(JsValue::UNDEFINED)?
//                 .as_ref()
//                 .unchecked_ref(),
//         )?;

//     Ok(())
// }

fn window_conf() -> Conf {
    Conf {
        window_title: "Particle Life".to_owned(),
        window_width: 1600,
        window_height: 900,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut universe = Universe::new(screen_width(), screen_height());

    universe.wrap = true;
    universe.seed(9, 400, &Settings::BALANCED);

    let steps_per_frame = 10;

    loop {
        clear_background(BLACK);

        for opacity in 1..=steps_per_frame {
            universe.step();
            universe.draw(opacity as f32 / steps_per_frame as f32);
        }

        next_frame().await
    }
}
