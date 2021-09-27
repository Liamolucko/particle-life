let kinds: u32 = 20u;
let radius: f32 = 5.0;

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
    // The width and height go first so that we don't need to bother replacing them when the actual settings are changed.
    width: f32;
    height: f32;

    friction: f32;
    // we can't use a boolean because spir-v doesn't actually define how it's represented for some reason.
    // so, this is either 0 or 1.
    flags: u32;

    colors: array<vec3<f32>, kinds>;
    symmetric_props: array<SymmetricProperties, 210>; // kinds * (kinds + 1) / 2 = 210
    attractions: array<f32, 400>; // kinds * kinds = 200

    camera: vec2<f32>;
    zoom: f32;
};

[[group(0), binding(0)]] var<uniform> settings: Settings;

struct Particle {
    pos: vec2<f32>;
    vel: vec2<f32>;
    kind: u32;
};


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

[[group(1), binding(0)]] var<storage, read> in_particles: Particles;
[[group(1), binding(1)]] var<storage, read_write> out_particles: Particles;

// Since the numbers of particles are always multiples of 100, the workgroup size is the size required for 100 particles.
[[stage(compute), workgroup_size(100)]]
fn update_velocity([[builtin(global_invocation_id)]] pos: vec3<u32>) {
    let i = pos.x;

    let num_particles = arrayLength(&in_particles.particles);

    let clip_width = radius * 2.0 / settings.width;
    let clip_height = radius * 2.0 / settings.height;

    let kind1 = in_particles.particles[i].kind;
    var pos1 = in_particles.particles[i].pos;

    var force = vec2<f32>(0.0, 0.0);

    for (var j: u32 = 0u; j < num_particles; j = j + 1u) {
        if (i == j) {
            continue;
        }

        let kind2 = in_particles.particles[j].kind;
        let pos2 = in_particles.particles[j].pos;

        let attraction = settings.attractions[kind1 * kinds + kind2];

        let symmetric_props = get_symmetric_props(kind1, kind2);

        var delta = pos2 - pos1;

        if ((settings.flags & wrap) != 0u) {
            if (delta.x > 1.0) {
                delta.x = delta.x - 2.0;
            } elseif (delta.x < -1.0) {
                delta.x = delta.x + 2.0;
            }

            if (delta.y > 1.0) {
                delta.y = delta.y - 2.0;
            } elseif (delta.y < -1.0) {
                delta.y = delta.y + 2.0;
            }
        }

        // positions are in clip space, but everything else is in pixels, so scale this up.
        delta = vec2<f32>(delta.x * settings.width / 2.0, delta.y * settings.height / 2.0);

        let dist2 = (delta.x * delta.x) + (delta.y * delta.y);

        // Disallow small distances so we don't end up with 0 / 0.
        if (dist2 > (symmetric_props.influence_radius * symmetric_props.influence_radius) || dist2 < 0.01) {
            continue;
        }

        let dist = sqrt(dist2);

        var magnitude: f32;
        if (dist <= symmetric_props.repel_distance) {
            magnitude = 2.0 * symmetric_props.repel_distance * ((1.0 / (symmetric_props.repel_distance + 2.0)) - (1.0 / (dist + 2.0)));
        } else {
            var coefficient = 1.0;

            if ((settings.flags & flat_force) == 0u) {
                let peak = 0.5 * (symmetric_props.repel_distance + symmetric_props.influence_radius);
                let base = 0.5 * (symmetric_props.influence_radius - symmetric_props.repel_distance);
                coefficient = 1.0 - (abs(dist - peak) / base);
            }

            magnitude = attraction * coefficient;
        }

        force = force + ((delta / dist) * magnitude);
    }

    var new_vel = in_particles.particles[i].vel + force;

    let pos_change = vec2<f32>(new_vel.x * 2.0 / settings.width, new_vel.y * 2.0 / settings.height);

    pos1 = pos1 + pos_change;

    new_vel = new_vel * (1.0 - settings.friction);

    if ((settings.flags & wrap) != 0u) {
        if (pos1.x < -1.0 + clip_width) {
            pos1.x = pos1.x + 2.0;
        } elseif (pos1.x > 1.0 - clip_width) {
            pos1.x = pos1.x - 2.0;
        }

        if (pos1.y < -1.0 + clip_height) {
            pos1.y = pos1.y + 2.0;
        } elseif (pos1.y > 1.0 - clip_height) {
            pos1.y = pos1.y - 2.0;
        }
    } else {
        if (pos1.x < -1.0 + clip_width) {
            new_vel.x = -new_vel.x;
            pos1.x = -1.0 + clip_width;
        } elseif (pos1.x > 1.0 - clip_width) {
            new_vel.x = -new_vel.x;
            pos1.x = 1.0 - clip_width;
        }

        if (pos1.y < -1.0 + clip_height) {
            new_vel.y = -new_vel.y;
            pos1.y = -1.0 + clip_height;
        } elseif (pos1.y > 1.0 - clip_height) {
            new_vel.y = -new_vel.y;
            pos1.y = 1.0 - clip_height;
        }
    }

    out_particles.particles[i].pos = pos1;
    out_particles.particles[i].vel = new_vel;
}