// SPDX-License-Identifier: MPL-2.0
//! Talking to a mod the user installed (ADR-0014).
//!
//! There is no injection anywhere in this crate. The mod is a server the user
//! put inside the game process through the game's own loader; we are a client
//! that connects to it and would fail cleanly if it were absent.
//!
//! A bridge replaces capture and vision, not the rest of the pipeline. What it
//! produces is an ordinary [`Observation`]; the agent, the Governor and the
//! kill switch cannot tell the difference.

pub mod protocol;
pub mod transport;
pub mod websocket;

#[cfg(test)]
mod tests;

use idlewarden_plugin_api::{
    ActionOutcome, Confidence, Intent, Observation, PluginId, Signal, API_VERSION,
};

use protocol::{Request, Response};
use transport::Transport;

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("`{endpoint}` is not a valid bridge endpoint name")]
    InvalidEndpoint { endpoint: String },
    #[error("cannot reach the bridge at `{endpoint}`: {source}")]
    Connect {
        endpoint: String,
        source: std::io::Error,
    },
    #[error("the bridge closed the connection")]
    Disconnected,
    #[error("bridge transport failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("the bridge sent something that is not a valid message: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error("expected a `{expected}` response, got `{got}`")]
    Unexpected {
        expected: &'static str,
        got: &'static str,
    },
    #[error("the bridge targets API version {found}, host implements {supported}")]
    IncompatibleApi { found: String, supported: String },
    #[error("the bridge refused: {0}")]
    Refused(String),
}

pub struct Bridge {
    transport: Box<dyn Transport>,
    plugin: PluginId,
    next_frame_id: u64,
}

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge")
            .field("plugin", &self.plugin)
            .field("next_frame_id", &self.next_frame_id)
            .finish()
    }
}

/// Connect to the named endpoint of a mod the user installed.
pub fn connect(name: &str) -> Result<Bridge, BridgeError> {
    if !transport::is_valid_endpoint_name(name) {
        return Err(BridgeError::InvalidEndpoint {
            endpoint: name.to_owned(),
        });
    }
    Bridge::open(transport::connect(name)?)
}

/// The pipe first, because a mod that serves one is already listening and says
/// no immediately. A page mod cannot serve anything, so it connects to us
/// instead, and that side has to be waited for (ADR-0018).
///
/// The pipe is tried again throughout the wait, not only once. A mod has no pipe
/// open for a moment between two hosts, and none at all until the game has
/// loaded it, so a single miss is not an answer. A mod that does answer, even to
/// refuse, is returned at once: its reason is the one the user needs, not a
/// timeout about a socket nobody was going to use.
pub fn connect_or_listen(name: &str, wait: std::time::Duration) -> Result<Bridge, BridgeError> {
    match connect(name) {
        Err(BridgeError::Connect { .. }) => {}
        answered => return answered,
    }

    tracing::info!("no pipe yet, watching for one while a page may connect over the socket");
    let listener = websocket::bind(websocket::DEFAULT_PORT)?;
    let deadline = std::time::Instant::now() + wait;
    loop {
        match websocket::accept(&listener, name, websocket::POLL) {
            Err(BridgeError::Connect { .. }) => {}
            accepted => return Bridge::open(accepted?),
        }

        match connect(name) {
            Err(BridgeError::Connect { .. }) => {}
            answered => return answered,
        }

        if std::time::Instant::now() >= deadline {
            return Err(BridgeError::Connect {
                endpoint: format!(
                    "{} or ws://127.0.0.1/{name}",
                    transport::endpoint_path(name)
                ),
                source: std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "no mod answered before the wait expired",
                ),
            });
        }
    }
}

impl Bridge {
    /// Handshake first: a mod built against an incompatible contract is refused
    /// here rather than misbehaving on the first observation (ADR-0010).
    pub fn open(mut transport: Box<dyn Transport>) -> Result<Self, BridgeError> {
        let response = exchange(
            transport.as_mut(),
            &Request::Hello {
                api_version: API_VERSION,
            },
        )?;

        let (plugin, api_version) = match response {
            Response::Hello {
                plugin,
                api_version,
            } => (plugin, api_version),
            Response::Error { message } => return Err(BridgeError::Refused(message)),
            other => {
                return Err(BridgeError::Unexpected {
                    expected: "hello",
                    got: other.label(),
                })
            }
        };

        if !api_version.is_satisfied_by_host() {
            return Err(BridgeError::IncompatibleApi {
                found: api_version.0.to_string(),
                supported: API_VERSION.to_owned(),
            });
        }

        tracing::info!(plugin = plugin.as_str(), "bridge connected");
        Ok(Bridge {
            transport,
            plugin,
            next_frame_id: 0,
        })
    }

    pub fn plugin(&self) -> &PluginId {
        &self.plugin
    }

    /// A bridge read is synchronous, so the Core stamps the clock itself: the
    /// observation is never stale and the mod cannot claim otherwise.
    pub fn observe(&mut self, now_ms: u64) -> Result<Observation, BridgeError> {
        let signals = match exchange(self.transport.as_mut(), &Request::Observe)? {
            Response::Observed { signals } => signals,
            Response::Error { message } => return Err(BridgeError::Refused(message)),
            other => {
                return Err(BridgeError::Unexpected {
                    expected: "observed",
                    got: other.label(),
                })
            }
        };

        self.next_frame_id += 1;
        Ok(Observation {
            frame_id: self.next_frame_id,
            captured_at_ms: now_ms,
            signals: signals
                .into_iter()
                .map(|s| Signal {
                    id: s.id,
                    value: s.value,
                    confidence: Confidence::CERTAIN,
                })
                .collect(),
        })
    }

    pub fn act(&mut self, intent: &Intent) -> Result<ActionOutcome, BridgeError> {
        match exchange(self.transport.as_mut(), &Request::Act { intent })? {
            Response::Acted { outcome } => Ok(outcome),
            Response::Error { message } => Err(BridgeError::Refused(message)),
            other => Err(BridgeError::Unexpected {
                expected: "acted",
                got: other.label(),
            }),
        }
    }
}

fn exchange(transport: &mut dyn Transport, request: &Request<'_>) -> Result<Response, BridgeError> {
    let line = transport.round_trip(&serde_json::to_string(request)?)?;
    Ok(serde_json::from_str(line.trim_end())?)
}
