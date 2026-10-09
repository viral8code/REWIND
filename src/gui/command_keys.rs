//! Shared native shortcut naming; text/IME and platform close gestures keep their routes.
pub(crate) fn command_key(
    letter: Option<char>,
    function: Option<u8>,
    control: bool,
    alt: bool,
    shift: bool,
    level_three: bool,
) -> Option<String> {
    if level_three || (control && alt) || (alt && function == Some(4)) {
        return None;
    }
    let base = if let Some(number) = function {
        if !(1..=24).contains(&number) {
            return None;
        }
        format!("F{number}")
    } else {
        let letter = letter?;
        if !letter.is_ascii_alphabetic() || (!control && !alt) {
            return None;
        }
        letter.to_ascii_uppercase().to_string()
    };
    let mut key = String::new();
    if control {
        key.push_str("Ctrl+");
    } else if alt {
        key.push_str("Alt+");
    }
    if shift {
        key.push_str("Shift+");
    }
    key.push_str(&base);
    Some(key)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn character_shortcuts_and_function_keys_have_stable_names() {
        assert_eq!(
            command_key(Some('o'), None, true, false, false, false).as_deref(),
            Some("Ctrl+O")
        );
        assert_eq!(
            command_key(Some('T'), None, true, false, true, false).as_deref(),
            Some("Ctrl+Shift+T")
        );
        assert_eq!(
            command_key(Some('f'), None, false, true, false, false).as_deref(),
            Some("Alt+F")
        );
        assert_eq!(
            command_key(None, Some(10), false, false, true, false).as_deref(),
            Some("Shift+F10")
        );
        assert_eq!(
            command_key(None, Some(24), true, false, false, false).as_deref(),
            Some("Ctrl+F24")
        );
    }
    #[test]
    fn ordinary_text_altgr_and_native_close_are_not_captured_as_commands() {
        assert!(command_key(Some('o'), None, false, false, false, false).is_none());
        assert!(command_key(Some('q'), None, true, true, false, false).is_none());
        assert!(command_key(Some('q'), None, true, false, false, true).is_none());
        assert!(command_key(Some('界'), None, true, false, false, false).is_none());
        assert!(command_key(None, Some(4), false, true, false, false).is_none());
        assert!(command_key(None, Some(25), false, false, false, false).is_none());
    }
}
