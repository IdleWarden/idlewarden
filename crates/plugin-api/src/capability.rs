// SPDX-License-Identifier: Apache-2.0
//! What a plugin is allowed to do, and how much the user is asked to trust it.

use serde::{Deserialize, Serialize};

/// Declared in the manifest, shown to the user at install time, enforced by the
/// host. A plugin that asks for nothing can still observe and act through the
/// Core — capabilities gate *direct* access to the machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Capability {
    /// Read captured frames of the game window.
    Capture,
    /// Emit mouse commands.
    InputMouse,
    /// Emit keyboard commands.
    InputKeyboard,
    /// Emit virtual gamepad commands (requires a third-party driver).
    InputGamepad,
    /// Read files under a named, user-approved directory (e.g. a save folder).
    FsRead { path: String },
    /// Reach the network. Never granted to `Unverified` plugins by default.
    Net { host: String },
}

impl Capability {
    pub fn label(&self) -> String {
        match self {
            Capability::Capture => "capture".into(),
            Capability::InputMouse => "input.mouse".into(),
            Capability::InputKeyboard => "input.keyboard".into(),
            Capability::InputGamepad => "input.gamepad".into(),
            Capability::FsRead { path } => format!("fs.read:{path}"),
            Capability::Net { host } => format!("net:{host}"),
        }
    }
}

/// How the plugin reached the user's machine. Drives which capabilities are
/// granted without asking, and whether auto-update is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Installed from a local file or arbitrary URL. Loud warning, no
    /// auto-update, no capability granted without an explicit click.
    Unverified,
    /// Reviewed via a registry pull request, signed by a registered author key.
    Verified,
    /// Built and signed by the IdleWarden project itself.
    Official,
}

impl TrustLevel {
    pub fn allows_auto_update(self) -> bool {
        self >= TrustLevel::Verified
    }

    /// Capabilities granted without an explicit per-capability prompt.
    pub fn grants_silently(self, cap: &Capability) -> bool {
        match self {
            TrustLevel::Official => true,
            TrustLevel::Verified => !matches!(cap, Capability::Net { .. }),
            TrustLevel::Unverified => false,
        }
    }
}
