// SPDX-License-Identifier: MPL-2.0

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use idlewarden_capture::{client_bounds, foreground, ScreenRect, WindowHandle};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State};

use crate::hotkeys::{self, Hotkeys};
use crate::session::SessionHandle;

pub const LABEL: &str = "overlay";
const EXPANDED_EVENT: &str = "overlay-expanded";
const FOLLOW_EVERY: Duration = Duration::from_millis(150);
const BADGE: (f64, f64) = (44.0, 44.0);
const PANEL: (f64, f64) = (300.0, 420.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlaySettings {
    pub shown: bool,
    pub corner: Corner,
    pub margin_x: i32,
    pub margin_y: i32,
    pub hotkeys: Hotkeys,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        OverlaySettings {
            shown: true,
            corner: Corner::default(),
            margin_x: 12,
            margin_y: 12,
            hotkeys: Hotkeys::default(),
        }
    }
}

impl OverlaySettings {
    fn clamped(mut self) -> Self {
        self.margin_x = self.margin_x.clamp(0, 2_000);
        self.margin_y = self.margin_y.clamp(0, 2_000);
        self
    }
}

pub struct Overlay {
    path: PathBuf,
    settings: Mutex<OverlaySettings>,
    expanded: AtomicBool,
}

impl Overlay {
    pub fn load(path: &Path) -> Self {
        let settings = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<OverlaySettings>(&raw).ok())
            .unwrap_or_default()
            .clamped();
        Overlay {
            path: path.to_owned(),
            settings: Mutex::new(settings),
            expanded: AtomicBool::new(false),
        }
    }

    pub fn settings(&self) -> OverlaySettings {
        self.settings.lock().expect("overlay lock").clone()
    }

    fn replace(&self, settings: OverlaySettings) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(serialised) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(&self.path, serialised);
        }
        *self.settings.lock().expect("overlay lock") = settings;
    }

    pub fn toggle_shown(&self) {
        let mut settings = self.settings();
        settings.shown = !settings.shown;
        self.replace(settings);
    }

    pub fn toggle_expanded(&self, app: &AppHandle) {
        self.set_expanded(app, !self.expanded.load(Ordering::Relaxed));
    }

    fn set_expanded(&self, app: &AppHandle, expanded: bool) {
        self.expanded.store(expanded, Ordering::Relaxed);
        let _ = app.emit_to(LABEL, EXPANDED_EVENT, expanded);
    }
}

pub fn place(game: ScreenRect, size: (i32, i32), corner: Corner, margin: (i32, i32)) -> (i32, i32) {
    let room_x = (game.width - size.0).max(0);
    let room_y = (game.height - size.1).max(0);
    let x = match corner {
        Corner::TopLeft | Corner::BottomLeft => margin.0,
        Corner::TopRight | Corner::BottomRight => room_x - margin.0,
    };
    let y = match corner {
        Corner::TopLeft | Corner::TopRight => margin.1,
        Corner::BottomLeft | Corner::BottomRight => room_y - margin.1,
    };
    (
        game.left + x.clamp(0, room_x),
        game.top + y.clamp(0, room_y),
    )
}

fn in_front(shown: bool, game: Option<WindowHandle>, front: Option<WindowHandle>) -> bool {
    shown && game.is_some() && front == game
}

type Placement = ((i32, i32), (i32, i32));

fn target(app: &AppHandle, scale: f64) -> Option<Placement> {
    let overlay = app.state::<Overlay>();
    let settings = overlay.settings();
    let game = app.state::<SessionHandle>().window();
    if !in_front(settings.shown, game, foreground()) {
        return None;
    }
    let bounds = client_bounds(game?)?;
    let (width, height) = if overlay.expanded.load(Ordering::Relaxed) {
        PANEL
    } else {
        BADGE
    };
    let scaled = |value: f64| (value * scale).round() as i32;
    let size = (scaled(width), scaled(height));
    let margin = (
        scaled(settings.margin_x.into()),
        scaled(settings.margin_y.into()),
    );
    Some((place(bounds, size, settings.corner, margin), size))
}

