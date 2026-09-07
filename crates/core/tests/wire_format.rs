// SPDX-License-Identifier: MPL-2.0
//! Pins the JSON the UI actually receives.
//!
//! The desktop app restates the Core's wire types by hand (ADR-0004 keeps
//! `ts-rs`/`specta` derives out of this crate), so nothing used to notice when
//! the two drifted. This test serialises one value of every variant the UI can
//! see and writes it as a TypeScript module the app type-checks against its
//! hand-written model. Rename a field here and either this test fails, or the
//! desktop `typecheck` does.
//!
//! Regenerate with `UPDATE_WIRE_FORMAT=1 cargo test -p idlewarden-core`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use idlewarden_core::{Command, Event, Refusal, Session, SessionState};
use idlewarden_plugin_api::{
    ActionOutcome, Confidence, Intent, Observation, PluginId, Signal, SignalId, Value,
};

fn generated_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src/app/session/wire-format.generated.ts")
}

fn intent() -> Intent {
    let mut params = BTreeMap::new();
    params.insert("slot".to_owned(), Value::Int(3));
    Intent {
        name: "collect_reward".to_owned(),
        params,
    }
}

fn observation() -> Observation {
    Observation {
        frame_id: 42,
        captured_at_ms: 1_250,
        signals: vec![
            Signal {
                id: SignalId("ui.screen_id".to_owned()),
                value: Value::Enum("main".to_owned()),
                confidence: Confidence::new(0.93),
            },
            Signal {
                id: SignalId("progress.bar".to_owned()),
                value: Value::Ratio(0.5),
                confidence: Confidence::CERTAIN,
            },
        ],
    }
}

fn every_value() -> Vec<Value> {
    vec![
        Value::Bool(true),
        Value::Int(-7),
        Value::Float(1.5),
        Value::Ratio(0.25),
        Value::Text("gold".to_owned()),
        Value::Point { x: 0.5, y: 0.75 },
        Value::Rect {
            x: 0.1,
            y: 0.2,
            w: 0.3,
            h: 0.4,
        },
        Value::Enum("main".to_owned()),
    ]
}

fn every_event() -> Vec<Event> {
    vec![
        Event::GameDetected {
            plugin: PluginId("dev.idlewarden.example-game".to_owned()),
            window_title: "Example Game".to_owned(),
        },
        Event::GameLost,
        Event::PluginLoaded {
            plugin: PluginId("dev.idlewarden.example-game".to_owned()),
            version: "26.9.5".to_owned(),
        },
        Event::PluginFailed {
            plugin: PluginId("dev.idlewarden.broken".to_owned()),
            reason: "rules.json is not valid JSON".to_owned(),
        },
        Event::Observed {
            observation: observation(),
        },
        Event::IntentProposed { intent: intent() },
        Event::IntentRejected {
            intent: intent(),
            reason: "rate ceiling reached".to_owned(),
        },
        Event::ActionStarted { intent: intent() },
        Event::ActionFinished {
            intent: intent(),
            outcome: ActionOutcome::Succeeded,
        },
        Event::ActionFinished {
            intent: intent(),
            outcome: ActionOutcome::Failed {
                reason: "the post-condition did not hold".to_owned(),
            },
        },
        Event::ActionFinished {
            intent: intent(),
            outcome: ActionOutcome::Rejected {
                reason: "the window lost focus".to_owned(),
            },
        },
        Event::ActionFinished {
            intent: intent(),
            outcome: ActionOutcome::Aborted,
        },
        Event::ActionFinished {
            intent: intent(),
            outcome: ActionOutcome::TimedOut { after_ms: 3_000 },
        },
        Event::AgentPaused {
            reason: "confidence dropped below the floor".to_owned(),
        },
        Event::AgentResumed,
        Event::KillSwitch,
        Event::Error {
            message: "capture backend unavailable".to_owned(),
        },
    ]
}

