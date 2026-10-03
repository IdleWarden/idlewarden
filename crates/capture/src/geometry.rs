// SPDX-License-Identifier: MPL-2.0
use crate::WindowHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

#[cfg(windows)]
pub fn client_bounds(handle: WindowHandle) -> Option<ScreenRect> {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, IsIconic, IsWindow};

    let hwnd = HWND(handle.0 as *mut std::ffi::c_void);
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() || unsafe { IsIconic(hwnd) }.as_bool() {
        return None;
    }
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect) }.ok()?;
    let mut origin = POINT { x: 0, y: 0 };
    if !unsafe { ClientToScreen(hwnd, &mut origin) }.as_bool() {
        return None;
    }
    Some(ScreenRect {
        left: origin.x,
        top: origin.y,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    })
}

#[cfg(windows)]
pub fn foreground() -> Option<WindowHandle> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let hwnd = unsafe { GetForegroundWindow() };
    (!hwnd.is_invalid()).then_some(WindowHandle(hwnd.0 as isize))
}

#[cfg(not(windows))]
pub fn client_bounds(_handle: WindowHandle) -> Option<ScreenRect> {
    None
}

#[cfg(not(windows))]
pub fn foreground() -> Option<WindowHandle> {
    None
}
