// SPDX-License-Identifier: MPL-2.0
use std::path::{Path, PathBuf};

use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

use crate::detect::GameWindow;
use crate::steam::Library;
use crate::WindowHandle;

/// Every visible top-level window that belongs to a process we can name.
pub fn windows() -> Vec<GameWindow> {
    let mut handles: Vec<HWND> = Vec::new();
    let _ = unsafe {
        EnumWindows(
            Some(collect),
            LPARAM(&mut handles as *mut Vec<HWND> as isize),
        )
    };

    let library = Library::cached();

    handles
        .into_iter()
        .filter_map(|hwnd| describe(hwnd, &library))
        .collect()
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        let handles = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
        handles.push(hwnd);
    }
    true.into()
}

fn describe(hwnd: HWND, library: &Library) -> Option<GameWindow> {
    let title = title(hwnd)?;
    if title.is_empty() {
        return None;
    }

    let path = executable_path(hwnd)?;
    let executable = path.file_name()?.to_string_lossy().into_owned();

    Some(GameWindow {
        handle: WindowHandle(hwnd.0 as isize),
        title,
        executable,
        steam_appid: library.appid_of(&path),
    })
}

fn title(hwnd: HWND) -> Option<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return None;
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let written = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if written <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..written as usize]))
}

pub fn game_directory(window: WindowHandle) -> Option<PathBuf> {
    executable_path(HWND(window.0 as *mut _))?
        .parent()
        .map(Path::to_path_buf)
}

fn executable_path(hwnd: HWND) -> Option<PathBuf> {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    let mut buffer = vec![0u16; MAX_PATH as usize];
    let mut length = buffer.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    let _ = unsafe { CloseHandle(process) };

    queried.ok()?;
    Some(PathBuf::from(String::from_utf16_lossy(
        &buffer[..length as usize],
    )))
}
