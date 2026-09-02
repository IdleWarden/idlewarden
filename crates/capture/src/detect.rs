// SPDX-License-Identifier: MPL-2.0
use idlewarden_plugin_api::{GameMatcher, PluginId};

use crate::WindowHandle;

/// A top-level window as the detector sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameWindow {
    pub handle: WindowHandle,
    pub title: String,
    /// File name of the owning process, e.g. `Game.exe`.
    pub executable: String,
    pub steam_appid: Option<u32>,
}

/// What the detector concluded about the desktop as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    /// No loaded plugin matched any window.
    None,
    Found {
        plugin: PluginId,
        window: WindowHandle,
    },
    /// More than one plugin matched. Picking one would be a guess.
    Ambiguous { plugins: Vec<PluginId> },
}

/// Every declared field must match. An empty matcher matches nothing, so a
/// plugin cannot claim every game by leaving its manifest blank.
pub fn matches(matcher: &GameMatcher, window: &GameWindow) -> bool {
    if matcher.is_empty() {
        return false;
    }

    if let Some(appid) = matcher.steam_appid {
        if window.steam_appid != Some(appid) {
            return false;
        }
    }

    if let Some(executable) = &matcher.executable {
        if !executable.eq_ignore_ascii_case(&window.executable) {
            return false;
        }
    }

    if let Some(title) = &matcher.window_title {
        if !window.title.to_lowercase().contains(&title.to_lowercase()) {
            return false;
        }
    }

    true
}

pub fn detect(plugins: &[(PluginId, GameMatcher)], windows: &[GameWindow]) -> Detection {
    let mut hits: Vec<(PluginId, WindowHandle)> = Vec::new();

    for (id, matcher) in plugins {
        if let Some(window) = windows.iter().find(|window| matches(matcher, window)) {
            hits.push((id.clone(), window.handle));
        }
    }

    match hits.len() {
        0 => Detection::None,
        1 => {
            let (plugin, window) = hits.remove(0);
            Detection::Found { plugin, window }
        }
        _ => Detection::Ambiguous {
            plugins: hits.into_iter().map(|(id, _)| id).collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(title: &str, executable: &str) -> GameWindow {
        GameWindow {
            handle: WindowHandle(1),
            title: title.to_owned(),
            executable: executable.to_owned(),
            steam_appid: None,
        }
    }

    fn plugin(name: &str) -> PluginId {
        PluginId(name.to_owned())
    }

    #[test]
    fn an_empty_matcher_claims_nothing() {
        let matcher = GameMatcher::default();

        assert!(
            !matches(&matcher, &window("Anything", "anything.exe")),
            "a blank manifest must not match every game on the desktop"
        );
    }

    #[test]
    fn the_executable_is_compared_without_case() {
        let matcher = GameMatcher {
            executable: Some("Game.EXE".to_owned()),
            ..Default::default()
        };

        assert!(matches(&matcher, &window("whatever", "game.exe")));
    }

    #[test]
    fn the_title_is_a_substring_not_an_equality() {
        let matcher = GameMatcher {
            window_title: Some("Idle Quest".to_owned()),
            ..Default::default()
        };

        assert!(matches(&matcher, &window("Idle Quest - v1.2", "game.exe")));
        assert!(!matches(&matcher, &window("Idle Adventure", "game.exe")));
    }

    #[test]
    fn every_declared_field_has_to_match() {
        let matcher = GameMatcher {
            executable: Some("game.exe".to_owned()),
            window_title: Some("Idle Quest".to_owned()),
            ..Default::default()
        };

        assert!(
            !matches(&matcher, &window("Idle Quest", "other.exe")),
            "matching the title alone must not be enough"
        );
        assert!(
            !matches(&matcher, &window("Other Game", "game.exe")),
            "matching the executable alone must not be enough"
        );
        assert!(matches(&matcher, &window("Idle Quest", "game.exe")));
    }

    #[test]
    fn a_steam_matcher_ignores_windows_with_no_appid() {
        let matcher = GameMatcher {
            steam_appid: Some(480),
            ..Default::default()
        };

        assert!(!matches(&matcher, &window("Idle Quest", "game.exe")));

        let steam = GameWindow {
            steam_appid: Some(480),
            ..window("Idle Quest", "game.exe")
        };
        assert!(matches(&matcher, &steam));
    }

    #[test]
    fn nothing_running_is_not_a_detection() {
        let plugins = [(
            plugin("quest"),
            GameMatcher {
                executable: Some("game.exe".to_owned()),
                ..Default::default()
            },
        )];

        assert_eq!(detect(&plugins, &[]), Detection::None);
    }

    #[test]
    fn one_match_carries_the_window_it_matched() {
        let plugins = [(
            plugin("quest"),
            GameMatcher {
                executable: Some("game.exe".to_owned()),
                ..Default::default()
            },
        )];
        let windows = [
            window("Browser", "chrome.exe"),
            GameWindow {
                handle: WindowHandle(42),
                ..window("Idle Quest", "game.exe")
            },
        ];

        assert_eq!(
            detect(&plugins, &windows),
            Detection::Found {
                plugin: plugin("quest"),
                window: WindowHandle(42),
            }
        );
    }

    #[test]
    fn two_plugins_claiming_the_desktop_is_reported_not_resolved() {
        let plugins = [
            (
                plugin("quest"),
                GameMatcher {
                    executable: Some("game.exe".to_owned()),
                    ..Default::default()
                },
            ),
            (
                plugin("other"),
                GameMatcher {
                    window_title: Some("Idle".to_owned()),
                    ..Default::default()
                },
            ),
        ];
        let windows = [window("Idle Quest", "game.exe")];

        assert_eq!(
            detect(&plugins, &windows),
            Detection::Ambiguous {
                plugins: vec![plugin("quest"), plugin("other")],
            }
        );
    }
}
