// SPDX-License-Identifier: MPL-2.0
use std::thread::sleep;
use std::time::Duration;

use idlewarden_plugin_api::{InputCommand, MouseButton, Point};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetForegroundWindow, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

use crate::coords::{to_absolute, to_screen, Rect};
use crate::humanise::Jitter;
use crate::keys::virtual_key;
use crate::{Humanisation, InputBackend, InputError, KillSwitch};

/// Real mouse and keyboard through `SendInput`, against a focused window.
pub struct SendInputBackend {
    window: isize,
    kill: KillSwitch,
    humanisation: Humanisation,
    jitter: Jitter,
}

impl SendInputBackend {
    pub fn new(window: isize, kill: KillSwitch, humanisation: Humanisation) -> Self {
        SendInputBackend {
            window,
            kill,
            humanisation,
            jitter: Jitter::new(window as u64 ^ 0xA5A5_5A5A),
        }
    }

    fn checkpoint(&self) -> Result<(), InputError> {
        if self.kill.is_engaged() {
            return Err(InputError::KillSwitchEngaged);
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<(), InputError> {
        self.checkpoint()?;
        sleep(Duration::from_millis(
            self.jitter.delay_ms(self.humanisation),
        ));
        self.checkpoint()
    }

    fn hwnd(&self) -> HWND {
        HWND(self.window as *mut std::ffi::c_void)
    }

    fn require_focus(&self) -> Result<(), InputError> {
        if unsafe { GetForegroundWindow() } == self.hwnd() {
            Ok(())
        } else {
            Err(InputError::NotFocused)
        }
    }

    fn client(&self) -> Result<Rect, InputError> {
        let mut rect = RECT::default();
        unsafe { GetClientRect(self.hwnd(), &mut rect) }
            .map_err(|error| InputError::Backend(error.message()))?;

        let mut origin = POINT { x: 0, y: 0 };
        if !unsafe { ClientToScreen(self.hwnd(), &mut origin) }.as_bool() {
            return Err(InputError::Backend(
                "client area has no screen origin".into(),
            ));
        }

        Ok(Rect {
            left: origin.x,
            top: origin.y,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        })
    }

    fn desktop() -> Rect {
        unsafe {
            Rect {
                left: GetSystemMetrics(SM_XVIRTUALSCREEN),
                top: GetSystemMetrics(SM_YVIRTUALSCREEN),
                width: GetSystemMetrics(SM_CXVIRTUALSCREEN),
                height: GetSystemMetrics(SM_CYVIRTUALSCREEN),
            }
        }
    }

    fn move_to(&mut self, point: Point) -> Result<(), InputError> {
        let (x, y) = to_screen(point, self.client()?)
            .ok_or_else(|| InputError::Backend("the game window has no client area".into()))?;
        let (dx, dy) = to_absolute(x, y, Self::desktop())
            .ok_or_else(|| InputError::Backend("the virtual desktop has no extent".into()))?;

        self.mouse(
            dx,
            dy,
            0,
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        )
    }

    fn mouse(
        &self,
        dx: i32,
        dy: i32,
        data: i32,
        flags: MOUSE_EVENT_FLAGS,
    ) -> Result<(), InputError> {
        self.checkpoint()?;
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: data as u32,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        send(&input)
    }

    fn key(&self, code: u16, flags: KEYBD_EVENT_FLAGS) -> Result<(), InputError> {
        self.checkpoint()?;
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(code),
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        send(&input)
    }

    fn press(&mut self, name: &str, down: bool, up: bool) -> Result<(), InputError> {
        let code = virtual_key(name)
            .ok_or_else(|| InputError::Backend(format!("unknown key `{name}`")))?;
        if down {
            self.key(code, KEYBD_EVENT_FLAGS(0))?;
        }
        if down && up {
            self.pause()?;
        }
        if up {
            self.key(code, KEYEVENTF_KEYUP)?;
        }
        Ok(())
    }
}

impl InputBackend for SendInputBackend {
    fn execute(&mut self, cmd: &InputCommand) -> Result<(), InputError> {
        self.checkpoint()?;
        self.require_focus()?;

        match cmd {
            InputCommand::MoveTo { to } => self.move_to(*to),
            InputCommand::Click { at, button } => {
                self.move_to(*at)?;
                self.pause()?;
                let (down, up) = match button {
                    MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
                    MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
                    MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
                };
                self.mouse(0, 0, 0, down)?;
                self.pause()?;
                self.mouse(0, 0, 0, up)
            }
            InputCommand::KeyPress { key } => self.press(&key.0, true, true),
            InputCommand::KeyDown { key } => self.press(&key.0, true, false),
            InputCommand::KeyUp { key } => self.press(&key.0, false, true),
            InputCommand::Scroll { at, delta } => {
                self.move_to(*at)?;
                self.pause()?;
                self.mouse(0, 0, delta * 120, MOUSEEVENTF_WHEEL)
            }
            InputCommand::Wait { ms } => {
                sleep(Duration::from_millis(*ms));
                self.checkpoint()
            }
        }
    }
}

fn send(input: &INPUT) -> Result<(), InputError> {
    let sent = unsafe {
        SendInput(
            std::slice::from_ref(input),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent == 1 {
        Ok(())
    } else {
        Err(InputError::Backend(
            "SendInput was blocked, most likely by UIPI".into(),
        ))
    }
}
