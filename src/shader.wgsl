let num_circle_points: u32 = 32u;

let pi: f32 = 3.14159265358979323846264338327950288;

struct Settings {
    wrap: u32;

    zoom: f32;
    camera: vec2<f32>;

    // Explicitly declare the padding here so that the dimensions and circle points are next to each other.
    // Also, I'm not sure this padding would even be there outside webgl, so we need to add it manually to make the layout consistent.
    padding1: u32;
    padding2: u32;

    /// The horizontal/vertical radius of a particle in clip space.
    /// (A perfect circle in pixel space isn't always a perfect circle in clip space, hence why can't just pass `radius`.)
    horiz_rad: f32;
    vert_rad: f32;

    // On WebGL, anything in an array is treated as though it is at minimum the size of a vec4,
    // so manually set the stride up on other platforms too.
    circle_points: [[stride(16)]] array<vec2<f32>, num_circle_points>;
};

/// Settings which differ between render passes.
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
        } else if (pos.x < -1.0) {
            pos.x = pos.x + 2.0;
        }

        if (pos.y > 1.0) {
            pos.y = pos.y - 2.0;
        } else if (pos.y < -1.0) {
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
            if (point == num_circle_points) {
                point = 0u;
            }
        }

        circle_point = settings.circle_points[point];
    }

    var vertex = pos + circle_point;

    if (settings.wrap != 0u) {
        if (pos.x + settings.horiz_rad > 1.0) {
            let middle = pos.x + cos(half_circle_angle * f32(2u * (idx / 3u) + 1u)) * settings.horiz_rad;
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
        } else if (pos.x - settings.horiz_rad < -1.0) {
            let middle = pos.x + cos(half_circle_angle * f32(2u * (idx / 3u) + 1u)) * settings.horiz_rad;
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

        if (pos.y + settings.vert_rad > 1.0) {
            let middle = pos.y + sin(half_circle_angle * f32(2u * (idx / 3u) + 1u)) * settings.vert_rad;
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
        } else if (pos.y - settings.vert_rad < -1.0) {
            let middle = pos.y + sin(half_circle_angle * f32(2u * (idx / 3u) + 1u)) * settings.vert_rad;
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