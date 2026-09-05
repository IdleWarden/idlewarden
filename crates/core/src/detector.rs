// SPDX-License-Identifier: MPL-2.0
use idlewarden_capture::{detect, Detection, GameWindow, WindowHandle};
use idlewarden_plugin_api::{GameMatcher, PluginId};

use crate::{Event, Session, SessionState};

/// Where the candidate windows come from. The desktop provides the real one;
/// tests hand over a fixed list so detection is exercised on any OS.
pub trait WindowSource: Send {
    fn windows(&mut self) -> Vec<GameWindow>;
}

/// Moves a session between `Searching` and `Ready` as the game comes and goes.
pub struct Detector {
    source: Box<dyn WindowSource>,
    plugins: Vec<(PluginId, GameMatcher)>,
    current: Option<WindowHandle>,
}

impl Detector {
    pub fn new(source: Box<dyn WindowSource>, plugins: Vec<(PluginId, GameMatcher)>) -> Self {
        Detector {
            source,
            plugins,
            current: None,
        }
    }

    /// The window the session is currently bound to, which is what a capture
    /// backend has to be built against.
    pub fn window(&self) -> Option<WindowHandle> {
        self.current
    }

    pub fn poll(&mut self, session: &mut Session) -> Vec<Event> {
        if session.state == SessionState::Halted {
            return Vec::new();
        }

        let windows = self.source.windows();
        match detect(&self.plugins, &windows) {
            Detection::Found { plugin, window } => {
                if self.current == Some(window) {
                    return Vec::new();
                }
                self.current = Some(window);
                let window_title = windows
                    .iter()
                    .find(|candidate| candidate.handle == window)
                    .map(|candidate| candidate.title.clone())
                    .unwrap_or_default();
                session.game_detected(plugin.clone());
                vec![Event::GameDetected {
                    plugin,
                    window_title,
                }]
            }
            Detection::None => {
                if self.current.take().is_none() {
                    return Vec::new();
                }
                session.game_lost();
                vec![Event::GameLost]
            }
            Detection::Ambiguous { plugins } => {
                let names: Vec<&str> = plugins.iter().map(|plugin| plugin.0.as_str()).collect();
                vec![Event::Error {
                    message: format!(
                        "{} plugins claim a running game ({}); refusing to pick one",
                        plugins.len(),
                        names.join(", ")
                    ),
                }]
            }
        }
    }
}

/// The real desktop. Off Windows it reports nothing, so a session stays in
/// `Searching` rather than pretending to have found a game (#11).
pub struct DesktopWindows;

impl WindowSource for DesktopWindows {
    #[cfg(windows)]
    fn windows(&mut self) -> Vec<GameWindow> {
        idlewarden_capture::enumerate_windows()
    }

    #[cfg(not(windows))]
    fn windows(&mut self) -> Vec<GameWindow> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Vec<GameWindow>);

    impl WindowSource for Fixed {
        fn windows(&mut self) -> Vec<GameWindow> {
            self.0.clone()
        }
    }

    fn window(handle: isize, title: &str, executable: &str) -> GameWindow {
        GameWindow {
            handle: WindowHandle(handle),
            title: title.to_owned(),
            executable: executable.to_owned(),
            steam_appid: None,
        }
    }

    fn matcher(executable: &str) -> GameMatcher {
        GameMatcher {
            executable: Some(executable.to_owned()),
            ..Default::default()
        }
    }

    fn detector(windows: Vec<GameWindow>, plugins: &[(&str, &str)]) -> Detector {
        Detector::new(
            Box::new(Fixed(windows)),
            plugins
                .iter()
                .map(|(id, exe)| (PluginId((*id).to_owned()), matcher(exe)))
                .collect(),
        )
    }

    #[test]
    fn a_matching_game_takes_the_session_out_of_searching() {
        let mut session = Session::default();
        let mut detector = detector(
            vec![window(7, "Idle Quest - v1.2", "game.exe")],
            &[("quest", "game.exe")],
        );

        let events = detector.poll(&mut session);

        assert_eq!(session.state, SessionState::Ready);
        assert_eq!(session.plugin, Some(PluginId("quest".to_owned())));
        assert_eq!(detector.window(), Some(WindowHandle(7)));
        assert!(matches!(
            events.as_slice(),
            [Event::GameDetected { window_title, .. }] if window_title == "Idle Quest - v1.2"
        ));
    }

    #[test]
    fn polling_again_on_the_same_window_says_nothing() {
        let mut session = Session::default();
        let mut detector = detector(
            vec![window(7, "Idle Quest", "game.exe")],
            &[("quest", "game.exe")],
        );

        detector.poll(&mut session);
        let events = detector.poll(&mut session);

        assert!(
            events.is_empty(),
            "a steady desktop must not republish detection on every tick"
        );
        assert_eq!(session.state, SessionState::Ready);
    }

    #[test]
    fn the_game_closing_returns_the_session_to_searching() {
        let mut session = Session::default();
        let mut detector = detector(
            vec![window(7, "Idle Quest", "game.exe")],
            &[("quest", "game.exe")],
        );
        detector.poll(&mut session);

        detector.source = Box::new(Fixed(Vec::new()));
        let events = detector.poll(&mut session);

        assert_eq!(session.state, SessionState::Searching);
        assert_eq!(session.plugin, None);
        assert_eq!(detector.window(), None);
        assert!(matches!(events.as_slice(), [Event::GameLost]));
    }

    #[test]
    fn an_empty_desktop_stays_quiet() {
        let mut session = Session::default();
        let mut detector = detector(Vec::new(), &[("quest", "game.exe")]);

        assert!(detector.poll(&mut session).is_empty());
        assert_eq!(session.state, SessionState::Searching);
    }

    #[test]
    fn two_plugins_claiming_the_game_is_reported_and_nothing_is_picked() {
        let mut session = Session::default();
        let mut detector = detector(
            vec![window(7, "Idle Quest", "game.exe")],
            &[("quest", "game.exe"), ("clone", "game.exe")],
        );

        let events = detector.poll(&mut session);

        assert_eq!(
            session.state,
            SessionState::Searching,
            "an ambiguous desktop must not start a session"
        );
        assert_eq!(session.plugin, None);
        assert!(matches!(
            events.as_slice(),
            [Event::Error { message }] if message.contains("quest") && message.contains("clone")
        ));
    }

    #[test]
    fn ambiguity_does_not_drop_a_game_already_running() {
        let mut session = Session::default();
        let mut detector = detector(
            vec![window(7, "Idle Quest", "game.exe")],
            &[("quest", "game.exe")],
        );
        detector.poll(&mut session);

        detector
            .plugins
            .push((PluginId("clone".to_owned()), matcher("game.exe")));
        detector.poll(&mut session);

        assert_eq!(
            session.state,
            SessionState::Ready,
            "a second plugin appearing must not tear down a running session"
        );
        assert_eq!(detector.window(), Some(WindowHandle(7)));
    }

    #[test]
    fn a_halted_session_is_left_alone() {
        let mut session = Session {
            state: SessionState::Halted,
            ..Default::default()
        };
        let mut detector = detector(
            vec![window(7, "Idle Quest", "game.exe")],
            &[("quest", "game.exe")],
        );

        let events = detector.poll(&mut session);

        assert!(events.is_empty());
        assert_eq!(session.state, SessionState::Halted);
        assert_eq!(detector.window(), None);
    }
}
