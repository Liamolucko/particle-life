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

    let mut zoom = 1.0;
    let mut zoom_dest = zoom;

    let mut cam_pos = vec2(screen_width() / 2.0, screen_height() / 2.0);
    let mut cam_dest = cam_pos;

    let mut last_scroll_time = 0.0;

    let mut tracking: Option<usize> = None;

    let mut prev_width = screen_width();
    let mut prev_height = screen_height();

    loop {
        clear_background(BLACK);

        let steps = if is_key_down(KeyCode::Space) { 1 } else { 10 };

        if is_key_pressed(KeyCode::B) {
            universe.seed(9, 400, &Settings::BALANCED);
            tracking = None;
        }
        if is_key_pressed(KeyCode::C) {
            universe.seed(6, 400, &Settings::CHAOS);
            tracking = None;
        }
        if is_key_pressed(KeyCode::D) {
            universe.seed(12, 400, &Settings::DIVERSITY);
            tracking = None;
        }
        if is_key_pressed(KeyCode::F) {
            universe.seed(6, 300, &Settings::FRICTIONLESS);
            tracking = None;
        }
        if is_key_pressed(KeyCode::G) {
            universe.seed(6, 400, &Settings::GLIDERS);
            tracking = None;
        }
        if is_key_pressed(KeyCode::H) {
            universe.seed(4, 400, &Settings::HOMOGENEITY);
            tracking = None;
        }
        if is_key_pressed(KeyCode::L) {
            universe.seed(6, 400, &Settings::LARGE_CLUSTERS);
            tracking = None;
        }
        if is_key_pressed(KeyCode::M) {
            universe.seed(6, 400, &Settings::MEDIUM_CLUSTERS);
            tracking = None;
        }
        if is_key_pressed(KeyCode::Q) {
            universe.seed(6, 300, &Settings::QUIESCENCE);
            tracking = None;
        }
        if is_key_pressed(KeyCode::S) {
            universe.seed(6, 600, &Settings::SMALL_CLUSTERS);
            tracking = None;
        }

        if is_key_pressed(KeyCode::W) {
            universe.wrap = !universe.wrap;
        }

        if is_key_pressed(KeyCode::Enter) {
            universe.randomize_particles();
        }

        let (_, mut scrolled) = mouse_wheel();
        if scrolled != 0.0 {
            scrolled = if scrolled > 0.0 { 1.0 } else { -1.0 };

            zoom_dest *= f32::powf(1.1, scrolled);
            zoom_dest = zoom_dest.min(10.0).max(1.0);

            let time = get_time();
            if time - last_scroll_time > 0.3 {
                let center = vec2(screen_width(), screen_height()) / 2.0;
                cam_dest = cam_pos + (Vec2::from(mouse_position()) - center) / zoom;
            }
            last_scroll_time = time;
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let center = vec2(screen_width(), screen_height()) / 2.0;
            cam_dest = cam_pos + (Vec2::from(mouse_position()) - center) / zoom;

            tracking = universe.particle_at(cam_dest);
        }

        if let Some(index) = tracking {
            let p = &universe.particles[index];
            cam_dest = vec2(p.x, p.y);

            if universe.wrap {
                if cam_dest.x - cam_pos.x > screen_width() * 0.5 {
                    cam_dest.x -= screen_width();
                } else if cam_dest.x - cam_pos.x < screen_width() * -0.5 {
                    cam_dest.x += screen_width();
                }

                if cam_dest.y - cam_pos.y > screen_height() * 0.5 {
                    cam_dest.y -= screen_height();
                } else if cam_dest.y - cam_pos.y < screen_height() * -0.5 {
                    cam_dest.y += screen_height();
                }
            }
        }

        cam_pos = cam_pos * 0.9 + cam_dest * 0.1;
        zoom = zoom * 0.8 + zoom_dest * 0.2;

        if universe.wrap {
            if cam_pos.x > screen_width() {
                cam_pos.x -= screen_width();
                cam_dest.x -= screen_width();
            } else if cam_pos.x < 0.0 {
                cam_pos.x += screen_width();
                cam_dest.x += screen_width();
            }

            if cam_pos.y > screen_height() {
                cam_pos.y -= screen_height();
                cam_dest.y -= screen_height();
            } else if cam_pos.y < 0.0 {
                cam_pos.y += screen_height();
                cam_dest.y += screen_height();
            }
        } else {
            cam_pos = cam_pos
                .min(vec2(
                    screen_width() * (1.0 - 0.5 / zoom),
                    screen_height() * (1.0 - 0.5 / zoom),
                ))
                .max(vec2(
                    screen_width() * (0.5 / zoom),
                    screen_height() * (0.5 / zoom),
                ));
        }

        if prev_width != screen_width() || prev_height != screen_height() {
            prev_width = screen_width();
            prev_height = screen_height();
            universe.resize(prev_width, prev_height);
        }

        for opacity in 1..=steps {
            universe.step();
            universe.draw(zoom, cam_pos, opacity as f32 / steps as f32);
        }

        next_frame().await;
    }
}
