//! Native preedit is transient OS state. Only committed text becomes a VM event.
use super::Item;
pub(super) const MAX_BYTES: usize = 4096;
// Includes bounded native event queues, preedit and conversion / geometry scratch.
pub(crate) const HOST_RESERVATION: usize = 1024 * 1024;
#[derive(Default)]
pub(super) struct Composition {
    pub text: String,
    pub cursor: usize,
    pub active: bool,
    pub accepting: bool,
    pub dirty: bool,
    pub invalid: bool,
}
impl Composition {
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.active = false;
        self.dirty = true;
    }
    pub fn start(&mut self) {
        self.clear();
        self.active = true;
    }
    pub fn replace(&mut self, first: i32, count: i32, text: &str, cursor: i32) -> bool {
        if first < 0 || count < 0 || cursor < 0 || text.len() > MAX_BYTES || text.contains('\0') {
            self.invalid = true;
            return false;
        }
        let mut offsets: Vec<usize> = self.text.char_indices().map(|(i, _)| i).collect();
        offsets.push(self.text.len());
        let first = first as usize;
        let Some(last) = first.checked_add(count as usize) else {
            self.invalid = true;
            return false;
        };
        if last >= offsets.len() || first >= offsets.len() {
            self.invalid = true;
            return false;
        }
        let bytes = self.text.len() - (offsets[last] - offsets[first]) + text.len();
        let scalars = offsets.len() - 1 - count as usize + text.chars().count();
        if bytes > MAX_BYTES || cursor as usize > scalars {
            self.invalid = true;
            return false;
        }
        self.text.replace_range(offsets[first]..offsets[last], text);
        self.cursor = cursor as usize;
        self.active = true;
        self.dirty = true;
        true
    }
    #[cfg(any(windows, test))]
    pub fn set_utf16(&mut self, text: &[u16], cursor: usize) -> bool {
        if text.len() > MAX_BYTES || cursor > text.len() {
            self.invalid = true;
            return false;
        }
        let (Ok(text), Ok(prefix)) = (
            String::from_utf16(text),
            String::from_utf16(&text[..cursor]),
        ) else {
            self.invalid = true;
            return false;
        };
        self.replace(
            0,
            self.text.chars().count() as i32,
            &text,
            prefix.chars().count() as i32,
        )
    }
}
/// Geometry/style publication moves preedit; content, selection or identity changes cancel it.
pub(super) fn same_input(before: Option<&Item>, after: Option<&Item>) -> bool {
    match (before, after) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.id == b.id
                && a.kind == b.kind
                && a.text == b.text
                && a.cursor == b.cursor
                && a.anchor == b.anchor
        }
        _ => false,
    }
}
/// Position follows the published widget's scalar cursor and renderer scroll.
pub(super) fn anchor(item: &Item, width: impl Fn(&str) -> i32) -> (i32, i32) {
    let mut offset = 0;
    for (row, line) in item.text.split('\n').enumerate() {
        let count = line.chars().count();
        if item.cursor >= offset && item.cursor <= offset + count {
            let prefix: String = line.chars().take(item.cursor - offset).collect();
            let advance = width(&prefix).max(0);
            let x =
                (item.x + 6 + advance.min((item.width - 14).max(0))).min(item.x + item.width - 1);
            let max_row = (item.height / 18 - 1).max(0) as usize;
            let visible = row.saturating_sub(item.scroll).min(max_row);
            return (
                x,
                (item.y + 6 + visible as i32 * 18).min(item.y + item.height - 1),
            );
        }
        offset += count + 1;
    }
    (
        (item.x + 6).min(item.x + item.width - 1),
        (item.y + 6).min(item.y + item.height - 1),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preedit_delta_is_bounded_atomic_and_uses_scalar_offsets() {
        let mut c = Composition::default();
        c.start();
        assert!(c.replace(0, 0, "A界🙂", 3));
        assert!(c.replace(1, 1, "かな", 3));
        assert_eq!(c.text, "Aかな🙂");
        assert!(c.replace(0, 1, "", 2));
        assert_eq!(c.text, "かな🙂");
        let before = c.text.clone();
        assert!(!c.replace(8, 1, "x", 0));
        assert_eq!(c.text, before);
        assert!(!c.replace(0, 0, &"x".repeat(MAX_BYTES + 1), 0));
        assert_eq!(c.text, before);
        c.clear();
        assert_eq!(c.text, "");
        assert!(!c.active);
    }
    #[test]
    fn candidate_anchor_stays_inside_narrow_inputs_and_tracks_scrolled_unicode_lines() {
        let mut item = Item {
            id: "input".into(),
            kind: "textarea".into(),
            x: 10,
            y: 20,
            width: 1,
            height: 1,
            text: "界\nabc".into(),
            foreground: 0,
            background: 0xffffff,
            enabled: true,
            checked: false,
            focused: true,
            cursor: 5,
            anchor: 5,
            scroll: 0,
        };
        assert_eq!(anchor(&item, |s| s.chars().count() as i32 * 10), (10, 20));
        item.width = 100;
        item.height = 50;
        assert_eq!(anchor(&item, |s| s.chars().count() as i32 * 10), (46, 44));
        item.scroll = 1;
        assert_eq!(anchor(&item, |s| s.chars().count() as i32 * 10), (46, 26));
    }
    #[test]
    fn utf16_cursor_cannot_split_surrogates_and_failed_changes_do_not_escape() {
        let mut c = Composition::default();
        let units: Vec<_> = "界🙂".encode_utf16().collect();
        assert!(c.set_utf16(&units, 3));
        assert_eq!(c.cursor, 2);
        assert!(!c.set_utf16(&units, 2));
        assert_eq!(c.text, "界🙂");
        assert!(!c.set_utf16(&[0xd800], 0));
        assert_eq!(c.text, "界🙂");
        assert!(!c.set_utf16(&[0], 0));
        assert_eq!(c.text, "界🙂");
    }
}
