// SPDX-License-Identifier: MPL-2.0
use std::path::PathBuf;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

use crate::detect::GameWindow;
use crate::steam::Library;
use crate::WindowHandle;

/// Every top-level window the window manager admits to, on an X11 session.
/// Wayland has no such list, and asking the compositor for one is not a gap
/// waiting for an API (ADR-0019): there the portal's picker is the selection.
pub fn windows() -> Vec<GameWindow> {
    let Ok((connection, screen)) = x11rb::connect(None) else {
        tracing::debug!("no X11 display; window enumeration is unavailable");
        return Vec::new();
    };

    let root = connection.setup().roots[screen].root;
    let library = Library::cached();

    listed(&connection, root)
        .into_iter()
        .filter_map(|window| {
            describe(
                window,
                title(&connection, window),
                pid(&connection, window),
                &library,
                executable_of,
            )
        })
        .collect()
}

/// The directory the window's process runs from, for a mod installed next to
/// the game.
pub fn game_directory(window: WindowHandle) -> Option<PathBuf> {
    let (connection, _) = x11rb::connect(None).ok()?;
    let pid = pid(&connection, window.0 as Window)?;
    executable_of(pid)?.parent().map(ToOwned::to_owned)
}

fn describe(
    window: Window,
    title: Option<String>,
    pid: Option<u32>,
    library: &Library,
    executable_of: impl Fn(u32) -> Option<PathBuf>,
) -> Option<GameWindow> {
    let title = title.filter(|title| !title.is_empty())?;
    let path = executable_of(pid?)?;
    let executable = path.file_name()?.to_string_lossy().into_owned();

    Some(GameWindow {
        handle: WindowHandle(window as isize),
        title,
        executable,
        steam_appid: library.appid_of(&path),
    })
}

fn executable_of(pid: u32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/exe")).ok()
}

fn listed(connection: &RustConnection, root: Window) -> Vec<Window> {
    let Some(clients) = atom(connection, b"_NET_CLIENT_LIST") else {
        return Vec::new();
    };

    connection
        .get_property(false, root, clients, AtomEnum::WINDOW, 0, u32::MAX)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .and_then(|reply| reply.value32().map(|windows| windows.collect()))
        .unwrap_or_default()
}

fn title(connection: &RustConnection, window: Window) -> Option<String> {
    let utf8 = atom(connection, b"UTF8_STRING")?;
    atom(connection, b"_NET_WM_NAME")
        .and_then(|name| text(connection, window, name, utf8))
        .or_else(|| {
            text(
                connection,
                window,
                AtomEnum::WM_NAME.into(),
                AtomEnum::STRING.into(),
            )
        })
}

fn text(connection: &RustConnection, window: Window, property: u32, kind: u32) -> Option<String> {
    let reply = connection
        .get_property(false, window, property, kind, 0, u32::MAX)
        .ok()?
        .reply()
        .ok()?;

    (!reply.value.is_empty()).then(|| String::from_utf8_lossy(&reply.value).into_owned())
}

fn pid(connection: &RustConnection, window: Window) -> Option<u32> {
    let property = atom(connection, b"_NET_WM_PID")?;
    connection
        .get_property(false, window, property, AtomEnum::CARDINAL, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()
}

fn atom(connection: &RustConnection, name: &[u8]) -> Option<u32> {
    connection
        .intern_atom(true, name)
        .ok()?
        .reply()
        .ok()
        .map(|reply| reply.atom)
        .filter(|atom| *atom != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        Library::default()
    }

    #[test]
    fn a_window_is_described_by_its_title_and_the_binary_behind_it() {
        let described = describe(
            0x2a,
            Some("Cookie Clicker".to_owned()),
            Some(4242),
            &library(),
            |pid| (pid == 4242).then(|| PathBuf::from("/home/p/games/CookieClicker")),
        )
        .expect("a titled window with a process is a candidate");

        assert_eq!(described.handle, WindowHandle(0x2a));
        assert_eq!(described.title, "Cookie Clicker");
        assert_eq!(described.executable, "CookieClicker");
    }

    #[test]
    fn a_window_without_a_title_is_not_a_game() {
        for title in [None, Some(String::new())] {
            assert!(
                describe(1, title, Some(1), &library(), |_| Some(PathBuf::from(
                    "/usr/bin/x"
                )))
                .is_none(),
                "an untitled window is a panel, a dock or a hidden helper"
            );
        }
    }

    #[test]
    fn a_window_whose_process_cannot_be_read_is_skipped() {
        assert!(
            describe(1, Some("Game".to_owned()), None, &library(), |_| Some(
                PathBuf::from("/usr/bin/x")
            ))
            .is_none(),
            "without a pid there is nothing to match a plugin against"
        );
        assert!(describe(1, Some("Game".to_owned()), Some(9), &library(), |_| None).is_none());
    }
}
