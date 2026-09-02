// SPDX-License-Identifier: MPL-2.0
use std::sync::Arc;

use crate::{CaptureBackend, CaptureError, Frame, Size, WindowHandle};

/// Blank frames at a fixed size, so the pipeline can be exercised on any OS.
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
        Ok(Arc::new(Frame {
            id: self.next_id,
            captured_at_ms: self.next_id * 250,
            size: self.size,
            bgra: vec![0u8; self.size.bytes()],
        }))
    }

    fn window(&self) -> WindowHandle {
        WindowHandle(0)
    }
}
