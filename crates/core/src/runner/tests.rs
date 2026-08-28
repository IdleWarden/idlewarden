// SPDX-License-Identifier: MPL-2.0

use idlewarden_agent::{Node, Tick};
use idlewarden_capture::{CaptureBackend, CaptureError, Frame, NullBackend, Size, WindowHandle};
use idlewarden_input::{DryRunBackend, InputBackend, InputError, KillSwitch};
use idlewarden_plugin_api::{
    ActionOutcome, Confidence, InputCommand, Intent, MouseButton, Observation, Point, SignalId,
    Value,
};
use idlewarden_vision::{Extracted, Perceiver, VisionError};

use super::*;
use crate::event::Command;
use crate::governor::GovernorConfig;

const SCREEN: &str = "ui.screen_id";

struct StubPerceiver {
    confidence: f64,
    fail_next: bool,
}

impl Perceiver for StubPerceiver {
    fn perceive(&mut self, _frame: &Frame) -> Result<Vec<Extracted>, VisionError> {
        if self.fail_next {
            self.fail_next = false;
            return Err(VisionError::AnchorLost("hud".to_owned()));
        }
        Ok(vec![Extracted {
            id: SignalId(SCREEN.to_owned()),
            value: Value::Enum("main".to_owned()),
            confidence: Confidence::new(self.confidence),
        }])
    }
}

struct AlwaysDecides(&'static str);

impl Node for AlwaysDecides {
    fn tick(&mut self, _obs: &Observation) -> Tick {
        Tick::act(Intent::new(self.0))
    }
}

struct NeverDecides;

impl Node for NeverDecides {
    fn tick(&mut self, _obs: &Observation) -> Tick {
        Tick::failure()
    }
}

struct StubActuator {
    commands: Vec<InputCommand>,
    outcome: ActionOutcome,
    verified: Vec<String>,
}

impl StubActuator {
    fn new() -> Self {
        StubActuator {
            commands: vec![InputCommand::Click {
                at: Point { x: 0.5, y: 0.5 },
                button: MouseButton::Left,
            }],
            outcome: ActionOutcome::Succeeded,
            verified: Vec::new(),
        }
    }
}

impl Actuator for StubActuator {
    fn plan(&mut self, _intent: &Intent) -> Vec<InputCommand> {
        self.commands.clone()
    }

    fn verify(&mut self, intent: &Intent, _after: &Observation) -> ActionOutcome {
        self.verified.push(intent.name.clone());
        self.outcome.clone()
    }
}

struct BrokenCapture(CaptureError);

impl CaptureBackend for BrokenCapture {
    fn next_frame(&mut self) -> Result<std::sync::Arc<Frame>, CaptureError> {
        Err(match &self.0 {
            CaptureError::WindowNotFound => CaptureError::WindowNotFound,
            other => CaptureError::Backend(other.to_string()),
        })
    }

    fn window(&self) -> WindowHandle {
        WindowHandle(0)
    }
}

struct RefusingInput;

impl InputBackend for RefusingInput {
    fn execute(&mut self, _cmd: &InputCommand) -> Result<(), InputError> {
        Err(InputError::NotFocused)
    }
}

struct Build {
    capture: Box<dyn CaptureBackend>,
    perceiver: StubPerceiver,
    tree: Box<dyn Node>,
    input: Box<dyn InputBackend>,
    config: GovernorConfig,
    running: bool,
}

impl Build {
    fn new() -> Self {
        Build {
            capture: Box::new(NullBackend::new(Size {
                width: 640,
                height: 480,
            })),
            perceiver: StubPerceiver {
                confidence: 0.95,
                fail_next: false,
            },
            tree: Box::new(AlwaysDecides("collect")),
            input: Box::new(DryRunBackend),
            config: GovernorConfig::default(),
            running: true,
        }
    }

    fn build(self) -> (Runner, KillSwitch) {
        let kill = KillSwitch::new();
        let session = Session {
            state: if self.running {
                SessionState::Running
            } else {
                SessionState::Searching
            },
            ..Default::default()
        };
        let runner = Runner::new(Parts {
            capture: self.capture,
            perceiver: Box::new(self.perceiver),
            tree: self.tree,
            actuator: Box::new(StubActuator::new()),
            input: self.input,
            kill: kill.clone(),
            governor: Governor::new(self.config, 0),
            session,
        });
        (runner, kill)
    }
}

fn names(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
            Event::GameDetected { .. } => "detected",
            Event::GameLost => "lost",
            Event::PluginLoaded { .. } => "plugin",
            Event::PluginFailed { .. } => "plugin_failed",
            Event::Observed { .. } => "observed",
            Event::IntentProposed { .. } => "proposed",
            Event::IntentRejected { .. } => "rejected",
            Event::ActionStarted { .. } => "started",
            Event::ActionFinished { .. } => "finished",
            Event::AgentPaused { .. } => "paused",
            Event::AgentResumed => "resumed",
            Event::KillSwitch => "kill",
            Event::Error { .. } => "error",
        })
        .collect()
}

