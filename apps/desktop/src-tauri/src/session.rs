// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use idlewarden_bridge::{Bridge, BridgeError};
use idlewarden_capture::CaptureBackend;
use idlewarden_capture::Frame;
use idlewarden_capture::WindowHandle;
#[cfg(windows)]
use idlewarden_capture::WindowsCapture;
use idlewarden_core::authoring::{self, AuthoringError, Draft};
use idlewarden_core::detector::{Candidate, DesktopWindows};
use idlewarden_core::{
    load_all, Command, Detector, Event, Governor, GovernorConfig, Link, Parts, PluginBundle,
    PluginId, Refusal, Runner, Session, SessionService, SessionState, DEFAULT_TICK,
};
#[cfg(windows)]
use idlewarden_input::{DryRunBackend, Humanisation, SendInputBackend};
use idlewarden_input::{InputBackend, KillSwitch};
use serde::Serialize;
use tauri::State;

use crate::profiles::{Profile, Profiles};

type Backends = (Box<dyn CaptureBackend>, Box<dyn InputBackend>);

/// Events are drained by the UI, and nothing guarantees the UI is polling.
/// Without a ceiling the buffer grows for as long as a session runs unwatched,
/// so the oldest are dropped once it is reached.
const MAX_BUFFERED_EVENTS: usize = 2_000;

/// An event with the moment the desktop observed it.
///
/// The Core does not timestamp events, and it should not have to: what a log
/// reader needs is wall-clock time, and the Core deliberately knows nothing
/// about clocks it has not been handed. Stamping happens here, at the adapter,
/// where a real clock exists.
#[derive(Debug, Clone, Serialize)]
pub struct Published {
    pub at_ms: u64,
    #[serde(flatten)]
    pub event: Event,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or_default()
}

pub struct SessionHandle(Mutex<Inner>);

struct Inner {
    plugin_root: PathBuf,
    plugins: Vec<PluginBundle>,
    detector: Detector,
    /// A projection of what the runner reports, not the source of truth. While
    /// no session is running the detector maintains it directly.
    session: Session,
    service: Option<SessionService>,
    events: Vec<Published>,
    kill: KillSwitch,
    profiles: Profiles,
}

/// One plugin as the sidebar and the automations list need it.
#[derive(Debug, Serialize)]
pub struct PluginSummary {
    pub id: String,
    pub detected: bool,
    pub intents: Vec<IntentSummary>,
    pub bridge: Option<String>,
    pub bridge_granted: bool,
}

/// One window detection looked at, as the Detect screen renders it.
#[derive(Debug, Serialize)]
pub struct WindowCandidate {
    pub handle: isize,
    pub title: String,
    pub executable: String,
    pub steam_appid: Option<u32>,
    pub plugins: Vec<String>,
}

impl From<Candidate> for WindowCandidate {
    fn from(candidate: Candidate) -> Self {
        WindowCandidate {
            handle: candidate.window.handle.0,
            title: candidate.window.title,
            executable: candidate.window.executable,
            steam_appid: candidate.window.steam_appid,
            plugins: candidate
                .plugins
                .into_iter()
                .map(|plugin| plugin.0)
                .collect(),
        }
    }
}

#[cfg(not(windows))]
const NO_CAPTURE: &str =
    "this plugin reads the screen, and capture is not implemented on this platform yet;      a plugin that speaks to a mod works";

/// How long a page mod has to connect once a session starts. Long enough for a
/// reconnect cycle, short enough that a missing mod is a pause and not a hang.
const BRIDGE_WAIT: std::time::Duration = std::time::Duration::from_secs(12);

pub struct Plan {
    plugin: PluginId,
    governor: GovernorConfig,
    wanted: Wanted,
}

impl Plan {
    fn split(self) -> (PluginId, GovernorConfig, Wanted) {
        (self.plugin, self.governor, self.wanted)
    }
}

