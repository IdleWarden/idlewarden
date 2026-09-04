// SPDX-License-Identifier: MPL-2.0
//! The Core.
//!
//! Two rules define this crate:
//!
//! * It knows **nothing** about any specific game. Game knowledge lives in
//!   plugins, always.
//! * It contains **no** UI code. There is not a single `tauri::` symbol here
//!   (ADR-0004); the desktop app is an adapter over [`event::Event`] and
//!   `Command`. That discipline is what makes a future headless daemon a
//!   refactor rather than a rewrite.

pub mod detector;
pub mod event;
pub mod governor;
pub mod recipe;
pub mod rules;
pub mod runner;
pub mod service;
pub mod session;

pub use detector::{Detector, WindowSource};
pub use event::{Command, Event};
pub use governor::{Governor, GovernorConfig, Verdict};
pub use recipe::{Recipe, RecipeActuator};
pub use rules::{IntentRule, PluginRules, RulesError};
pub use runner::{Actuator, Parts, Runner};
pub use service::{SessionService, DEFAULT_TICK};
pub use session::{Refusal, Session, SessionState};
