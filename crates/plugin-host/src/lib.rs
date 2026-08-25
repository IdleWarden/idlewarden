// SPDX-License-Identifier: MPL-2.0
//! Plugin loading (ADR-0001, ADR-0010).
//!
//! **The root rule of this project: the Core never loads third-party native
//! code into its own process.** Rust has no stable ABI, a `cdylib` crash takes
//! the Core with it, and downloaded native code is arbitrary code execution.
//!
//! Three tiers instead, in order of preference:
//!
//! | Tier | Form | Crash blast radius |
//! |------|------|--------------------|
//! | 1 | Declarative: manifest + template assets + rules. No code. | none |
//! | 2 | Sandboxed script (Rhai): pure Rust interpreter, no FFI. | trapped |
//! | 3 | Out-of-process: child process over IPC, for official integrations. | its own process |

use idlewarden_plugin_api::{Capability, PluginError, PluginManifest, TrustLevel};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("no manifest at {0}")]
    ManifestMissing(PathBuf),
    #[error("manifest is not valid json: {0}")]
    ManifestUnreadable(String),
    #[error(transparent)]
    Contract(#[from] PluginError),
}

/// How a loaded plugin executes. Note there is deliberately no `NativeLibrary`
/// variant, and adding one would be a breaking architectural change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginKind {
    Declarative,
    Script,
    OutOfProcess,
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub kind: PluginKind,
    pub trust: TrustLevel,
    pub root: PathBuf,
    /// Capabilities actually granted, after trust level and user consent.
    pub granted: Vec<Capability>,
}

impl LoadedPlugin {
    pub fn is_granted(&self, cap: &Capability) -> bool {
        self.granted.contains(cap)
    }

    pub fn require(&self, cap: &Capability) -> Result<(), PluginError> {
        if self.is_granted(cap) {
            Ok(())
        } else {
            Err(PluginError::CapabilityDenied(cap.label()))
        }
    }
}

/// Reads and validates a plugin directory. Compatibility and structure are
/// reported as two different failures on purpose: "this plugin is broken" and
/// "this plugin is for a newer IdleWarden" need different messages to the user.
pub fn load_manifest(root: &Path) -> Result<PluginManifest, LoadError> {
    let path = root.join("plugin.json");
    let raw = std::fs::read_to_string(&path).map_err(|_| LoadError::ManifestMissing(path))?;
    let manifest: PluginManifest =
        serde_json::from_str(&raw).map_err(|e| LoadError::ManifestUnreadable(e.to_string()))?;

    manifest.validate()?;

    if !manifest.api_version.is_satisfied_by_host() {
        return Err(LoadError::Contract(PluginError::IncompatibleApi {
            found: manifest.api_version.0.to_string(),
            supported: idlewarden_plugin_api::API_VERSION.to_string(),
        }));
    }
    Ok(manifest)
}

/// Applies the trust policy: which requested capabilities are granted without
/// asking the user, and which must be confirmed one by one.
pub fn resolve_capabilities(
    manifest: &PluginManifest,
    trust: TrustLevel,
) -> (Vec<Capability>, Vec<Capability>) {
    let mut granted = Vec::new();
    let mut needs_consent = Vec::new();
    for cap in &manifest.capabilities {
        if trust.grants_silently(cap) {
            granted.push(cap.clone());
        } else {
            needs_consent.push(cap.clone());
        }
    }
    (granted, needs_consent)
}
