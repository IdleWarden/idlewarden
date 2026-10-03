// SPDX-License-Identifier: MPL-2.0

use idlewarden_core::{Command, Session, SessionState};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::overlay::Overlay;
use crate::session::SessionHandle;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub toggle_watch: String,
    pub toggle_panel: String,
    pub toggle_overlay: String,
    pub kill_switch: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Hotkeys {
            toggle_watch: "Ctrl+Shift+F7".to_owned(),
            toggle_panel: "Ctrl+Shift+F8".to_owned(),
            toggle_overlay: "Ctrl+Shift+F9".to_owned(),
            kill_switch: "Ctrl+Shift+F10".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    ToggleWatch,
    TogglePanel,
    ToggleOverlay,
    KillSwitch,
}

impl Hotkeys {
    fn bindings(&self) -> Result<Vec<(Shortcut, Action)>, String> {
        if self.kill_switch.trim().is_empty() {
            return Err("the kill switch must keep a key".to_owned());
        }

        let mut bindings: Vec<(Shortcut, Action)> = Vec::new();
        for (keys, action) in [
            (&self.toggle_watch, Action::ToggleWatch),
            (&self.toggle_panel, Action::TogglePanel),
            (&self.toggle_overlay, Action::ToggleOverlay),
            (&self.kill_switch, Action::KillSwitch),
        ] {
            if keys.trim().is_empty() {
                continue;
            }
            let shortcut: Shortcut = keys
                .parse()
                .map_err(|error| format!("`{keys}` is not a key combination: {error}"))?;
            if bindings.iter().any(|(taken, _)| *taken == shortcut) {
                return Err(format!("`{keys}` is bound twice"));
            }
            bindings.push((shortcut, action));
        }
        Ok(bindings)
    }
}

pub fn register(app: &AppHandle, hotkeys: &Hotkeys) -> Result<(), String> {
    let bindings = hotkeys.bindings()?;
    let shortcuts = app.global_shortcut();
    shortcuts
        .unregister_all()
        .map_err(|error| error.to_string())?;

    for (shortcut, action) in bindings {
        shortcuts
            .on_shortcut(shortcut, move |app, _, event| {
                if event.state == ShortcutState::Pressed {
                    perform(app, action);
                }
            })
            .map_err(|error| format!("`{shortcut}` could not be registered: {error}"))?;
    }
    Ok(())
}

fn perform(app: &AppHandle, action: Action) {
    match action {
        Action::KillSwitch => {
            app.state::<SessionHandle>().engage_kill_switch();
        }
        Action::ToggleWatch => {
            let Some(command) = toggle(&app.state::<SessionHandle>().state()) else {
                return;
            };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = app.state::<SessionHandle>().dispatch(command).await;
            });
        }
        Action::TogglePanel => app.state::<Overlay>().toggle_expanded(app),
        Action::ToggleOverlay => app.state::<Overlay>().toggle_shown(),
    }
}

fn toggle(session: &Session) -> Option<Command> {
    match session.state {
        SessionState::Running => Some(Command::Pause),
        SessionState::Paused => Some(Command::Resume),
        SessionState::Ready => session.plugin.clone().map(|plugin| Command::Start {
            plugin,
            profile: "default".to_owned(),
        }),
        SessionState::Searching | SessionState::Halted => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlewarden_core::PluginId;

    fn session(state: SessionState, plugin: Option<&str>) -> Session {
        Session {
            state,
            plugin: plugin.map(|id| PluginId(id.to_owned())),
            ..Session::default()
        }
    }

    #[test]
    fn the_defaults_all_parse_and_differ() {
        assert_eq!(Hotkeys::default().bindings().map(|b| b.len()), Ok(4));
    }

    #[test]
    fn the_kill_switch_cannot_be_unbound() {
        let hotkeys = Hotkeys {
            kill_switch: " ".to_owned(),
            ..Hotkeys::default()
        };
        assert!(
            hotkeys.bindings().is_err(),
            "ADR-0007: someone who cannot use the mouse must still be able to stop it"
        );
    }

    #[test]
    fn the_other_actions_can_be_left_without_a_key() {
        let hotkeys = Hotkeys {
            toggle_panel: String::new(),
            toggle_overlay: String::new(),
            ..Hotkeys::default()
        };
        assert_eq!(hotkeys.bindings().map(|b| b.len()), Ok(2));
    }

    #[test]
    fn one_combination_bound_to_two_actions_is_refused() {
        let hotkeys = Hotkeys {
            toggle_watch: "ctrl+shift+F10".to_owned(),
            ..Hotkeys::default()
        };
        assert!(hotkeys.bindings().unwrap_err().contains("twice"));
    }

    #[test]
    fn a_combination_that_does_not_parse_is_named_in_the_error() {
        let hotkeys = Hotkeys {
            toggle_watch: "Ctrl+Nope".to_owned(),
            ..Hotkeys::default()
        };
        assert!(hotkeys.bindings().unwrap_err().contains("Ctrl+Nope"));
    }

    #[test]
    fn toggling_the_watch_follows_the_session() {
        assert_eq!(
            toggle(&session(SessionState::Running, Some("kale"))),
            Some(Command::Pause)
        );
        assert_eq!(
            toggle(&session(SessionState::Paused, Some("kale"))),
            Some(Command::Resume)
        );
        assert_eq!(
            toggle(&session(SessionState::Ready, Some("kale"))),
            Some(Command::Start {
                plugin: PluginId("kale".to_owned()),
                profile: "default".to_owned(),
            })
        );
    }

    #[test]
    fn nothing_to_toggle_without_a_game_or_after_the_kill_switch() {
        assert_eq!(toggle(&session(SessionState::Searching, None)), None);
        assert_eq!(
            toggle(&session(SessionState::Halted, Some("kale"))),
            None,
            "a hotkey must not undo the kill switch"
        );
    }
}
