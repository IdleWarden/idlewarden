// SPDX-License-Identifier: MPL-2.0
use idlewarden_plugin_api::{InputCommand, MouseButton, Point};

use crate::coords::{to_absolute, to_screen, Rect};
use crate::keys::evdev_key;
use crate::InputError;

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_REL: u16 = 0x02;
pub const EV_ABS: u16 = 0x03;

pub const SYN_REPORT: u16 = 0x00;
pub const REL_WHEEL: u16 = 0x08;
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const BTN_LEFT: u16 = 0x110;
pub const BTN_RIGHT: u16 = 0x111;
pub const BTN_MIDDLE: u16 = 0x112;

/// The range the virtual device declares for its absolute axes, matching what
/// `to_absolute` already produces for Windows.
pub const ABS_MAX: i32 = 65535;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

impl Event {
    pub fn new(kind: u16, code: u16, value: i32) -> Self {
        Event { kind, code, value }
    }

    pub fn report() -> Self {
        Event::new(EV_SYN, SYN_REPORT, 0)
    }
}

pub fn button(button: MouseButton) -> u16 {
    match button {
        MouseButton::Left => BTN_LEFT,
        MouseButton::Right => BTN_RIGHT,
        MouseButton::Middle => BTN_MIDDLE,
    }
}

/// Window-relative to the absolute axis units the virtual device is set up
/// with. A pointer device reports over the whole screen, so the window rect
/// only decides where inside it the point lands.
pub fn absolute(point: Point, window: Rect, screen: Rect) -> Option<(i32, i32)> {
    let (x, y) = to_screen(point, window)?;
    to_absolute(x, y, screen)
}

/// One command as the event stream a uinput device expects, terminated by the
/// report that makes the batch visible to the compositor.
pub fn encode(
    command: &InputCommand,
    window: Rect,
    screen: Rect,
) -> Result<Vec<Event>, InputError> {
    let mut events = Vec::new();

    match command {
        InputCommand::Wait { .. } => {}
        InputCommand::MoveTo { to } => {
            events.extend(move_to(*to, window, screen)?);
            events.push(Event::report());
        }
        InputCommand::Click { at, button: which } => {
            events.extend(move_to(*at, window, screen)?);
            events.push(Event::report());
            events.push(Event::new(EV_KEY, button(*which), 1));
            events.push(Event::report());
            events.push(Event::new(EV_KEY, button(*which), 0));
            events.push(Event::report());
        }
        InputCommand::Scroll { at, delta } => {
            events.extend(move_to(*at, window, screen)?);
            events.push(Event::report());
            events.push(Event::new(EV_REL, REL_WHEEL, *delta));
            events.push(Event::report());
        }
        InputCommand::KeyPress { key } => {
            let code = key_code(&key.0)?;
            events.push(Event::new(EV_KEY, code, 1));
            events.push(Event::report());
            events.push(Event::new(EV_KEY, code, 0));
            events.push(Event::report());
        }
        InputCommand::KeyDown { key } => {
            events.push(Event::new(EV_KEY, key_code(&key.0)?, 1));
            events.push(Event::report());
        }
        InputCommand::KeyUp { key } => {
            events.push(Event::new(EV_KEY, key_code(&key.0)?, 0));
            events.push(Event::report());
        }
    }

    Ok(events)
}

fn move_to(point: Point, window: Rect, screen: Rect) -> Result<[Event; 2], InputError> {
    let (x, y) = absolute(point, window, screen)
        .ok_or_else(|| InputError::Backend("the window or the screen has no extent".into()))?;
    Ok([Event::new(EV_ABS, ABS_X, x), Event::new(EV_ABS, ABS_Y, y)])
}

fn key_code(name: &str) -> Result<u16, InputError> {
    evdev_key(name).ok_or_else(|| InputError::Backend(format!("unknown key `{name}`")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use idlewarden_plugin_api::Key;

    const WINDOW: Rect = Rect {
        left: 100,
        top: 50,
        width: 801,
        height: 601,
    };

    const SCREEN: Rect = Rect {
        left: 0,
        top: 0,
        width: 1921,
        height: 1081,
    };

    fn codes(events: &[Event]) -> Vec<(u16, u16, i32)> {
        events
            .iter()
            .map(|event| (event.kind, event.code, event.value))
            .collect()
    }

    #[test]
    fn every_batch_ends_with_a_report_or_nothing_reaches_the_compositor() {
        for command in [
            InputCommand::MoveTo {
                to: Point { x: 0.5, y: 0.5 },
            },
            InputCommand::Click {
                at: Point { x: 0.5, y: 0.5 },
                button: MouseButton::Left,
            },
            InputCommand::KeyPress {
                key: Key("a".to_owned()),
            },
            InputCommand::Scroll {
                at: Point { x: 0.5, y: 0.5 },
                delta: -2,
            },
        ] {
            let events = encode(&command, WINDOW, SCREEN).expect("encodes");
            assert_eq!(
                events.last().copied(),
                Some(Event::report()),
                "{command:?} would sit in the kernel's buffer"
            );
        }
    }

    #[test]
    fn a_click_moves_first_then_presses_and_releases() {
        let events = encode(
            &InputCommand::Click {
                at: Point { x: 0.0, y: 0.0 },
                button: MouseButton::Right,
            },
            WINDOW,
            SCREEN,
        )
        .expect("encodes");

        assert_eq!(
            codes(&events),
            [
                (EV_ABS, ABS_X, 3413),
                (EV_ABS, ABS_Y, 3034),
                (EV_SYN, SYN_REPORT, 0),
                (EV_KEY, BTN_RIGHT, 1),
                (EV_SYN, SYN_REPORT, 0),
                (EV_KEY, BTN_RIGHT, 0),
                (EV_SYN, SYN_REPORT, 0),
            ]
        );
    }

    #[test]
    fn the_far_corner_of_the_window_lands_inside_the_screen() {
        let (x, y) = absolute(Point { x: 1.0, y: 1.0 }, WINDOW, SCREEN).expect("in range");

        assert!(x <= ABS_MAX && y <= ABS_MAX);
        assert_eq!((x, y), (30719, 39442));
    }

    #[test]
    fn a_window_with_no_extent_is_refused_rather_than_clicking_at_the_origin() {
        let flat = Rect {
            left: 0,
            top: 0,
            width: 0,
            height: 0,
        };

        assert!(matches!(
            encode(
                &InputCommand::MoveTo {
                    to: Point { x: 0.5, y: 0.5 }
                },
                flat,
                SCREEN
            ),
            Err(InputError::Backend(_))
        ));
    }

    #[test]
    fn a_key_the_plugin_invented_is_named_in_the_error() {
        let error = encode(
            &InputCommand::KeyPress {
                key: Key("any key".to_owned()),
            },
            WINDOW,
            SCREEN,
        )
        .expect_err("there is no such key");

        assert!(error.to_string().contains("any key"), "{error}");
    }

    #[test]
    fn a_wait_emits_nothing_because_the_backend_sleeps_instead() {
        assert!(encode(&InputCommand::Wait { ms: 250 }, WINDOW, SCREEN)
            .expect("encodes")
            .is_empty());
    }

    #[test]
    fn scrolling_carries_the_sign_the_plugin_asked_for() {
        let events = encode(
            &InputCommand::Scroll {
                at: Point { x: 0.5, y: 0.5 },
                delta: -3,
            },
            WINDOW,
            SCREEN,
        )
        .expect("encodes");

        assert!(codes(&events).contains(&(EV_REL, REL_WHEEL, -3)));
    }
}
