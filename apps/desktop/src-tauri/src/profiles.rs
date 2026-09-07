// SPDX-License-Identifier: MPL-2.0
//! Per-game Governor limits, kept on disk (#17).
//!
//! The limits themselves live in `idlewarden_core::GovernorConfig` and are
//! enforced there. Nothing in this file decides whether an action is allowed;
//! it stores what the user asked for and hands it to the Governor at the start
//! of a session.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use idlewarden_core::GovernorConfig;
use serde::{Deserialize, Serialize};

/// What the Profiles screen edits for one plugin.
///
/// `disabled_intents` is a deny list rather than the Governor's allow list
/// because the set of intents belongs to the plugin: a plugin that gains an
/// intent should have it enabled by default, not silently excluded because an
/// allow list written months earlier never heard of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub max_actions_per_minute: u32,
    pub min_confidence: f64,
    pub max_observation_age_ms: u64,
    pub max_session_minutes: u32,
    pub disabled_intents: Vec<String>,
}

impl Default for Profile {
    fn default() -> Self {
        let governor = GovernorConfig::default();
        Profile {
            max_actions_per_minute: governor.max_actions_per_minute,
            min_confidence: governor.min_confidence,
            max_observation_age_ms: governor.max_observation_age_ms,
            max_session_minutes: governor.max_session_minutes,
            disabled_intents: Vec::new(),
        }
    }
}

impl Profile {
    /// Bounds every field to a range where the limit still means something.
    ///
    /// A confidence floor of 0 or a rate ceiling of 0 does not relax a limit,
    /// it removes it, and a UI slip should not be able to do that quietly. The
    /// Governor keeps making the decisions; this only refuses to store a
    /// setting that would make them meaningless.
    pub fn clamped(mut self) -> Self {
        self.max_actions_per_minute = self.max_actions_per_minute.clamp(1, 600);
        self.min_confidence = if self.min_confidence.is_finite() {
            self.min_confidence.clamp(0.05, 1.0)
        } else {
            Profile::default().min_confidence
        };
        self.max_observation_age_ms = self.max_observation_age_ms.clamp(100, 60_000);
        self.max_session_minutes = self.max_session_minutes.clamp(1, 1_440);
        self.disabled_intents.sort();
        self.disabled_intents.dedup();
        self
    }

    /// The Governor's own configuration, with the deny list resolved against
    /// the intents this plugin actually declares.
    pub fn governor(&self, intents: &[String]) -> GovernorConfig {
        let allowed: Vec<String> = intents
            .iter()
            .filter(|name| self.is_enabled(name))
            .cloned()
            .collect();

        GovernorConfig {
            max_actions_per_minute: self.max_actions_per_minute,
            min_confidence: self.min_confidence,
            max_observation_age_ms: self.max_observation_age_ms,
            max_session_minutes: self.max_session_minutes,
            allowed_intents: Some(allowed),
        }
    }

    pub fn is_enabled(&self, intent: &str) -> bool {
        !self.disabled_intents.iter().any(|name| name == intent)
    }

    pub fn set_enabled(&mut self, intent: &str, enabled: bool) {
        if enabled {
            self.disabled_intents.retain(|name| name != intent);
        } else if self.is_enabled(intent) {
            self.disabled_intents.push(intent.to_owned());
            self.disabled_intents.sort();
        }
    }
}

/// Every plugin's profile, backed by one JSON file.
#[derive(Debug)]
pub struct Profiles {
    path: PathBuf,
    entries: BTreeMap<String, Profile>,
}

impl Profiles {
    /// A missing or unreadable file is not an error worth stopping for: it
    /// means "no profile has been saved yet", and defaults are a correct answer
    /// to that.
    pub fn load(path: &Path) -> Self {
        let entries = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        Profiles {
            path: path.to_owned(),
            entries,
        }
    }

    pub fn get(&self, plugin: &str) -> Profile {
        self.entries.get(plugin).cloned().unwrap_or_default()
    }

    pub fn set(&mut self, plugin: &str, profile: Profile) -> Profile {
        let stored = profile.clamped();
        self.entries.insert(plugin.to_owned(), stored.clone());
        self.save();
        stored
    }

    pub fn update(&mut self, plugin: &str, change: impl FnOnce(&mut Profile)) -> Profile {
        let mut profile = self.get(plugin);
        change(&mut profile);
        self.set(plugin, profile)
    }

    fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(serialised) = serde_json::to_string_pretty(&self.entries) {
            let _ = std::fs::write(&self.path, serialised);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("idlewarden-{name}-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn a_missing_file_reads_as_defaults_rather_than_failing() {
        let profiles = Profiles::load(&temp("absent"));

        assert_eq!(profiles.get("anything"), Profile::default());
    }

    #[test]
    fn a_corrupt_file_reads_as_defaults_rather_than_failing() {
        let path = temp("corrupt");
        std::fs::write(&path, "{ this is not json").expect("the fixture could be written");

        let profiles = Profiles::load(&path);

        assert_eq!(profiles.get("anything"), Profile::default());
    }

    #[test]
    fn a_saved_profile_survives_a_reload() {
        let path = temp("roundtrip");
        let mut profiles = Profiles::load(&path);

        profiles.set(
            "quest",
            Profile {
                max_actions_per_minute: 4,
                min_confidence: 0.9,
                disabled_intents: vec!["buy_upgrade".to_owned()],
                ..Profile::default()
            },
        );

        let reloaded = Profiles::load(&path);
        let profile = reloaded.get("quest");
        assert_eq!(profile.max_actions_per_minute, 4);
        assert_eq!(profile.min_confidence, 0.9);
        assert_eq!(profile.disabled_intents, vec!["buy_upgrade".to_owned()]);
    }

    #[test]
    fn a_zero_rate_ceiling_is_refused_because_it_removes_the_limit() {
        let profile = Profile {
            max_actions_per_minute: 0,
            min_confidence: 0.0,
            max_observation_age_ms: 0,
            max_session_minutes: 0,
            ..Profile::default()
        }
        .clamped();

        assert_eq!(profile.max_actions_per_minute, 1);
        assert_eq!(profile.min_confidence, 0.05);
        assert_eq!(profile.max_observation_age_ms, 100);
        assert_eq!(profile.max_session_minutes, 1);
    }

    #[test]
    fn an_absurd_rate_ceiling_is_capped() {
        let profile = Profile {
            max_actions_per_minute: 10_000,
            min_confidence: 4.0,
            max_observation_age_ms: 10_000_000,
            max_session_minutes: 100_000,
            ..Profile::default()
        }
        .clamped();

        assert_eq!(profile.max_actions_per_minute, 600);
        assert_eq!(profile.min_confidence, 1.0);
        assert_eq!(profile.max_observation_age_ms, 60_000);
        assert_eq!(profile.max_session_minutes, 1_440);
    }

    #[test]
    fn a_confidence_floor_that_is_not_a_number_falls_back_to_the_default() {
        let profile = Profile {
            min_confidence: f64::NAN,
            ..Profile::default()
        }
        .clamped();

        assert_eq!(profile.min_confidence, Profile::default().min_confidence);
    }

    #[test]
    fn the_deny_list_becomes_the_governors_allow_list() {
        let mut profile = Profile::default();
        profile.set_enabled("buy_upgrade", false);

        let config = profile.governor(&[
            "collect_reward".to_owned(),
            "buy_upgrade".to_owned(),
            "dismiss_popup".to_owned(),
        ]);

        assert_eq!(
            config.allowed_intents,
            Some(vec![
                "collect_reward".to_owned(),
                "dismiss_popup".to_owned()
            ])
        );
    }

    #[test]
    fn a_plugin_that_gains_an_intent_has_it_enabled() {
        let mut profile = Profile::default();
        profile.set_enabled("buy_upgrade", false);

        let config = profile.governor(&["buy_upgrade".to_owned(), "brand_new".to_owned()]);

        assert_eq!(
            config.allowed_intents,
            Some(vec!["brand_new".to_owned()]),
            "a deny list must not exclude intents it never heard of"
        );
    }

    #[test]
    fn disabling_every_intent_does_not_collapse_into_allowing_all() {
        let mut profile = Profile::default();
        profile.set_enabled("collect_reward", false);

        let config = profile.governor(&["collect_reward".to_owned()]);

        assert_eq!(
            config.allowed_intents,
            Some(Vec::new()),
            "switching every intent off must reach the Governor as `none allowed`,              not as `no restriction`"
        );
    }

    #[test]
    fn disabling_the_same_intent_twice_does_not_duplicate_it() {
        let mut profile = Profile::default();
        profile.set_enabled("buy_upgrade", false);
        profile.set_enabled("buy_upgrade", false);

        assert_eq!(profile.disabled_intents, vec!["buy_upgrade".to_owned()]);
    }

    #[test]
    fn re_enabling_an_intent_removes_it_from_the_deny_list() {
        let mut profile = Profile::default();
        profile.set_enabled("buy_upgrade", false);
        profile.set_enabled("buy_upgrade", true);

        assert!(profile.disabled_intents.is_empty());
        assert!(profile.is_enabled("buy_upgrade"));
    }
}
