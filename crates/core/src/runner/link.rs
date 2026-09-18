// SPDX-License-Identifier: MPL-2.0
use idlewarden_bridge::{Bridge, BridgeError};
use idlewarden_capture::{CaptureBackend, CaptureError};
use idlewarden_input::{GuardedInput, InputBackend, KillSwitch};
use idlewarden_plugin_api::{Observation, Signal};
use idlewarden_vision::Perceiver;

pub enum Link {
    Perceived {
        capture: Box<dyn CaptureBackend>,
        perceiver: Box<dyn Perceiver>,
        input: Box<dyn InputBackend>,
    },
    Bridged(Bridge),
}

pub(super) enum Wired {
    Perceived {
        capture: Box<dyn CaptureBackend>,
        perceiver: Box<dyn Perceiver>,
        input: GuardedInput<Box<dyn InputBackend>>,
    },
    Bridged(Bridge),
}

pub(super) enum Miss {
    WindowGone,
    Broken(String),
    Unreadable(String),
    BridgeLost(String),
}

impl Wired {
    pub(super) fn new(link: Link, kill: &KillSwitch) -> Self {
        match link {
            Link::Perceived {
                capture,
                perceiver,
                input,
            } => Wired::Perceived {
                capture,
                perceiver,
                input: GuardedInput::new(input, kill.clone()),
            },
            Link::Bridged(bridge) => Wired::Bridged(bridge),
        }
    }

    pub(super) fn sense(&mut self, now_ms: u64) -> Result<Observation, Miss> {
        match self {
            Wired::Perceived {
                capture, perceiver, ..
            } => perceive(capture.as_mut(), perceiver.as_mut(), now_ms),
            Wired::Bridged(bridge) => bridge.observe(now_ms).map_err(Miss::from),
        }
    }
}

impl From<BridgeError> for Miss {
    fn from(error: BridgeError) -> Self {
        if is_lost(&error) {
            Miss::BridgeLost(error.to_string())
        } else {
            Miss::Unreadable(error.to_string())
        }
    }
}

pub(super) fn is_lost(error: &BridgeError) -> bool {
    matches!(
        error,
        BridgeError::Disconnected | BridgeError::Io(_) | BridgeError::Connect { .. }
    )
}

fn perceive(
    capture: &mut dyn CaptureBackend,
    perceiver: &mut dyn Perceiver,
    now_ms: u64,
) -> Result<Observation, Miss> {
    let frame = capture.next_frame().map_err(|error| match error {
        CaptureError::WindowNotFound => Miss::WindowGone,
        other => Miss::Broken(other.to_string()),
    })?;

    let extracted = perceiver
        .perceive(&frame)
        .map_err(|error| Miss::Unreadable(error.to_string()))?;

    Ok(Observation {
        frame_id: frame.id,
        captured_at_ms: now_ms,
        signals: extracted
            .into_iter()
            .map(|e| Signal {
                id: e.id,
                value: e.value,
                confidence: e.confidence,
            })
            .collect(),
    })
}