pub fn follow(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last: Option<Placement> = None;
        loop {
            std::thread::sleep(FOLLOW_EVERY);
            let Some(window) = app.get_webview_window(LABEL) else {
                continue;
            };
            let next = target(&app, window.scale_factor().unwrap_or(1.0));
            if next == last {
                continue;
            }
            match next {
                Some(((x, y), (width, height))) => {
                    let _ = window.set_size(PhysicalSize::new(width as u32, height as u32));
                    let _ = window.set_position(PhysicalPosition::new(x, y));
                    let _ = window.show();
                }
                None => {
                    let _ = window.hide();
                }
            }
            last = next;
        }
    });
}

#[tauri::command]
pub fn overlay_settings(overlay: State<'_, Overlay>) -> OverlaySettings {
    overlay.settings()
}

#[tauri::command]
pub fn set_overlay_settings(
    app: AppHandle,
    overlay: State<'_, Overlay>,
    settings: OverlaySettings,
) -> Result<OverlaySettings, String> {
    let settings = settings.clamped();
    if let Err(error) = hotkeys::register(&app, &settings.hotkeys) {
        let _ = hotkeys::register(&app, &overlay.settings().hotkeys);
        return Err(error);
    }
    overlay.replace(settings.clone());
    Ok(settings)
}

#[tauri::command]
pub fn set_overlay_expanded(app: AppHandle, overlay: State<'_, Overlay>, expanded: bool) {
    overlay.set_expanded(&app, expanded);
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAME: ScreenRect = ScreenRect {
        left: 100,
        top: 50,
        width: 800,
        height: 600,
    };

    #[test]
    fn each_corner_is_measured_from_the_game_not_the_screen() {
        let size = (40, 40);
        let margin = (10, 20);
        assert_eq!(place(GAME, size, Corner::TopLeft, margin), (110, 70));
        assert_eq!(place(GAME, size, Corner::TopRight, margin), (850, 70));
        assert_eq!(place(GAME, size, Corner::BottomLeft, margin), (110, 590));
        assert_eq!(place(GAME, size, Corner::BottomRight, margin), (850, 590));
    }

    #[test]
    fn a_margin_larger_than_the_game_keeps_the_overlay_inside_it() {
        assert_eq!(
            place(GAME, (40, 40), Corner::TopLeft, (5_000, 5_000)),
            (860, 610)
        );
        assert_eq!(
            place(GAME, (40, 40), Corner::BottomRight, (5_000, 5_000)),
            (100, 50)
        );
    }

    #[test]
    fn a_game_smaller_than_the_panel_pins_it_to_the_game_origin() {
        let tiny = ScreenRect {
            width: 100,
            height: 100,
            ..GAME
        };
        assert_eq!(
            place(tiny, (300, 420), Corner::BottomRight, (12, 12)),
            (100, 50)
        );
    }

    #[test]
    fn the_overlay_shows_only_over_the_game_in_front() {
        let game = Some(WindowHandle(7));
        assert!(in_front(true, game, game));
        assert!(
            !in_front(true, game, Some(WindowHandle(8))),
            "floating over whatever the user switched to would follow them out of the game"
        );
        assert!(!in_front(false, game, game), "hidden by the user");
        assert!(!in_front(true, None, None), "no game detected");
    }

    #[test]
    fn missing_or_partial_settings_fall_back_field_by_field() {
        let dir = std::env::temp_dir().join(format!("idlewarden-overlay-{}", std::process::id()));
        let path = dir.join("overlay.json");
        let _ = std::fs::create_dir_all(&dir);
        assert_eq!(Overlay::load(&path).settings(), OverlaySettings::default());

        std::fs::write(&path, r#"{ "corner": "bottom_left", "margin_x": -40 }"#).unwrap();
        let loaded = Overlay::load(&path).settings();
        assert_eq!(loaded.corner, Corner::BottomLeft);
        assert_eq!(
            loaded.margin_x, 0,
            "a negative margin would push it off the game"
        );
        assert!(loaded.shown);
        assert_eq!(loaded.hotkeys, Hotkeys::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hiding_it_survives_a_restart() {
        let dir =
            std::env::temp_dir().join(format!("idlewarden-overlay-hide-{}", std::process::id()));
        let path = dir.join("overlay.json");
        let _ = std::fs::remove_dir_all(&dir);

        Overlay::load(&path).toggle_shown();

        assert!(!Overlay::load(&path).settings().shown);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
