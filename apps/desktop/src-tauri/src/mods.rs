// SPDX-License-Identifier: MPL-2.0
use std::path::{Path, PathBuf};

use idlewarden_capture::WindowHandle;
use idlewarden_core::PluginId;
use idlewarden_mod_install::install;
use serde::Serialize;
use tauri::State;

use crate::registry;
use crate::session::{ModRequest, SessionHandle};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Installed {
    pub name: String,
    pub version: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    UnknownPlugin,
    NoBridge,
    NotGranted,
    GameUnknown,
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Refused {
    pub why: Why,
    pub message: String,
}

impl Refused {
    fn new(why: Why, message: impl Into<String>) -> Self {
        Refused {
            why,
            message: message.into(),
        }
    }

    fn failed(message: impl ToString) -> Self {
        Refused::new(Why::Failed, message.to_string())
    }
}

#[tauri::command]
pub async fn install_mod(
    handle: State<'_, SessionHandle>,
    plugin: String,
    game: Option<String>,
) -> Result<Installed, Refused> {
    let (bridge, game) = target(handle.mod_request(&plugin), game.map(PathBuf::from), locate)?;
    let plugin = PluginId(plugin);
    tauri::async_runtime::spawn_blocking(move || fetch_and_install(&plugin, &bridge, &game))
        .await
        .map_err(Refused::failed)?
}

fn target(
    request: Option<ModRequest>,
    chosen: Option<PathBuf>,
    locate: impl FnOnce(WindowHandle) -> Option<PathBuf>,
) -> Result<(String, PathBuf), Refused> {
    let request = request
        .ok_or_else(|| Refused::new(Why::UnknownPlugin, "no plugin with this id is installed"))?;
    let bridge = request
        .bridge
        .ok_or_else(|| Refused::new(Why::NoBridge, "this plugin does not use a mod"))?;
    if !request.granted {
        return Err(Refused::new(
            Why::NotGranted,
            "allow the mod for this plugin before installing it",
        ));
    }

    let game = match chosen {
        Some(folder) if folder.is_dir() => folder,
        Some(folder) => {
            return Err(Refused::new(
                Why::GameUnknown,
                format!("{} is not a folder", folder.display()),
            ))
        }
        None => request.window.and_then(locate).ok_or_else(|| {
            Refused::new(
                Why::GameUnknown,
                "the game is not running, so its folder is unknown",
            )
        })?,
    };
    Ok((bridge, game))
}

#[cfg(any(windows, target_os = "linux"))]
fn locate(window: WindowHandle) -> Option<PathBuf> {
    idlewarden_capture::game_directory(window)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn locate(_window: WindowHandle) -> Option<PathBuf> {
    None
}

fn fetch_and_install(plugin: &PluginId, bridge: &str, game: &Path) -> Result<Installed, Refused> {
    let index = registry::index().map_err(Refused::failed)?;
    let release = index.release_for(plugin, bridge).ok_or_else(|| {
        Refused::failed(format!(
            "the registry lists no mod serving `{bridge}` for {}",
            plugin.0
        ))
    })?;

    let reloaded = std::env::var_os("RELOADEDIIMODS").map(PathBuf::from);
    let destination = release
        .destination(game, reloaded.as_deref())
        .map_err(Refused::failed)?;

    let archive = registry::download(&release.version.url, registry::LARGEST_ARCHIVE)
        .map_err(Refused::failed)?;
    let path = install(
        &archive,
        &release.version.sha256,
        &release.entry.id,
        &destination,
    )
    .map_err(Refused::failed)?;

    tracing::info!(
        plugin = plugin.0.as_str(),
        r#mod = release.entry.id.as_str(),
        version = %release.version.version,
        path = %path.display(),
        "mod installed"
    );
    Ok(Installed {
        name: release.entry.name.clone(),
        version: release.version.version.to_string(),
        path: path.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: WindowHandle = WindowHandle(7);

    fn request(granted: bool, window: Option<WindowHandle>) -> Option<ModRequest> {
        Some(ModRequest {
            bridge: Some("reference".to_owned()),
            granted,
            window,
        })
    }

    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("idlewarden-mods-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("folder");
        dir
    }

    fn nowhere(_: WindowHandle) -> Option<PathBuf> {
        None
    }

    #[test]
    fn a_bridge_the_user_has_not_allowed_is_refused_even_with_a_folder() {
        let refused = target(
            request(false, Some(WINDOW)),
            Some(folder("refused")),
            nowhere,
        )
        .expect_err("the capability gates the download, not only the connection");

        assert_eq!(refused.why, Why::NotGranted);
    }

    #[test]
    fn a_plugin_without_a_bridge_has_nothing_to_install() {
        let request = Some(ModRequest {
            bridge: None,
            granted: true,
            window: Some(WINDOW),
        });

        assert_eq!(
            target(request, None, nowhere).unwrap_err().why,
            Why::NoBridge
        );
        assert_eq!(
            target(None, None, nowhere).unwrap_err().why,
            Why::UnknownPlugin
        );
    }

    #[test]
    fn the_running_game_tells_where_it_is_installed() {
        let game = folder("detected");

        let (bridge, found) = target(request(true, Some(WINDOW)), None, |window| {
            (window == WINDOW).then(|| game.clone())
        })
        .expect("the detected window locates the game");

        assert_eq!(bridge, "reference");
        assert_eq!(found, game);
    }

    #[test]
    fn a_folder_the_user_picked_wins_over_the_detected_one() {
        let picked = folder("picked");

        let (_, found) = target(request(true, Some(WINDOW)), Some(picked.clone()), |_| {
            Some(folder("detected-elsewhere"))
        })
        .expect("installs where the user said");

        assert_eq!(found, picked);
    }

    #[test]
    fn with_no_game_running_and_no_folder_the_user_is_asked() {
        let refused = target(request(true, None), None, nowhere).unwrap_err();

        assert_eq!(refused.why, Why::GameUnknown);
    }

    #[test]
    fn a_picked_path_that_is_not_a_folder_is_asked_again() {
        let refused = target(
            request(true, None),
            Some(folder("parent").join("missing")),
            nowhere,
        )
        .unwrap_err();

        assert_eq!(refused.why, Why::GameUnknown);
    }
}
