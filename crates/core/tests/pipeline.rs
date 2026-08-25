// SPDX-License-Identifier: MPL-2.0

use idlewarden_capture::{CaptureBackend, NullBackend, Size};
use idlewarden_core::{Governor, GovernorConfig, Session, SessionState, Verdict};
use idlewarden_input::{DryRunBackend, GuardedInput, InputError, KillSwitch};
use idlewarden_plugin_api::{
    Confidence, InputCommand, Intent, MouseButton, Observation, Point, Signal, SignalId, Value,
};

const CONFIDENT: f64 = 0.95;

struct Run {
    session: Session,
    verdicts: Vec<Verdict>,
    refusals: Vec<InputError>,
}

fn observe(frame_id: u64, captured_at_ms: u64, confidence: f64) -> Observation {
    Observation {
        frame_id,
        captured_at_ms,
        signals: vec![Signal {
            id: SignalId("ui.screen_id".into()),
            value: Value::Enum("main".into()),
            confidence: Confidence::new(confidence),
        }],
    }
}

fn drive(config: GovernorConfig, kill: KillSwitch, confidence: f64, ticks: u32) -> Run {
    let mut capture = NullBackend::new(Size {
        width: 1280,
        height: 720,
    });
    let mut input = GuardedInput::new(DryRunBackend, kill);
    let mut governor = Governor::new(config, 0);
    let mut session = Session {
        state: SessionState::Running,
        ..Default::default()
    };

    let mut verdicts = Vec::new();
    let mut refusals = Vec::new();

    for _ in 0..ticks {
        let frame = capture.next_frame().expect("the null backend never fails");
        let observation = observe(frame.id, frame.captured_at_ms, confidence);
        let now = frame.captured_at_ms + 20;

        let verdict = governor.review(&Intent::new("collect_reward"), &observation, now);
        verdicts.push(verdict.clone());

        match verdict {
            Verdict::Allow => {
                let command = InputCommand::Click {
                    at: Point { x: 0.5, y: 0.5 },
                    button: MouseButton::Left,
                };
                match input.execute(&command) {
                    Ok(()) => session.actions_taken += 1,
                    Err(error) => refusals.push(error),
                }
            }
            Verdict::Reject { .. } => {}
            Verdict::Halt { reason } => {
                session.halt(reason);
                break;
            }
        }
    }

    Run {
        session,
        verdicts,
        refusals,
    }
}

#[test]
fn a_headless_session_drives_the_whole_pipeline_without_a_ui() {
    let run = drive(GovernorConfig::default(), KillSwitch::new(), CONFIDENT, 3);

    assert!(
        run.verdicts.iter().all(|v| *v == Verdict::Allow),
        "{:?}",
        run.verdicts
    );
    assert_eq!(run.session.actions_taken, 3);
    assert_eq!(run.session.state, SessionState::Running);
    assert!(run.refusals.is_empty());
}

#[test]
fn the_kill_switch_blocks_input_the_governor_had_already_allowed() {
    let kill = KillSwitch::new();
    kill.engage();

    let run = drive(GovernorConfig::default(), kill, CONFIDENT, 3);

    assert!(
        run.verdicts.iter().all(|v| *v == Verdict::Allow),
        "{:?}",
        run.verdicts
    );
    assert_eq!(run.session.actions_taken, 0);
    assert!(run
        .refusals
        .iter()
        .all(|e| matches!(e, InputError::KillSwitchEngaged)));
}

#[test]
fn collapsing_confidence_halts_the_session_before_any_action() {
    let run = drive(GovernorConfig::default(), KillSwitch::new(), 0.10, 3);

    assert_eq!(run.session.state, SessionState::Halted);
    assert_eq!(run.session.actions_taken, 0);
    assert!(run.session.last_reason.is_some());
}

#[test]
fn the_rate_limit_caps_actions_without_stopping_the_session() {
    let config = GovernorConfig {
        max_actions_per_minute: 1,
        ..Default::default()
    };
    let run = drive(config, KillSwitch::new(), CONFIDENT, 3);

    assert_eq!(run.session.actions_taken, 1);
    assert_eq!(run.session.state, SessionState::Running);
    assert_eq!(
        run.verdicts
            .iter()
            .filter(|v| matches!(v, Verdict::Reject { .. }))
            .count(),
        2
    );
}
