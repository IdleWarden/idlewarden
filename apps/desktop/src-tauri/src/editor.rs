// SPDX-License-Identifier: MPL-2.0
use std::sync::{Arc, Mutex};

use idlewarden_capture::{Frame, WindowHandle};
use idlewarden_core::authoring::Draft;
use idlewarden_vision::png_from_bgra;
use tauri::ipc::Response;
use tauri::State;

use crate::session::SessionHandle;

#[derive(Default)]
pub struct Editor(Mutex<Option<Arc<Frame>>>);

impl Editor {
    fn keep(&self, frame: Arc<Frame>) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(frame);
    }

    fn kept(&self) -> Option<Arc<Frame>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

#[tauri::command]
pub fn capture_frame(editor: State<'_, Editor>, window: isize) -> Result<Response, String> {
    let frame = grab(WindowHandle(window))?;
    let png = png_from_bgra(frame.size.width, frame.size.height, &frame.bgra)
        .map_err(|error| error.to_string())?;
    editor.keep(frame);
    Ok(Response::new(png))
}

#[tauri::command]
pub fn save_plugin(
    editor: State<'_, Editor>,
    session: State<'_, SessionHandle>,
    draft: Draft,
) -> Result<String, String> {
    let frame = editor
        .kept()
        .ok_or_else(|| "capture a frame before saving a plugin".to_owned())?;
    session
        .author(&draft, &frame)
        .map(|written| written.display().to_string())
        .map_err(|error| error.to_string())
}

#[cfg(windows)]
fn grab(window: WindowHandle) -> Result<Arc<Frame>, String> {
    use idlewarden_capture::{CaptureBackend, WindowsCapture};

    WindowsCapture::new(window)
        .and_then(|mut capture| capture.next_frame())
        .map_err(|error| error.to_string())
}

#[cfg(not(windows))]
fn grab(_window: WindowHandle) -> Result<Arc<Frame>, String> {
    Err("capture is only implemented on Windows".to_owned())
}
