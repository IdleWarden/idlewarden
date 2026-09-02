// SPDX-License-Identifier: MPL-2.0
//! Frame capture (ADR-0005).
//!
//! Three decisions are baked into this module and they matter:
//!
//! 1. We capture the **window**, not the screen, so moving it or changing
//!    monitor does not break every template.
//! 2. We target **2-4 fps**, not 60. Idle games do not move fast, and this
//!    simplification removes an entire class of pipeline complexity.
//! 3. Frames are `Arc<Frame>` and never cloned.

use std::sync::Arc;

mod frame;
mod null;

#[cfg(windows)]
mod wgc;

pub use frame::{Frame, Size};
pub use null::NullBackend;

#[cfg(windows)]
pub use wgc::WindowsCapture;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("no window matched the game")]
    WindowNotFound,
    #[error("the game window is in exclusive fullscreen; borderless is required")]
    ExclusiveFullscreen,
    #[error("capture backend failed: {0}")]
    Backend(String),
}

/// A handle to the game window, resolved by the detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowHandle(pub isize);

pub trait CaptureBackend: Send {
    /// Blocking; called from a dedicated thread, never from an async task.
    fn next_frame(&mut self) -> Result<Arc<Frame>, CaptureError>;
    fn window(&self) -> WindowHandle;
}
