use bytemuck::Pod;
use bytemuck::Zeroable;
use palette::Hsv;
use palette::IntoColor;
use palette::Srgb;
use rand::Rng;
use rand_distr::Distribution;
use rand_distr::Normal;
use rand_distr::Uniform;

use crate::DIAMETER;

#[derive(Clone, Copy)]
pub struct Settings {
    pub particles: usize,
    pub kinds: usize,

    pub attraction_distr: Normal<f32>,
    pub repel_distance_distr: Uniform<f32>,
    pub influence_radius_distr: Uniform<f32>,

    pub friction: f32,
    pub flat_force: bool,
}

impl Settings {
    // Ideally these would be constants, but `Normal` and `Uniform` can't yet be created in `const` contexts because they're generic.
    pub fn balanced() -> Settings {
        Settings {
            kinds: 9,
            particles: 400,
            attraction_distr: Normal::new(-0.02, 0.06).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(0.0, 20.0),
            influence_radius_distr: Uniform::new_inclusive(20.0, 70.0),
            friction: 0.05,
            flat_force: false,
        }
    }

    pub fn chaos() -> Settings {
        Settings {
            kinds: 6,
            particles: 400,
            attraction_distr: Normal::new(0.02, 0.04).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(0.0, 30.0),
            influence_radius_distr: Uniform::new_inclusive(30.0, 100.0),
            friction: 0.01,
            flat_force: false,
        }
    }

    pub fn diversity() -> Settings {
        Settings {
            kinds: 12,
            particles: 400,
            attraction_distr: Normal::new(-0.01, 0.04).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(0.0, 20.0),
            influence_radius_distr: Uniform::new_inclusive(10.0, 60.0),
            friction: 0.05,
            flat_force: true,
        }
    }

    pub fn frictionless() -> Settings {
        Settings {
            kinds: 6,
            particles: 300,
            attraction_distr: Normal::new(0.01, 0.005).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(10.0, 10.0),
            influence_radius_distr: Uniform::new_inclusive(10.0, 60.0),
            friction: 0.0,
            flat_force: true,
        }
    }

    pub fn gliders() -> Settings {
        Settings {
            kinds: 6,
            particles: 400,
            attraction_distr: Normal::new(0.0, 0.06).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(0.0, 20.0),
            influence_radius_distr: Uniform::new_inclusive(10.0, 50.0),
            friction: 0.01,
            flat_force: true,
        }
    }

    pub fn homogeneity() -> Settings {
        Settings {
            kinds: 4,
            particles: 400,
            attraction_distr: Normal::new(0.0, 0.04).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(10.0, 10.0),
            influence_radius_distr: Uniform::new_inclusive(10.0, 80.0),
            friction: 0.05,
            flat_force: true,
        }
    }

    pub fn large_clusters() -> Settings {
        Settings {
            kinds: 6,
            particles: 400,
            attraction_distr: Normal::new(0.025, 0.02).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(0.0, 30.0),
            influence_radius_distr: Uniform::new_inclusive(30.0, 100.0),
            friction: 0.2,
            flat_force: false,
        }
    }

    pub fn medium_clusters() -> Settings {
        Settings {
            kinds: 6,
            particles: 400,
            attraction_distr: Normal::new(0.02, 0.05).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(0.0, 20.0),
            influence_radius_distr: Uniform::new_inclusive(20.0, 50.0),
            friction: 0.05,
            flat_force: false,
        }
    }

    pub fn quiescence() -> Settings {
        Settings {
            kinds: 6,
            particles: 300,
            attraction_distr: Normal::new(-0.02, 0.1).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(10.0, 20.0),
            influence_radius_distr: Uniform::new_inclusive(20.0, 60.0),
            friction: 0.2,
            flat_force: false,
        }
    }

    pub fn small_clusters() -> Settings {
        Settings {
            kinds: 6,
            particles: 600,
            attraction_distr: Normal::new(-0.005, 0.01).unwrap(),
            repel_distance_distr: Uniform::new_inclusive(10.0, 10.0),
            influence_radius_distr: Uniform::new_inclusive(20.0, 50.0),
            friction: 0.01,
            flat_force: false,
        }
    }
}

/// The symmetric properties of two kinds of particles.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct SymmetricProperties {
    /// The distance below which particles begin to unconditionally repel each other.
    pub repel_distance: f32,
    /// The distance above which particles have no influence on each other.
    pub influence_radius: f32,
}

// This is used instead of a plain [f32; 3] so that we can give it an alignment of 16, which vec3 has for some reason.
#[repr(C, align(16))]
#[derive(Pod, Zeroable, Clone, Copy, Default, Debug)]
pub struct Color {
    red: f32,
    green: f32,
    blue: f32,
    // make an actual field for this padding, so that it's accepted by bytemuck.
    padding: [u8; 4],
}

impl From<Srgb> for Color {
    fn from(color: Srgb) -> Self {
        Self {
            red: color.red,
            green: color.green,
            blue: color.blue,
            padding: [0; 4],
        }
    }
}

/// The number of kinds of particles which are always generated.
/// The `kinds` field of `Settings` then just specifies which are actually used.
const KINDS: usize = 20;

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug)]
pub struct RuntimeSettings {
    friction: f32,
    // 1 << 0: flat_force
    // 1 << 1: wrap
    flags: u32,

    width: f32,
    height: f32,

    colors: [Color; KINDS],
    symmetric_props: [SymmetricProperties; KINDS * (KINDS + 1) / 2],
    attractions: [f32; KINDS * KINDS],
}

impl RuntimeSettings {
    pub fn generate<R: Rng>(settings: Settings, width: u32, height: u32, rng: &mut R) -> Self {
        let mut this = Self {
            friction: settings.friction,
            flags: 0b10 & settings.flat_force as u32,

            width: width as f32,
            height: height as f32,

            colors: [Color::default(); KINDS],
            symmetric_props: [SymmetricProperties {
                influence_radius: 0.0,
                repel_distance: 0.0,
            }; KINDS * (KINDS + 1) / 2],
            attractions: [0.0; KINDS * KINDS],
        };

        // The angle between each color's hue.
        let angle = 360.0 / settings.kinds as f32;

        for i in 0..KINDS {
            let value = if i % 2 == 0 { 0.5 } else { 1.0 };
            let color: Srgb = Hsv::new(angle * i as f32, 1.0, value).into_color();
            this.colors[i] = color.into();

            for j in 0..KINDS {
                let index = i * KINDS + j;
                this.attractions[index] = if i == j {
                    -f32::abs(settings.attraction_distr.sample(rng))
                } else {
                    settings.attraction_distr.sample(rng)
                };

                if j <= i {
                    let repel_distance = if i == j {
                        DIAMETER
                    } else {
                        f32::max(settings.repel_distance_distr.sample(rng), DIAMETER)
                    };
                    let mut influence_radius = settings.influence_radius_distr.sample(rng);
                    if influence_radius < repel_distance {
                        influence_radius = repel_distance;
                    }

                    let index = i * (i + 1) / 2 + j;

                    this.symmetric_props[index] = SymmetricProperties {
                        repel_distance,
                        influence_radius,
                    };
                }
            }
        }

        this
    }
}
