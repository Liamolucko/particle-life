use quicksilver::geom::Vector;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Particle {
    pub pos: Vector,
    // This isn't needed for rendering, so don't waste time serializing/deserializing it.
    #[serde(skip)]
    pub vel: Vector,
    pub r#type: usize,
}
