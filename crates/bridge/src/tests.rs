// SPDX-License-Identifier: MPL-2.0

use std::io::{BufReader, Cursor};

use idlewarden_plugin_api::{ActionOutcome, Confidence, Intent, Value, API_VERSION};

use crate::transport::{is_valid_endpoint_name, LineTransport, Transport};
use crate::{connect, Bridge, BridgeError};

struct ScriptedTransport {
    responses: Vec<String>,
    pub sent: Vec<String>,
}

impl ScriptedTransport {
    fn new(responses: &[&str]) -> Box<Self> {
        Box::new(ScriptedTransport {
            responses: responses.iter().rev().map(|s| (*s).to_owned()).collect(),
            sent: Vec::new(),
        })
    }
}

impl Transport for ScriptedTransport {
    fn round_trip(&mut self, request: &str) -> Result<String, BridgeError> {
        self.sent.push(request.to_owned());
        self.responses.pop().ok_or(BridgeError::Disconnected)
    }
}

fn hello() -> String {
    format!(
        r#"{{"response":"hello","plugin":"dev.example.game","api_version":"^{}"}}"#,
        "0.1"
    )
}

fn open(responses: &[&str]) -> Result<Bridge, BridgeError> {
    let mut all = vec![hello()];
    all.extend(responses.iter().map(|s| (*s).to_owned()));
    let refs: Vec<&str> = all.iter().map(String::as_str).collect();
    Bridge::open(ScriptedTransport::new(&refs))
}

#[test]
fn the_handshake_records_the_plugin_the_mod_belongs_to() {
    let bridge = open(&[]).expect("handshake");
    assert_eq!(bridge.plugin().as_str(), "dev.example.game");
}

#[test]
fn a_mod_built_against_an_incompatible_contract_is_refused_at_connect() {
    let incompatible = r#"{"response":"hello","plugin":"dev.example.game","api_version":"^99.0"}"#;
    let error = Bridge::open(ScriptedTransport::new(&[incompatible])).unwrap_err();

    match error {
        BridgeError::IncompatibleApi { supported, .. } => assert_eq!(supported, API_VERSION),
        other => panic!("expected an api mismatch, got {other:?}"),
    }
}

#[test]
fn a_bridge_observation_is_always_certain() {
    let observed = r#"{"response":"observed","signals":[
        {"id":"resource.gold","value":{"type":"int","value":42}},
        {"id":"ui.screen_id","value":{"type":"enum","value":"main"}}]}"#;
    let mut bridge = open(&[observed]).unwrap();

    let observation = bridge.observe(1_000).unwrap();
    assert_eq!(observation.weakest_confidence(), Confidence::CERTAIN);
    assert_eq!(observation.captured_at_ms, 1_000);
    assert_eq!(observation.age_ms(1_000), 0);
    assert_eq!(
        observation.get("resource.gold").unwrap().value,
        Value::Int(42)
    );
}

#[test]
fn a_mod_cannot_talk_its_way_into_a_lower_confidence() {
    let lying = r#"{"response":"observed","signals":[
        {"id":"resource.gold","value":{"type":"int","value":42},"confidence":0.1}]}"#;
    let mut bridge = open(&[lying]).unwrap();

    let observation = bridge.observe(0).unwrap();
    assert_eq!(observation.weakest_confidence(), Confidence::CERTAIN);
}

#[test]
fn frame_ids_advance_so_observations_stay_orderable() {
    let observed = r#"{"response":"observed","signals":[]}"#;
    let mut bridge = open(&[observed, observed]).unwrap();

    assert_eq!(bridge.observe(0).unwrap().frame_id, 1);
    assert_eq!(bridge.observe(250).unwrap().frame_id, 2);
}

#[test]
fn acting_forwards_the_intent_and_returns_the_mods_outcome() {
    let failed = r#"{"response":"acted","outcome":{"outcome":"failed","reason":"not affordable"}}"#;
    let mut bridge = open(&[failed]).unwrap();

    let outcome = bridge.act(&Intent::new("buy_upgrade")).unwrap();
    assert_eq!(
        outcome,
        ActionOutcome::Failed {
            reason: "not affordable".into()
        }
    );
    assert!(!outcome.is_success());
}

#[test]
fn an_error_response_surfaces_the_mods_message_instead_of_panicking() {
    let refused = r#"{"response":"error","message":"the game is loading"}"#;
    let mut bridge = open(&[refused]).unwrap();

    match bridge.observe(0).unwrap_err() {
        BridgeError::Refused(message) => assert_eq!(message, "the game is loading"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn the_wrong_response_type_is_reported_rather_than_silently_accepted() {
    let acted = r#"{"response":"acted","outcome":{"outcome":"succeeded"}}"#;
    let mut bridge = open(&[acted]).unwrap();

    match bridge.observe(0).unwrap_err() {
        BridgeError::Unexpected { expected, got } => {
            assert_eq!(expected, "observed");
            assert_eq!(got, "acted");
        }
        other => panic!("expected an unexpected-response error, got {other:?}"),
    }
}

#[test]
fn garbage_on_the_wire_is_an_error_not_a_crash() {
    let mut bridge = open(&["not json at all"]).unwrap();
    assert!(matches!(
        bridge.observe(0).unwrap_err(),
        BridgeError::Malformed(_)
    ));
}

#[test]
fn a_mod_that_goes_away_mid_session_is_reported_as_disconnected() {
    let mut bridge = open(&[]).unwrap();
    assert!(matches!(
        bridge.observe(0).unwrap_err(),
        BridgeError::Disconnected
    ));
}

#[test]
fn endpoint_names_from_a_manifest_cannot_escape_the_namespace() {
    assert!(is_valid_endpoint_name("cookie-clicker"));

    for hostile in [
        "../../etc/passwd",
        "a/b",
        r"a\b",
        "UPPER",
        "with space",
        "",
        &"x".repeat(65),
    ] {
        assert!(!is_valid_endpoint_name(hostile), "{hostile:?} was accepted");
    }
}

#[test]
fn connecting_to_an_invalid_endpoint_name_never_touches_the_filesystem() {
    match connect("../escape").unwrap_err() {
        BridgeError::InvalidEndpoint { endpoint } => assert_eq!(endpoint, "../escape"),
        other => panic!("expected the name to be rejected first, got {other:?}"),
    }
}

#[test]
fn the_line_transport_frames_one_message_per_line() {
    let mut written = Vec::new();
    let mut transport = LineTransport::new(
        BufReader::new(Cursor::new(b"first\nsecond\n".to_vec())),
        &mut written,
    );

    assert_eq!(transport.round_trip("ping").unwrap(), "first\n");
    assert_eq!(transport.round_trip("pong").unwrap(), "second\n");
    assert!(matches!(
        transport.round_trip("again").unwrap_err(),
        BridgeError::Disconnected
    ));

    assert_eq!(String::from_utf8(written).unwrap(), "ping\npong\nagain\n");
}
