// SPDX-License-Identifier: MPL-2.0
//! Headless driver.
//!
//! This exists before the UI on purpose: it proves the Core is genuinely
//! UI-independent, and it is far easier to debug a vision pipeline from a
//! terminal than through a web view.

use idlewarden_capture::{CaptureBackend, NullBackend, Size};
use idlewarden_core::{Governor, GovernorConfig, Session, SessionState};
use idlewarden_input::{DryRunBackend, GuardedInput, KillSwitch};
use idlewarden_plugin_api::{
    Confidence, InputCommand, Intent, MouseButton, Observation, Point, Signal, SignalId, Value,
};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tracing::info!(
        api = idlewarden_plugin_api::API_VERSION,
        "IdleWarden starting"
    );

    let kill = KillSwitch::new();
    let mut input = GuardedInput::new(DryRunBackend, kill.clone());
    let mut capture = NullBackend::new(Size {
        width: 1280,
        height: 720,
    });
    let mut governor = Governor::new(GovernorConfig::default(), 0);
    let mut session = Session {
        state: SessionState::Running,
        ..Default::default()
    };

    // A stand-in for the perception pass, until the vision backend lands.
    for _ in 0..3 {
        let frame = match capture.next_frame() {
            Ok(f) => f,
            Err(e) => {
                tracing::error!(%e, "capture failed");
                break;
            }
        };

        let observation = Observation {
            frame_id: frame.id,
            captured_at_ms: frame.captured_at_ms,
            signals: vec![Signal {
                id: SignalId("ui.screen_id".into()),
                value: Value::Enum("main".into()),
                confidence: Confidence::new(0.95),
            }],
        };

        let intent = Intent::new("collect_reward");
        let now = frame.captured_at_ms + 20;

        match governor.review(&intent, &observation, now) {
            idlewarden_core::Verdict::Allow => {
                let cmd = InputCommand::Click {
                    at: Point { x: 0.5, y: 0.5 },
                    button: MouseButton::Left,
                };
                match input.execute(&cmd) {
                    Ok(()) => {
                        session.actions_taken += 1;
                        tracing::info!(intent = %intent.name, "action executed");
                    }
                    Err(e) => tracing::warn!(%e, "input refused"),
                }
            }
            idlewarden_core::Verdict::Reject { reason } => {
                tracing::warn!(%reason, "intent rejected by governor")
            }
            idlewarden_core::Verdict::Halt { reason } => {
                session.halt(reason.clone());
                tracing::error!(%reason, "governor halted the session");
                break;
            }
        }
    }

    tracing::info!(
        actions = session.actions_taken,
        state = ?session.state,
        "session finished"
    );
}
