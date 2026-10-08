//! Clipboard gestures use the last published scene, never pending VM edits.
use super::{Event, Frame, Item};

pub(super) const MAX_TEXT_BYTES: usize = 4096;
pub(crate) const HOST_RESERVATION: usize = 32 * 1024;

pub(super) fn focused_input(frame: &Frame) -> Option<&Item> {
    frame.items.iter().find(|item| {
        item.enabled && item.focused && matches!(item.kind.as_str(), "textbox" | "textarea")
    })
}

pub(super) fn selected_text(frame: &Frame) -> Option<String> {
    let item = focused_input(frame)?;
    let start = item.cursor.min(item.anchor);
    let end = item.cursor.max(item.anchor);
    if start == end {
        return None;
    }
    let text: String = item.text.chars().skip(start).take(end - start).collect();
    valid_text(&text).then_some(text)
}

pub(super) fn valid_text(text: &str) -> bool {
    text.len() <= MAX_TEXT_BYTES && !text.contains('\0')
}

pub(super) fn text_event(text: String) -> Option<Event> {
    if text.is_empty() || !valid_text(&text) {
        return None;
    }
    let mut event = Event::simple("text");
    event.key = text;
    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_clipboard_transfers_unicode_between_independent_windows() {
        #[cfg(target_os = "linux")]
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        #[cfg(any(target_os = "linux", windows))]
        {
            use crate::gui::Host;
            let text = "界🙂\nclipboard";
            let mut frame = Frame {
                title: "REWIND clipboard acceptance".into(),
                width: 240,
                height: 160,
                background: 0xffffff,
                items: vec![Item {
                    id: "input".into(),
                    kind: "textarea".into(),
                    x: 0,
                    y: 0,
                    width: 220,
                    height: 120,
                    text: text.into(),
                    foreground: 0,
                    background: 0xffffff,
                    enabled: true,
                    checked: false,
                    focused: true,
                    cursor: text.chars().count(),
                    anchor: 0,
                    scroll: 0,
                }],
            };
            let mut owner = Host::prepare().unwrap();
            owner.configure_clipboard(true);
            owner.present(&frame).unwrap();
            owner.backend.inject_clipboard_key(b'C');
            for _ in 0..20 {
                owner.poll().unwrap();
            }
            frame.items[0].text.clear();
            frame.items[0].cursor = 0;
            let mut receiver = Host::prepare().unwrap();
            receiver.configure_clipboard(true);
            receiver.present(&frame).unwrap();
            receiver.backend.inject_clipboard_key(b'V');
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let mut received = None;
            while std::time::Instant::now() < deadline {
                owner.poll().unwrap();
                if let Some(event) = receiver.poll().unwrap() {
                    if event.kind == "text" {
                        received = Some(event.key);
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert_eq!(received.as_deref(), Some(text));
            owner.backend.inject_clipboard_key(b'X');
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let mut cut = None;
            while std::time::Instant::now() < deadline {
                if let Some(event) = owner.poll().unwrap() {
                    if event.kind == "key" {
                        cut = Some(event.key);
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert_eq!(cut.as_deref(), Some("Delete"));
            // Reading clipboard text emits an input event; it does not mutate a scene.
            assert_eq!(frame.items[0].text, "");
            receiver.close();
            owner.close();
        }
    }

    #[test]
    fn clipboard_text_preserves_unicode_and_rejects_nul_or_oversize_input() {
        let text = "A界🙂\n".to_owned();
        assert_eq!(text_event(text.clone()).unwrap().key, text);
        assert!(text_event(String::new()).is_none());
        assert!(text_event("x\0y".into()).is_none());
        assert!(text_event("x".repeat(MAX_TEXT_BYTES + 1)).is_none());
        assert!(text_event("界".repeat(1366)).is_none());
        assert!(text_event("x".repeat(MAX_TEXT_BYTES)).is_some());
    }

    #[test]
    fn selection_uses_scalar_indices_and_only_enabled_focused_text_inputs() {
        let mut frame = Frame {
            title: "clipboard".into(),
            width: 240,
            height: 160,
            background: 0,
            items: vec![Item {
                id: "input".into(),
                kind: "textbox".into(),
                x: 0,
                y: 0,
                width: 120,
                height: 40,
                text: "A界🙂Z".into(),
                foreground: 0,
                background: 0,
                enabled: true,
                checked: false,
                focused: true,
                cursor: 3,
                anchor: 1,
                scroll: 0,
            }],
        };
        frame.validate().unwrap();
        assert_eq!(selected_text(&frame).as_deref(), Some("界🙂"));
        frame.items[0].cursor = 1;
        frame.items[0].anchor = 3;
        assert_eq!(selected_text(&frame).as_deref(), Some("界🙂"));
        frame.items[0].enabled = false;
        assert!(selected_text(&frame).is_none());
        frame.items[0].enabled = true;
        frame.items[0].focused = false;
        assert!(selected_text(&frame).is_none());
        frame.items[0].focused = true;
        frame.items[0].kind = "button".into();
        assert!(selected_text(&frame).is_none());
    }
}
