// SPDX-License-Identifier: MPL-2.0
//! Session state machine: the lifecycle the UI renders.
//!
//! Commands come from the UI and go through [`Session::apply`]; detection
//! transitions come from the pipeline. The UI owns neither.

use idlewarden_plugin_api::PluginId;
use serde::{Deserialize, Serialize};

use crate::event::Command;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum Refusal {
    #[error("no game is ready, so there is nothing to start")]
    NoGameReady,
    #[error("the session is halted and cannot be resumed; restart it")]
    Halted,
    #[error("the session is not running")]
    NotRunning,
    #[error("the session is not paused")]
    NotPaused,
    #[error("dry-run cannot be changed while the session is running")]
    RunningDryRunChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub state: SessionState,
    pub dry_run: bool,
    pub actions_taken: u64,
    pub last_reason: Option<String>,
    pub plugin: Option<PluginId>,
    pub profile: Option<String>,
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
            plugin: None,
            profile: None,
        }
    }
}

impl Session {
    pub fn apply(&mut self, command: &Command) -> Result<(), Refusal> {
        if self.state == SessionState::Halted {
            return Err(Refusal::Halted);
        }

        match command {
            Command::Start { plugin, profile } => {
                if self.state != SessionState::Ready {
                    return Err(Refusal::NoGameReady);
                }
                self.plugin = Some(plugin.clone());
                self.profile = Some(profile.clone());
                self.state = SessionState::Running;
                self.last_reason = None;
            }
            Command::Stop => {
                if !matches!(self.state, SessionState::Running | SessionState::Paused) {
                    return Err(Refusal::NotRunning);
                }
                self.state = SessionState::Ready;
                self.last_reason = None;
            }
            Command::Pause => {
                if self.state != SessionState::Running {
                    return Err(Refusal::NotRunning);
                }
                self.pause("paused by the user");
            }
            Command::Resume => {
                if self.state != SessionState::Paused {
                    return Err(Refusal::NotPaused);
                }
                self.state = SessionState::Running;
                self.last_reason = None;
            }
            Command::SetDryRun { enabled } => {
                if self.state == SessionState::Running {
                    return Err(Refusal::RunningDryRunChange);
                }
                self.dry_run = *enabled;
            }
        }

        Ok(())
    }

    pub fn game_detected(&mut self, plugin: PluginId) {
        if self.state == SessionState::Halted {
            return;
        }
        self.plugin = Some(plugin);
        self.state = SessionState::Ready;
    }

    pub fn game_lost(&mut self) {
        if self.state == SessionState::Halted {
            return;
        }
        self.state = SessionState::Searching;
        self.plugin = None;
    }

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

#[cfg(test)]
mod tests;
