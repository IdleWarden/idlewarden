// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use idlewarden_capture::CaptureBackend;
#[cfg(windows)]
use idlewarden_capture::WindowsCapture;
use idlewarden_core::detector::{Candidate, DesktopWindows};
use idlewarden_core::{
    load_all, Command, Detector, Event, Governor, GovernorConfig, Parts, PluginBundle, Refusal,
    Runner, Session, SessionService, SessionState, DEFAULT_TICK,
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
    plugins: Vec<PluginBundle>,
    detector: Detector,
    /// A projection of what the runner reports, not the source of truth. While
    /// no session is running the detector maintains it directly.
    session: Session,
    service: Option<SessionService>,
    events: Vec<Published>,
    kill: KillSwitch,
    /// Per-game limits, on disk. The Governor is handed them when a session
    /// starts; nothing here decides whether an action is allowed.
    profiles: Profiles,
}

/// One plugin as the sidebar and the automations list need it.
#[derive(Debug, Serialize)]
pub struct PluginSummary {
    pub id: String,
    pub detected: bool,
    pub intents: Vec<IntentSummary>,
}

/// One window detection looked at, as the Detect screen renders it.
#[derive(Debug, Serialize)]
pub struct WindowCandidate {
    pub title: String,
    pub executable: String,
    pub steam_appid: Option<u32>,
    pub plugins: Vec<String>,
}

impl From<Candidate> for WindowCandidate {
    fn from(candidate: Candidate) -> Self {
        WindowCandidate {
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

#[derive(Debug, Serialize)]
pub struct IntentSummary {
    pub name: String,
    pub enabled: bool,
}

impl SessionHandle {
    pub fn new(data_dir: PathBuf) -> Self {
        let plugin_root = data_dir.join("plugins");
        let profiles = Profiles::load(&data_dir.join("profiles.json"));
        let mut plugins = Vec::new();
        let mut events = Vec::new();

        for (path, loaded) in load_all(&plugin_root) {
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

        let matchers = plugins
            .iter()
            .map(|bundle| (bundle.id.clone(), bundle.matcher.clone()))
            .collect();

        let at_ms = now_ms();
        let events = events
            .into_iter()
            .map(|event| Published { at_ms, event })
            .collect();

        SessionHandle(Mutex::new(Inner {
            plugins,
            detector: Detector::new(Box::new(DesktopWindows), matchers),
            session: Session::default(),
            service: None,
            events,
            kill: KillSwitch::new(),
            profiles,
        }))
    }
}

impl Inner {
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

    /// No capture or input backend exists off Windows yet (#11). Saying so is
    /// better than running a session over blank frames.
    #[cfg(not(windows))]
    fn backends(&self, _window: idlewarden_capture::WindowHandle) -> Result<Backends, String> {
        Err("capture and input are only implemented on Windows".to_owned())
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
            .map(|bundle| PluginSummary {
                id: bundle.id.0.clone(),
                detected: self.session.plugin.as_ref() == Some(&bundle.id),
                intents: {
                    let profile = self.profiles.get(&bundle.id.0);
                    bundle
                        .rules
                        .intents
                        .iter()
                        .map(|intent| IntentSummary {
                            name: intent.name.clone(),
                            enabled: profile.is_enabled(&intent.name),
                        })
                        .collect()
                },
            })
            .collect()
    }

    fn start(&mut self, command: &Command) -> Result<(), Refusal> {
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

        let (capture, input) = match self.backends(window) {
            Ok(backends) => backends,
            Err(reason) => {
                self.session.pause(reason.clone());
                self.publish([Event::Error { message: reason }]);
                return Ok(());
            }
        };

        self.service = Some(self.spawn(bundle, capture, input, governor));
        Ok(())
    }

    /// Puts a runner on its own thread for this bundle.
    ///
    /// Split from [`Inner::start`] because acquiring backends and assembling a
    /// session are different jobs with very different testability: the first
    /// needs a real game window, the second is the glue #31 is about and can be
    /// driven with any backend.
    fn spawn(
        &self,
        bundle: &PluginBundle,
        capture: Box<dyn CaptureBackend>,
        input: Box<dyn InputBackend>,
        governor: GovernorConfig,
    ) -> SessionService {
        SessionService::spawn(
            Runner::new(Parts {
                capture,
                perceiver: bundle.perceiver(),
                tree: bundle.tree(),
                actuator: Box::new(bundle.actuator()),
                input,
                kill: self.kill.clone(),
                governor: Governor::new(governor, 0),
                session: self.session.clone(),
            }),
            DEFAULT_TICK,
        )
    }
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
pub fn dispatch(handle: State<'_, SessionHandle>, command: Command) -> Result<Session, Refused> {
    let mut inner = handle.0.lock().expect("session lock");
    inner.refresh();

    match &command {
        Command::Start { .. } => inner.start(&command)?,
        Command::Stop => {
            inner.session.apply(&command)?;
            inner.service = None;
            inner.kill.reset();
        }
        _ => {
            inner.session.apply(&command)?;
            if let Some(service) = &inner.service {
                service.send(command);
            }
        }
    }

    Ok(inner.session.clone())
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

/// The limits one plugin runs under. Defaults until the user saves something.
#[tauri::command]
pub fn profile(handle: State<'_, SessionHandle>, plugin: String) -> Profile {
    let inner = handle.0.lock().expect("session lock");
    inner.profiles.get(&plugin)
}

/// Stores the edited limits and returns what was actually kept, which is the
/// clamped form rather than the raw input.
#[tauri::command]
pub fn set_profile(handle: State<'_, SessionHandle>, plugin: String, profile: Profile) -> Profile {
    let mut inner = handle.0.lock().expect("session lock");
    inner.profiles.set(&plugin, profile)
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
    use idlewarden_core::PluginId;
    use idlewarden_core::WindowSource;
    use idlewarden_input::DryRunBackend;
    use std::sync::Arc;

    const WINDOW: WindowHandle = WindowHandle(4242);

    /// The desktop as the test hands it over. `DesktopWindows` is the real one;
    /// this exists so the glue can be driven on any machine, which is the same
    /// seam `Detector` already documents.
    struct Fixed(Vec<GameWindow>);

    impl WindowSource for Fixed {
        fn windows(&mut self) -> Vec<GameWindow> {
            self.0.clone()
        }
    }

    /// A capture backend that hands out one prepared frame for ever.
    ///
    /// Not a blank frame: the pixel the plugin probes is lit, so perception,
    /// the tree and the Governor all do real work on it. A blank frame would
    /// make the loop turn while proving nothing, which is exactly what #31
    /// warned against.
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
                // The example rules probe a gold pixel at 0.49..0.51 x
                // 0.71..0.73 to decide a reward is collectable.
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

    fn plugin_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("idlewarden-{name}-{}", std::process::id()));
        let plugin = root.join("plugins").join("test-game");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&plugin).expect("the fixture plugin could be created");
        std::fs::write(plugin.join("plugin.json"), MANIFEST).expect("manifest written");
        std::fs::write(plugin.join("rules.json"), RULES).expect("rules written");
        root
    }

    /// A handle whose detector reports one window, matched by the fixture
    /// plugin, so `start` has a game to bind to.
    fn ready(name: &str) -> Inner {
        let handle = SessionHandle::new(plugin_root(name));
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

    /// Runs the session for a while, collecting everything it publishes, the
    /// way the UI's poll does.
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
        let service = inner.spawn(bundle, Box::new(capture), Box::new(DryRunBackend), governor);
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
}
