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

pub mod event;
pub mod governor;
pub mod session;

pub use event::{Command, Event};
pub use governor::{Governor, GovernorConfig, Verdict};
pub use session::{Refusal, Session, SessionState};
