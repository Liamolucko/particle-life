#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
use std::pin::Pin;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

#[cfg(not(target_arch = "wasm32"))]
use futures::stream::FusedStream;
#[cfg(not(target_arch = "wasm32"))]
use futures::FutureExt;
#[cfg(not(target_arch = "wasm32"))]
use futures::SinkExt;
use futures::Stream;
#[cfg(not(target_arch = "wasm32"))]
use futures::StreamExt;
use quicksilver::geom::Vector;
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::AddEventListenerOptions;
use web_sys::MessageEvent;
#[cfg(target_arch = "wasm32")]
use web_sys::Worker;

use crate::particle::Particle;
use crate::universe::Settings;
#[cfg(not(target_arch = "wasm32"))]
use crate::universe::Universe;

#[cfg(not(target_arch = "wasm32"))]
pub struct StepChannel {
    /// This is incremented every time the game is reset, and attached to every set of particles sent from the worker to the main thread.
    /// This way, every time the game is reset, any leftover messages from the previous 'round' can be ignored because this number won't match.
    round: u32,
    rx: futures::channel::mpsc::Receiver<(u32, Vec<Particle>)>,
    tx: futures::channel::mpsc::Sender<Command>,
}

#[cfg(target_arch = "wasm32")]
pub struct StepChannel {
    buf: Rc<RefCell<Vec<(u32, Vec<Particle>)>>>,
    worker: Worker,
    round: u32,
    // Queue up initial messages until the worker communicates that it's ready to start recieving. Replaced with `None` once it is.
    tx_buf: Rc<RefCell<Option<Vec<Command>>>>,
}

#[derive(Serialize, Deserialize)]
pub enum Command {
    Resize(Vector),
    Seed(Settings),
    ToggleWrap,
    RandomizeParticles,
    Step,
}

impl StepChannel {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(size: Vector) -> Self {
        use futures::channel::mpsc;

        // Channel which sends particles from worker thread to main thread
        let (mut p_tx, p_rx) = mpsc::channel(10);
        // Channel which sends messages from main thread to worker thread
        let (cmd_tx, mut cmd_rx) = mpsc::channel(10);

        thread::spawn(move || {
            futures::executor::block_on(async {
                let mut universe = Universe::new(size);
                let mut round = 0;

                loop {
                    universe.step();

                    futures::select! {
                        _ = p_tx.send((round, universe.particles.clone())).fuse() => {},
                        // Create a future which never completes, so select! will continue to poll this while waiting for the message to send
                        _ = async {
                            while let Some(cmd) = cmd_rx.next().await {
                                match cmd {
                                    Command::Resize(size) => universe.resize(size),
                                    Command::Seed(settings) => {
                                        universe.seed(&settings);
                                        round += 1;
                                    },
                                    Command::ToggleWrap => universe.wrap = !universe.wrap,
                                    Command::RandomizeParticles => {
                                        universe.randomize_particles();
                                        round += 1;
                                    },
                                }
                            }
                        }.fuse() => {},
                    };

                    if cmd_rx.is_terminated() {
                        return;
                    }
                }
            })
        });

        Self {
            rx: p_rx,
            tx: cmd_tx,
            round: 0,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(size: Vector) -> Self {
        let worker = Worker::new("./worker.js").unwrap();

        let buf = Rc::new(RefCell::new(Vec::with_capacity(10)));
        let tx_buf = Rc::new(RefCell::new(Some(vec![Command::Resize(size)])));

        let chan = Self {
            worker: worker.clone(),
            buf: buf.clone(),
            round: 0,
            tx_buf: tx_buf.clone(),
        };

        let closure = Closure::wrap(Box::new(move |msg: MessageEvent| {
            if let Ok(msg) = msg.data().into_serde() {
                buf.borrow_mut().push(msg);
            } else {
                for msg in tx_buf.borrow_mut().take().unwrap() {
                    worker
                        .post_message(&JsValue::from_serde(&msg).unwrap())
                        .unwrap();
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        chan.worker
            .set_onmessage(Some(closure.as_ref().unchecked_ref()));

        closure.forget();

        chan
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn send(&mut self, command: Command) {
        if matches!(command, Command::Seed(_) | Command::RandomizeParticles) {
            self.round += 1;
        }
        self.tx.send(command).await.unwrap();
    }

    // not actually async, but needs to have the same signature as the native version
    #[cfg(target_arch = "wasm32")]
    pub async fn send(&mut self, command: Command) {
        if let Some(buf) = self.tx_buf.borrow_mut().as_mut() {
            let reset = matches!(command, Command::Seed(_) | Command::RandomizeParticles);
            buf.push(command);

            if reset {
                self.round += 1;
                self.buf.borrow_mut().clear();

                for _ in 0..10 {
                    buf.push(Command::Step)
                }
            }
        } else {
            self.worker
                .post_message(&JsValue::from_serde(&command).unwrap())
                .unwrap();

            if matches!(command, Command::Seed(_) | Command::RandomizeParticles) {
                self.round += 1;
                self.buf.borrow_mut().clear();

                for _ in 0..10 {
                    self.worker
                        .post_message(&JsValue::from_serde(&Command::Step).unwrap())
                        .unwrap();
                }
            }
        }
    }
}

impl Stream for StepChannel {
    type Item = Vec<Particle>;

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let inner = self.get_mut();
        match inner.rx.poll_next_unpin(cx) {
            Poll::Ready(Some((round, particles))) => {
                if round == inner.round {
                    Poll::Ready(Some(particles))
                } else {
                    // Skip it and try again
                    inner.poll_next_unpin(cx)
                }
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let el = self.buf.borrow_mut().pop();
        match el {
            Some((round, v)) => {
                self.worker
                    .post_message(&JsValue::from_serde(&Command::Step).unwrap())
                    .unwrap();
                if round == self.round {
                    Poll::Ready(Some(v))
                } else {
                    self.poll_next(cx)
                }
            }
            None => {
                let waker = cx.waker().clone();
                let closure = Closure::once(Box::new(move || waker.wake()));
                self.worker
                    .add_event_listener_with_callback_and_add_event_listener_options(
                        "message",
                        closure.as_ref().unchecked_ref(),
                        &AddEventListenerOptions::new().once(true),
                    )
                    .unwrap();
                // TODO: This is a memory leak, but I'm not sure where I could store this and how to know when to drop it.
                closure.forget();
                Poll::Pending
            }
        }
    }
}