enum Wanted {
    Bridge(String),
    Link(Result<Link, String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModRequest {
    pub bridge: Option<String>,
    pub granted: bool,
    pub window: Option<WindowHandle>,
}

#[derive(Debug, Serialize)]
pub struct IntentSummary {
    pub name: String,
    pub enabled: bool,
}

impl SessionHandle {
    pub fn new(data_dir: PathBuf) -> Self {
        let mut inner = Inner {
            plugin_root: data_dir.join("plugins"),
            plugins: Vec::new(),
            detector: Detector::new(Box::new(DesktopWindows), Vec::new()),
            session: Session::default(),
            service: None,
            events: Vec::new(),
            kill: KillSwitch::new(),
            profiles: Profiles::load(&data_dir.join("profiles.json")),
        };
        inner.load_plugins();
        SessionHandle(Mutex::new(inner))
    }

    pub fn mod_request(&self, plugin: &str) -> Option<ModRequest> {
        let inner = self.0.lock().expect("session lock");
        let bundle = inner.plugins.iter().find(|bundle| bundle.id.0 == plugin)?;
        let detected = inner.session.plugin.as_ref() == Some(&bundle.id);
        Some(ModRequest {
            bridge: bundle.bridge.clone(),
            granted: inner.profiles.get(plugin).bridge_granted,
            window: detected.then(|| inner.detector.window()).flatten(),
        })
    }

    pub fn author(&self, draft: &Draft, frame: &Frame) -> Result<PathBuf, AuthoringError> {
        let mut inner = self.0.lock().expect("session lock");
        let written = authoring::write(draft, frame, &inner.plugin_root)?;
        inner.load_plugins();
        Ok(written)
    }
}

impl Inner {
    fn load_plugins(&mut self) {
        let mut plugins = Vec::new();
        let mut events = Vec::new();

        for (path, loaded) in load_all(&self.plugin_root) {
            match loaded {
                Ok(bundle) => {
                    events.push(Event::PluginLoaded {
                        plugin: bundle.id.clone(),
                        version: String::new(),
                    });
                    plugins.push(bundle);
                }
                Err(error) => events.push(Event::Error {
                    message: format!("{} could not be loaded: {error}", path.display()),
                }),
            }
        }

        self.detector.set_plugins(
            plugins
                .iter()
                .map(|bundle| (bundle.id.clone(), bundle.matcher.clone()))
                .collect(),
        );
        self.plugins = plugins;
        self.publish(events);
    }

    /// Detection while idle, published events while running. Called before
    /// anything reads the session, so the UI never sees a stale state.
    fn publish(&mut self, events: impl IntoIterator<Item = Event>) {
        let at_ms = now_ms();
        self.events
            .extend(events.into_iter().map(|event| Published { at_ms, event }));

        if self.events.len() > MAX_BUFFERED_EVENTS {
            let overflow = self.events.len() - MAX_BUFFERED_EVENTS;
            self.events.drain(..overflow);
        }
    }

    fn refresh(&mut self) {
        if self.service.is_none() {
            let found = self.detector.poll(&mut self.session);
            self.publish(found);
            return;
        }

        let published: Vec<Event> = self
            .service
            .as_ref()
            .map(|service| service.poll())
            .unwrap_or_default();

        for event in &published {
            project(&mut self.session, event);
        }
        self.publish(published);

        if self.session.state == SessionState::Halted {
            self.service = None;
        }
    }

    #[cfg(windows)]
    fn backends(&self, window: idlewarden_capture::WindowHandle) -> Result<Backends, String> {
        let capture = WindowsCapture::new(window).map_err(|error| error.to_string())?;
        let input: Box<dyn InputBackend> = if self.session.dry_run {
            Box::new(DryRunBackend)
        } else {
            Box::new(SendInputBackend::new(
                window.0,
                self.kill.clone(),
                Humanisation::default(),
            ))
        };
        Ok((Box::new(capture), input))
    }

    /// Linux has input and detection but no capture yet (#11, ADR-0019), so a
    /// plugin that reads the screen has nothing to read. A bridged plugin needs
    /// neither and starts normally.
    #[cfg(not(windows))]
    fn backends(&self, _window: idlewarden_capture::WindowHandle) -> Result<Backends, String> {
        Err(NO_CAPTURE.to_owned())
    }

    fn declared(bundle: &PluginBundle) -> Vec<String> {
        bundle
            .rules
            .intents
            .iter()
            .map(|intent| intent.name.clone())
            .collect()
    }

    fn summaries(&self) -> Vec<PluginSummary> {
        self.plugins
            .iter()
            .map(|bundle| {
                let profile = self.profiles.get(&bundle.id.0);
                PluginSummary {
                    id: bundle.id.0.clone(),
                    detected: self.session.plugin.as_ref() == Some(&bundle.id),
                    intents: bundle
                        .rules
                        .intents
                        .iter()
                        .map(|intent| IntentSummary {
                            name: intent.name.clone(),
                            enabled: profile.is_enabled(&intent.name),
                        })
                        .collect(),
                    bridge: bundle.bridge.clone(),
                    bridge_granted: profile.bridge_granted,
                }
            })
            .collect()
    }

    /// A pipe answers or refuses at once, but a mod that lives in a page has to
    /// connect to us, and that waits (ADR-0018). So planning happens under the
    /// lock and the connecting happens outside it.
    fn plan(&mut self, command: &Command) -> Result<Plan, Refusal> {
        self.session.apply(command)?;

        let Some(window) = self.detector.window() else {
            return Err(Refusal::NoGameReady);
        };
        let Some(bundle) = self
            .plugins
            .iter()
            .find(|bundle| Some(&bundle.id) == self.session.plugin.as_ref())
        else {
            return Err(Refusal::NoGameReady);
        };

        let profile = self.profiles.get(&bundle.id.0);
        let governor = profile.governor(&Self::declared(bundle));
        let plugin = bundle.id.clone();

        match bundle.bridge.clone() {
            Some(bridge) if profile.bridge_granted => Ok(Plan {
                plugin,
                governor,
                wanted: Wanted::Bridge(bridge),
            }),
            _ => {
                let wanted = self
                    .backends(window)
                    .map(|(capture, input)| Link::Perceived {
                        capture,
                        perceiver: bundle.perceiver(),
                        input,
                    });
                Ok(Plan {
                    plugin,
                    governor,
                    wanted: Wanted::Link(wanted),
                })
            }
        }
    }

    fn launch(&mut self, plugin: &PluginId, governor: GovernorConfig, link: Result<Link, String>) {
        let Some(bundle) = self.plugins.iter().find(|bundle| &bundle.id == plugin) else {
            return;
        };

        match link {
            Ok(link) => self.service = Some(self.spawn(bundle, link, governor)),
            Err(reason) => {
                self.session.pause(reason.clone());
                self.publish([Event::Error { message: reason }]);
            }
        }
    }

    fn spawn(&self, bundle: &PluginBundle, link: Link, governor: GovernorConfig) -> SessionService {
        SessionService::spawn(
            Runner::new(Parts {
                link,
                tree: bundle.tree(),
                actuator: Box::new(bundle.actuator()),
                kill: self.kill.clone(),
                governor: Governor::new(governor, 0),
                session: self.session.clone(),
            }),
            DEFAULT_TICK,
        )
    }
}

fn bridged(plugin: &PluginId, connected: Result<Bridge, BridgeError>) -> Result<Link, String> {
    let bridge = connected.map_err(|error| format!("the mod did not answer: {error}"))?;
    if bridge.plugin() != plugin {
        return Err(format!(
            "the mod on this endpoint speaks for `{}`, not `{}`",
            bridge.plugin().0,
            plugin.0
        ));
    }
    Ok(Link::Bridged(bridge))
}

/// The runner owns the session; this mirrors what it publishes so the UI has
/// something to render between polls.
fn project(session: &mut Session, event: &Event) {
    match event {
        Event::GameDetected { plugin, .. } => session.game_detected(plugin.clone()),
        Event::GameLost => session.game_lost(),
        Event::AgentPaused { reason } => session.pause(reason.clone()),
        Event::AgentResumed => session.state = SessionState::Running,
        Event::KillSwitch => session.state = SessionState::Halted,
        Event::ActionFinished { .. } => session.actions_taken += 1,
        Event::IntentRejected { reason, .. } => session.last_reason = Some(reason.clone()),
        _ => {}
    }
}

#[derive(Debug, Serialize)]
pub struct Refused {
    refusal: Refusal,
    message: String,
}

impl From<Refusal> for Refused {
    fn from(refusal: Refusal) -> Self {
        Refused {
            message: refusal.to_string(),
            refusal,
        }
    }
}

#[tauri::command]
pub fn session_state(handle: State<'_, SessionHandle>) -> Session {
    let mut inner = handle.0.lock().expect("session lock");
    inner.refresh();
    inner.session.clone()
}

#[tauri::command]
pub fn session_events(handle: State<'_, SessionHandle>) -> Vec<Published> {
    let mut inner = handle.0.lock().expect("session lock");
    inner.refresh();
    std::mem::take(&mut inner.events)
}

#[tauri::command]
pub async fn dispatch(
    handle: State<'_, SessionHandle>,
    command: Command,
) -> Result<Session, Refused> {
    let plan = {
        let mut inner = handle.0.lock().expect("session lock");
        inner.refresh();

        match &command {
            Command::Start { .. } => Some(inner.plan(&command)?),
            Command::Stop => {
                inner.session.apply(&command)?;
                inner.service = None;
                inner.kill.reset();
                None
            }
            _ => {
                inner.session.apply(&command)?;
                if let Some(service) = &inner.service {
                    service.send(command.clone());
                }
                None
            }
        }
    };

    if let Some(plan) = plan {
        let (plugin, governor, wanted) = plan.split();
        let link = open(wanted, &plugin).await;
        let mut inner = handle.0.lock().expect("session lock");
        inner.launch(&plugin, governor, link);
    }

    let inner = handle.0.lock().expect("session lock");
    Ok(inner.session.clone())
}

async fn open(wanted: Wanted, plugin: &PluginId) -> Result<Link, String> {
    let Wanted::Bridge(name) = wanted else {
        let Wanted::Link(link) = wanted else {
            unreachable!()
        };
        return link;
    };

    let connected = tauri::async_runtime::spawn_blocking(move || {
        idlewarden_bridge::connect_or_listen(&name, BRIDGE_WAIT)
    })
    .await
    .map_err(|error| error.to_string())?;

    bridged(plugin, connected)
}

/// What detection saw on the last poll. Refreshing first means the screen
/// shows the desktop as the Detector ruled on it, not a second enumeration
/// describing a different moment.
#[tauri::command]
pub fn window_candidates(handle: State<'_, SessionHandle>) -> Vec<WindowCandidate> {
    let mut inner = handle.0.lock().expect("session lock");
    inner.refresh();
    inner
        .detector
        .candidates()
        .into_iter()
        .map(WindowCandidate::from)
        .collect()
}

#[tauri::command]
pub fn plugins(handle: State<'_, SessionHandle>) -> Vec<PluginSummary> {
    let mut inner = handle.0.lock().expect("session lock");
    inner.refresh();
    inner.summaries()
}

/// Switching an intent off keeps it out of the Governor's allow list for the
/// next session. It does not reach into a session already running.
#[tauri::command]
pub fn set_intent_enabled(
    handle: State<'_, SessionHandle>,
    plugin: String,
    intent: String,
    enabled: bool,
) -> Vec<PluginSummary> {
    let mut inner = handle.0.lock().expect("session lock");
    inner
        .profiles
        .update(&plugin, |profile| profile.set_enabled(&intent, enabled));
    inner.summaries()
}

#[tauri::command]
pub fn profile(handle: State<'_, SessionHandle>, plugin: String) -> Profile {
    let inner = handle.0.lock().expect("session lock");
    inner.profiles.get(&plugin)
}

#[tauri::command]
pub fn set_profile(
    handle: State<'_, SessionHandle>,
    plugin: String,
    mut profile: Profile,
) -> Profile {
    let mut inner = handle.0.lock().expect("session lock");
    profile.bridge_granted = inner.profiles.get(&plugin).bridge_granted;
    inner.profiles.set(&plugin, profile)
}

#[tauri::command]
pub fn set_bridge_granted(
    handle: State<'_, SessionHandle>,
    plugin: String,
    granted: bool,
) -> Vec<PluginSummary> {
    let mut inner = handle.0.lock().expect("session lock");
    inner
        .profiles
        .update(&plugin, |profile| profile.bridge_granted = granted);
    inner.summaries()
}

#[tauri::command]
pub fn engage_kill_switch(handle: State<'_, SessionHandle>) -> Session {
    let mut inner = handle.0.lock().expect("session lock");
    inner.kill.engage();
    inner.session.state = SessionState::Halted;
    inner.service = None;
    inner.publish([Event::KillSwitch]);
    inner.session.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    use idlewarden_capture::{CaptureError, Frame, GameWindow, Size, WindowHandle};
    use idlewarden_core::authoring::RegionKind;
    use idlewarden_core::PluginId;
    use idlewarden_core::WindowSource;
    use idlewarden_input::DryRunBackend;
    use std::sync::Arc;

    const WINDOW: WindowHandle = WindowHandle(4242);

    struct Fixed(Vec<GameWindow>);

    impl WindowSource for Fixed {
        fn windows(&mut self) -> Vec<GameWindow> {
            self.0.clone()
        }
    }

    struct Painted {
        frame: Arc<Frame>,
        served: u64,
    }

    impl Painted {
        fn new(reward_ready: bool) -> Self {
            let (width, height) = (200u32, 200u32);
            let mut bgra = vec![20u8; (width * height * 4) as usize];
            for pixel in bgra.chunks_exact_mut(4) {
                pixel[3] = 255;
            }

            if reward_ready {
                for y in (0.71 * height as f64) as u32..(0.74 * height as f64) as u32 {
                    for x in (0.49 * width as f64) as u32..(0.52 * width as f64) as u32 {
                        let index = ((y * width + x) * 4) as usize;
                        bgra[index] = 62;
                        bgra[index + 1] = 185;
                        bgra[index + 2] = 232;
                    }
                }
            }

            Painted {
                frame: Arc::new(Frame {
                    id: 1,
                    captured_at_ms: 0,
                    size: Size { width, height },
                    bgra,
                }),
                served: 0,
            }
        }
    }

    impl CaptureBackend for Painted {
        fn next_frame(&mut self) -> Result<Arc<Frame>, CaptureError> {
            self.served += 1;
            Ok(Arc::clone(&self.frame))
        }

        fn window(&self) -> WindowHandle {
            WINDOW
        }
    }

    const RULES: &str = r#"{
      "signals": [
        {
          "id": "ui.reward_ready",
          "extractor": {
            "method": "color_probe",
            "roi": { "x": 0.49, "y": 0.71, "w": 0.02, "h": 0.02 },
            "rgb": [232, 185, 62],
            "tolerance": 24
          }
        }
      ],
      "intents": [
        {
          "name": "collect_reward",
          "when": [{ "op": "is_true", "signal": "ui.reward_ready" }],
          "commands": [{ "op": "click", "at": { "x": 0.5, "y": 0.72 }, "button": "left" }],
          "post_condition": [{ "op": "is_false", "signal": "ui.reward_ready" }],
          "min_confidence": 0.5
        }
      ]
    }"#;

    const MANIFEST: &str = r#"{
      "id": "dev.idlewarden.test-game",
      "name": "Test Game",
      "version": "0.0.0",
      "api_version": "^0.1",
      "game": { "executable": "TestGame.exe", "window_title": "Test Game" },
      "signals": [{ "id": "ui.reward_ready", "value_type": "bool" }],
      "intents": ["collect_reward"],
      "capabilities": ["capture", "input.mouse"]
    }"#;

