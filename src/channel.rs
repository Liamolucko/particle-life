use serde::{Deserialize, Serialize};

use crate::universe::Settings;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;

#[derive(Serialize, Deserialize)]
pub enum Command {
    Seed(Settings),
    ToggleWrap,
    RandomizeParticles,
    Run(usize),
}
