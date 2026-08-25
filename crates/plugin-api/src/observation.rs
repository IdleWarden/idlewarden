// SPDX-License-Identifier: Apache-2.0
//! What the Core perceives, and how sure it is about it.
//!
//! Vision is probabilistic. Carrying that uncertainty all the way to the agent
//! is what stops it from clicking on ghosts (ADR-0002), so every signal has a
//! [`Confidence`] and every observation has an age.

use crate::{manifest::SignalId, value::Value};
use serde::{Deserialize, Serialize};

/// A perception confidence in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(f64);

impl Confidence {
    pub const CERTAIN: Confidence = Confidence(1.0);

    pub fn new(v: f64) -> Self {
        Confidence(v.clamp(0.0, 1.0))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    pub id: SignalId,
    pub value: Value,
    pub confidence: Confidence,
}

/// One perception pass over one captured frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// Monotonic id of the frame this was derived from.
    pub frame_id: u64,
    /// Milliseconds since the session started, when the frame was captured.
    pub captured_at_ms: u64,
    pub signals: Vec<Signal>,
}

impl Observation {
    pub fn get(&self, id: &str) -> Option<&Signal> {
        self.signals.iter().find(|s| s.id.as_str() == id)
    }

    /// The lowest confidence across all signals — the Governor pauses the agent
    /// when this drops below the configured floor.
    pub fn weakest_confidence(&self) -> Confidence {
        self.signals
            .iter()
            .map(|s| s.confidence)
            .fold(Confidence::CERTAIN, |a, b| if b < a { b } else { a })
    }

    /// How stale this observation is, given the current session clock.
    pub fn age_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.captured_at_ms)
    }
}
