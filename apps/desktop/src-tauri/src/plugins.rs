// SPDX-License-Identifier: MPL-2.0
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use idlewarden_core::{PluginBundle, PluginId};
use idlewarden_mod_install::{install, Destination, Index};
use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::registry;
use crate::session::SessionHandle;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CatalogueEntry {
    pub id: String,
    pub name: String,
    pub game: String,
    pub description: Option<String>,
    pub available: Option<String>,
    pub installed: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub path: String,
}

#[tauri::command]
pub async fn catalogue(handle: State<'_, SessionHandle>) -> Result<Vec<CatalogueEntry>, String> {
    let index = tauri::async_runtime::spawn_blocking(registry::index)
        .await
        .map_err(|error| error.to_string())??;
    let installed = handle.installed_versions().into_iter().collect();
    Ok(entries(&index, &installed))
}

#[tauri::command]
pub async fn install_plugin(
    handle: State<'_, SessionHandle>,
    plugin: String,
) -> Result<InstalledPlugin, String> {
    let root = handle.plugin_root();
    let plugin = PluginId(plugin);
    let installed = tauri::async_runtime::spawn_blocking(move || fetch_and_install(&plugin, &root))
        .await
        .map_err(|error| error.to_string())??;
    handle.reload_plugins();
    Ok(installed)
}

fn entries(index: &Index, installed: &HashMap<PluginId, Version>) -> Vec<CatalogueEntry> {
    index
        .plugins
        .iter()
        .map(|entry| CatalogueEntry {
            id: entry.id.0.clone(),
            name: entry.name.clone(),
            game: entry.game.title.clone(),
            description: entry.description.clone(),
            available: index
                .plugin_release(&entry.id)
                .map(|release| release.version.version.to_string()),
            installed: installed.get(&entry.id).map(Version::to_string),
        })
        .collect()
}

fn fetch_and_install(plugin: &PluginId, root: &Path) -> Result<InstalledPlugin, String> {
    let index = registry::index()?;
    let release = index.plugin_release(plugin).ok_or_else(|| {
        format!(
            "the registry lists no release of {} this version of IdleWarden can run",
            plugin.0
        )
    })?;
    let archive = registry::download(&release.version.url, registry::LARGEST_ARCHIVE)?;
    let path = unpack(&archive, &release.version.sha256, plugin, root)?;

    tracing::info!(
        plugin = plugin.0.as_str(),
        version = %release.version.version,
        path = %path.display(),
        "plugin installed"
    );
    Ok(InstalledPlugin {
        name: release.entry.name.clone(),
        version: release.version.version.to_string(),
        path: path.display().to_string(),
    })
}

fn unpack(archive: &[u8], sha256: &str, plugin: &PluginId, root: &Path) -> Result<PathBuf, String> {
    let staging = root.with_file_name("plugin-staging").join(&plugin.0);
    install(
        archive,
        sha256,
        &plugin.0,
        &Destination::Own(staging.clone()),
    )
    .map_err(|error| error.to_string())?;

    let checked = match PluginBundle::load(&staging) {
        Ok(bundle) if bundle.id == *plugin => Ok(()),
        Ok(bundle) => Err(format!(
            "the archive holds {}, not {}",
            bundle.id.0, plugin.0
        )),
        Err(error) => Err(format!("the archive is not a plugin that loads: {error}")),
    };
    if let Err(refused) = checked {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(refused);
    }

    let target = folder_for(root, plugin);
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
    }
    std::fs::rename(&staging, &target).map_err(|error| error.to_string())?;
    Ok(target)
}

#[derive(Deserialize)]
struct Identity {
    id: PluginId,
}

