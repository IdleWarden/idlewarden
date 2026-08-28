// SPDX-License-Identifier: MPL-2.0
//! Running a [`Runner`] on its own thread.
//!
//! Deliberately thin. Everything worth testing lives in `Runner::tick`, which
//! needs no thread and no clock; this only decides when to call it and where the
//! events go.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::event::{Command, Event};
use crate::runner::Runner;

/// Idle games do not move fast, and a slow loop is the single biggest
/// simplification available (ADR-0005).
pub const DEFAULT_TICK: Duration = Duration::from_millis(250);

pub struct SessionService {
    commands: Sender<Command>,
    events: Receiver<Event>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl SessionService {
    pub fn spawn(mut runner: Runner, tick: Duration) -> Self {
        let (commands, inbox) = channel::<Command>();
        let (outbox, events) = channel::<Event>();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);

        let worker = thread::Builder::new()
            .name("idlewarden-session".to_owned())
            .spawn(move || {
                let started = Instant::now();
                while !stopping.load(Ordering::Relaxed) {
                    loop {
                        match inbox.try_recv() {
                            Ok(command) => runner.apply(&command),
                            Err(TryRecvError::Empty) => break,
                            // The handle is gone, so nothing can read the
                            // events either. Stop rather than spin.
                            Err(TryRecvError::Disconnected) => return,
                        }
                    }

                    runner.tick(started.elapsed().as_millis() as u64);

                    for event in runner.drain_events() {
                        if outbox.send(event).is_err() {
                            return;
                        }
                    }

                    thread::sleep(tick);
                }
            })
            .expect("the session thread could not be started");

        SessionService {
            commands,
            events,
            stop,
            worker: Some(worker),
        }
    }

    pub fn send(&self, command: Command) -> bool {
        self.commands.send(command).is_ok()
    }

    /// Everything published since the last call. Never blocks: a UI polls this
    /// on its own schedule and must not be able to stall the session.
    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

impl Drop for SessionService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
