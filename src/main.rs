mod particle;
mod universe;

use std::collections::VecDeque;

use macroquad::prelude::*;
use particle::Particle;
use universe::Settings;
use universe::Universe;
use universe::RADIUS;

fn window_conf() -> Conf {
    Conf {
        window_title: "Particle Life".to_owned(),
        window_width: 1600,
        window_height: 900,
        ..Default::default()
    }
}

fn draw(
    particles: &[Particle],
    colors: &[Color],
    width: f32,
    height: f32,
    wrap: bool,
    zoom: f32,
    target: Vec2,
    opacity: f32,
) {
    for p in particles.iter() {
        let color = Color {
            a: opacity,
            ..colors[p.r#type]
        };

        let mut rel_x = p.x - target.x;
        let mut rel_y = p.y - target.y;

        // Wrapping render position
        if wrap {
            if rel_x > width * 0.5 {
                rel_x -= width;
            } else if rel_x < -width * 0.5 {
                rel_x += width;
            }
            if rel_y > height * 0.5 {
                rel_y -= height;
            } else if rel_y < -height * 0.5 {
                rel_y += height;
            }
        }

        let x = rel_x * zoom + width * 0.5;
        let y = rel_y * zoom + height * 0.5;

        draw_circle(x, y, RADIUS * zoom, color);

        if wrap {
            let zoomed_width = width * zoom;
            let zoomed_height = height * zoom;
            if x > width - RADIUS {
                if y > height - RADIUS {
                    draw_circle(x - zoomed_width, y - zoomed_height, RADIUS * zoom, color);
                } else if y < RADIUS {
                    draw_circle(x - zoomed_width, y + zoomed_height, RADIUS * zoom, color);
                }
                draw_circle(x - zoomed_width, y, RADIUS * zoom, color);
            } else if x < RADIUS {
                if y > height - RADIUS {
                    draw_circle(x + zoomed_width, y - zoomed_height, RADIUS * zoom, color);
                } else if y < RADIUS {
                    draw_circle(x + zoomed_width, y + zoomed_height, RADIUS * zoom, color);
                }
                draw_circle(x + zoomed_width, y, RADIUS * zoom, color);
            }
            if y > height - RADIUS {
                draw_circle(x, y - zoomed_height, RADIUS * zoom, color);
            } else if y < RADIUS {
                draw_circle(x, y + zoomed_height, RADIUS * zoom, color);
            }
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut universe = Universe::new(screen_width(), screen_height());

    universe.wrap = true;
    universe.seed(9, 400, &Settings::BALANCED);

    let mut zoom = 1.0;
    let mut zoom_dest = zoom;

    let mut cam_pos = vec2(universe.width * 0.5, universe.height * 0.5);
    let mut cam_dest = cam_pos;

    let mut last_scroll_time = 0.0;

    let mut tracking: Option<usize> = None;

    // Store the last 10 particle positions so we can maintain the nice trails even when doing less than 10 steps per frame.
    let mut particle_hist = VecDeque::with_capacity(10);

    loop {
        clear_background(BLACK);

        if universe.width != screen_width() || universe.height != screen_height() {
            universe.resize(screen_width(), screen_height());
        }

        if is_key_pressed(KeyCode::B) {
            universe.seed(9, 400, &Settings::BALANCED);
            tracking = None;
            particle_hist.clear();
        }
        if is_key_pressed(KeyCode::C) {
            universe.seed(6, 400, &Settings::CHAOS);
            tracking = None;
            particle_hist.clear();
        }
        if is_key_pressed(KeyCode::D) {
            universe.seed(12, 400, &Settings::DIVERSITY);
            tracking = None;
            particle_hist.clear();
        }
        if is_key_pressed(KeyCode::F) {
            universe.seed(6, 300, &Settings::FRICTIONLESS);
            tracking = None;
            particle_hist.clear();
        }
        if is_key_pressed(KeyCode::G) {
            universe.seed(6, 400, &Settings::GLIDERS);
            tracking = None;
            particle_hist.clear();
        }
        if is_key_pressed(KeyCode::H) {
            universe.seed(4, 400, &Settings::HOMOGENEITY);
            tracking = None;
            particle_hist.clear();
        }
        if is_key_pressed(KeyCode::L) {
            universe.seed(6, 400, &Settings::LARGE_CLUSTERS);
            tracking = None;
            particle_hist.clear();
        }
        if is_key_pressed(KeyCode::M) {
            universe.seed(6, 400, &Settings::MEDIUM_CLUSTERS);
            tracking = None;
            particle_hist.clear();
        }
        if is_key_pressed(KeyCode::Q) {
            universe.seed(6, 300, &Settings::QUIESCENCE);
            tracking = None;
            particle_hist.clear();
        }
        if is_key_pressed(KeyCode::S) {
            universe.seed(6, 600, &Settings::SMALL_CLUSTERS);
            tracking = None;
            particle_hist.clear();
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
                let center = vec2(universe.width, universe.height) * 0.5;
                cam_dest = cam_pos + (Vec2::from(mouse_position()) - center) / zoom;
            }
            last_scroll_time = time;
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let center = vec2(universe.width, universe.height) * 0.5;
            cam_dest = cam_pos + (Vec2::from(mouse_position()) - center) / zoom;

            tracking = universe.particle_at(cam_dest);
        }

        if let Some(index) = tracking {
            let p = &universe.particles[index];
            cam_dest = vec2(p.x, p.y);

            if universe.wrap {
                if cam_dest.x - cam_pos.x > universe.width * 0.5 {
                    cam_dest.x -= universe.width;
                } else if cam_dest.x - cam_pos.x < universe.width * -0.5 {
                    cam_dest.x += universe.width;
                }

                if cam_dest.y - cam_pos.y > universe.height * 0.5 {
                    cam_dest.y -= universe.height;
                } else if cam_dest.y - cam_pos.y < universe.height * -0.5 {
                    cam_dest.y += universe.height;
                }
            }
        }

        cam_pos = cam_pos * 0.9 + cam_dest * 0.1;
        zoom = zoom * 0.8 + zoom_dest * 0.2;

        if universe.wrap {
            if cam_pos.x > universe.width {
                cam_pos.x -= universe.width;
                cam_dest.x -= universe.width;
            } else if cam_pos.x < 0.0 {
                cam_pos.x += universe.width;
                cam_dest.x += universe.width;
            }

            if cam_pos.y > universe.height {
                cam_pos.y -= universe.height;
                cam_dest.y -= universe.height;
            } else if cam_pos.y < 0.0 {
                cam_pos.y += universe.height;
                cam_dest.y += universe.height;
            }
        } else {
            cam_pos = cam_pos
                .min(vec2(
                    universe.width * (1.0 - 0.5 / zoom),
                    universe.height * (1.0 - 0.5 / zoom),
                ))
                .max(vec2(
                    universe.width * (0.5 / zoom),
                    universe.height * (0.5 / zoom),
                ));
        }

        let steps = if is_key_down(KeyCode::Space) {
            1
        } else {
            (300.0 * get_frame_time()) as i32
        };
        for _ in 0..steps {
            universe.step();
            if particle_hist.len() == 10 {
                particle_hist.pop_front();
            }
            particle_hist.push_back(universe.particles.clone());
        }

        for (opacity, particles) in particle_hist.iter().enumerate() {
            draw(
                particles,
                &universe.colors,
                universe.width,
                universe.height,
                universe.wrap,
                zoom,
                cam_pos,
                opacity as f32 / particle_hist.len() as f32,
            );
        }

        next_frame().await;
    }
}
