use futures::{Sink, SinkExt, Stream, StreamExt};
use quicksilver::geom::Vector;
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::{particle::Particle, universe::Settings};

pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[derive(Serialize, Deserialize)]
pub enum Command {
    Resize(Vector),
    Seed(Settings),
    ToggleWrap,
    RandomizeParticles,
    Run(usize),
}

pub enum StepChannel {
    Native(native::StepChannel),
    #[cfg(target_arch = "wasm32")]
    Wasm(wasm::StepChannel),
}

impl StepChannel {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        if !js_sys::global().has_own_property(&JsValue::from_str("SharedArrayBuffer")) {
            return Self::Wasm(wasm::StepChannel::new());
        }
        Self::Native(native::StepChannel::new())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn req(&self) {
        if let Self::Wasm(chan) = self {
            chan.req()
        }
    }
}

impl Stream for StepChannel {
    type Item = Vec<Particle>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.get_mut() {
            Self::Native(chan) => chan.poll_next_unpin(cx),
            #[cfg(target_arch = "wasm32")]
            Self::Wasm(chan) => chan.poll_next_unpin(cx),
        }
    }
}

impl Sink<Command> for StepChannel {
    type Error = ();

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match self.get_mut() {
            // TODO fix errors
            Self::Native(chan) => chan.poll_ready_unpin(cx).map_err(|_| ()),
            #[cfg(target_arch = "wasm32")]
            Self::Wasm(chan) => chan.poll_ready_unpin(cx).map_err(|_| ()),
        }
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: Command) -> Result<(), Self::Error> {
        match self.get_mut() {
            // TODO fix errors
            Self::Native(chan) => chan.start_send_unpin(item).map_err(|_| ()),
            #[cfg(target_arch = "wasm32")]
            Self::Wasm(chan) => chan.start_send_unpin(item).map_err(|_| ()),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match self.get_mut() {
            // TODO fix errors
            Self::Native(chan) => chan.poll_flush_unpin(cx).map_err(|_| ()),
            #[cfg(target_arch = "wasm32")]
            Self::Wasm(chan) => chan.poll_flush_unpin(cx).map_err(|_| ()),
        }
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        match self.get_mut() {
            // TODO fix errors
            Self::Native(chan) => chan.poll_close_unpin(cx).map_err(|_| ()),
            #[cfg(target_arch = "wasm32")]
            Self::Wasm(chan) => chan.poll_close_unpin(cx).map_err(|_| ()),
        }
    }
}
