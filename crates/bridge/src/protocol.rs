// SPDX-License-Identifier: MPL-2.0
//! The wire format, newline-delimited JSON, one request per line.
//!
//! The mod is the server. Every type here is deliberately small: a mod is
//! usually C# and every field costs someone hand-written marshalling.

use idlewarden_plugin_api::{ActionOutcome, ApiVersion, Intent, PluginId, SignalId, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum Request<'a> {
    Hello { api_version: &'a str },
    Observe,
    Act { intent: &'a Intent },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum Response {
    Hello {
        plugin: PluginId,
        api_version: ApiVersion,
    },
    Observed {
        signals: Vec<BridgeSignal>,
    },
    Acted {
        outcome: ActionOutcome,
    },
    Error {
        message: String,
    },
}

impl Response {
    pub fn label(&self) -> &'static str {
        match self {
            Response::Hello { .. } => "hello",
            Response::Observed { .. } => "observed",
            Response::Acted { .. } => "acted",
            Response::Error { .. } => "error",
        }
    }
}

/// A signal as the mod reports it.
///
/// There is no confidence field, and that is the point (ADR-0014): a bridge
/// reads what the game already knows, so the Core stamps `Confidence::CERTAIN`
/// itself. A mod that is unsure should not emit the signal at all.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BridgeSignal {
    pub id: SignalId,
    pub value: Value,
}
