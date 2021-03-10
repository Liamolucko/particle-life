use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use futures::Sink;
use futures::Stream;
use js_sys::Uint8Array;
use serde::Serialize;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::MessageEvent;
use web_sys::Worker;

use crate::particle::Particle;

use super::Command;

/// Serializes a value to CBOR and stores it in a Uint8Array
fn serialize(value: &impl Serialize) -> serde_cbor::Result<Uint8Array> {
    Ok(Uint8Array::from(serde_cbor::to_vec(value)?.as_slice()))
}

pub struct StepChannel {
    buf: Rc<RefCell<VecDeque<(u32, Vec<Particle>)>>>,
    worker: Worker,
    /// Used to ignore leftover messages from prior to resetting
    round: u32,
    /// The worker takes a little bit to start receiving messages,
    /// so this is false until it sends a message saying it's ready.
    ready: Rc<AtomicBool>,
    listener: Rc<Cell<Option<Box<dyn FnOnce()>>>>,
    /// The worker sends back steps in batches of 10, so we need to only send step commands every 10th request.
    req_num: u8,
    /// Don't queue up too many steps while waiting for the previous round's steps to run out.
    /// Previously it would build up every time you reset until it was queueing thousands of step commands.
    round_catchup: bool,
}

impl StepChannel {
    pub fn new() -> Self {
        let worker = Worker::new("./worker.js").unwrap();

        let buf = Rc::new(RefCell::new(VecDeque::with_capacity(10)));
        let ready = Rc::new(AtomicBool::new(false));
        let listener = Rc::new(Cell::new(None));

        let chan = Self {
            worker: worker.clone(),
            buf: buf.clone(),
            round: 0,
            ready: ready.clone(),
            listener: listener.clone(),
            req_num: 0,
            round_catchup: false,
        };

        let closure = Closure::wrap(Box::new(move |msg: MessageEvent| {
            if let Ok(slice) = msg.data().dyn_into::<Uint8Array>() {
                let (round, particles): (u32, Vec<Vec<Particle>>) =
                    serde_cbor::from_slice(&slice.to_vec()).unwrap();
                for particles in particles {
                    buf.borrow_mut().push_back((round, particles));
                }
            } else {
                ready.swap(true, Ordering::Relaxed);
            }

            if let Some(listener) = listener.take() {
                listener()
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        chan.worker
            .set_onmessage(Some(closure.as_ref().unchecked_ref()));

        closure.forget();

        chan
    }
}

impl Stream for StepChannel {
    type Item = Vec<Particle>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let el = self.buf.borrow_mut().pop_front();
        match el {
            Some((round, v)) => {
                if self.req_num == 9 {
                    self.req_num = 0;
                    self.worker
                        .post_message(&serialize(&Command::Step).unwrap())
                        .unwrap();
                } else {
                    self.req_num += 1;
                }
                if round == self.round {
                    self.round_catchup = false;
                    Poll::Ready(Some(v))
                } else {
                    self.round_catchup = true;
                    self.poll_next(cx)
                }
            }
            None => {
                if !self.round_catchup {
                    // This should build up a queue of steps until it never runs out.
                    self.worker
                        .post_message(&serialize(&Command::Step).unwrap())
                        .unwrap();
                }
                let waker = cx.waker().clone();
                self.listener.set(Some(Box::new(|| waker.wake())));
                Poll::Pending
            }
        }
    }
}

impl Sink<Command> for StepChannel {
    type Error = JsValue;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.ready.load(Ordering::Relaxed) {
            Poll::Ready(Ok(()))
        } else {
            let waker = cx.waker().clone();
            self.listener.set(Some(Box::new(|| waker.wake())));
            Poll::Pending
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Command) -> Result<(), Self::Error> {
        self.worker.post_message(&serialize(&item).unwrap())?;

        if matches!(item, Command::Seed(_) | Command::RandomizeParticles) {
            self.round += 1;
            self.buf.borrow_mut().clear();

            self.worker
                .post_message(&serialize(&Command::Step).unwrap())?;
        }

        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        panic!("StepChannel cannot be closed")
    }
}
