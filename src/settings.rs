use rand_distr::Normal;
use rand_distr::Uniform;

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
    // Ideally these would be constants, but `Normal` and `Uniform` can't yet be
    // created in `const` contexts because they're generic.
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
