// SPDX-License-Identifier: Apache-2.0
//! The plugin manifest — the actual contract (ADR-0010).
//!
//! Stability is achievable here precisely *because* this is a data schema and
//! not a Rust ABI. A declarative plugin only ever breaks when this schema
//! changes, and the schema is versioned.

use crate::capability::Capability;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

/// The plugin API version this build of the host implements.
pub const API_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(pub String);

impl PluginId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reverse-DNS, lowercase, dot-separated. Keeps ids collision-free across
    /// authors without a central name authority.
    pub fn is_valid(&self) -> bool {
        let s = &self.0;
        !s.is_empty()
            && s.len() <= 128
            && s.split('.').count() >= 2
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SignalId(pub String);

impl SignalId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiVersion(pub VersionReq);

impl ApiVersion {
    /// Refuse loudly at load time rather than misbehaving at runtime.
    pub fn is_satisfied_by_host(&self) -> bool {
        Version::parse(API_VERSION)
            .map(|v| self.0.matches(&v))
            .unwrap_or(false)
    }
}

/// How to recognise that the game is running. Declarative on purpose: detection
/// is configuration, not code (ADR-0001).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GameMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_appid: Option<u32>,
    /// Executable file name, case-insensitive, e.g. "Game.exe".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    /// Substring or regex the window title must contain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
}

impl GameMatcher {
    pub fn is_empty(&self) -> bool {
        self.steam_appid.is_none() && self.executable.is_none() && self.window_title.is_none()
    }
}

/// One entry of the plugin's declared state schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalDecl {
    pub id: SignalId,
    /// Must match `Value::type_name()` of the values the plugin emits.
    pub value_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: String,
    pub version: Version,
    pub api_version: ApiVersion,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// SPDX identifier of the plugin's own licence — plugins are Apache-2.0
    /// downstream and may pick anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    pub game: GameMatcher,
    /// `true` when the target game has any multiplayer or competitive mode.
    /// The registry refuses such plugins; see PLUGIN_POLICY.md.
    #[serde(default)]
    pub multiplayer: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<SignalDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intents: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
}

impl PluginManifest {
    /// Structural validation. Compatibility is checked separately by the host
    /// so it can report the two failures differently.
    pub fn validate(&self) -> Result<(), crate::PluginError> {
        use crate::PluginError::InvalidManifest;
        if !self.id.is_valid() {
            return Err(InvalidManifest(format!(
                "id `{}` must be reverse-DNS, lowercase, e.g. `dev.bryan.mygame`",
                self.id.0
            )));
        }
        if self.name.trim().is_empty() {
            return Err(InvalidManifest("name must not be empty".into()));
        }
        if self.game.is_empty() {
            return Err(InvalidManifest(
                "game matcher must set at least one of steam_appid, executable, window_title"
                    .into(),
            ));
        }
        let mut ids: Vec<&SignalId> = self.signals.iter().map(|s| &s.id).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        if ids.len() != before {
            return Err(InvalidManifest("duplicate signal id in schema".into()));
        }
        Ok(())
    }
}
