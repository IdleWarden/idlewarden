// SPDX-License-Identifier: MPL-2.0
//! Actuation (ADR-0007).
//!
//! Two non-negotiables live here:
//!
//! * The game window must hold focus. Background input (`PostMessage`) works
//!   for very few games and we do not promise it.
//! * A global kill switch must be registered before any backend is allowed to
//!   emit a single event.

mod coords;
mod humanise;
mod keys;

#[cfg(windows)]
mod sendinput;

#[cfg(windows)]
pub use sendinput::SendInputBackend;

use idlewarden_plugin_api::{InputCommand, Point};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("the game window does not have focus")]
    NotFocused,
    #[error("point {0:?} is outside the game window")]
    OutsideWindow(Point),
    #[error("the kill switch is engaged")]
    KillSwitchEngaged,
    #[error("input backend failed: {0}")]
    Backend(String),
}

/// Shared, cheap to check, flipped by a global hotkey. Every backend checks it
/// before each command, not once per sequence.
#[derive(Clone, Default)]
pub struct KillSwitch(Arc<AtomicBool>);

impl KillSwitch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn engage(&self) {
        self.0.store(true, Ordering::SeqCst);
        tracing::warn!("kill switch engaged; all input halted");
    }

    pub fn reset(&self) {
        self.0.store(false, Ordering::SeqCst);
    }

    pub fn is_engaged(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Timing jitter, so a sequence does not look like a metronome.
#[derive(Debug, Clone, Copy)]
pub struct Humanisation {
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for Humanisation {
    fn default() -> Self {
        Humanisation {
            min_delay_ms: 40,
            max_delay_ms: 160,
        }
    }
}

pub trait InputBackend: Send {
    fn execute(&mut self, cmd: &InputCommand) -> Result<(), InputError>;
}

impl InputBackend for Box<dyn InputBackend> {
    fn execute(&mut self, cmd: &InputCommand) -> Result<(), InputError> {
        (**self).execute(cmd)
    }
}

/// Wraps any backend with the checks that must never be skipped.
pub struct GuardedInput<B: InputBackend> {
    backend: B,
    kill: KillSwitch,
}

impl<B: InputBackend> GuardedInput<B> {
    pub fn new(backend: B, kill: KillSwitch) -> Self {
        GuardedInput { backend, kill }
    }

    pub fn execute(&mut self, cmd: &InputCommand) -> Result<(), InputError> {
        if self.kill.is_engaged() {
            return Err(InputError::KillSwitchEngaged);
        }
        if let Some(p) = point_of(cmd) {
            if !(0.0..=1.0).contains(&p.x) || !(0.0..=1.0).contains(&p.y) {
                return Err(InputError::OutsideWindow(p));
            }
        }
        self.backend.execute(cmd)
    }
}

fn point_of(cmd: &InputCommand) -> Option<Point> {
    match cmd {
        InputCommand::MoveTo { to } => Some(*to),
        InputCommand::Click { at, .. } => Some(*at),
        InputCommand::Scroll { at, .. } => Some(*at),
        _ => None,
    }
}

/// No-op backend: logs what it would have done. The default in dry-run mode.
pub struct DryRunBackend;

impl InputBackend for DryRunBackend {
    fn execute(&mut self, cmd: &InputCommand) -> Result<(), InputError> {
        tracing::info!(?cmd, "dry-run: not sending input");
        Ok(())
    }
}
