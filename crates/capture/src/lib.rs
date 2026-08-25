// SPDX-License-Identifier: MPL-2.0
//! Frame capture (ADR-0005).
//!
//! Three decisions are baked into this module and they matter:
//!
//! 1. We capture the **window**, not the screen, so moving it or changing
//!    monitor does not break every template.
//! 2. We target **2–4 fps**, not 60. Idle games do not move fast, and this
//!    simplification removes an entire class of pipeline complexity.
//! 3. Frames are `Arc<Frame>` and never cloned.

use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("no window matched the game")]
    WindowNotFound,
    #[error("the game window is in exclusive fullscreen; borderless is required")]
    ExclusiveFullscreen,
    #[error("capture backend failed: {0}")]
    Backend(String),
}

/// Size of the window's client area, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// One captured frame, BGRA8, tightly packed rows.
pub struct Frame {
    pub id: u64,
    pub captured_at_ms: u64,
    pub size: Size,
    pub bgra: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("id", &self.id)
            .field("size", &self.size)
            .field("bytes", &self.bgra.len())
            .finish()
    }
}

/// A handle to the game window, resolved by the detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowHandle(pub isize);

pub trait CaptureBackend: Send {
    /// Blocking; called from a dedicated thread, never from an async task.
    fn next_frame(&mut self) -> Result<Arc<Frame>, CaptureError>;
    fn window(&self) -> WindowHandle;
}

/// Placeholder backend so the workspace builds and the pipeline can be
/// exercised on any OS. Phase 1 replaces this with the real per-platform
/// backends.
pub struct NullBackend {
    next_id: u64,
    size: Size,
}

impl NullBackend {
    pub fn new(size: Size) -> Self {
        NullBackend { next_id: 0, size }
    }
}

impl CaptureBackend for NullBackend {
    fn next_frame(&mut self) -> Result<Arc<Frame>, CaptureError> {
        self.next_id += 1;
        let px = (self.size.width as usize) * (self.size.height as usize) * 4;
        Ok(Arc::new(Frame {
            id: self.next_id,
            captured_at_ms: self.next_id * 250,
            size: self.size,
            bgra: vec![0u8; px],
        }))
    }

    fn window(&self) -> WindowHandle {
        WindowHandle(0)
    }
}
