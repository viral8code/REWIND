//! Bounded Unicode-scalar editing. Composition is committed by the host before
//! entering this model; only the resulting model participates in checkpoints.
use serde::Serialize;
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Edit {
    pub text: String,
    pub cursor: usize,
    pub anchor: usize,
}
pub fn apply(
    text: &str,
    cursor: i64,
    anchor: i64,
    key: &str,
    typed: &str,
    multiline: bool,
) -> Result<Edit, &'static str> {
    if text.len() > 4096 || typed.len() > 4096 || typed.contains('\0') {
        return Err("GuiInvalidText");
    }
    let mut chars: Vec<char> = text.chars().collect();
    if cursor < 0 || anchor < 0 || cursor as usize > chars.len() || anchor as usize > chars.len() {
        return Err("GuiInvalidSelection");
    }
    let mut cursor = cursor as usize;
    let mut anchor = anchor as usize;
    let shift = key.starts_with("Shift+");
    let key = key.strip_prefix("Shift+").unwrap_or(key);
    match key {
        "Ctrl+A" => {
            cursor = chars.len();
            anchor = 0;
        }
        "Left" => {
            cursor = if !shift && cursor != anchor {
                cursor.min(anchor)
            } else {
                cursor.saturating_sub(1)
            };
            if !shift {
                anchor = cursor;
            }
        }
        "Right" => {
            cursor = if !shift && cursor != anchor {
                cursor.max(anchor)
            } else {
                (cursor + 1).min(chars.len())
            };
            if !shift {
                anchor = cursor;
            }
        }
        "Home" => {
            cursor = if multiline {
                chars[..cursor]
                    .iter()
                    .rposition(|c| *c == '\n')
                    .map_or(0, |n| n + 1)
            } else {
                0
            };
            if !shift {
                anchor = cursor;
            }
        }
        "End" => {
            cursor = if multiline {
                chars[cursor..]
                    .iter()
                    .position(|c| *c == '\n')
                    .map_or(chars.len(), |n| cursor + n)
            } else {
                chars.len()
            };
            if !shift {
                anchor = cursor;
            }
        }
        "Up" | "Down" if multiline => {
            let start = chars[..cursor]
                .iter()
                .rposition(|c| *c == '\n')
                .map_or(0, |n| n + 1);
            let col = cursor - start;
            if key == "Up" && start > 0 {
                let end = start - 1;
                let previous = chars[..end]
                    .iter()
                    .rposition(|c| *c == '\n')
                    .map_or(0, |n| n + 1);
                cursor = previous + col.min(end - previous);
            }
            if key == "Down" {
                if let Some(n) = chars[cursor..].iter().position(|c| *c == '\n') {
                    let next = cursor + n + 1;
                    let end = chars[next..]
                        .iter()
                        .position(|c| *c == '\n')
                        .map_or(chars.len(), |n| next + n);
                    cursor = next + col.min(end - next);
                }
            }
            if !shift {
                anchor = cursor;
            }
        }
        "Backspace" | "Delete" => {
            let (mut start, mut end) = (cursor.min(anchor), cursor.max(anchor));
            if start == end {
                if key == "Backspace" {
                    start = start.saturating_sub(1);
                } else {
                    end = (end + 1).min(chars.len());
                }
            }
            chars.drain(start..end);
            cursor = start;
            anchor = start;
        }
        _ => {
            let insertion = if key == "Enter" && multiline {
                "\n"
            } else if key == "Space" {
                " "
            } else {
                typed
            };
            if !multiline && insertion.contains(['\n', '\r']) {
                return Err("GuiSingleLine");
            }
            if !insertion.is_empty() {
                let (start, end) = (cursor.min(anchor), cursor.max(anchor));
                let added: Vec<char> = insertion.chars().collect();
                let count = added.len();
                chars.splice(start..end, added);
                cursor = start + count;
                anchor = cursor;
            }
        }
    }
    let text: String = chars.into_iter().collect();
    if text.len() > 4096 {
        return Err("GuiInvalidText");
    }
    Ok(Edit {
        text,
        cursor,
        anchor,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_selection_and_deletion_are_scalar_safe() {
        let e = apply("a😀界", 3, 3, "Backspace", "", false).unwrap();
        assert_eq!(e.text, "a😀");
        let e = apply(&e.text, 2, 1, "", "日本語", false).unwrap();
        assert_eq!(
            e,
            Edit {
                text: "a日本語".into(),
                cursor: 4,
                anchor: 4
            }
        );
        assert_eq!(apply("", 0, 0, "", "\n", false), Err("GuiSingleLine"));
    }
    #[test]
    fn selection_movement_does_not_split_utf8() {
        let e = apply("😀界", 2, 2, "Shift+Left", "", false).unwrap();
        assert_eq!((e.cursor, e.anchor), (1, 2));
        let e = apply(
            &e.text,
            e.cursor as i64,
            e.anchor as i64,
            "Delete",
            "",
            false,
        )
        .unwrap();
        assert_eq!(e.text, "😀");
    }
}
