struct Particle {
    [[location(0)]] pos: vec2<f32>;
    [[location(1)]] kind: u32;
};

// Group 0 is the stuff which is used by rendering; group 1 is only used by the compute shader.

/// A pointless wrapper struct which is only needed because global variables have to have the [[block]] attribute.
[[block]]
struct Colors {
    colors: array<vec3<f32>>;
};

/// The colors of each particle kind.
[[group(0), binding(0)]] var<storage> colors: Colors;

struct VertexOutput {
    [[builtin(position)]] pos: vec4<f32>;
    [[location(0)]] color: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(particle: Particle, [[location(2)]] vertex: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.pos = vec4<f32>(particle.pos + vertex, 0.0, 1.0);
    out.color = colors.colors[particle.kind];
    return out;
}

[[stage(fragment)]]
fn fs_main([[location(0)]] color: vec3<f32>) -> [[location(0)]] vec4<f32> {
    return vec4<f32>(color, 1.0);
}

/// The symmetric properties of two kinds of particles.
struct SymmetricProperties {
    /// The distance below which particles begin to unconditionally repel each other.
    repel_distance: f32;
    /// The distance above which particles have no influence on each other (squared).
    influence_radius: f32;
};

[[block]]
struct SymmetricPropsStore {
    properties: array<SymmetricProperties>;
};

/// The symmetric properties of each particle.
/// This is indexed in a fancy 'triangular' fashion which means each pair's properties are only stored once.
[[group(1), binding(1)]] var<storage> symmetric_props: SymmetricPropsStore;

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

    return symmetric_props.properties[larger * (larger + 1u) / 2u + smaller];
}

[[block]]
struct Attractions {
    attractions: array<f32>;
};

/// The attractions of every particle to every other particle.
[[group(0), binding(2)]] var<storage> attractions: Attractions;

[[block]]
struct Velocities {
    velocities: array<vec2<f32>>;
};

[[group(0), binding(3)]] var<storage, read> velocities: Velocities;
/// The buffer to write new velocities into.
[[group(0), binding(4)]] var<storage, read_write> back_velocities: Velocities;

[[block]]
struct Particles {
    particles: array<Particle>;
};

[[group(0), binding(5)]] var<storage, read_write> particles: Particles;

// Since the numbers of particles are always multiples of 100, the workgroup size is the size required for 100 particles.
[[stage(compute), workgroup_size(100)]]
fn update_velocity([[builtin(global_invocation_id)]] pos: vec3<u32>) {
    let i = pos.x;

    let part_a = &particles.particles[i];
    let kind_a = *part_a.kind;

    let num_particles = arrayLength(&velocities.velocities);
    let num_kinds = arrayLength(&colors.colors);

    var force = vec2<f32>(0.0, 0.0);

    for (var j: u32 = 0u; j < num_particles; j = j + 1u) {
        if (i == j) {
            continue;
        }

        let part_b = &particles.particles[j];
        let kind_b = *part_b.kind;

        let attraction = attractions.attractions[kind_a * num_kinds + kind_b];

        let symmetric_props = get_symmetric_props(kind_a, kind_b);

        let delta = *part_b.pos - *part_a.pos;

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

    back_velocities.velocities[i] = back_velocities.velocities[i] + force;
    part_a.pos = *part_a.pos + back_velocities.velocities[i];
}