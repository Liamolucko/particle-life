use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use futures::channel::mpsc::SendError;
use futures::stream::FusedStream;
use futures::FutureExt;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use quicksilver::geom::Vector;

use crate::particle::Particle;

use super::Command;

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
        let (mut p_tx, p_rx) = mpsc::channel(10);
        // Channel which sends messages from main thread to worker thread
        let (cmd_tx, mut cmd_rx) = mpsc::channel(10);

        std::thread::spawn(move || {
            futures::executor::block_on(async {
                use crate::universe::Universe;

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
                                    Command::Step => {
                                        // This one is only used for wasm mode - native mode's stepping is automatically limited by the channel buffer.
                                    }
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
}

impl Stream for StepChannel {
    type Item = Vec<Particle>;

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
}

impl Sink<Command> for StepChannel {
    type Error = SendError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.tx.poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Command) -> Result<(), Self::Error> {
        if matches!(item, Command::Seed(_) | Command::RandomizeParticles) {
            self.round += 1;
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
