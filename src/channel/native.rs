use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use futures::channel::mpsc::Receiver;
use futures::channel::mpsc::SendError;
use futures::channel::mpsc::Sender;
use futures::stream::FusedStream;
use futures::FutureExt;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use quicksilver::geom::Vector;

use crate::particle::Particle;
use crate::universe::Universe;

use super::Command;

pub async fn run_worker(mut p_tx: Sender<(u32, Vec<Particle>)>, mut cmd_rx: Receiver<Command>) {
    let mut universe = Universe::new(Vector::ZERO);
    let mut round = 0;

    loop {
        universe.step();

        futures::select! {
            _ = p_tx.send((round, universe.particles.clone())).fuse() => {},
            // Create a future which never completes, so select! will continue to poll this while waiting for the message to send
            _ = async {
                while let Some(cmd) = cmd_rx.next().await {
                    match cmd {
                        Command::Resize(size) => {
                            universe.resize(size);
                            round += 1;
                        },
                        Command::Seed(settings) => {
                            universe.seed(&settings);
                            round += 1;
                        },
                        Command::ToggleWrap => universe.wrap = !universe.wrap,
                        Command::RandomizeParticles => {
                            universe.randomize_particles();
                            round += 1;
                        },
                        _ => {}
                    }
                }
            }.fuse() => {},
        };

        if cmd_rx.is_terminated() {
            return;
        }
    }
}

pub struct StepChannel {
    /// This is incremented every time the game is reset, and attached to every set of particles sent from the worker to the main thread.
    /// This way, every time the game is reset, any leftover messages from the previous 'round' can be ignored because this number won't match.
    round: u32,
    rx: futures::channel::mpsc::Receiver<(u32, Vec<Particle>)>,
    tx: futures::channel::mpsc::Sender<Command>,
}

impl StepChannel {
    pub fn new() -> Self {
        use futures::channel::mpsc;

        // Channel which sends particles from worker thread to main thread
        let (p_tx, p_rx) = mpsc::channel(10);
        // Channel which sends messages from main thread to worker thread
        let (cmd_tx, cmd_rx) = mpsc::channel(10);

        #[cfg(target_arch = "wasm32")]
        {
            use js_sys::Array;
            use wasm_bindgen::prelude::*;
            use web_sys::Worker;

            let worker = Worker::new("./worker.js").unwrap();

            worker
                .post_message(&Array::of2(
                    &wasm_bindgen::module(),
                    &wasm_bindgen::memory(),
                ))
                .unwrap();
            worker
                .post_message(&Array::of2(
                    &JsValue::from(Box::into_raw(Box::new(p_tx)) as u32),
                    &JsValue::from(Box::into_raw(Box::new(cmd_rx)) as u32),
                ))
                .unwrap();

            // Don't drop the worker, otherwise it gets GC'd and killed.
            std::mem::forget(worker);
        }

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || futures::executor::block_on(run_worker(p_tx, cmd_rx)));

        Self {
            rx: p_rx,
            tx: cmd_tx,
            round: 0,
        }
    }
}

impl Stream for StepChannel {
    type Item = Vec<Particle>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        match self.rx.poll_next_unpin(cx) {
            Poll::Ready(Some((round, particles))) => {
                if round == self.round {
                    Poll::Ready(Some(particles))
                } else {
                    // Skip it and try again
                    self.poll_next_unpin(cx)
                }
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Sink<Command> for StepChannel {
    type Error = SendError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.tx.poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Command) -> Result<(), Self::Error> {
        if matches!(
            item,
            Command::Seed(_) | Command::RandomizeParticles | Command::Resize(_)
        ) {
            self.round += 1;
            while let Ok(_) = self.rx.try_next() {} // clear the buffer of outdated messages
        }
        self.tx.start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.tx.poll_flush_unpin(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.tx.poll_close_unpin(cx)
    }
}
