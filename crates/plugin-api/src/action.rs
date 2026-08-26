// SPDX-License-Identifier: Apache-2.0
//! Two strictly separated layers (ADR-0003):
//!
//! * [`Intent`], what the *agent* decides ("buy_upgrade"). Game vocabulary.
//! * [`InputCommand`], what the *Core* executes. Window-relative, never
//!   screen-absolute, so it survives the window being moved or the display
//!   changing.
//!
//! The plugin owns the translation between them. Every action reports an
//! [`ActionOutcome`]; an action with no verifiable post-condition is a bug.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A point in window-client space, normalised to `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key(pub String);

/// What the agent decided to do, in the plugin's own vocabulary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Intent {
    pub name: String,
    #[serde(default)]
    pub params: BTreeMap<String, crate::value::Value>,
}

impl Intent {
    pub fn new(name: impl Into<String>) -> Self {
        Intent {
            name: name.into(),
            params: BTreeMap::new(),
        }
    }
}

/// A primitive the Core knows how to execute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum InputCommand {
    MoveTo { to: Point },
    Click { at: Point, button: MouseButton },
    KeyPress { key: Key },
    KeyDown { key: Key },
    KeyUp { key: Key },
    Scroll { at: Point, delta: i32 },
    Wait { ms: u64 },
}

/// The result of executing one intent. Transactional and interruptible:
/// without this, robustness is impossible (ADR-0003).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ActionOutcome {
    /// The post-condition was observed to hold.
    Succeeded,
    /// The sequence ran but the post-condition did not hold.
    Failed {
        reason: String,
    },
    /// A precondition (focus, resolution, known screen) was not met.
    Rejected {
        reason: String,
    },
    /// The Governor or the user stopped it mid-flight.
    Aborted,
    TimedOut {
        after_ms: u64,
    },
}

impl ActionOutcome {
    pub fn is_success(&self) -> bool {
        matches!(self, ActionOutcome::Succeeded)
    }
}
