use std::cell::RefCell;
use std::collections::VecDeque;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

use futures::Stream;
use js_sys::Uint8Array;
use quicksilver::geom::Vector;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::AddEventListenerOptions;
use web_sys::MessageEvent;
use web_sys::Worker;

use crate::particle::Particle;

use super::Command;

pub struct StepChannel {
    buf: Rc<RefCell<VecDeque<(u32, Vec<Particle>)>>>,
    worker: Worker,
    round: u32,
    // Queue up initial messages until the worker communicates that it's ready to start recieving. Replaced with `None` once it is.
    tx_buf: Rc<RefCell<Option<Vec<Command>>>>,
}

impl StepChannel {
    pub fn new(size: Vector) -> Self {
        let worker = Worker::new("./worker.js").unwrap();

        let buf = Rc::new(RefCell::new(VecDeque::with_capacity(10)));
        let tx_buf = Rc::new(RefCell::new(Some(vec![Command::Resize(size)])));

        let chan = Self {
            worker: worker.clone(),
            buf: buf.clone(),
            round: 0,
            tx_buf: tx_buf.clone(),
        };

        let closure = Closure::wrap(Box::new(move |msg: MessageEvent| {
            if let Ok(slice) = msg.data().dyn_into::<Uint8Array>() {
                buf.borrow_mut()
                    .push_back(serde_cbor::from_slice(&slice.to_vec()).unwrap());
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

    // not actually async, but needs to have the same signature as the native version
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

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let el = self.buf.borrow_mut().pop_front();
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
                let closure = Closure::once(Box::new(move |ev: MessageEvent| {
                    ev.prevent_default();
                    waker.wake()
                }));
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
