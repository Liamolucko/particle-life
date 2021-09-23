use bytemuck::Pod;
use bytemuck::Zeroable;
use palette::FromColor;
use palette::Hsv;
use palette::Srgb;
use rand::Rng;
use rand_distr::Distribution;

use crate::settings::Settings;
use crate::DIAMETER;

/// The symmetric properties of two kinds of particles.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SymmetricProperties {
    /// The distance below which particles begin to unconditionally repel each other.
    repel_distance: f32,
    /// The distance above which particles have no influence on each other.
    influence_radius: f32,
}

pub struct ParticleKinds {
    pub colors: Vec<Srgb>,
    pub attractions: Vec<f32>,
    pub symmetric_properties: Vec<SymmetricProperties>,
}

impl ParticleKinds {
    pub fn random<R: Rng>(settings: Settings, rng: &mut R) -> Self {
        let num = settings.kinds;

        let mut colors = Vec::with_capacity(num);
        let mut attractions = Vec::with_capacity(num * num);
        let mut symmetric_properties = Vec::with_capacity(num * (num + 1) / 2);

        // The angle between each color's hue.
        let angle = 360.0 / num as f32;

        for i in 0..num {
            let value = if i % 2 == 0 { 0.5 } else { 1.0 };
            let color = Hsv::new(angle * i as f32, 1.0, value);
            colors.push(Srgb::from_color(color));

            for j in 0..num {
                attractions.push(if i == j {
                    -f32::abs(settings.attraction_distr.sample(rng))
                } else {
                    settings.attraction_distr.sample(rng)
                });

                if j <= i {
                    let repel_distance = if i == j {
                        DIAMETER
                    } else {
                        settings.repel_distance_distr.sample(rng)
                    };
                    let mut influence_radius = settings.influence_radius_distr.sample(rng);
                    if influence_radius < repel_distance {
                        influence_radius = repel_distance;
                    }
                    symmetric_properties.push(SymmetricProperties {
                        repel_distance,
                        influence_radius,
                    });
                }
            }
        }

        Self {
            colors,
            attractions,
            symmetric_properties,
        }
    }
}
