// SPDX-License-Identifier: MPL-2.0
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use idlewarden_capture::WindowHandle;
use idlewarden_core::PluginId;
use idlewarden_mod_install::{install, Index};
use serde::Serialize;
use tauri::State;

use crate::session::{ModRequest, SessionHandle};

const REGISTRY_INDEX: &str = "https://idlewarden.github.io/registry/index.json";
const LARGEST_INDEX: u64 = 8 * 1024 * 1024;
const LARGEST_ARCHIVE: u64 = 128 * 1024 * 1024;

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

#[cfg(windows)]
fn locate(window: WindowHandle) -> Option<PathBuf> {
    idlewarden_capture::game_directory(window)
}

#[cfg(not(windows))]
fn locate(_window: WindowHandle) -> Option<PathBuf> {
    None
}

fn fetch_and_install(plugin: &PluginId, bridge: &str, game: &Path) -> Result<Installed, Refused> {
    let listing = registry_index(std::env::var_os("IDLEWARDEN_REGISTRY_INDEX"))?;
    let index: Index = serde_json::from_slice(&listing)
        .map_err(|error| Refused::failed(format!("the registry index is unreadable: {error}")))?;
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

    let archive = download(&release.version.url, LARGEST_ARCHIVE)?;
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

fn registry_index(local: Option<OsString>) -> Result<Vec<u8>, Refused> {
    let Some(path) = local.map(PathBuf::from) else {
        return download(REGISTRY_INDEX, LARGEST_INDEX);
    };
    tracing::warn!(path = %path.display(), "reading the registry index from a local file");
    std::fs::read(&path).map_err(|error| {
        Refused::failed(format!(
            "cannot read the registry index at {}: {error}",
            path.display()
        ))
    })
}

fn download(url: &str, limit: u64) -> Result<Vec<u8>, Refused> {
    if !url.starts_with("https://") {
        return Err(Refused::failed(format!(
            "refusing to download over anything but https: {url}"
        )));
    }
    ureq::get(url)
        .call()
        .map_err(|error| Refused::failed(format!("cannot download {url}: {error}")))?
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|error| Refused::failed(format!("cannot read {url}: {error}")))
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

    #[test]
    fn a_local_index_is_read_from_disk_instead_of_the_registry() {
        let path = folder("local-index").join("index.json");
        std::fs::write(&path, br#"{"mods": []}"#).expect("index written");

        let listing = registry_index(Some(path.into_os_string())).expect("reads the file");

        assert_eq!(listing, br#"{"mods": []}"#);
    }

    #[test]
    fn a_local_index_that_is_missing_says_where_it_looked() {
        let path = folder("absent-index").join("index.json");

        let refused = registry_index(Some(path.clone().into_os_string())).unwrap_err();

        assert!(
            refused.message.contains(&path.display().to_string()),
            "{}",
            refused.message
        );
    }

    #[test]
    fn a_download_that_is_not_https_never_leaves_the_machine() {
        let refused = download("http://example.com/mod.zip", LARGEST_ARCHIVE).unwrap_err();

        assert_eq!(refused.why, Why::Failed);
        assert!(refused.message.contains("https"), "{}", refused.message);
    }
}
