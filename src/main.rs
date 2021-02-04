mod particle;
mod universe;

use macroquad::prelude::*;
use universe::Settings;
use universe::Universe;

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

    loop {
        clear_background(BLACK);

        let steps = if is_key_down(KeyCode::Space) { 1 } else { 10 };

        if is_key_pressed(KeyCode::B) {
            universe.seed(9, 400, &Settings::BALANCED)
        }
        if is_key_pressed(KeyCode::C) {
            universe.seed(6, 400, &Settings::CHAOS)
        }
        if is_key_pressed(KeyCode::D) {
            universe.seed(12, 400, &Settings::DIVERSITY)
        }
        if is_key_pressed(KeyCode::F) {
            universe.seed(6, 300, &Settings::FRICTIONLESS)
        }
        if is_key_pressed(KeyCode::G) {
            universe.seed(6, 400, &Settings::GLIDERS)
        }
        if is_key_pressed(KeyCode::H) {
            universe.seed(4, 400, &Settings::HOMOGENEITY)
        }
        if is_key_pressed(KeyCode::L) {
            universe.seed(6, 400, &Settings::LARGE_CLUSTERS)
        }
        if is_key_pressed(KeyCode::M) {
            universe.seed(6, 400, &Settings::MEDIUM_CLUSTERS)
        }
        if is_key_pressed(KeyCode::Q) {
            universe.seed(6, 300, &Settings::QUIESCENCE)
        }
        if is_key_pressed(KeyCode::S) {
            universe.seed(6, 600, &Settings::SMALL_CLUSTERS)
        }

        if is_key_pressed(KeyCode::W) {
            universe.wrap = !universe.wrap;
        }

        if is_key_pressed(KeyCode::Enter) {
            universe.randomize_particles();
        }

        for opacity in 1..=steps {
            universe.step();
            universe.draw(opacity as f32 / steps as f32);
        }

        next_frame().await;
    }
}
