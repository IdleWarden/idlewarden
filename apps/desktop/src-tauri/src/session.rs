// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;
use std::sync::Mutex;

use idlewarden_capture::CaptureBackend;
#[cfg(windows)]
use idlewarden_capture::WindowsCapture;
use idlewarden_core::detector::DesktopWindows;
use idlewarden_core::{
    load_all, Command, Detector, Event, Governor, GovernorConfig, Parts, PluginBundle, Refusal,
    Runner, Session, SessionService, SessionState, DEFAULT_TICK,
};
#[cfg(windows)]
use idlewarden_input::{DryRunBackend, Humanisation, SendInputBackend};
use idlewarden_input::{InputBackend, KillSwitch};
use serde::Serialize;
use tauri::State;

type Backends = (Box<dyn CaptureBackend>, Box<dyn InputBackend>);

pub struct SessionHandle(Mutex<Inner>);

struct Inner {
    plugins: Vec<PluginBundle>,
    detector: Detector,
    /// A projection of what the runner reports, not the source of truth. While
    /// no session is running the detector maintains it directly.
    session: Session,
    service: Option<SessionService>,
    events: Vec<Event>,
    kill: KillSwitch,
}

impl SessionHandle {
    pub fn new(plugin_root: PathBuf) -> Self {
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

        SessionHandle(Mutex::new(Inner {
            plugins,
            detector: Detector::new(Box::new(DesktopWindows), matchers),
            session: Session::default(),
            service: None,
            events,
            kill: KillSwitch::new(),
        }))
    }
}

impl Inner {
    /// Detection while idle, published events while running. Called before
    /// anything reads the session, so the UI never sees a stale state.
    fn refresh(&mut self) {
        if self.service.is_none() {
            let found = self.detector.poll(&mut self.session);
            self.events.extend(found);
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
        self.events.extend(published);

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

        let (capture, input) = match self.backends(window) {
            Ok(backends) => backends,
            Err(reason) => {
                self.session.pause(reason.clone());
                self.events.push(Event::Error { message: reason });
                return Ok(());
            }
        };

        self.service = Some(SessionService::spawn(
            Runner::new(Parts {
                capture,
                perceiver: bundle.perceiver(),
                tree: bundle.tree(),
                actuator: Box::new(bundle.actuator()),
                input,
                kill: self.kill.clone(),
                governor: Governor::new(GovernorConfig::default(), 0),
                session: self.session.clone(),
            }),
            DEFAULT_TICK,
        ));
        Ok(())
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
pub fn session_events(handle: State<'_, SessionHandle>) -> Vec<Event> {
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

#[tauri::command]
pub fn engage_kill_switch(handle: State<'_, SessionHandle>) -> Session {
    let mut inner = handle.0.lock().expect("session lock");
    inner.kill.engage();
    inner.session.state = SessionState::Halted;
    inner.service = None;
    inner.events.push(Event::KillSwitch);
    inner.session.clone()
}
