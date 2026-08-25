// SPDX-License-Identifier: MPL-2.0
//! Session state machine: the lifecycle the UI renders and the CLI drives.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// No game window matched yet.
    Searching,
    /// Game found, plugin loaded, not acting.
    Ready,
    Running,
    /// Stopped by the Governor or the user; resumable.
    Paused,
    /// Stopped by the kill switch or a fatal error; requires a restart.
    Halted,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub state: SessionState,
    pub dry_run: bool,
    pub actions_taken: u64,
    pub last_reason: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            state: SessionState::Searching,
            // Dry-run is the default on purpose: nothing touches the mouse
            // until the user explicitly asks for it.
            dry_run: true,
            actions_taken: 0,
            last_reason: None,
        }
    }
}

impl Session {
    pub fn pause(&mut self, reason: impl Into<String>) {
        self.state = SessionState::Paused;
        self.last_reason = Some(reason.into());
    }

    pub fn halt(&mut self, reason: impl Into<String>) {
        self.state = SessionState::Halted;
        self.last_reason = Some(reason.into());
    }

    pub fn can_act(&self) -> bool {
        self.state == SessionState::Running
    }
}