    fn plugin_root(name: &str, manifest: &str, rules: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("idlewarden-{name}-{}", std::process::id()));
        let plugin = root.join("plugins").join("test-game");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&plugin).expect("the fixture plugin could be created");
        std::fs::write(plugin.join("plugin.json"), manifest).expect("manifest written");
        std::fs::write(plugin.join("rules.json"), rules).expect("rules written");
        root
    }

    fn ready(name: &str) -> Inner {
        ready_with(name, MANIFEST, RULES)
    }

    fn ready_with(name: &str, manifest: &str, rules: &str) -> Inner {
        let handle = SessionHandle::new(plugin_root(name, manifest, rules));
        let mut inner = handle.0.into_inner().expect("session lock");

        let matchers = inner
            .plugins
            .iter()
            .map(|bundle| (bundle.id.clone(), bundle.matcher.clone()))
            .collect();
        inner.detector = Detector::new(
            Box::new(Fixed(vec![GameWindow {
                handle: WINDOW,
                title: "Test Game".to_owned(),
                executable: "TestGame.exe".to_owned(),
                steam_appid: None,
            }])),
            matchers,
        );
        assert!(
            !inner.plugins.is_empty(),
            "the fixture plugin did not load: {:?}",
            inner
                .events
                .iter()
                .map(|published| format!("{:?}", published.event))
                .collect::<Vec<_>>()
        );
        inner.events.clear();
        inner
    }

    fn drain_for(inner: &mut Inner, ticks: u32) -> Vec<Event> {
        let mut seen = Vec::new();
        for _ in 0..ticks {
            std::thread::sleep(DEFAULT_TICK);
            inner.refresh();
            seen.extend(
                std::mem::take(&mut inner.events)
                    .into_iter()
                    .map(|p| p.event),
            );
        }
        seen
    }

    fn names(events: &[Event]) -> Vec<&'static str> {
        events
            .iter()
            .map(|event| match event {
                Event::GameDetected { .. } => "game_detected",
                Event::GameLost => "game_lost",
                Event::PluginLoaded { .. } => "plugin_loaded",
                Event::PluginFailed { .. } => "plugin_failed",
                Event::Observed { .. } => "observed",
                Event::IntentProposed { .. } => "intent_proposed",
                Event::IntentRejected { .. } => "intent_rejected",
                Event::ActionStarted { .. } => "action_started",
                Event::ActionFinished { .. } => "action_finished",
                Event::AgentPaused { .. } => "agent_paused",
                Event::AgentResumed => "agent_resumed",
                Event::KillSwitch => "kill_switch",
                Event::Error { .. } => "error",
            })
            .collect()
    }

    fn start_with(inner: &mut Inner, capture: Painted, governor: GovernorConfig) {
        inner.refresh();
        assert_eq!(
            inner.session.state,
            SessionState::Ready,
            "detection has to bind a window before Start means anything"
        );

        inner
            .session
            .apply(&Command::Start {
                plugin: PluginId("dev.idlewarden.test-game".to_owned()),
                profile: "default".to_owned(),
            })
            .expect("a ready session starts");

        let bundle = inner.plugins.first().expect("the fixture plugin loaded");
        let link = Link::Perceived {
            capture: Box::new(capture),
            perceiver: bundle.perceiver(),
            input: Box::new(DryRunBackend),
        };
        let service = inner.spawn(bundle, link, governor);
        inner.service = Some(service);
    }

    fn inner() -> Inner {
        let empty = std::env::temp_dir().join("idlewarden-no-plugins-here");
        let handle = SessionHandle::new(empty);
        let mut inner = handle.0.into_inner().expect("session lock");
        inner.events.clear();
        inner
    }

    #[test]
    fn pressing_start_runs_a_real_session_and_the_events_reach_the_caller() {
        let mut inner = ready("runs");
        start_with(&mut inner, Painted::new(true), GovernorConfig::default());

        let published = drain_for(&mut inner, 6);
        let seen = names(&published);

        assert!(
            seen.contains(&"observed"),
            "no observation means the loop is not perceiving anything: {seen:?}"
        );
        assert!(
            seen.contains(&"intent_proposed"),
            "the tree never chose an intent, so nothing was decided: {seen:?}"
        );
        assert!(
            seen.contains(&"action_finished"),
            "the intent never became an action: {seen:?}"
        );
        assert_eq!(inner.session.state, SessionState::Running);
        assert!(
            inner.session.actions_taken > 0,
            "the projection the UI renders has to move with the runner"
        );
    }

    #[test]
    fn a_screen_with_nothing_to_do_proposes_nothing() {
        let mut inner = ready("idle");
        start_with(&mut inner, Painted::new(false), GovernorConfig::default());

        let seen = names(&drain_for(&mut inner, 5));

        assert!(
            seen.contains(&"observed"),
            "perception still runs, it just has nothing to act on: {seen:?}"
        );
        assert!(
            !seen.contains(&"intent_proposed"),
            "acting on an unlit reward would be acting on a ghost: {seen:?}"
        );
    }

    #[test]
    fn a_governor_refusal_reaches_the_user_rather_than_looking_like_silence() {
        let mut inner = ready("refused");
        start_with(
            &mut inner,
            Painted::new(true),
            GovernorConfig {
                allowed_intents: Some(Vec::new()),
                ..GovernorConfig::default()
            },
        );

        let published = drain_for(&mut inner, 5);
        let seen = names(&published);

        assert!(
            seen.contains(&"intent_rejected"),
            "a refused agent looks exactly like an idle one unless it says so: {seen:?}"
        );
        assert!(
            !seen.contains(&"action_finished"),
            "the Governor refused it, so nothing may have run: {seen:?}"
        );

        let reason = published.iter().find_map(|event| match event {
            Event::IntentRejected { reason, .. } => Some(reason.clone()),
            _ => None,
        });
        assert!(
            reason.is_some_and(|reason| reason.contains("collect_reward")),
            "the reason has to name what was refused"
        );
    }

    #[test]
    fn stop_shuts_the_thread_down_and_leaves_nothing_running() {
        let mut inner = ready("stop");
        start_with(&mut inner, Painted::new(true), GovernorConfig::default());
        drain_for(&mut inner, 2);

        inner
            .session
            .apply(&Command::Stop)
            .expect("a running session stops");
        inner.service = None;

        let before = inner.session.actions_taken;
        let after_stop = names(&drain_for(&mut inner, 3));

        assert!(inner.service.is_none());
        assert_eq!(
            inner.session.actions_taken, before,
            "a stopped session must not still be acting"
        );
        assert!(
            !after_stop.contains(&"action_finished"),
            "the thread outlived Stop: {after_stop:?}"
        );
    }

    #[test]
    fn the_kill_switch_halts_the_session_and_drops_the_thread() {
        let mut inner = ready("kill");
        start_with(&mut inner, Painted::new(true), GovernorConfig::default());
        drain_for(&mut inner, 2);

        inner.kill.engage();
        inner.session.state = SessionState::Halted;
        inner.service = None;
        inner.publish([Event::KillSwitch]);

        let before = inner.session.actions_taken;
        let after = names(&drain_for(&mut inner, 3));

        assert_eq!(inner.session.state, SessionState::Halted);
        assert_eq!(
            inner.session.actions_taken, before,
            "the kill switch has to stop the work, not just the label"
        );
        assert!(after.iter().all(|name| *name != "action_finished"));
    }

    fn drawn(id: &str, kind: idlewarden_core::authoring::RegionKind) -> Draft {
        use idlewarden_core::authoring::Region;
        use idlewarden_core::GameMatcher;
        use idlewarden_vision::Roi;

        Draft {
            id: PluginId(id.to_owned()),
            name: "Drawn".to_owned(),
            game: GameMatcher {
                executable: Some("TestGame.exe".to_owned()),
                ..Default::default()
            },
            regions: vec![Region {
                name: "ui.reward_ready".to_owned(),
                kind,
                area: Roi {
                    x: 0.495,
                    y: 0.715,
                    w: 0.02,
                    h: 0.02,
                },
            }],
            intents: Vec::new(),
        }
    }

    #[test]
    fn a_plugin_written_by_the_editor_is_loaded_without_a_restart() {
        let data = std::env::temp_dir().join(format!("idlewarden-author-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        let handle = SessionHandle::new(data);

        handle
            .author(
                &drawn("local.drawn", RegionKind::ColorProbe),
                &Painted::new(true).frame,
            )
            .expect("a region over the gold patch recognises itself");

        let inner = handle.0.into_inner().expect("session lock");
        assert!(
            inner
                .plugins
                .iter()
                .any(|bundle| bundle.id.0 == "local.drawn"),
            "L0 is only real if the plugin runs as soon as it is drawn"
        );
        assert!(inner.events.iter().any(|published| matches!(
            &published.event,
            Event::PluginLoaded { plugin, .. } if plugin.0 == "local.drawn"
        )));
    }

    #[test]
    fn a_draft_the_editor_refuses_leaves_the_loaded_plugins_alone() {
        let handle = SessionHandle(Mutex::new(ready("refused-draft")));

        let error = handle
            .author(
                &drawn("local.drawn", RegionKind::TemplateMatch),
                &Painted::new(false).frame,
            )
            .expect_err("a flat patch has no texture to match, not even against itself");

        assert!(matches!(error, AuthoringError::Unrecognised(..)));
        let loaded: Vec<String> = handle
            .0
            .into_inner()
            .expect("session lock")
            .plugins
            .iter()
            .map(|bundle| bundle.id.0.clone())
            .collect();
        assert_eq!(loaded, vec!["dev.idlewarden.test-game".to_owned()]);
    }

    #[test]
    fn published_events_carry_the_moment_they_were_observed() {
        let mut inner = inner();

        inner.publish([Event::KillSwitch]);

        assert_eq!(inner.events.len(), 1);
        assert!(
            inner.events[0].at_ms > 1_700_000_000_000,
            "an event without a real wall-clock stamp cannot answer `why at 3am`"
        );
    }

    #[test]
    fn the_buffer_drops_the_oldest_rather_than_growing_without_bound() {
        let mut inner = inner();

        for index in 0..MAX_BUFFERED_EVENTS + 50 {
            inner.publish([Event::Error {
                message: index.to_string(),
            }]);
        }

        assert_eq!(
            inner.events.len(),
            MAX_BUFFERED_EVENTS,
            "a session nobody is watching must not grow the buffer forever"
        );

        let first = match &inner.events[0].event {
            Event::Error { message } => message.clone(),
            other => panic!("unexpected event {other:?}"),
        };
        assert_eq!(first, "50", "the oldest events are the ones dropped");
    }

    #[test]
    fn draining_the_buffer_leaves_it_empty_for_the_next_poll() {
        let mut inner = inner();
        inner.publish([Event::AgentResumed, Event::KillSwitch]);

        let drained = std::mem::take(&mut inner.events);

        assert_eq!(drained.len(), 2);
        assert!(inner.events.is_empty());
    }

    mod bridged {
        use std::sync::{Arc, Mutex};

        use idlewarden_bridge::transport::Transport;
        use idlewarden_plugin_api::ActionOutcome;
        use serde_json::json;

        use super::*;

        const REFERENCE_RULES: &str = r#"{
          "intents": [
            {
              "name": "buy_upgrade",
              "when": [{ "op": "at_least", "signal": "resource.cookies", "value": 100 }],
              "params": { "tier": { "type": "int", "value": 1 } },
              "post_condition": [{ "op": "at_most", "signal": "resource.cookies", "value": 99 }],
              "min_confidence": 0.9
            }
          ]
        }"#;

        fn manifest(endpoint: &str) -> String {
            MANIFEST.replace(
                r#""input.mouse""#,
                &format!(r#""input.mouse", "bridge:{endpoint}""#),
            )
        }

        struct ReferenceMod {
            speaks_for: &'static str,
            cookies: Arc<Mutex<i64>>,
        }

        impl Transport for ReferenceMod {
            fn round_trip(&mut self, request: &str) -> Result<String, BridgeError> {
                let request: serde_json::Value = serde_json::from_str(request).unwrap();
                let mut cookies = self.cookies.lock().unwrap();
                let response = match request["request"].as_str().unwrap() {
                    "hello" => json!({
                        "response": "hello",
                        "plugin": self.speaks_for,
                        "api_version": "^0.1",
                    }),
                    "observe" => {
                        *cookies += 1;
                        json!({
                            "response": "observed",
                            "signals": [{
                                "id": "resource.cookies",
                                "value": { "type": "int", "value": *cookies },
                            }],
                        })
                    }
                    "act" => {
                        let price = request["intent"]["params"]["tier"]["value"]
                            .as_i64()
                            .unwrap()
                            * 100;
                        let outcome = if *cookies < price {
                            json!({ "outcome": "failed", "reason": "not affordable" })
                        } else {
                            *cookies -= price;
                            json!({ "outcome": "succeeded" })
                        };
                        json!({ "response": "acted", "outcome": outcome })
                    }
                    other => panic!("unexpected request {other}"),
                };
                Ok(response.to_string())
            }
        }

        fn reference(speaks_for: &'static str, cookies: i64) -> (Bridge, Arc<Mutex<i64>>) {
            let cookies = Arc::new(Mutex::new(cookies));
            let bridge = Bridge::open(Box::new(ReferenceMod {
                speaks_for,
                cookies: Arc::clone(&cookies),
            }))
            .expect("handshake");
            (bridge, cookies)
        }

        fn endpoint(tag: &str) -> String {
            format!("absent-{tag}-{}", std::process::id())
        }

        fn start(inner: &mut Inner) {
            inner.refresh();
            let plan = inner
                .plan(&Command::Start {
                    plugin: PluginId("dev.idlewarden.test-game".to_owned()),
                    profile: "default".to_owned(),
                })
                .expect("a ready session accepts Start");

            let (plugin, governor, wanted) = plan.split();
            let link = match wanted {
                Wanted::Link(link) => link,
                Wanted::Bridge(name) => bridged(
                    &plugin,
                    idlewarden_bridge::connect_or_listen(
                        &name,
                        std::time::Duration::from_millis(200),
                    ),
                ),
            };
            inner.launch(&plugin, governor, link);
        }

        #[test]
        fn a_bridge_the_user_granted_is_tried_and_its_absence_pauses_the_session() {
            let name = endpoint("granted");
            let mut inner = ready_with("bridge-granted", &manifest(&name), REFERENCE_RULES);
            inner
                .profiles
                .update("dev.idlewarden.test-game", |profile| {
                    profile.bridge_granted = true
                });

            start(&mut inner);

            assert!(inner.service.is_none());
            assert_eq!(inner.session.state, SessionState::Paused);
            let reason = inner.session.last_reason.clone().unwrap_or_default();
            assert!(
                reason.starts_with("the mod did not answer") && reason.contains(&name),
                "the user has to learn the mod is missing, and which one: {reason}"
            );
        }

        #[test]
        fn a_bridge_the_user_never_granted_is_not_even_attempted() {
            let name = endpoint("refused");
            let mut inner = ready_with("bridge-refused", &manifest(&name), REFERENCE_RULES);

            start(&mut inner);

            let reason = inner.session.last_reason.clone().unwrap_or_default();
            assert!(
                !reason.contains(&name),
                "declaring a bridge must not be enough to open it: {reason}"
            );
        }

        #[test]
        fn a_mod_that_speaks_for_another_plugin_is_refused() {
            let inner = ready_with("bridge-imposter", &manifest("reference"), REFERENCE_RULES);
            let bundle = inner.plugins.first().expect("the fixture plugin loaded");

            let refused = bridged(&bundle.id, Ok(reference("dev.someone.else", 0).0))
                .err()
                .expect("an imposter is refused");

            assert!(refused.contains("dev.someone.else"), "{refused}");
        }

        #[test]
        fn the_reference_mod_drives_a_session_end_to_end() {
            let mut inner = ready_with("bridge-e2e", &manifest("reference"), REFERENCE_RULES);
            inner.refresh();
            inner
                .session
                .apply(&Command::SetDryRun { enabled: false })
                .expect("dry run can change before a session starts");
            inner
                .session
                .apply(&Command::Start {
                    plugin: PluginId("dev.idlewarden.test-game".to_owned()),
                    profile: "default".to_owned(),
                })
                .expect("a ready session starts");

            let (bridge, cookies) = reference("dev.idlewarden.test-game", 150);
            let bundle = inner.plugins.first().expect("the fixture plugin loaded");
            let link = bridged(&bundle.id, Ok(bridge)).expect("the mod speaks for this plugin");
            inner.service = Some(inner.spawn(bundle, link, GovernorConfig::default()));

            let published = drain_for(&mut inner, 6);

            let succeeded = published.iter().any(|event| {
                matches!(
                    event,
                    Event::ActionFinished {
                        intent,
                        outcome: ActionOutcome::Succeeded,
                    } if intent.name == "buy_upgrade"
                )
            });
            assert!(
                succeeded,
                "the purchase has to be confirmed by the next observation: {:?}",
                names(&published)
            );
            assert!(
                *cookies.lock().unwrap() < 100,
                "the mod has to have actually spent the cookies"
            );
            assert_eq!(inner.session.state, SessionState::Running);
        }
    }
}
