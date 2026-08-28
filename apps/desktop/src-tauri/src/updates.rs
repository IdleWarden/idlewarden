// SPDX-License-Identifier: MPL-2.0
//! Asking the update endpoint whether a newer build exists.
//!
//! Ce module décide, le plugin exécute.
//!
//! L'interrogation passe par `tauri-plugin-updater` et non par un appel HTTP
//! maison : l'endpoint sert déjà le format que le plugin attend — `version`,
//! `pub_date`, `url`, `signature`, et un 204 pour « rien de neuf » — donc il
//! n'y a rien à traduire entre les deux.
//!
//! Un seul chemin de requête, et c'est ce qui compte : le déploiement
//! progressif est arbitré par l'endpoint à partir de l'en-tête d'installation.
//! Deux chemins de vérification laisseraient une machine écartée du rollout
//! recevoir la mise à jour par l'autre, sans que rien ne le signale.
//!
//! Ce qui reste ici est ce que le plugin ne sait pas faire : retenir le canal
//! choisi, et donner à l'installation un identifiant stable.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::UpdaterExt;

const ENDPOINT: &str = "https://idlewarden.com/api";
const INSTALL_HEADER: &str = "x-idlewarden-install";
const SETTINGS_FILE: &str = "updates.json";
const INSTALL_FILE: &str = "install-id";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
    Stable,
    Beta,
}

impl Channel {
    fn as_str(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub channel: Channel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    pub version: String,
    pub pub_date: String,
    pub url: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CheckResult {
    UpToDate,
    Available { offer: Box<Offer> },
}

/// Les échecs que ce module peut rendre au front.
///
/// Plus de variante `Status` : c'est le plugin qui parle HTTP désormais, et il
/// range les codes de réponse dans le message de ses propres erreurs, qui
/// arrivent ici en `Unreachable`. Garder une variante que plus rien ne
/// construit aurait laissé croire au front qu'il peut encore la distinguer.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("cannot reach the update endpoint: {0}")]
    Unreachable(String),
    #[error("the update endpoint sent something unreadable: {0}")]
    Malformed(String),
    #[error("cannot read or write {path}: {source}")]
    Storage {
        path: String,
        source: std::io::Error,
    },
}

impl serde::Serialize for UpdateError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// The address the endpoint is asked for, built the way the service parses it.
pub fn check_url(endpoint: &str, target: &str, current: &str, channel: Channel) -> String {
    format!(
        "{}/v1/update/{target}/{current}?channel={}",
        endpoint.trim_end_matches('/'),
        channel.as_str()
    )
}

/// Tauri's own target triple names, which the endpoint keys its catalogue on.
pub fn target() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else {
        "linux-x86_64"
    }
}

/// A stable per-installation identifier, so a staged rollout can bucket this
/// machine consistently. It identifies an install, never a person: it is a
/// random value generated locally and sent to nothing but our own endpoint.
pub fn install_id(dir: &Path) -> Result<String, UpdateError> {
    let path = dir.join(INSTALL_FILE);
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim();
        if !existing.is_empty() {
            return Ok(existing.to_owned());
        }
    }

    let fresh = uuid::Uuid::new_v4().to_string();
    write(dir, &path, &fresh)?;
    Ok(fresh)
}