fn every_command() -> Vec<Command> {
    vec![
        Command::Start {
            plugin: PluginId("dev.idlewarden.example-game".to_owned()),
            profile: "default".to_owned(),
        },
        Command::Stop,
        Command::Pause,
        Command::Resume,
        Command::SetDryRun { enabled: false },
    ]
}

fn every_session() -> Vec<Session> {
    let running = Session {
        state: SessionState::Running,
        dry_run: false,
        actions_taken: 12,
        plugin: Some(PluginId("dev.idlewarden.example-game".to_owned())),
        profile: Some("default".to_owned()),
        ..Session::default()
    };

    let paused = Session {
        state: SessionState::Paused,
        last_reason: Some("confidence dropped below the floor".to_owned()),
        ..running.clone()
    };

    let ready = Session {
        state: SessionState::Ready,
        ..Session::default()
    };

    let halted = Session {
        state: SessionState::Halted,
        ..Session::default()
    };

    vec![Session::default(), ready, running, paused, halted]
}

fn every_refusal() -> Vec<Refusal> {
    vec![
        Refusal::NoGameReady,
        Refusal::Halted,
        Refusal::NotRunning,
        Refusal::NotPaused,
        Refusal::RunningDryRunChange,
    ]
}

fn block<T: serde::Serialize>(name: &str, ts_type: &str, values: &[T]) -> String {
    let rendered: Vec<String> = values
        .iter()
        .map(|value| {
            let json = serde_json::to_string_pretty(value).expect("the wire types serialise");
            json.lines()
                .map(|line| format!("  {line}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect();

    format!(
        "export const {name}: readonly {ts_type}[] = [\n{},\n];\n",
        rendered.join(",\n")
    )
}

fn generate() -> String {
    let mut out = String::from(
        "import type {\n  Command,\n  Refusal,\n  Session,\n  SessionEvent,\n  SignalValue,\n} from \"./session.model\";\n\n",
    );
    out.push_str(&block("COMMANDS", "Command", &every_command()));
    out.push('\n');
    out.push_str(&block("EVENTS", "SessionEvent", &every_event()));
    out.push('\n');
    out.push_str(&block("SESSIONS", "Session", &every_session()));
    out.push('\n');
    out.push_str(&block("REFUSALS", "Refusal", &every_refusal()));
    out.push('\n');
    out.push_str(&block("SIGNAL_VALUES", "SignalValue", &every_value()));
    out
}

#[test]
fn the_generated_wire_format_matches_what_the_core_serialises() {
    let path = generated_path();
    let generated = generate();

    if std::env::var_os("UPDATE_WIRE_FORMAT").is_some() {
        std::fs::write(&path, &generated).expect("the generated module could be written");
        return;
    }

    let committed = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");

    assert_eq!(
        committed, generated,
        "the committed wire format is stale; regenerate it with \
         `UPDATE_WIRE_FORMAT=1 cargo test -p idlewarden-core`"
    );
}

#[test]
fn every_event_variant_is_covered() {
    let names: Vec<String> = every_event()
        .iter()
        .map(|event| {
            serde_json::to_value(event).expect("events serialise")["event"]
                .as_str()
                .expect("every event carries its tag")
                .to_owned()
        })
        .collect();

    for expected in [
        "game_detected",
        "game_lost",
        "plugin_loaded",
        "plugin_failed",
        "observed",
        "intent_proposed",
        "intent_rejected",
        "action_started",
        "action_finished",
        "agent_paused",
        "agent_resumed",
        "kill_switch",
        "error",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "{expected} is not in the fixtures, so the UI's handling of it is unchecked"
        );
    }
}

#[test]
fn every_value_variant_is_covered() {
    let tags: Vec<String> = every_value()
        .iter()
        .map(|value| {
            serde_json::to_value(value).expect("values serialise")["type"]
                .as_str()
                .expect("every value carries its tag")
                .to_owned()
        })
        .collect();

    for expected in [
        "bool", "int", "float", "ratio", "text", "point", "rect", "enum",
    ] {
        assert!(
            tags.iter().any(|tag| tag == expected),
            "{expected} is not in the fixtures, so the UI's rendering of it is unchecked"
        );
    }
}
