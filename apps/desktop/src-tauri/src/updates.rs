// SPDX-License-Identifier: MPL-2.0
//! Asking the update endpoint whether a newer build exists.
//!
//! This crate only *asks*. Downloading, verifying a signature and replacing the
//! binary belong to the updater plugin and to a signing key that does not exist
//! yet, so nothing here touches the installed application.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

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

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("cannot reach the update endpoint: {0}")]
    Unreachable(String),
    #[error("the update endpoint answered {0}")]
    Status(u16),
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

/// Turns the endpoint's answer into a result. 204 is the documented way of
/// saying "you are already current", so it is not an error.
pub fn interpret(status: u16, body: &str) -> Result<CheckResult, UpdateError> {
    match status {
        204 => Ok(CheckResult::UpToDate),
        200 => serde_json::from_str::<Offer>(body)
            .map(|offer| CheckResult::Available {
                offer: Box::new(offer),
            })
            .map_err(|error| UpdateError::Malformed(error.to_string())),
        other => Err(UpdateError::Status(other)),
    }
}

fn ask(url: &str, install: &str) -> Result<CheckResult, UpdateError> {
    let mut response = match ureq::get(url).header(INSTALL_HEADER, install).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(code)) => return Err(UpdateError::Status(code)),
        Err(error) => return Err(UpdateError::Unreachable(error.to_string())),
    };

    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| UpdateError::Malformed(error.to_string()))?;

    interpret(status, &body)
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

#[tauri::command]
pub async fn check_for_update(
    updates: State<'_, Updates>,
    app: AppHandle,
) -> Result<CheckResult, UpdateError> {
    let directory = updates.directory.clone();
    let current = app.package_info().version.to_string();

    tauri::async_runtime::spawn_blocking(move || {
        let settings = load_settings(&directory);
        let install = install_id(&directory)?;
        let url = check_url(ENDPOINT, target(), &current, settings.channel);
        ask(&url, &install)
    })
    .await
    .map_err(|error| UpdateError::Unreachable(error.to_string()))?
}
