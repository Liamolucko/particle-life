let kinds: u32 = 20u;

let flat_force: u32 = 1u;
let wrap: u32 = 2u;

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

[[group(0), binding(0)]] var<uniform> settings: Settings;
[[group(1), binding(0)]] var<uniform> pass_settings: PassSettings;

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
fn vs_main(particle: Particle, [[location(3)]] vertex: vec2<f32>) -> VertexOutput {
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

    var out: VertexOutput;
    out.pos = vec4<f32>((pos + vertex) * settings.zoom, 0.0, 1.0);
    out.color = settings.colors[particle.kind];
    return out;
}

[[stage(fragment)]]
fn fs_main([[location(0)]] color: vec3<f32>) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(color, pass_settings.opacity);
}