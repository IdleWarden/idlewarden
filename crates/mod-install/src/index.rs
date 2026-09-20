// SPDX-License-Identifier: MPL-2.0
use idlewarden_plugin_api::{ApiVersion, PluginId};
use semver::Version;
use serde::Deserialize;

use crate::loader::Loader;

#[derive(Debug, Clone, Deserialize)]
pub struct Index {
    #[serde(default)]
    pub mods: Vec<ModEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub plugin: PluginId,
    pub bridge: Bridge,
    pub loader: Loader,
    #[serde(default)]
    pub mods_path: Option<String>,
    pub versions: Vec<ModVersion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Bridge {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModVersion {
    pub version: Version,
    pub api_version: ApiVersion,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub yanked: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Release<'a> {
    pub entry: &'a ModEntry,
    pub version: &'a ModVersion,
}

impl Release<'_> {
    pub fn destination(
        &self,
        game: &std::path::Path,
        reloaded_mods: Option<&std::path::Path>,
    ) -> Result<crate::loader::Destination, crate::loader::LoaderError> {
        self.entry.loader.destination(
            game,
            reloaded_mods,
            self.entry.mods_path.as_deref(),
            &self.entry.id,
        )
    }
}

impl Index {
    pub fn release_for(&self, plugin: &PluginId, bridge: &str) -> Option<Release<'_>> {
        self.mods
            .iter()
            .filter(|entry| &entry.plugin == plugin && entry.bridge.name == bridge)
            .flat_map(|entry| {
                entry
                    .versions
                    .iter()
                    .filter(|version| !version.yanked && version.api_version.is_satisfied_by_host())
                    .map(move |version| Release { entry, version })
            })
            .max_by(|a, b| a.version.version.cmp(&b.version.version))
    }
}
