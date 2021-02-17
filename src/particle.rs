use quicksilver::geom::Vector;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Particle {
    pub pos: Vector,
    pub vel: Vector,
    pub r#type: usize,
}
