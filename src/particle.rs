use quicksilver::geom::Vector;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Particle {
    // We don't actually need to access any of this data from JS, just to be able to pass it around.
    #[wasm_bindgen(skip)]
    pub pos: Vector,
    #[wasm_bindgen(skip)]
    pub vel: Vector,
    #[wasm_bindgen(skip)]
    pub r#type: usize,
}
