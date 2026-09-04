// SPDX-License-Identifier: MPL-2.0

/// Virtual-key code for a key name declared by a plugin.
///
/// Unknown names return `None` rather than a default, because pressing the
/// wrong key in a game is worse than pressing none.
pub(crate) fn virtual_key(name: &str) -> Option<u16> {
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
}