fn folder_for(root: &Path, plugin: &PluginId) -> PathBuf {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .find(|folder| {
            std::fs::read_to_string(folder.join("plugin.json"))
                .ok()
                .and_then(|json| serde_json::from_str::<Identity>(&json).ok())
                .is_some_and(|identity| identity.id == *plugin)
        })
        .unwrap_or_else(|| root.join(&plugin.0))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use idlewarden_mod_install::sha256_hex;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    use super::*;

    const KALE: &str = "dev.idlewarden.master-healer-kale";

    fn manifest(id: &str, version: &str) -> String {
        format!(
            r#"{{
              "id": "{id}",
              "name": "Master Healer Kale",
              "version": "{version}",
              "api_version": "^0.1",
              "game": {{ "executable": "MasterHealerKale.exe" }}
            }}"#
        )
    }

    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, body) in files {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .expect("entry");
            writer.write_all(body.as_bytes()).expect("bytes");
        }
        writer.finish().expect("archive").into_inner()
    }

    fn plugin(id: &str, version: &str) -> Vec<u8> {
        archive(&[
            ("plugin.json", &manifest(id, version)),
            ("rules.json", r#"{ "intents": [] }"#),
        ])
    }

    fn data_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("idlewarden-plugins-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("plugins")).expect("plugins folder");
        dir
    }

    fn installed_version(folder: &Path) -> String {
        PluginBundle::load(folder)
            .expect("the installed plugin loads")
            .version
            .to_string()
    }

    fn folders(root: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(root)
            .expect("plugins folder")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_new_plugin_lands_in_a_folder_named_after_it_and_loads() {
        let root = data_dir("fresh").join("plugins");
        let bytes = plugin(KALE, "26.9.3");

        let path = unpack(
            &bytes,
            &sha256_hex(&bytes),
            &PluginId(KALE.to_owned()),
            &root,
        )
        .expect("installs");

        assert_eq!(path, root.join(KALE));
        assert_eq!(installed_version(&path), "26.9.3");
    }

    #[test]
    fn an_update_replaces_the_folder_already_holding_the_plugin_instead_of_adding_a_second() {
        let root = data_dir("update").join("plugins");
        let by_hand = root.join("master-healer-kale");
        std::fs::create_dir_all(&by_hand).expect("folder");
        std::fs::write(by_hand.join("plugin.json"), manifest(KALE, "26.9.2")).expect("manifest");
        std::fs::write(by_hand.join("rules.json"), r#"{ "intents": [] }"#).expect("rules");
        let bytes = plugin(KALE, "26.9.3");

        let path = unpack(
            &bytes,
            &sha256_hex(&bytes),
            &PluginId(KALE.to_owned()),
            &root,
        )
        .expect("updates");

        assert_eq!(path, by_hand);
        assert_eq!(installed_version(&by_hand), "26.9.3");
        assert_eq!(
            folders(&root),
            ["master-healer-kale"],
            "two folders with one id would load the plugin twice"
        );
    }

    #[test]
    fn an_archive_carrying_another_plugin_is_refused_and_nothing_changes() {
        let root = data_dir("impostor").join("plugins");
        let bytes = plugin("dev.someone.else", "1.0.0");

        let refused = unpack(
            &bytes,
            &sha256_hex(&bytes),
            &PluginId(KALE.to_owned()),
            &root,
        )
        .unwrap_err();

        assert!(refused.contains("dev.someone.else"), "{refused}");
        assert!(folders(&root).is_empty());
    }

    #[test]
    fn an_archive_that_does_not_load_leaves_the_installed_version_in_place() {
        let root = data_dir("broken").join("plugins");
        let installed = root.join(KALE);
        std::fs::create_dir_all(&installed).expect("folder");
        std::fs::write(installed.join("plugin.json"), manifest(KALE, "26.9.2")).expect("manifest");
        std::fs::write(installed.join("rules.json"), r#"{ "intents": [] }"#).expect("rules");
        let bytes = archive(&[("plugin.json", &manifest(KALE, "26.9.3"))]);

        let refused = unpack(
            &bytes,
            &sha256_hex(&bytes),
            &PluginId(KALE.to_owned()),
            &root,
        )
        .unwrap_err();

        assert!(refused.contains("not a plugin that loads"), "{refused}");
        assert_eq!(installed_version(&installed), "26.9.2");
    }

    #[test]
    fn a_tampered_archive_is_refused_before_anything_is_unpacked() {
        let root = data_dir("tampered").join("plugins");
        let bytes = plugin(KALE, "26.9.3");

        let refused =
            unpack(&bytes, &"0".repeat(64), &PluginId(KALE.to_owned()), &root).unwrap_err();

        assert!(
            refused.contains("does not match the registry checksum"),
            "{refused}"
        );
        assert!(folders(&root).is_empty());
    }

    #[test]
    fn the_catalogue_says_what_is_installed_and_what_could_be() {
        let index: Index = serde_json::from_value(serde_json::json!({
            "plugins": [
                {
                    "id": KALE,
                    "name": "Master Healer Kale",
                    "game": { "title": "Master Healer Kale" },
                    "versions": [{
                        "version": "26.9.3",
                        "api_version": "^0.1",
                        "url": "https://example.com/kale.zip",
                        "sha256": "0".repeat(64)
                    }]
                },
                {
                    "id": "dev.idlewarden.future",
                    "name": "Future",
                    "game": { "title": "Future" },
                    "versions": [{
                        "version": "1.0.0",
                        "api_version": "^9.0",
                        "url": "https://example.com/future.zip",
                        "sha256": "0".repeat(64)
                    }]
                }
            ]
        }))
        .expect("index");
        let installed = HashMap::from([(
            PluginId(KALE.to_owned()),
            Version::parse("26.9.2").expect("version"),
        )]);

        let listed = entries(&index, &installed);

        assert_eq!(listed[0].installed.as_deref(), Some("26.9.2"));
        assert_eq!(listed[0].available.as_deref(), Some("26.9.3"));
        assert_eq!(listed[1].installed, None);
        assert_eq!(
            listed[1].available, None,
            "a plugin built for a protocol this app does not speak cannot be offered"
        );
    }
}
