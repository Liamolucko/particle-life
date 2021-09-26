let kinds: u32 = 20u;
let radius: f32 = 10.0;
let num_circle_points: u32 = 32u;

// let flat_force: u32 = 1u;
let wrap: u32 = 2u;

let frac_pi_20: f32 = 0.15707963267948966;

/// The symmetric properties of two kinds of particles.
struct SymmetricProperties {
    /// The distance below which particles begin to unconditionally repel each other.
    repel_distance: f32;
    /// The distance above which particles have no influence on each other (squared).
    influence_radius: f32;
};

[[block]]
struct Settings {
    width: f32;
    height: f32;

    friction: f32;
    flags: u32;

    colors: array<vec3<f32>, kinds>;
    symmetric_props: array<SymmetricProperties, 210>; // kinds * (kinds + 1) / 2 = 210
    attractions: array<f32, 400>; // kinds * kinds = 200

    camera: vec2<f32>;
    zoom: f32;
};

/// Settings which differ between render passes.
[[block]]
struct PassSettings {
    opacity: f32;
};

[[block]]
struct CirclePoints {
    points: array<vec2<f32>, num_circle_points>;
};

[[group(0), binding(0)]] var<uniform> circle_points: CirclePoints;
[[group(1), binding(0)]] var<uniform> settings: Settings;
[[group(2), binding(0)]] var<uniform> pass_settings: PassSettings;

struct Particle {
    [[location(0)]] pos: vec2<f32>;
    [[location(1)]] vel: vec2<f32>;
    [[location(2)]] kind: u32;
};

struct VertexOutput {
    [[builtin(position)]] pos: vec4<f32>;
    [[location(0)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(particle: Particle, [[builtin(vertex_index)]] idx: u32) -> VertexOutput {
    var pos = settings.camera + particle.pos;

    if ((settings.flags & wrap) != 0u) {
        if (pos.x > 1.0) {
            pos.x = pos.x - 2.0;
        } elseif (pos.x < -1.0) {
            pos.x = pos.x + 2.0;
        }

        if (pos.y > 1.0) {
            pos.y = pos.y - 2.0;
        } elseif (pos.y < -1.0) {
            pos.y = pos.y + 2.0;
        }
    }

    var circle_point: vec2<f32>;

    if (idx % 3u == 0u) {
        circle_point = vec2<f32>(0.0, 0.0);
    } elseif (idx % 3u == 1u) {
        circle_point = circle_points.points[idx / 3u];
    } else {
        circle_point = circle_points.points[(idx / 3u + 1u) % num_circle_points];
    }

    var vertex = pos + circle_point;

    if ((settings.flags & wrap) != 0u) {
        let clip_width = radius / settings.width;
        let clip_height = radius / settings.height;

        if (pos.x + clip_width > 1.0) {
            let middle = pos.x + cos(frac_pi_20 * f32(2u * (idx / 3u) + 1u)) * clip_width;
            if (middle > 1.0) {
                if (idx % 3u == 0u) {
                    vertex.x = -1.0;
                } else {
                    vertex.x = max(-1.0, vertex.x - 2.0);
                }
            } else {
                if (idx % 3u == 0u) {
                    vertex.x = 1.0;
                } else {
                    vertex.x = min(1.0, vertex.x);
                }
            }
        } elseif (pos.x - clip_width < -1.0) {
            let middle = pos.x + cos(frac_pi_20 * f32(2u * (idx / 3u) + 1u)) * clip_width;
            if (middle < -1.0) {
                if (idx % 3u == 0u) {
                    vertex.x = 1.0;
                } else {
                    vertex.x = min(1.0, vertex.x + 2.0);
                }
            } else {
                if (idx % 3u == 0u) {
                    vertex.x = -1.0;
                } else {
                    vertex.x = max(-1.0, vertex.x);
                }
            }
        }

        if (pos.y + clip_height > 1.0) {
            let middle = pos.y + sin(frac_pi_20 * f32(2u * (idx / 3u) + 1u)) * clip_height;
            if (middle > 1.0) {
                if (idx % 3u == 0u) {
                    vertex.y = -1.0;
                } else {
                    vertex.y = max(-1.0, vertex.y - 2.0);
                }
            } else {
                if (idx % 3u == 0u) {
                    vertex.y = 1.0;
                } else {
                    vertex.y = min(1.0, vertex.y);
                }
            }
        } elseif (pos.y - clip_height < -1.0) {
            let middle = pos.y + sin(frac_pi_20 * f32(2u * (idx / 3u) + 1u)) * clip_height;
            if (middle < -1.0) {
                if (idx % 3u == 0u) {
                    vertex.y = 1.0;
                } else {
                    vertex.y = min(1.0, vertex.y + 2.0);
                }
            } else {
                if (idx % 3u == 0u) {
                    vertex.y = -1.0;
                } else {
                    vertex.y = max(-1.0, vertex.y);
                }
            }
        }
    }

    var out: VertexOutput;
    out.pos = vec4<f32>(vertex * settings.zoom, 0.0, 1.0);
    out.color = settings.colors[particle.kind];
    return out;
}

[[stage(fragment)]]
fn fs_main([[location(0)]] color: vec3<f32>) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(color, pass_settings.opacity);
}