// SPDX-License-Identifier: MPL-2.0
//! The only vocabulary the UI and the Core share.
//!
//! Everything that happens is published as an [`Event`]. This is what makes the
//! session replayable, and replay is what makes "why did it click there at 3am"
//! an answerable question.

use idlewarden_plugin_api::{ActionOutcome, Intent, Observation, PluginId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    GameDetected {
        plugin: PluginId,
        window_title: String,
    },
    GameLost,
    PluginLoaded {
        plugin: PluginId,
        version: String,
    },
    PluginFailed {
        plugin: PluginId,
        reason: String,
    },
    Observed {
        observation: Observation,
    },
    /// The agent chose an intent, before the Governor has ruled on it.
    IntentProposed {
        intent: Intent,
    },
    /// The Governor refused an intent, and why.
    IntentRejected {
        intent: Intent,
        reason: String,
    },
    ActionStarted {
        intent: Intent,
    },
    ActionFinished {
        intent: Intent,
        outcome: ActionOutcome,
    },
    /// Confidence dropped, the screen became unrecognisable, a budget expired.
    AgentPaused {
        reason: String,
    },
    AgentResumed,
    KillSwitch,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Start {
        plugin: PluginId,
        profile: String,
    },
    Stop,
    Pause,
    Resume,
    /// Never execute input; log what would have been sent.
    SetDryRun {
        enabled: bool,
    },
}