#[test]
fn one_tick_observes_proposes_and_acts_but_finishes_nothing_yet() {
    let (mut runner, _kill) = Build::new().build();

    runner.tick(1000);

    assert_eq!(
        names(&runner.drain_events()),
        ["observed", "proposed", "started"]
    );
    assert_eq!(runner.session().actions_taken, 1);
}

#[test]
fn the_post_condition_is_checked_against_the_next_observation() {
    let (mut runner, _kill) = Build::new().build();

    runner.tick(1000);
    runner.drain_events();
    runner.tick(2000);

    let events = runner.drain_events();
    assert_eq!(
        names(&events),
        ["observed", "finished", "proposed", "started"]
    );

    let Event::ActionFinished { outcome, .. } = &events[1] else {
        panic!("expected the previous action to finish");
    };
    assert_eq!(*outcome, ActionOutcome::Succeeded);
}

#[test]
fn a_failed_post_condition_is_reported_rather_than_assumed_successful() {
    let kill = KillSwitch::new();
    let mut actuator = StubActuator::new();
    actuator.outcome = ActionOutcome::Failed {
        reason: "the counter did not move".to_owned(),
    };
    let mut runner = Runner::new(Parts {
        capture: Box::new(NullBackend::new(Size {
            width: 640,
            height: 480,
        })),
        perceiver: Box::new(StubPerceiver {
            confidence: 0.95,
            fail_next: false,
        }),
        tree: Box::new(AlwaysDecides("collect")),
        actuator: Box::new(actuator),
        input: Box::new(DryRunBackend),
        kill,
        governor: Governor::new(GovernorConfig::default(), 0),
        session: Session {
            state: SessionState::Running,
            ..Default::default()
        },
    });

    runner.tick(1000);
    runner.drain_events();
    runner.tick(2000);

    let events = runner.drain_events();
    let Event::ActionFinished { outcome, .. } = &events[1] else {
        panic!("expected a finish");
    };
    assert!(matches!(outcome, ActionOutcome::Failed { .. }));
}

#[test]
fn a_session_that_cannot_act_still_observes() {
    let mut build = Build::new();
    build.running = false;
    let (mut runner, _kill) = build.build();

    runner.tick(1000);

    assert_eq!(names(&runner.drain_events()), ["observed"]);
    assert_eq!(runner.session().actions_taken, 0);
}

#[test]
fn a_rejected_intent_is_published_with_its_reason_and_costs_no_action() {
    let mut build = Build::new();
    build.config.allowed_intents = vec!["something_else".to_owned()];
    let (mut runner, _kill) = build.build();

    runner.tick(1000);

    let events = runner.drain_events();
    assert_eq!(names(&events), ["observed", "proposed", "rejected"]);
    assert_eq!(runner.session().actions_taken, 0);
}

#[test]
fn collapsing_confidence_halts_the_session_and_stops_later_ticks() {
    let mut build = Build::new();
    build.perceiver.confidence = 0.05;
    let (mut runner, _kill) = build.build();

    runner.tick(1000);
    let events = runner.drain_events();
    assert_eq!(names(&events), ["observed", "proposed", "paused"]);
    assert_eq!(runner.session().state, SessionState::Halted);

    runner.tick(2000);
    assert!(
        runner.drain_events().is_empty(),
        "a halted session must stop doing work, not merely stop acting"
    );
}

