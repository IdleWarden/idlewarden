// SPDX-License-Identifier: MPL-2.0
//! Every plugin this repository ships has to load, and its rules have to name
//! signals the plugin actually reports.
//!
//! A rule whose signal id is misspelled does not fail: the decider finds nothing
//! to read and abstains, so the plugin runs and simply never acts. That is the
//! worst shape a bug can take here, and only a check like this one catches it
//! before a release does.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use idlewarden_core::{PluginBundle, PluginRules};
use idlewarden_plugin_api::PluginManifest;

fn plugin_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

fn bundles() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(plugin_root())
        .expect("the plugins directory is part of the repository")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("rules.json").is_file())
        .collect();
    found.sort();
    found
}

#[test]
fn every_shipped_bundle_is_present_and_parses() {
    let bundles = bundles();
    assert!(
        bundles.len() >= 3,
        "expected the example, Cookie Clicker and Master Healer Kale, found {bundles:?}"
    );

    for bundle in bundles {
        let rules = PluginRules::load(&bundle.join("rules.json"))
            .unwrap_or_else(|error| panic!("{} does not parse: {error}", bundle.display()));

        for recipe in rules.recipes() {
            assert!(
                !recipe.post_condition.is_empty(),
                "`{}` in {} would never be confirmable",
                recipe.intent,
                bundle.display()
            );
        }
    }
}

/// One direction only. A rule proposing an intent the manifest never mentions is
/// a plugin lying about what it does, but a manifest offering an intent no starter
/// rule uses is an invitation: the user writes the rule.
#[test]
fn a_rule_never_proposes_an_intent_the_manifest_hides() {
    for bundle in bundles() {
        let manifest_path = bundle.join("plugin.json");
        if !manifest_path.is_file() {
            continue;
        }

        let manifest: PluginManifest = serde_json::from_str(
            &std::fs::read_to_string(&manifest_path).expect("readable manifest"),
        )
        .unwrap_or_else(|error| panic!("{} does not parse: {error}", manifest_path.display()));

        let rules = PluginRules::load(&bundle.join("rules.json")).expect("parsed above");
        let declared: BTreeSet<&str> = manifest.intents.iter().map(String::as_str).collect();

        for recipe in rules.recipes() {
            assert!(
                declared.contains(recipe.intent.as_str()),
                "{} proposes `{}` without declaring it, so the manifest understates the plugin",
                bundle.display(),
                recipe.intent
            );
        }
    }
}

#[test]
fn every_shipped_bundle_assembles_the_way_the_app_loads_it() {
    for bundle in bundles() {
        if !bundle.join("plugin.json").is_file() {
            continue;
        }
        PluginBundle::load(&bundle)
            .unwrap_or_else(|error| panic!("{} does not load whole: {error}", bundle.display()));
    }
}

#[test]
fn a_rule_never_names_a_signal_the_plugin_does_not_report() {
    for bundle in bundles() {
        let manifest_path = bundle.join("plugin.json");
        if !manifest_path.is_file() {
            continue;
        }

        let manifest: PluginManifest = serde_json::from_str(
            &std::fs::read_to_string(&manifest_path).expect("readable manifest"),
        )
        .expect("parsed above");

        let rules = PluginRules::load(&bundle.join("rules.json")).expect("parsed above");

        let mut known: BTreeSet<&str> = manifest
            .signals
            .iter()
            .map(|signal| signal.id.as_str())
            .collect();
        // A plugin that reads the screen declares its signals in `rules.json`
        // instead, next to the extractor that produces them.
        known.extend(rules.signals.iter().map(|signal| signal.id.as_str()));

        for intent in &rules.intents {
            for condition in intent.when.iter().chain(intent.post_condition.iter()) {
                assert!(
                    known.contains(condition.signal()),
                    "{} names `{}` in `{}`, which nothing reports",
                    bundle.display(),
                    condition.signal(),
                    intent.name
                );
            }
        }
    }
}
