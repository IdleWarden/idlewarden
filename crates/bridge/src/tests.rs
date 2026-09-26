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

#[cfg(windows)]
#[test]
fn an_endpoint_lives_in_the_pipe_namespace_the_mod_serves_from() {
    assert_eq!(
        crate::transport::endpoint_path("reference"),
        r"\\.\pipe\idlewarden.reference",
        "the C# server opens NamedPipeServerStream(\"idlewarden.\" + name)"
    );
}

/// Tests that start a host bind the one websocket port the bridge listens on,
/// so they take turns rather than failing on whichever lost the race.
pub(crate) fn shared_port() -> std::sync::MutexGuard<'static, ()> {
    static PORT: std::sync::Mutex<()> = std::sync::Mutex::new(());
    PORT.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A mod on a real named pipe that answers one hello with `answer`, once the
/// host connects. Returns the request it received.
#[cfg(windows)]
fn a_pipe_mod(name: &str, answer: String) -> std::thread::JoinHandle<String> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::windows::io::FromRawHandle;

    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
    };

    let path = crate::transport::endpoint_path(name);
    let server = unsafe {
        CreateNamedPipeW(
            &HSTRING::from(path.as_str()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1,
            4096,
            4096,
            0,
            None,
        )
    };
    assert!(!server.is_invalid(), "the test pipe could not be created");
    let raw = server.0 as usize;

    std::thread::spawn(move || {
        let handle = windows::Win32::Foundation::HANDLE(raw as *mut std::ffi::c_void);
        let _ = unsafe { ConnectNamedPipe(handle, None) };
        let pipe = unsafe { std::fs::File::from_raw_handle(raw as *mut std::ffi::c_void) };
        let mut reader = BufReader::new(pipe.try_clone().expect("pipe clones"));
        let mut writer = pipe;

        let mut request = String::new();
        reader.read_line(&mut request).expect("the hello arrives");
        writeln!(writer, "{answer}").expect("the hello is answered");
        writer.flush().expect("flushed");
        request
    })
}

#[cfg(windows)]
#[test]
fn a_client_reaches_a_real_named_pipe_and_completes_the_handshake() {
    let name = format!("bridge-test-{}", std::process::id());
    let mod_side = a_pipe_mod(&name, hello());

    let bridge = connect(&name).expect("the client finds the pipe the server created");
    let received = mod_side.join().expect("the mod side finishes");

    assert_eq!(bridge.plugin().as_str(), "dev.example.game");
    assert!(received.contains("\"request\":\"hello\""));
}

/// A mod briefly has no pipe open: between two hosts it takes a moment to open
/// the next one, and at game start it may not be listening yet. The host used to
/// try the pipe once and then wait on the websocket alone, so it never saw the
/// pipe that opened a moment later.
#[cfg(windows)]
#[test]
fn a_pipe_that_opens_after_the_host_started_waiting_is_still_found() {
    let _port = shared_port();
    let name = format!("late-pipe-{}", std::process::id());

    let late = {
        let name = name.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(400));
            a_pipe_mod(&name, hello()).join()
        })
    };

    let bridge = crate::connect_or_listen(&name, std::time::Duration::from_secs(5))
        .expect("the pipe that opened late is found");

    assert_eq!(bridge.plugin().as_str(), "dev.example.game");
    drop(bridge);
    let _ = late.join();
}

/// A mod that answers is a mod that was found. Treating its refusal as "no pipe"
/// sent the host off to wait for a websocket, and the user read a timeout about a
/// socket instead of the reason the mod gave.
#[cfg(windows)]
#[test]
fn a_mod_that_answers_with_a_refusal_is_reported_rather_than_waited_past() {
    let _port = shared_port();
    let name = format!("refusing-pipe-{}", std::process::id());
    let incompatible =
        r#"{"response":"hello","plugin":"dev.example.game","api_version":"^99.0"}"#.to_owned();
    let mod_side = a_pipe_mod(&name, incompatible);

    let started = std::time::Instant::now();
    let error = crate::connect_or_listen(&name, std::time::Duration::from_secs(5)).unwrap_err();

    assert!(
        matches!(error, BridgeError::IncompatibleApi { .. }),
        "the mod's own answer must come back, got {error:?}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "a refusal is an answer; nothing should have been waited for"
    );
    let _ = mod_side.join();
}

mod over_a_local_websocket {
    use std::net::TcpListener;
    use std::time::Duration;

    use tungstenite::client::IntoClientRequest;
    use tungstenite::Message;

    use crate::websocket::{accept, bind};
    use crate::{Bridge, BridgeError};

    const WAIT: Duration = Duration::from_secs(5);

    fn refused(
        result: Result<Box<dyn crate::transport::Transport>, BridgeError>,
        expectation: &str,
    ) -> BridgeError {
        match result {
            Ok(_) => panic!("{expectation}"),
            Err(error) => error,
        }
    }

