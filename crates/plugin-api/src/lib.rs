// SPDX-License-Identifier: Apache-2.0
//! The IdleWarden plugin contract.
//!
//! This crate is the **only** stable surface a plugin depends on, and it is
//! licensed Apache-2.0 (the rest of IdleWarden is MPL-2.0) so that anyone can
//! build plugins under any license they like. See `docs/adr/0010-*.md`.
//!
//! The contract is **data**, not Rust symbols: a plugin is described by a
//! [`PluginManifest`] and speaks in [`Observation`]s and [`Intent`]s. The Core
//! never loads third-party native code into its own process (ADR-0001), so the
//! types here are all serialisable.

pub mod action;
pub mod capability;
pub mod manifest;
pub mod observation;
pub mod value;

pub use action::{ActionOutcome, InputCommand, Intent, Key, MouseButton, Point};
pub use capability::{Capability, TrustLevel};
pub use manifest::{
    ApiVersion, GameMatcher, PluginId, PluginManifest, SignalDecl, SignalId, API_VERSION,
};
pub use observation::{Confidence, Observation, Signal};
pub use value::Value;

/// Errors that cross the plugin boundary.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("plugin targets API version {found}, host supports {supported}")]
    IncompatibleApi { found: String, supported: String },
    #[error("manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("capability `{0}` was not granted to this plugin")]
    CapabilityDenied(String),
    #[error("observation failed: {0}")]
    Observe(String),
    #[error("action failed: {0}")]
    Act(String),
}
