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

/// The expected size for the step buffer to be. Not actually guaranteed though.
/// The bigger the buffer, the less likely there are to be dropped steps,
/// but it means that the simulation will run behind when you do something like resize.
const BUF_SIZE: usize = 10;

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
}

impl StepChannel {
    pub fn new() -> Self {
        let worker = Worker::new("./worker.js").unwrap();

        let buf = Rc::new(RefCell::new(VecDeque::with_capacity(BUF_SIZE)));
        let ready = Rc::new(AtomicBool::new(false));
        let listener = Rc::new(Cell::new(None));

        let chan = Self {
            worker: worker.clone(),
            buf: buf.clone(),
            round: 0,
            ready: ready.clone(),
            listener: listener.clone(),
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

    /// Sends a message to the worker with the number of steps which need to be run before the next frame.
    pub fn req(&self) {
        self.worker
            .post_message(
                &serialize(&Command::Run(
                    BUF_SIZE - usize::min(self.buf.borrow().len(), BUF_SIZE),
                ))
                .unwrap(),
            )
            .unwrap();
    }
}

impl Stream for StepChannel {
    type Item = Vec<Particle>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let el = self.buf.borrow_mut().pop_front();
        match el {
            Some((round, v)) => {
                if round == self.round {
                    Poll::Ready(Some(v))
                } else {
                    self.poll_next(cx)
                }
            }
            None => {
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

        if matches!(
            item,
            Command::Seed(_) | Command::RandomizeParticles | Command::Resize(_)
        ) {
            self.round += 1;
            self.buf.borrow_mut().clear();
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