    fn port(listener: &TcpListener) -> u16 {
        listener.local_addr().expect("bound").port()
    }

    fn request(port: u16, path: &str, origin: Option<&str>) -> tungstenite::http::Request<()> {
        let mut request = format!("ws://127.0.0.1:{port}{path}")
            .into_client_request()
            .expect("a well formed url");
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert("origin", origin.parse().expect("a header value"));
        }
        request
    }

    fn a_mod_page(port: u16, path: &str, origin: Option<&str>) -> std::thread::JoinHandle<bool> {
        let request = request(port, path, origin);
        std::thread::spawn(move || {
            let mut socket = None;
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while std::time::Instant::now() < deadline {
                if let Ok((connected, _)) = tungstenite::connect(request.clone()) {
                    socket = Some(connected);
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let Some(mut socket) = socket else {
                return false;
            };

            while let Ok(Message::Text(incoming)) = socket.read() {
                let incoming: serde_json::Value =
                    serde_json::from_str(&incoming).expect("the host speaks JSON");
                let answer = match incoming["request"].as_str().expect("a request") {
                    "hello" => serde_json::json!({
                        "response": "hello",
                        "plugin": "dev.example.cookie-clicker",
                        "api_version": "^0.1",
                    }),
                    "observe" => serde_json::json!({
                        "response": "observed",
                        "signals": [{
                            "id": "resource.cookies",
                            "value": { "type": "int", "value": 1200 },
                        }],
                    }),
                    other => panic!("unexpected request {other}"),
                };
                if socket.send(Message::text(answer.to_string())).is_err() {
                    break;
                }
            }
            true
        })
    }

    #[test]
    fn a_mod_running_in_the_game_page_drives_the_bridge() {
        let listener = bind(0).expect("binds a port");
        let page = a_mod_page(port(&listener), "/cookie-clicker", Some("file://"));

        let transport = accept(&listener, "cookie-clicker", WAIT).expect("the mod connects");
        let mut bridge = Bridge::open(transport).expect("the handshake completes");
        let observation = bridge.observe(1_000).expect("the mod answers");

        assert_eq!(bridge.plugin().as_str(), "dev.example.cookie-clicker");
        assert_eq!(observation.signals[0].id.as_str(), "resource.cookies");
        drop(bridge);
        let _ = page.join();
    }

    #[test]
    fn a_page_served_from_the_web_is_turned_away() {
        let listener = bind(0).expect("binds a port");
        let page = a_mod_page(
            port(&listener),
            "/cookie-clicker",
            Some("https://evil.example"),
        );

        let refused = refused(
            accept(&listener, "cookie-clicker", Duration::from_millis(600)),
            "a remote page must never answer for a mod",
        );
        drop(listener);

        assert!(matches!(refused, BridgeError::Connect { .. }), "{refused}");
        assert!(
            !page.join().expect("the page thread finishes"),
            "the handshake itself has to fail, not the conversation after it"
        );
    }

    #[test]
    fn a_connection_asking_for_another_endpoint_is_refused() {
        let listener = bind(0).expect("binds a port");
        let page = a_mod_page(port(&listener), "/some-other-game", Some("file://"));

        let refused = refused(
            accept(&listener, "cookie-clicker", Duration::from_millis(600)),
            "one game's mod must not answer for another's plugin",
        );
        drop(listener);

        assert!(matches!(refused, BridgeError::Connect { .. }), "{refused}");
        assert!(!page.join().expect("the page thread finishes"));
    }

    #[test]
    fn a_mod_that_never_connects_says_so_rather_than_waiting_forever() {
        let listener = bind(0).expect("binds a port");

        let refused = refused(
            accept(&listener, "cookie-clicker", Duration::from_millis(300)),
            "nothing is listening on the other side",
        );

        assert!(
            refused.to_string().contains("cookie-clicker"),
            "the reason has to name the endpoint: {refused}"
        );
    }

    #[test]
    fn with_no_pipe_to_be_found_the_host_waits_for_the_page_to_connect() {
        let _port = super::shared_port();
        let name = format!("ws-fallback-{}", std::process::id());
        let listener = bind(crate::websocket::DEFAULT_PORT).expect("the shared port is free");
        let page = a_mod_page(port(&listener), &format!("/{name}"), Some("file://"));
        drop(listener);

        let bridge = crate::connect_or_listen(&name, WAIT).expect("the mod connects instead");

        assert_eq!(bridge.plugin().as_str(), "dev.example.cookie-clicker");
        drop(bridge);
        let _ = page.join();
    }

    #[test]
    fn an_endpoint_name_that_could_escape_the_url_is_refused() {
        let listener = bind(0).expect("binds a port");

        let refused = refused(
            accept(&listener, "../other", Duration::from_millis(100)),
            "a name is checked before it reaches a path",
        );

        assert!(matches!(refused, BridgeError::InvalidEndpoint { .. }));
    }
}
