// SPDX-License-Identifier: MPL-2.0
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use idlewarden_agent::Node;
use idlewarden_plugin_api::{Capability, GameMatcher, PluginId, PluginManifest};
use idlewarden_vision::{png_to_gray, Gray, Perceiver, RuleSet};

use crate::recipe::RecipeActuator;
use crate::rules::{PluginRules, RulesError};

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("plugin manifest could not be read: {0}")]
    Manifest(String),
    #[error(transparent)]
    Rules(#[from] RulesError),
    #[error("template `{name}` could not be loaded: {reason}")]
    Template { name: String, reason: String },
}

/// Everything one plugin contributes to a session: how to recognise the game,
/// how to see it, what to want, and how to act.
#[derive(Debug)]
pub struct PluginBundle {
    pub id: PluginId,
    pub matcher: GameMatcher,
    pub rules: PluginRules,
    pub bridge: Option<String>,
    templates: HashMap<String, Gray>,
}

impl PluginBundle {
    pub fn load(root: &Path) -> Result<Self, BundleError> {
        let manifest = read_manifest(root)?;
        let rules = PluginRules::load(&root.join("rules.json"))?;

        let mut templates = HashMap::new();
        for name in rules.templates() {
            let path = root.join(&name);
            let bytes = std::fs::read(&path).map_err(|error| BundleError::Template {
                name: name.clone(),
                reason: error.to_string(),
            })?;
            let gray = png_to_gray(&bytes).map_err(|error| BundleError::Template {
                name: name.clone(),
                reason: error.to_string(),
            })?;
            templates.insert(name, gray);
        }

        let bridge = manifest
            .capabilities
            .into_iter()
            .find_map(|capability| match capability {
                Capability::Bridge { name } => Some(name),
                _ => None,
            });

        Ok(PluginBundle {
            id: manifest.id,
            matcher: manifest.game,
            rules,
            bridge,
            templates,
        })
    }

    pub fn perceiver(&self) -> Box<dyn Perceiver> {
        Box::new(RuleSet::new(
            self.rules.anchors.clone(),
            self.rules.signals.clone(),
            self.templates.clone(),
        ))
    }

    pub fn tree(&self) -> Box<dyn Node> {
        self.rules.tree().build()
    }

    pub fn actuator(&self) -> RecipeActuator {
        RecipeActuator::new(self.rules.recipes())
    }
}

/// Every plugin directly under `root`, skipping anything that does not load.
pub fn load_all(root: &Path) -> Vec<(PathBuf, Result<PluginBundle, BundleError>)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("plugin.json").is_file())
        .collect();
    found.sort();

    found
        .into_iter()
        .map(|path| {
            let bundle = PluginBundle::load(&path);
            (path, bundle)
        })
        .collect()
}

fn read_manifest(root: &Path) -> Result<PluginManifest, BundleError> {
    let json = std::fs::read_to_string(root.join("plugin.json"))
        .map_err(|error| BundleError::Manifest(error.to_string()))?;
    serde_json::from_str(&json).map_err(|error| BundleError::Manifest(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::Actuator;
    use idlewarden_plugin_api::Intent;

    fn examples() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
    }

    #[test]
    fn the_reference_plugin_assembles_into_every_part_of_a_session() {
        let bundle = PluginBundle::load(&examples().join("example-game"))
            .expect("the shipped plugin must load whole");

        assert_eq!(bundle.id.0, "dev.idlewarden.example-game");
        assert_eq!(
            bundle.matcher.executable.as_deref(),
            Some("ExampleGame.exe")
        );
        assert_eq!(bundle.templates.len(), 2);

        let _ = bundle.perceiver();
        let _ = bundle.tree();
        assert_eq!(
            bundle.actuator().plan(&Intent::new("collect_reward")).len(),
            2,
            "the actuator must know how to carry out a declared intent"
        );
    }

    #[test]
    fn the_shipped_bridged_plugin_declares_its_mod_and_verifies_every_intent() {
        let bundle = PluginBundle::load(&examples().join("cookie-clicker"))
            .expect("the shipped plugin must load whole");

        assert_eq!(bundle.bridge.as_deref(), Some("cookie-clicker"));
        assert_eq!(
            bundle
                .rules
                .intents
                .iter()
                .map(|intent| intent.name.as_str())
                .collect::<Vec<_>>(),
            ["pop_golden_cookie", "buy_upgrade", "buy_best_building"]
        );
        assert!(
            bundle
                .rules
                .intents
                .iter()
                .all(|intent| intent.commands.is_empty()),
            "a bridged plugin acts through the mod, so it declares no clicks"
        );
    }

    #[test]
    fn a_plugin_that_captures_the_screen_declares_no_bridge() {
        let bundle = PluginBundle::load(&examples().join("example-game")).expect("loads");

        assert_eq!(bundle.bridge, None);
    }

    #[test]
    fn a_bridge_capability_in_the_manifest_names_the_endpoint() {
        let root = std::env::temp_dir().join(format!("idlewarden-bridged-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir");
        let manifest = std::fs::read_to_string(examples().join("example-game/plugin.json"))
            .expect("manifest")
            .replace(r#""input.mouse""#, r#""input.mouse", "bridge:reference""#);
        std::fs::write(root.join("plugin.json"), manifest).expect("manifest");
        std::fs::write(root.join("rules.json"), "{}").expect("rules");

        let bundle = PluginBundle::load(&root);
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(bundle.expect("loads").bridge.as_deref(), Some("reference"));
    }

    #[test]
    fn a_directory_with_no_manifest_is_not_a_plugin() {
        let error = PluginBundle::load(&examples().join("does-not-exist")).expect_err("refused");

        assert!(matches!(error, BundleError::Manifest(_)));
    }

    #[test]
    fn a_template_that_is_not_on_disk_names_the_asset() {
        let root = std::env::temp_dir().join("idlewarden-bundle-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir");
        std::fs::copy(
            examples().join("example-game/plugin.json"),
            root.join("plugin.json"),
        )
        .expect("manifest");
        std::fs::write(
            root.join("rules.json"),
            r#"{"anchors":[{"name":"a","search_area":{"x":0.0,"y":0.0,"w":0.2,"h":0.2},"template":"assets/absent.png","min_score":0.8}]}"#,
        )
        .expect("rules");

        let error = PluginBundle::load(&root).expect_err("refused");
        let _ = std::fs::remove_dir_all(&root);

        assert!(
            matches!(error, BundleError::Template { name, .. } if name == "assets/absent.png"),
            "a plugin author needs to be told which asset is missing"
        );
    }

    #[test]
    fn scanning_the_plugin_directory_finds_the_reference_plugin() {
        let found = load_all(&examples());

        assert!(
            found
                .iter()
                .any(|(path, bundle)| path.ends_with("example-game") && bundle.is_ok()),
            "the example must be discoverable the same way a user's plugins are"
        );
    }

    #[test]
    fn a_directory_that_does_not_exist_yields_no_plugins_rather_than_panicking() {
        assert!(load_all(Path::new("nowhere-at-all")).is_empty());
    }
}
