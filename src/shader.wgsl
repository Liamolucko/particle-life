let kinds: u32 = 20u;
let radius: f32 = 5.0;
let num_circle_points: u32 = 32u;

// let flat_force: u32 = 1u;
let wrap: u32 = 2u;

let pi: f32 = 3.14159265358979323846264338327950288;

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

    wrap: u32;

    zoom: f32;
    camera: vec2<f32>;
};

/// Settings which differ between render passes.
[[block]]
struct PassSettings {
    opacity: f32;
};

[[group(0), binding(0)]] var<uniform> settings: Settings;
[[group(1), binding(0)]] var<uniform> pass_settings: PassSettings;

struct Particle {
    [[location(0)]] pos: vec2<f32>;
    [[location(1)]] color: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] pos: vec4<f32>;
    [[location(0)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(particle: Particle, [[builtin(vertex_index)]] idx: u32) -> VertexOutput {
    // Half the angle between each line from the centre.
    // This isn't a proper constant because WGSL won't let me do division there.
    let half_circle_angle: f32 = pi / f32(num_circle_points);

    var pos = settings.camera + particle.pos;

    if (settings.wrap != 0u) {
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
    } else {
        var point: u32;
        if (idx % 3u == 1u) {
            point = idx / 3u;
        } else {
            point = idx / 3u + 1u;
        }
        let angle = f32(point) / f32(num_circle_points) * 2.0 * pi;
        circle_point = vec2<f32>(cos(angle) * radius * 2.0 / settings.width, sin(angle) * radius * 2.0 / settings.height);
    }

    var vertex = pos + circle_point;

    if (settings.wrap != 0u) {
        let clip_width = radius * 2.0 / settings.width;
        let clip_height = radius * 2.0 / settings.height;

        if (pos.x + clip_width > 1.0) {
            let middle = pos.x + cos(half_circle_angle * f32(2u * (idx / 3u) + 1u)) * clip_width;
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
            let middle = pos.x + cos(half_circle_angle * f32(2u * (idx / 3u) + 1u)) * clip_width;
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
            let middle = pos.y + sin(half_circle_angle * f32(2u * (idx / 3u) + 1u)) * clip_height;
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
            let middle = pos.y + sin(half_circle_angle * f32(2u * (idx / 3u) + 1u)) * clip_height;
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
    out.color = particle.color;
    return out;
}

[[stage(fragment)]]
fn fs_main([[location(0)]] color: vec3<f32>) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(color, pass_settings.opacity);
}