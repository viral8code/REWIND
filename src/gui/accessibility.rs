//! Published native semantics; OS clients never receive VM checkpoint state.
use super::{Event, Frame};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    Window,
    Label,
    Button,
    CheckBox,
    Entry,
    TextArea,
}
#[derive(Clone, Debug)]
pub(super) struct Node {
    #[cfg(windows)]
    pub id: Option<String>,
    pub role: Role,
    pub name: String,
    pub value: String,
    pub enabled: bool,
    pub checked: bool,
    pub focused: bool,
    pub focusable: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub cursor: usize,
    pub anchor: usize,
}
#[derive(Default)]
struct Published {
    frame: Option<Arc<Frame>>,
    focused: bool,
    #[cfg(windows)]
    handles: usize,
}
#[derive(Clone, Default)]
pub(super) struct Tree(Arc<Mutex<Published>>);
impl Tree {
    #[cfg(windows)]
    pub fn lease(&self) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.frame.is_none() || state.handles >= 4096 {
            return false;
        }
        state.handles += 1;
        true
    }
    #[cfg(windows)]
    pub fn release(&self) {
        let mut state = self.0.lock().unwrap();
        state.handles = state.handles.saturating_sub(1);
    }
    pub fn publish(&self, frame: Arc<Frame>) {
        self.0.lock().unwrap().frame = Some(frame);
    }
    pub fn focus(&self, focused: bool) {
        self.0.lock().unwrap().focused = focused;
    }
    pub fn close(&self) {
        let mut state = self.0.lock().unwrap();
        state.frame = None;
        state.focused = false;
    }
    pub fn children(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .frame
            .as_ref()
            .map_or_else(Vec::new, |f| {
                f.items
                    .iter()
                    .filter(|i| i.kind != "rect")
                    .map(|i| i.id.clone())
                    .collect()
            })
    }
    pub fn node(&self, id: Option<&str>) -> Option<Node> {
        let state = self.0.lock().unwrap();
        let frame = state.frame.as_ref()?;
        let Some(id) = id else {
            return Some(Node {
                #[cfg(windows)]
                id: None,
                role: Role::Window,
                name: frame.title.clone(),
                value: String::new(),
                enabled: true,
                checked: false,
                focused: state.focused,
                focusable: true,
                x: 0,
                y: 0,
                width: frame.width,
                height: frame.height,
                cursor: 0,
                anchor: 0,
            });
        };
        let item = frame.items.iter().find(|i| i.id == id)?;
        let role = match item.kind.as_str() {
            "label" => Role::Label,
            "button" => Role::Button,
            "checkbox" => Role::CheckBox,
            "textbox" => Role::Entry,
            "textarea" => Role::TextArea,
            _ => return None,
        };
        let editable = matches!(role, Role::Entry | Role::TextArea);
        Some(Node {
            #[cfg(windows)]
            id: Some(item.id.clone()),
            role,
            name: if editable {
                item.id.clone()
            } else {
                item.text.clone()
            },
            value: if editable {
                item.text.clone()
            } else {
                String::new()
            },
            enabled: item.enabled,
            checked: role == Role::CheckBox && item.checked,
            focused: item.focused && state.focused,
            focusable: role != Role::Label,
            x: item.x,
            y: item.y,
            width: item.width,
            height: item.height,
            cursor: item.cursor,
            anchor: item.anchor,
        })
    }
    pub fn default_action(&self, id: &str) -> Option<Event> {
        let n = self.node(Some(id))?;
        if !n.enabled || !n.focusable {
            return None;
        }
        let mut event = Event::simple("pointer");
        event.x = n.x + n.width / 2;
        event.y = n.y + n.height / 2;
        Some(event)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_published_controls_are_visible_and_native_handles_become_defunct_on_close() {
        let frame = Frame {
            title: "Accessible window".into(),
            width: 320,
            height: 160,
            background: 0xffffff,
            items: vec![super::super::Item {
                id: "Email address".into(),
                kind: "textbox".into(),
                x: 10,
                y: 10,
                width: 100,
                height: 24,
                text: "a@example.test".into(),
                foreground: 0,
                background: 0xffffff,
                enabled: true,
                checked: false,
                focused: true,
                cursor: 1,
                anchor: 0,
                scroll: 0,
            }],
        };
        let tree = Tree::default();
        assert!(tree.node(None).is_none());
        tree.publish(Arc::new(frame.clone()));
        assert_eq!(tree.children(), vec!["Email address"]);
        let control = tree.node(Some("Email address")).unwrap();
        assert_eq!(control.name, "Email address");
        assert_eq!(control.value, "a@example.test");
        assert_eq!(control.role, Role::Entry);
        assert!(!control.focused);
        tree.focus(true);
        assert!(tree.node(Some("Email address")).unwrap().focused);
        let mut disabled = frame;
        disabled.items[0].enabled = false;
        let current = Arc::new(disabled);
        let weak = Arc::downgrade(&current);
        tree.publish(current);
        assert!(tree.default_action("Email address").is_none());
        let retained = tree.clone();
        tree.close();
        assert!(retained.node(None).is_none());
        assert!(retained.children().is_empty());
        assert!(retained.default_action("Email address").is_none());
        assert!(
            weak.upgrade().is_none(),
            "retained native clients must not keep the closed scene alive"
        );
    }
}
