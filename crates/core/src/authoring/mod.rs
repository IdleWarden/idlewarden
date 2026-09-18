// SPDX-License-Identifier: MPL-2.0
mod intents;
mod regions;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use idlewarden_capture::Frame;
use idlewarden_plugin_api::{
    ApiVersion, Capability, GameMatcher, PluginId, PluginManifest, SignalDecl, SignalId, Value,
    API_VERSION,
};
use idlewarden_vision::{png_from_bgra, Roi, SignalRule, VisionError};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

pub use intents::IntentDraft;

use crate::bundle::PluginBundle;
use crate::rules::PluginRules;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionKind {
    Anchor,
    ColorProbe,
    TemplateMatch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub name: String,
    pub kind: RegionKind,
    pub area: Roi,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub id: PluginId,
    pub name: String,
    pub game: GameMatcher,
    pub regions: Vec<Region>,
    #[serde(default)]
    pub intents: Vec<IntentDraft>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthoringError {
    #[error("`{0}` is not a valid plugin id: use lowercase reverse-DNS such as `local.my-game`")]
    BadId(String),
    #[error("the plugin needs a name")]
    Unnamed,
    #[error("the game matcher is empty, so the plugin would match no window")]
    NoMatcher,
    #[error("region names must be unique, lowercase and use only a-z, 0-9, `.`, `_` or `-`: `{0}` is not")]
    BadRegionName(String),
    #[error("region `{0}` falls outside the frame")]
    OutOfFrame(String),
    #[error("a plugin with id `{0}` already exists")]
    Exists(String),
    #[error("the plugin could not be written: {0}")]
    Io(String),
    #[error("the written plugin does not load, so it was removed: {0}")]
    Invalid(String),
    #[error("`{0}` does not recognise the frame it was drawn on: {1}")]
    Unrecognised(String, String),
    #[error("intent names must be unique, lowercase and use only a-z, 0-9, `.`, `_` or `-`: `{0}` is not")]
    BadIntentName(String),
    #[error("intent `{0}` has no condition, so it would fire on every tick")]
    NoTrigger(String),
    #[error("intent `{0}` has no post-condition, so nothing could confirm it worked")]
    NoPostCondition(String),
    #[error("intent `{0}` tests `{1}` with a condition the editor cannot draw; drawn signals are true or false")]
    UnsupportedCondition(String, String),
    #[error("intent `{0}` tests `{1}`, which is not a signal drawn in this plugin")]
    UnknownSignal(String, String),
    #[error("intent `{0}` expects afterwards only what was already true before, so it cannot tell a click that worked from one that did nothing")]
    UnprovablePostCondition(String),
    #[error("intent `{0}` clicks outside the window")]
    ClickOutside(String),
    #[error(transparent)]
    Vision(#[from] VisionError),
}

pub fn write(draft: &Draft, frame: &Frame, plugins_root: &Path) -> Result<PathBuf, AuthoringError> {
    validate(draft)?;

    let dir = plugins_root.join(&draft.id.0);
    if dir.exists() {
        return Err(AuthoringError::Exists(draft.id.0.clone()));
    }

    let built = regions::build(&draft.regions, frame)?;

    let result = persist(draft, &built, &dir).and_then(|()| recognises(&dir, frame));

    match result {
        Ok(()) => Ok(dir),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(error)
        }
    }
}

fn recognises(dir: &Path, frame: &Frame) -> Result<(), AuthoringError> {
    let bundle =
        PluginBundle::load(dir).map_err(|error| AuthoringError::Invalid(error.to_string()))?;

    let extracted = bundle
        .perceiver()
        .perceive(frame)
        .map_err(|error| match error {
            VisionError::AnchorLost(name) => {
                AuthoringError::Unrecognised(name, "the anchor was not found".to_owned())
            }
            other => AuthoringError::Invalid(other.to_string()),
        })?;

    for signal in extracted {
        if signal.value != Value::Bool(true) {
            return Err(AuthoringError::Unrecognised(
                signal.id.0,
                format!(
                    "it reads false at confidence {:.2}",
                    signal.confidence.get()
                ),
            ));
        }
    }
    Ok(())
}

fn validate(draft: &Draft) -> Result<(), AuthoringError> {
    if !draft.id.is_valid() {
        return Err(AuthoringError::BadId(draft.id.0.clone()));
    }
    if draft.name.trim().is_empty() {
        return Err(AuthoringError::Unnamed);
    }
    if draft.game.is_empty() {
        return Err(AuthoringError::NoMatcher);
    }

    let mut seen = HashSet::new();
    for region in &draft.regions {
        if !well_formed(&region.name) || !seen.insert(region.name.as_str()) {
            return Err(AuthoringError::BadRegionName(region.name.clone()));
        }
        if !region.area.is_within_unit_square() {
            return Err(AuthoringError::OutOfFrame(region.name.clone()));
        }
    }

    intents::validate(&draft.intents, &draft.regions)
}

fn well_formed(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

fn manifest(draft: &Draft, signals: &[SignalRule]) -> PluginManifest {
    PluginManifest {
        id: draft.id.clone(),
        name: draft.name.trim().to_owned(),
        version: Version::new(0, 1, 0),
        api_version: ApiVersion(
            VersionReq::parse(&format!("^{API_VERSION}"))
                .expect("the host version is valid semver"),
        ),
        authors: Vec::new(),
        description: Some("Drawn in the IdleWarden region editor.".to_owned()),
        license: None,
        game: draft.game.clone(),
        multiplayer: false,
        signals: signals
            .iter()
            .map(|rule| SignalDecl {
                id: SignalId(rule.id.0.clone()),
                value_type: "bool".to_owned(),
                description: None,
                unit: None,
            })
            .collect(),
        intents: draft
            .intents
            .iter()
            .map(|intent| intent.name.clone())
            .collect(),
        capabilities: if draft.intents.is_empty() {
            vec![Capability::Capture]
        } else {
            vec![Capability::Capture, Capability::InputMouse]
        },
    }
}

fn persist(draft: &Draft, built: &regions::Built, dir: &Path) -> Result<(), AuthoringError> {
    let io = |error: std::io::Error| AuthoringError::Io(error.to_string());

    std::fs::create_dir_all(dir.join("assets")).map_err(io)?;
    for asset in &built.assets {
        let png = png_from_bgra(asset.width, asset.height, &asset.bgra)?;
        std::fs::write(dir.join(&asset.path), png).map_err(io)?;
    }

    let rules = PluginRules {
        anchors: built.anchors.clone(),
        signals: built.signals.clone(),
        intents: intents::rules(&draft.intents),
    };
    std::fs::write(dir.join("rules.json"), pretty(&rules)).map_err(io)?;
    std::fs::write(
        dir.join("plugin.json"),
        pretty(&manifest(draft, &built.signals)),
    )
    .map_err(io)?;
    Ok(())
}

fn pretty<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("plugin files always serialise")
}
