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

    fn verify(
        &mut self,
        intent: &Intent,
        _before: &Observation,
        _after: &Observation,
    ) -> ActionOutcome {
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
            link: Link::Perceived {
                capture: self.capture,
                perceiver: Box::new(self.perceiver),
                input: self.input,
            },
            tree: self.tree,
            actuator: Box::new(StubActuator::new()),
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
        link: Link::Perceived {
            capture: Box::new(NullBackend::new(Size {
                width: 640,
                height: 480,
            })),
            perceiver: Box::new(StubPerceiver {
                confidence: 0.95,
                fail_next: false,
            }),
            input: Box::new(DryRunBackend),
        },
        tree: Box::new(AlwaysDecides("collect")),
        actuator: Box::new(actuator),
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
    build.config.allowed_intents = Some(vec!["something_else".to_owned()]);
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

mod bridged {
    use std::sync::{Arc, Mutex};

    use idlewarden_bridge::transport::Transport;
    use idlewarden_bridge::{Bridge, BridgeError};
    use serde_json::json;

    use super::*;

    struct ModState {
        connected: bool,
        drop_on_act: bool,
        answer: serde_json::Value,
        acted: Vec<String>,
    }

    impl ModState {
        fn new() -> Arc<Mutex<Self>> {
            Arc::new(Mutex::new(ModState {
                connected: true,
                drop_on_act: false,
                answer: json!({ "response": "acted", "outcome": { "outcome": "succeeded" } }),
                acted: Vec::new(),
            }))
        }
    }

    struct FakeMod(Arc<Mutex<ModState>>);

    impl Transport for FakeMod {
        fn round_trip(&mut self, request: &str) -> Result<String, BridgeError> {
            let mut state = self.0.lock().unwrap();
            if !state.connected {
                return Err(BridgeError::Disconnected);
            }
            let request: serde_json::Value = serde_json::from_str(request).unwrap();
            let response = match request["request"].as_str().unwrap() {
                "hello" => json!({
                    "response": "hello",
                    "plugin": "dev.example.game",
                    "api_version": "^0.1",
                }),
                "observe" => json!({
                    "response": "observed",
                    "signals": [{ "id": SCREEN, "value": { "type": "enum", "value": "main" } }],
                }),
                "act" => {
                    if state.drop_on_act {
                        state.connected = false;
                        return Err(BridgeError::Disconnected);
                    }
                    let name = request["intent"]["name"].as_str().unwrap().to_owned();
                    state.acted.push(name);
                    state.answer.clone()
                }
                other => panic!("unexpected request {other}"),
            };
            Ok(response.to_string())
        }
    }

    fn runner(state: &Arc<Mutex<ModState>>, actuator: StubActuator) -> (Runner, KillSwitch) {
        runner_with(state, actuator, false)
    }

    fn runner_with(
        state: &Arc<Mutex<ModState>>,
        actuator: StubActuator,
        dry_run: bool,
    ) -> (Runner, KillSwitch) {
        let bridge = Bridge::open(Box::new(FakeMod(Arc::clone(state)))).expect("handshake");
        let kill = KillSwitch::new();
        let runner = Runner::new(Parts {
            link: Link::Bridged(bridge),
            tree: Box::new(AlwaysDecides("collect")),
            actuator: Box::new(actuator),
            kill: kill.clone(),
            governor: Governor::new(GovernorConfig::default(), 0),
            session: Session {
                state: SessionState::Running,
                dry_run,
                ..Default::default()
            },
        });
        (runner, kill)
    }

    fn finished(events: &[Event]) -> Option<ActionOutcome> {
        events.iter().find_map(|event| match event {
            Event::ActionFinished { outcome, .. } => Some(outcome.clone()),
            _ => None,
        })
    }

    #[test]
    fn a_bridged_observation_comes_from_the_mod_and_is_certain() {
        let state = ModState::new();
        let (mut runner, _kill) = runner(&state, StubActuator::new());

        runner.tick(1000);

        let events = runner.drain_events();
        let Some(Event::Observed { observation }) = events.first() else {
            panic!("the tick should start with an observation");
        };
        assert_eq!(observation.signals[0].id.as_str(), SCREEN);
        assert_eq!(observation.signals[0].confidence, Confidence::CERTAIN);
    }

    #[test]
    fn a_bridged_intent_is_carried_out_by_the_mod() {
        let state = ModState::new();
        let (mut runner, _kill) = runner(&state, StubActuator::new());

        runner.tick(1000);

        assert_eq!(
            names(&runner.drain_events()),
            ["observed", "proposed", "started"]
        );
        assert_eq!(state.lock().unwrap().acted, ["collect"]);
        assert_eq!(runner.session().actions_taken, 1);
    }

    #[test]
    fn the_mods_success_is_still_checked_against_the_next_observation() {
        let state = ModState::new();
        let mut actuator = StubActuator::new();
        actuator.outcome = ActionOutcome::Failed {
            reason: "the counter did not move".to_owned(),
        };
        let (mut runner, _kill) = runner(&state, actuator);

        runner.tick(1000);
        runner.drain_events();
        runner.tick(2000);

        assert_eq!(
            finished(&runner.drain_events()),
            Some(ActionOutcome::Failed {
                reason: "the counter did not move".to_owned()
            }),
            "a mod saying `succeeded` is not evidence; the post-condition is"
        );
    }

    #[test]
    fn a_failure_the_mod_reports_finishes_the_intent_at_once() {
        let state = ModState::new();
        state.lock().unwrap().answer = json!({
            "response": "acted",
            "outcome": { "outcome": "failed", "reason": "no gold to collect" },
        });
        let (mut runner, _kill) = runner(&state, StubActuator::new());

        runner.tick(1000);

        let events = runner.drain_events();
        assert_eq!(
            names(&events),
            ["observed", "proposed", "started", "finished"]
        );
        assert_eq!(
            finished(&events),
            Some(ActionOutcome::Failed {
                reason: "no gold to collect".to_owned()
            })
        );
        assert_eq!(runner.session().actions_taken, 0);
    }

    #[test]
    fn a_refusal_from_the_mod_is_a_rejection_not_a_lost_connection() {
        let state = ModState::new();
        state.lock().unwrap().answer = json!({ "response": "error", "message": "not in a menu" });
        let (mut runner, _kill) = runner(&state, StubActuator::new());

        runner.tick(1000);

        assert_eq!(
            finished(&runner.drain_events()),
            Some(ActionOutcome::Rejected {
                reason: "not in a menu".to_owned()
            })
        );
        assert_eq!(runner.session().state, SessionState::Running);
    }

    #[test]
    fn a_lost_connection_pauses_the_session_once_with_the_reason() {
        let state = ModState::new();
        let (mut runner, _kill) = runner(&state, StubActuator::new());
        runner.tick(1000);
        runner.drain_events();

        state.lock().unwrap().connected = false;
        runner.tick(2000);
        let first = runner.drain_events();
        runner.tick(3000);
        let second = runner.drain_events();

        assert_eq!(names(&first), ["paused"]);
        let Event::AgentPaused { reason } = &first[0] else {
            unreachable!()
        };
        assert!(
            reason.contains("closed"),
            "the reason has to say what happened: {reason}"
        );
        assert_eq!(runner.session().state, SessionState::Paused);
        assert!(
            second.is_empty(),
            "a dead mod must not repeat the same pause on every tick"
        );
    }

    #[test]
    fn losing_the_mod_mid_action_aborts_the_intent_and_pauses() {
        let state = ModState::new();
        state.lock().unwrap().drop_on_act = true;
        let (mut runner, _kill) = runner(&state, StubActuator::new());

        runner.tick(1000);

        let events = runner.drain_events();
        assert_eq!(
            names(&events),
            ["observed", "proposed", "started", "finished", "paused"]
        );
        assert_eq!(finished(&events), Some(ActionOutcome::Aborted));
        assert_eq!(runner.session().state, SessionState::Paused);
    }

    #[test]
    fn a_dry_run_still_observes_through_the_mod_but_never_asks_it_to_act() {
        let state = ModState::new();
        let (mut runner, _kill) = runner_with(&state, StubActuator::new(), true);

        runner.tick(1000);

        assert_eq!(
            names(&runner.drain_events()),
            ["observed", "proposed", "started"]
        );
        assert!(
            state.lock().unwrap().acted.is_empty(),
            "a mod acts on the real game, so a dry run must not reach it"
        );
    }

    #[test]
    fn the_kill_switch_stops_a_bridged_intent_before_it_reaches_the_mod() {
        let state = ModState::new();
        let (mut runner, kill) = runner(&state, StubActuator::new());

        kill.engage();
        runner.tick(1000);

        let events = runner.drain_events();
        assert!(names(&events).contains(&"kill"));
        assert_eq!(finished(&events), Some(ActionOutcome::Aborted));
        assert!(
            state.lock().unwrap().acted.is_empty(),
            "the mod must never hear about an intent the kill switch stopped"
        );
        assert_eq!(runner.session().state, SessionState::Halted);
    }
}