#[test]
fn the_kill_switch_aborts_the_action_and_halts() {
    let (mut runner, kill) = Build::new().build();
    kill.engage();

    runner.tick(1000);

    let events = runner.drain_events();
    assert_eq!(
        names(&events),
        ["observed", "proposed", "started", "kill", "paused", "finished"]
    );
    let Event::ActionFinished { outcome, .. } = events.last().unwrap() else {
        panic!("expected a finish");
    };
    assert_eq!(*outcome, ActionOutcome::Aborted);
    assert_eq!(runner.session().state, SessionState::Halted);
    assert_eq!(runner.session().actions_taken, 0);
}

#[test]
fn input_refusing_reports_a_rejected_action_without_halting() {
    let mut build = Build::new();
    build.input = Box::new(RefusingInput);
    let (mut runner, _kill) = build.build();

    runner.tick(1000);

    let events = runner.drain_events();
    assert_eq!(
        names(&events),
        ["observed", "proposed", "started", "finished"]
    );
    let Event::ActionFinished { outcome, .. } = events.last().unwrap() else {
        panic!("expected a finish");
    };
    assert!(matches!(outcome, ActionOutcome::Rejected { .. }));
    assert_eq!(runner.session().state, SessionState::Running);
}

#[test]
fn losing_the_window_drops_back_to_searching_instead_of_failing() {
    let mut build = Build::new();
    build.capture = Box::new(BrokenCapture(CaptureError::WindowNotFound));
    let (mut runner, _kill) = build.build();

    runner.tick(1000);

    assert_eq!(names(&runner.drain_events()), ["lost"]);
    assert_eq!(runner.session().state, SessionState::Searching);
}

#[test]
fn a_structural_capture_failure_halts_rather_than_looping_blind() {
    let mut build = Build::new();
    build.capture = Box::new(BrokenCapture(CaptureError::ExclusiveFullscreen));
    let (mut runner, _kill) = build.build();

    runner.tick(1000);

    assert_eq!(runner.session().state, SessionState::Halted);
}

#[test]
fn a_perception_failure_costs_the_tick_but_not_the_session() {
    let mut build = Build::new();
    build.perceiver.fail_next = true;
    let (mut runner, _kill) = build.build();

    runner.tick(1000);
    assert_eq!(names(&runner.drain_events()), ["error"]);
    assert_eq!(runner.session().state, SessionState::Running);

    runner.tick(2000);
    assert_eq!(
        names(&runner.drain_events()),
        ["observed", "proposed", "started"]
    );
}

#[test]
fn a_tree_that_decides_nothing_produces_no_intent() {
    let mut build = Build::new();
    build.tree = Box::new(NeverDecides);
    let (mut runner, _kill) = build.build();

    runner.tick(1000);

    assert_eq!(names(&runner.drain_events()), ["observed"]);
}

#[test]
fn commands_reach_the_session_and_a_refusal_is_published() {
    let mut build = Build::new();
    build.running = false;
    let (mut runner, _kill) = build.build();

    runner.apply(&Command::Resume);

    let events = runner.drain_events();
    assert_eq!(names(&events), ["error"]);
    let Event::Error { message } = &events[0] else {
        panic!("expected the refusal to surface");
    };
    assert!(message.contains("not paused"), "{message}");
}

#[test]
fn pausing_stops_the_agent_acting_and_resuming_starts_it_again() {
    let (mut runner, _kill) = Build::new().build();

    runner.apply(&Command::Pause);
    runner.drain_events();
    runner.tick(1000);
    assert_eq!(names(&runner.drain_events()), ["observed"]);

    runner.apply(&Command::Resume);
    runner.drain_events();
    runner.tick(2000);
    assert_eq!(
        names(&runner.drain_events()),
        ["observed", "proposed", "started"]
    );
}

#[test]
fn the_rate_limit_caps_actions_without_ending_the_session() {
    let mut build = Build::new();
    build.config.max_actions_per_minute = 1;
    let (mut runner, _kill) = build.build();

    for tick in 1..=3 {
        runner.tick(tick * 1000);
    }

    assert_eq!(runner.session().actions_taken, 1);
    assert_eq!(runner.session().state, SessionState::Running);
}
