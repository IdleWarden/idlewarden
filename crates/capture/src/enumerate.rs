// SPDX-License-Identifier: MPL-2.0
use std::path::{Path, PathBuf};

use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

use crate::detect::GameWindow;
use crate::steam;
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

    let library = SteamLibrary::load();

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

fn describe(hwnd: HWND, library: &SteamLibrary) -> Option<GameWindow> {
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

struct SteamLibrary {
    games: Vec<(PathBuf, u32)>,
}

impl SteamLibrary {
    fn load() -> Self {
        let mut games = Vec::new();
        let Some(root) = steam_path() else {
            return SteamLibrary { games };
        };

        let mut libraries = vec![root.clone()];
        if let Ok(vdf) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            libraries.extend(steam::library_paths(&vdf));
        }

        for library in libraries {
            let apps = library.join("steamapps");
            let Ok(entries) = std::fs::read_dir(&apps) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "acf") {
                    if let Some((appid, installdir)) = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|acf| steam::app_manifest(&acf))
                    {
                        games.push((apps.join("common").join(installdir), appid));
                    }
                }
            }
        }

        SteamLibrary { games }
    }

    fn appid_of(&self, executable: &Path) -> Option<u32> {
        self.games
            .iter()
            .find(|(install, _)| steam::owns(install, executable))
            .map(|(_, appid)| *appid)
    }
}

fn steam_path() -> Option<PathBuf> {
    let mut buffer = vec![0u16; MAX_PATH as usize];
    let mut size = (buffer.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!("Software\\Valve\\Steam"),
            windows::core::w!("SteamPath"),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }

    let chars = (size as usize / 2).saturating_sub(1);
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..chars])))
}
