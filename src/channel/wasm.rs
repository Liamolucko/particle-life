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
    round: u32,
    ready: Rc<AtomicBool>,
    listener: Rc<Cell<Option<Box<dyn FnOnce()>>>>,
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
        };

        let closure = Closure::wrap(Box::new(move |msg: MessageEvent| {
            if let Ok(slice) = msg.data().dyn_into::<Uint8Array>() {
                buf.borrow_mut()
                    .push_back(serde_cbor::from_slice(&slice.to_vec()).unwrap());
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

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let el = self.buf.borrow_mut().pop_front();
        match el {
            Some((round, v)) => {
                self.worker
                    .post_message(&serialize(&Command::Step).unwrap())
                    .unwrap();
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

        if matches!(item, Command::Seed(_) | Command::RandomizeParticles) {
            self.round += 1;
            self.buf.borrow_mut().clear();

            for _ in 0..10 {
                self.worker
                    .post_message(&serialize(&Command::Step).unwrap())?;
            }
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
