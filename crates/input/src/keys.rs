// SPDX-License-Identifier: MPL-2.0

/// Virtual-key code for a key name declared by a plugin.
///
/// Unknown names return `None` rather than a default, because pressing the
/// wrong key in a game is worse than pressing none.
pub fn virtual_key(name: &str) -> Option<u16> {
    let name = name.trim().to_ascii_lowercase();

    if name.len() == 1 {
        let ch = name.chars().next()?;
        if ch.is_ascii_lowercase() {
            return Some(0x41 + (ch as u16 - 'a' as u16));
        }
        if ch.is_ascii_digit() {
            return Some(0x30 + (ch as u16 - '0' as u16));
        }
    }

    if let Some(number) = name.strip_prefix('f') {
        if let Ok(index) = number.parse::<u16>() {
            if (1..=24).contains(&index) {
                return Some(0x70 + index - 1);
            }
        }
    }

    Some(match name.as_str() {
        "enter" | "return" => 0x0D,
        "escape" | "esc" => 0x1B,
        "tab" => 0x09,
        "space" => 0x20,
        "backspace" => 0x08,
        "delete" | "del" => 0x2E,
        "insert" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "shift" => 0x10,
        "ctrl" | "control" => 0x11,
        "alt" => 0x12,
        _ => return None,
    })
}

/// Linux input event code for the same key name, from `input-event-codes.h`.
/// The names a plugin may write are the ones `virtual_key` accepts, so a
/// recipe that runs on Windows runs here too.
pub fn evdev_key(name: &str) -> Option<u16> {
    const LETTERS: [u16; 26] = [
        30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17,
        45, 21, 44,
    ];

    let name = name.trim().to_ascii_lowercase();

    if name.len() == 1 {
        let ch = name.chars().next()?;
        if ch.is_ascii_lowercase() {
            return Some(LETTERS[ch as usize - 'a' as usize]);
        }
        if ch.is_ascii_digit() {
            return Some(if ch == '0' {
                11
            } else {
                2 + (ch as u16 - '1' as u16)
            });
        }
    }

    if let Some(number) = name.strip_prefix('f') {
        if let Ok(index) = number.parse::<u16>() {
            return match index {
                1..=10 => Some(59 + index - 1),
                11 => Some(87),
                12 => Some(88),
                13..=24 => Some(183 + index - 13),
                _ => None,
            };
        }
    }

    Some(match name.as_str() {
        "enter" | "return" => 28,
        "escape" | "esc" => 1,
        "tab" => 15,
        "space" => 57,
        "backspace" => 14,
        "delete" | "del" => 111,
        "insert" => 110,
        "home" => 102,
        "end" => 107,
        "pageup" => 104,
        "pagedown" => 109,
        "left" => 105,
        "up" => 103,
        "right" => 106,
        "down" => 108,
        "shift" => 42,
        "ctrl" | "control" => 29,
        "alt" => 56,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_map_to_their_virtual_key() {
        assert_eq!(virtual_key("a"), Some(0x41));
        assert_eq!(virtual_key("z"), Some(0x5A));
    }

    #[test]
    fn the_name_is_read_without_case_or_padding() {
        assert_eq!(virtual_key("  Enter "), Some(0x0D));
        assert_eq!(virtual_key("A"), Some(0x41));
        assert_eq!(virtual_key("F5"), Some(0x74));
    }

    #[test]
    fn digits_are_the_number_row_not_the_numpad() {
        assert_eq!(virtual_key("0"), Some(0x30));
        assert_eq!(virtual_key("9"), Some(0x39));
    }

    #[test]
    fn function_keys_run_from_one_to_twenty_four() {
        assert_eq!(virtual_key("f1"), Some(0x70));
        assert_eq!(virtual_key("f24"), Some(0x87));
        assert_eq!(virtual_key("f0"), None);
        assert_eq!(virtual_key("f25"), None);
    }

    #[test]
    fn an_unknown_name_presses_nothing() {
        assert_eq!(virtual_key("wumpus"), None);
        assert_eq!(virtual_key(""), None);
        assert_eq!(
            virtual_key("ctrl+c"),
            None,
            "a chord is not a key name and must not silently become one key"
        );
    }

    #[test]
    fn a_name_the_windows_table_knows_is_a_name_linux_knows() {
        for name in [
            "a",
            "z",
            "0",
            "9",
            "f1",
            "f12",
            "f24",
            "enter",
            "return",
            "escape",
            "esc",
            "tab",
            "space",
            "backspace",
            "delete",
            "del",
            "insert",
            "home",
            "end",
            "pageup",
            "pagedown",
            "left",
            "up",
            "right",
            "down",
            "shift",
            "ctrl",
            "control",
            "alt",
        ] {
            assert!(
                virtual_key(name).is_some() && evdev_key(name).is_some(),
                "`{name}` is only pressable on one platform"
            );
        }
    }

    #[test]
    fn letters_and_digits_map_to_their_event_code() {
        assert_eq!(evdev_key("a"), Some(30));
        assert_eq!(evdev_key("z"), Some(44));
        assert_eq!(evdev_key("q"), Some(16));
        assert_eq!(evdev_key("1"), Some(2));
        assert_eq!(evdev_key("9"), Some(10));
        assert_eq!(evdev_key("0"), Some(11));
    }

    #[test]
    fn function_keys_survive_the_gap_in_the_middle_of_the_table() {
        assert_eq!(evdev_key("f1"), Some(59));
        assert_eq!(evdev_key("f10"), Some(68));
        assert_eq!(evdev_key("f11"), Some(87));
        assert_eq!(evdev_key("f12"), Some(88));
        assert_eq!(evdev_key("f13"), Some(183));
        assert_eq!(evdev_key("f24"), Some(194));
        assert_eq!(evdev_key("f25"), None);
    }

    #[test]
    fn an_unknown_name_presses_nothing_on_linux_either() {
        assert_eq!(evdev_key("any key"), None);
        assert_eq!(evdev_key(""), None);
    }
}