pub fn load_settings(dir: &Path) -> Settings {
    std::fs::read_to_string(dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_settings(dir: &Path, settings: &Settings) -> Result<(), UpdateError> {
    let body = serde_json::to_string_pretty(settings).expect("settings are always serialisable");
    write(dir, &dir.join(SETTINGS_FILE), &body)
}

fn write(dir: &Path, path: &Path, body: &str) -> Result<(), UpdateError> {
    std::fs::create_dir_all(dir).map_err(|source| UpdateError::Storage {
        path: dir.display().to_string(),
        source,
    })?;
    std::fs::write(path, body).map_err(|source| UpdateError::Storage {
        path: path.display().to_string(),
        source,
    })
}

pub struct Updates {
    pub directory: PathBuf,
}

impl Updates {
    pub fn new(app: &AppHandle) -> Self {
        let directory = app
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        Updates { directory }
    }
}

#[tauri::command]
pub fn update_settings(updates: State<'_, Updates>) -> Settings {
    load_settings(&updates.directory)
}

#[tauri::command]
pub fn set_update_channel(
    updates: State<'_, Updates>,
    channel: Channel,
) -> Result<Settings, UpdateError> {
    let settings = Settings { channel };
    save_settings(&updates.directory, &settings)?;
    Ok(settings)
}

/// Construit un updater visant le canal choisi, en annonçant l'installation.
///
/// L'endpoint est reconstruit à chaque appel plutôt que laissé à celui de
/// `tauri.conf.json` : le canal est un paramètre de requête, et l'utilisateur
/// peut en changer sans redémarrer l'application.
async fn updater_for(
    updates: &State<'_, Updates>,
    app: &AppHandle,
) -> Result<tauri_plugin_updater::Updater, UpdateError> {
    let directory = updates.directory.clone();
    let (settings, install) = tauri::async_runtime::spawn_blocking(move || {
        let settings = load_settings(&directory);
        install_id(&directory).map(|install| (settings, install))
    })
    .await
    .map_err(|error| UpdateError::Unreachable(error.to_string()))??;

    let current = app.package_info().version.to_string();
    let url = check_url(ENDPOINT, target(), &current, settings.channel);
    // Pas d'annotation de type : la cible est déduite de `endpoints`, ce qui
    // évite de déclarer le crate `url` juste pour nommer une erreur.
    let endpoint = url
        .parse()
        .map_err(|_| UpdateError::Malformed(format!("endpoint invalide : {url}")))?;

    app.updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|error| UpdateError::Unreachable(error.to_string()))?
        .header(INSTALL_HEADER, install)
        .map_err(|error| UpdateError::Unreachable(error.to_string()))?
        .build()
        .map_err(|error| UpdateError::Unreachable(error.to_string()))
}

fn offer_from(update: &tauri_plugin_updater::Update) -> Offer {
    Offer {
        version: update.version.clone(),
        pub_date: update.date.map(|date| date.to_string()).unwrap_or_default(),
        url: update.download_url.to_string(),
        notes: update.body.clone(),
    }
}

#[tauri::command]
pub async fn check_for_update(
    updates: State<'_, Updates>,
    app: AppHandle,
) -> Result<CheckResult, UpdateError> {
    let updater = updater_for(&updates, &app).await?;
    match updater
        .check()
        .await
        .map_err(|error| UpdateError::Unreachable(error.to_string()))?
    {
        None => Ok(CheckResult::UpToDate),
        Some(update) => Ok(CheckResult::Available {
            offer: Box::new(offer_from(&update)),
        }),
    }
}

/// Télécharge, vérifie la signature, et remplace l'installation.
///
/// La vérification est faite par le plugin contre la clé publique de
/// `tauri.conf.json`. C'est elle qui empêche l'URL servie par l'endpoint d'être
/// une exécution de code arbitraire : un binaire qui n'est pas signé par la clé
/// privée correspondante est rejeté avant d'être écrit sur le disque.
///
/// La vérification est refaite ici au lieu de réutiliser l'offre précédente :
/// le plugin ne rend pas d'objet transportable entre deux appels, et redemander
/// garantit qu'on installe ce que l'endpoint propose maintenant — pas ce qu'il
/// proposait quand l'utilisateur a cliqué, ce qui compte si la version vient
/// d'être retirée.
#[tauri::command]
pub async fn install_update(
    updates: State<'_, Updates>,
    app: AppHandle,
) -> Result<CheckResult, UpdateError> {
    let updater = updater_for(&updates, &app).await?;
    let Some(update) = updater
        .check()
        .await
        .map_err(|error| UpdateError::Unreachable(error.to_string()))?
    else {
        return Ok(CheckResult::UpToDate);
    };

    let offer = offer_from(&update);
    update
        .download_and_install(|_downloaded, _total| {}, || {})
        .await
        .map_err(|error| UpdateError::Unreachable(error.to_string()))?;

    Ok(CheckResult::Available {
        offer: Box::new(offer),
    })
}
