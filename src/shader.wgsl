let kinds: u32 = 20u;

/// The symmetric properties of two kinds of particles.
struct SymmetricProperties {
    /// The distance below which particles begin to unconditionally repel each other.
    repel_distance: f32;
    /// The distance above which particles have no influence on each other (squared).
    influence_radius: f32;
};

[[block]]
struct Settings {
    friction: f32;
    // we can't use a boolean because spir-v doesn't actually define how it's represented for some reason.
    // so, this is either 0 or 1.
    flat_force: u32;

    width: f32;
    height: f32;

    colors: array<vec3<f32>, kinds>;
    symmetric_props: array<SymmetricProperties, 210>; // kinds * (kinds + 1) / 2 = 210
    attractions: array<f32, 400>; // kinds * kinds = 200
};

[[group(1), binding(0)]] var<uniform> settings: Settings;

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
    var out: VertexOutput;
    out.pos = vec4<f32>(particle.pos + vertex, 0.0, 1.0);
    out.color = settings.colors[particle.kind];
    return out;
}

[[stage(fragment)]]
fn fs_main([[location(0)]] color: vec3<f32>) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(color, 1.0);
}


/// Get a pair of particles' symmetric properties.
/// This is mainly here to decode the triangular indexing scheme.
///
/// It's logically a triangular arrangement, like so:
/// 0
/// 1 2
/// 3 4 5
///
/// This allows indexing uniquely for each pair by finding the greater of the two,
/// getting the offset of its row in this triangle, and then adding on the smaller index.
/// It's done this way around so that the offset of row n is just the nth triangular number,
/// with the simple formula n(n + 1) / 2.
fn get_symmetric_props(kind_a: u32, kind_b: u32) -> SymmetricProperties {
    var larger: u32;
    var smaller: u32;
    if (kind_a > kind_b) {
        smaller = kind_b;
        larger = kind_a;
    } else {
        smaller = kind_a;
        larger = kind_b;
    }

    return settings.symmetric_props[larger * (larger + 1u) / 2u + smaller];
}

[[block]]
struct Particles {
    particles: array<Particle>;
};

[[group(0), binding(0)]] var<storage, read> in_particles: Particles;
/// The buffer to write new velocities into.
[[group(0), binding(1)]] var<storage, read_write> out_particles: Particles;

// Since the numbers of particles are always multiples of 100, the workgroup size is the size required for 100 particles.
[[stage(compute), workgroup_size(100)]]
fn update_velocity([[builtin(global_invocation_id)]] pos: vec3<u32>) {
    let i = pos.x;

    let kind_a = in_particles.particles[i].kind;

    let num_particles = arrayLength(&in_particles.particles);

    var force = vec2<f32>(0.0, 0.0);

    for (var j: u32 = 0u; j < num_particles; j = j + 1u) {
        if (i == j) {
            continue;
        }

        let kind_b = in_particles.particles[j].kind;

        let attraction = settings.attractions[kind_a * kinds + kind_b];

        let symmetric_props = get_symmetric_props(kind_a, kind_b);

        let pos1 = in_particles.particles[i].pos;
        let pos2 = in_particles.particles[j].pos;

        // positions are in clip space, but everything else is in pixels, so scale this up.
        let delta = vec2<f32>((pos2.x - pos1.x) * settings.width, (pos2.y - pos1.y) * settings.height);

        let dist2 = delta.x * delta.x + delta.y * delta.y;

        if (dist2 > symmetric_props.influence_radius) {
            continue;
        }

        let dist = sqrt(dist2);

        var magnitude: f32;
        if (dist < symmetric_props.repel_distance) {
            magnitude = 2.0 * symmetric_props.repel_distance * (1.0 / (symmetric_props.repel_distance + 2.0) - 1.0 / (dist + 2.0));
        } else {
            let peak = 0.5 * (symmetric_props.repel_distance + symmetric_props.influence_radius);
            let base = symmetric_props.influence_radius - symmetric_props.repel_distance;

            magnitude = abs(dist - peak) * attraction / base;
        }

        force = force + (delta / dist) * magnitude;
    }

    let new_vel = in_particles.particles[i].vel + force;

    out_particles.particles[i].vel = new_vel;

    let pos_change = vec2<f32>(new_vel.x / settings.width, new_vel.y / settings.height);

    out_particles.particles[i].pos = in_particles.particles[i].pos + pos_change;
}