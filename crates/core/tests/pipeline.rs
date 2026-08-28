// SPDX-License-Identifier: MPL-2.0
//! The enforcement mechanism for ADR-0004.
//!
//! It drives a whole session through the public API, capture to input, with no
//! UI and no binary. If the Core ever grows a dependency on a toolkit, this
//! stops compiling, which is the point.
//!
//! It exercises the runner rather than reimplementing the loop: an integration
//! test that plays the pipeline by hand proves only that the test can, which is
//! what this file used to do.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use idlewarden_agent::{Node, Tick};
use idlewarden_capture::{Frame, NullBackend, Size};
use idlewarden_core::{
    Actuator, Command, Event, Governor, GovernorConfig, Parts, Runner, Session, SessionService,
    SessionState,
};
use idlewarden_input::{DryRunBackend, KillSwitch};
use idlewarden_plugin_api::{
    ActionOutcome, Confidence, InputCommand, Intent, MouseButton, Observation, Point, SignalId,
    Value,
};
use idlewarden_vision::{Extracted, Perceiver, VisionError};

struct Eyes;

impl Perceiver for Eyes {
    fn perceive(&mut self, _frame: &Frame) -> Result<Vec<Extracted>, VisionError> {
        Ok(vec![Extracted {
            id: SignalId("ui.screen_id".to_owned()),
            value: Value::Enum("main".to_owned()),
            confidence: Confidence::CERTAIN,
        }])
    }
}

struct Brain;

impl Node for Brain {
    fn tick(&mut self, _obs: &Observation) -> Tick {
        Tick::act(Intent::new("collect_reward"))
    }
}

struct Hands {
    planned: Arc<AtomicU32>,
}

impl Actuator for Hands {
    fn plan(&mut self, _intent: &Intent) -> Vec<InputCommand> {
        self.planned.fetch_add(1, Ordering::Relaxed);
        vec![InputCommand::Click {
            at: Point { x: 0.5, y: 0.5 },
            button: MouseButton::Left,
        }]
    }

    fn verify(&mut self, _intent: &Intent, _after: &Observation) -> ActionOutcome {
        ActionOutcome::Succeeded
    }
}

fn runner(planned: Arc<AtomicU32>, config: GovernorConfig) -> Runner {
    Runner::new(Parts {
        capture: Box::new(NullBackend::new(Size {
            width: 1280,
            height: 720,
        })),
        perceiver: Box::new(Eyes),
        tree: Box::new(Brain),
        actuator: Box::new(Hands { planned }),
        input: Box::new(DryRunBackend),
        kill: KillSwitch::new(),
        governor: Governor::new(config, 0),
        session: Session {
            state: SessionState::Running,
            ..Default::default()
        },
    })
}

fn collect_until(
    service: &SessionService,
    deadline: Duration,
    mut done: impl FnMut(&[Event]) -> bool,
) -> Vec<Event> {
    let started = Instant::now();
    let mut seen = Vec::new();
    while started.elapsed() < deadline {
        seen.extend(service.poll());
        if done(&seen) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    seen
}

#[test]
fn a_session_runs_on_its_own_thread_and_publishes_what_it_did() {
    let planned = Arc::new(AtomicU32::new(0));
    let service = SessionService::spawn(
        runner(Arc::clone(&planned), GovernorConfig::default()),
        Duration::from_millis(5),
    );

    let events = collect_until(&service, Duration::from_secs(5), |seen| {
        seen.iter()
            .any(|e| matches!(e, Event::ActionFinished { .. }))
    });

    assert!(
        events.iter().any(|e| matches!(e, Event::Observed { .. })),
        "the session never observed anything: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::IntentProposed { .. })),
        "the agent never proposed an intent"
    );
    let finished = events
        .iter()
        .find_map(|e| match e {
            Event::ActionFinished { outcome, .. } => Some(outcome),
            _ => None,
        })
        .expect("no action was ever verified");
    assert_eq!(*finished, ActionOutcome::Succeeded);
    assert!(planned.load(Ordering::Relaxed) > 0);
}

#[test]
fn a_paused_session_keeps_watching_but_stops_acting() {
    let planned = Arc::new(AtomicU32::new(0));
    let service = SessionService::spawn(
        runner(Arc::clone(&planned), GovernorConfig::default()),
        Duration::from_millis(5),
    );

    assert!(service.send(Command::Pause));
    collect_until(&service, Duration::from_millis(300), |seen| {
        seen.iter().any(|e| matches!(e, Event::AgentPaused { .. }))
    });

    let before = planned.load(Ordering::Relaxed);
    let events = collect_until(&service, Duration::from_millis(300), |_| false);
    let after = planned.load(Ordering::Relaxed);

    assert_eq!(before, after, "a paused session must not plan new actions");
    assert!(
        events.iter().any(|e| matches!(e, Event::Observed { .. })),
        "a paused session should still be watching"
    );
}

#[test]
fn the_governor_stops_a_runaway_agent_rather_than_the_agent_stopping_itself() {
    let planned = Arc::new(AtomicU32::new(0));
    let config = GovernorConfig {
        max_actions_per_minute: 2,
        ..Default::default()
    };
    let service = SessionService::spawn(
        runner(Arc::clone(&planned), config),
        Duration::from_millis(5),
    );

    let events = collect_until(&service, Duration::from_secs(5), |seen| {
        seen.iter()
            .filter(|e| matches!(e, Event::IntentRejected { .. }))
            .count()
            >= 3
    });

    let rejected = events
        .iter()
        .filter(|e| matches!(e, Event::IntentRejected { .. }))
        .count();
    assert!(
        rejected >= 3,
        "the rate limit never bit: {rejected} refusals"
    );
    assert_eq!(
        planned.load(Ordering::Relaxed),
        2,
        "the agent kept proposing, and exactly two actions got through"
    );
}

#[test]
fn dropping_the_handle_stops_the_thread() {
    let planned = Arc::new(AtomicU32::new(0));
    let service = SessionService::spawn(
        runner(Arc::clone(&planned), GovernorConfig::default()),
        Duration::from_millis(5),
    );
    collect_until(&service, Duration::from_millis(200), |seen| {
        !seen.is_empty()
    });

    drop(service);
    let after_drop = planned.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(100));

    assert_eq!(
        planned.load(Ordering::Relaxed),
        after_drop,
        "the session thread kept running after its handle was dropped"
    );
}
