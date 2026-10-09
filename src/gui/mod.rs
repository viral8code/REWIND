//! Native single-window surfaces. Scenes are data; only publish touches a surface.
use serde::{Deserialize, Serialize};
pub(crate) mod clipboard;
mod command_keys;
pub mod edit;
mod live;
mod runtime;
mod windows_registry;
pub(crate) use runtime::Request;
use std::collections::BTreeSet;
use std::io;
pub use windows_registry::WindowEvent;
pub(crate) use windows_registry::WindowInput;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: String,
    pub foreground: u32,
    pub background: u32,
    pub enabled: bool,
    pub checked: bool,
    pub focused: bool,
    #[serde(default)]
    pub cursor: usize,
    #[serde(default)]
    pub anchor: usize,
    #[serde(default)]
    pub scroll: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub background: u32,
    pub items: Vec<Item>,
}
impl Frame {
    pub fn validate(&self) -> io::Result<()> {
        let mut ids = BTreeSet::new();
        if self.title.len() > 1024
            || self.title.contains('\0')
            || !(64..=4096).contains(&self.width)
            || !(64..=4096).contains(&self.height)
            || self.background > 0xffffff
            || self.items.len() > 2048
        {
            return Err(invalid("GuiInvalidScene"));
        }
        for v in &self.items {
            if !matches!(
                v.kind.as_str(),
                "label" | "button" | "checkbox" | "rect" | "textbox" | "textarea"
            ) || v.id.is_empty()
                || v.id.len() > 128
                || v.id.contains('\0')
                || !ids.insert(&v.id)
                || v.text.len() > 4096
                || v.text.contains('\0')
                || v.scroll > 4096
                || v.cursor > v.text.chars().count()
                || v.anchor > v.text.chars().count()
                || v.foreground > 0xffffff
                || v.background > 0xffffff
                || v.x < 0
                || v.y < 0
                || v.width < 1
                || v.height < 1
                || v.x as i64 + v.width as i64 > self.width as i64
                || v.y as i64 + v.height as i64 > self.height as i64
            {
                return Err(invalid("GuiInvalidScene"));
            }
        }
        Ok(())
    }
    pub fn bytes(&self) -> usize {
        self.title.len()
            + 128
            + self
                .items
                .iter()
                .map(|v| v.id.len() + v.text.len() + 128)
                .sum::<usize>()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub key: String,
    pub width: i32,
    pub height: i32,
}
impl Event {
    pub fn simple(kind: &str) -> Self {
        Self {
            kind: kind.into(),
            x: 0,
            y: 0,
            key: String::new(),
            width: 0,
            height: 0,
        }
    }
    pub fn validate(&self) -> io::Result<()> {
        if !matches!(
            self.kind.as_str(),
            "pointer" | "key" | "resize" | "close" | "text" | "idle" | "wheel"
        ) || self.key.len() > if self.kind == "text" { 4096 } else { 64 }
            || self.key.contains('\0')
            || !(-4096..=8192).contains(&self.x)
            || !(-4096..=8192).contains(&self.y)
            || !(0..=8192).contains(&self.width)
            || !(0..=8192).contains(&self.height)
        {
            return Err(invalid("GuiInvalidEvent"));
        }
        Ok(())
    }
}
pub(super) fn invalid(s: &str) -> io::Error {
    io::Error::other(s)
}
pub(crate) struct Host {
    #[cfg(target_os = "linux")]
    backend: x11::Surface,
    #[cfg(windows)]
    backend: windows::Surface,
}
impl Host {
    pub fn configure_command_keys(&mut self, enabled: bool) {
        #[cfg(any(target_os = "linux", windows))]
        self.backend.configure_command_keys(enabled);
        #[cfg(not(any(target_os = "linux", windows)))]
        let _ = enabled;
    }

    pub fn configure_clipboard(&mut self, enabled: bool) {
        #[cfg(any(target_os = "linux", windows))]
        self.backend.configure_clipboard(enabled);
        #[cfg(not(any(target_os = "linux", windows)))]
        let _ = enabled;
    }
    pub fn prepare() -> io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                backend: x11::Surface::new()?,
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {
                backend: windows::Surface::new()?,
            })
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            Err(invalid("GuiUnsupportedPlatform"))
        }
    }
    pub fn present(&mut self, frame: &Frame) -> io::Result<()> {
        frame.validate()?;
        #[cfg(any(target_os = "linux", windows))]
        {
            self.backend.present(frame)
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            Err(invalid("GuiUnsupportedPlatform"))
        }
    }
    pub fn event(&mut self) -> io::Result<Event> {
        #[cfg(any(target_os = "linux", windows))]
        {
            self.backend.event()
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            Err(invalid("GuiUnsupportedPlatform"))
        }
    }
    pub fn poll(&mut self) -> io::Result<Option<Event>> {
        #[cfg(any(target_os = "linux", windows))]
        {
            self.backend.poll()
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            Err(invalid("GuiUnsupportedPlatform"))
        }
    }
    pub fn close(&mut self) {
        #[cfg(any(target_os = "linux", windows))]
        self.backend.close();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_gui_surface_paints_receives_pointer_and_closes() {
        #[cfg(target_os = "linux")]
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            return;
        }
        #[cfg(any(target_os = "linux", windows))]
        {
            let mut host = Host::prepare().unwrap();
            let frame = Frame {
                title: "REWIND GUI test 界".into(),
                width: 240,
                height: 160,
                background: 0xffffff,
                items: vec![Item {
                    id: "button".into(),
                    kind: "button".into(),
                    x: 10,
                    y: 10,
                    width: 120,
                    height: 40,
                    text: "Click 界".into(),
                    foreground: 0,
                    background: 0xcccccc,
                    enabled: true,
                    checked: false,
                    focused: true,
                    cursor: 0,
                    anchor: 0,
                    scroll: 0,
                }],
            };
            host.present(&frame).unwrap();
            host.backend.inject_pointer(25, 25);
            host.backend.inject_key_and_close();
            let mut pointer = false;
            let mut key = false;
            for _ in 0..32 {
                let e = host.event().unwrap();
                if e.kind == "pointer" {
                    assert_eq!((e.x, e.y), (25, 25));
                    pointer = true;
                }
                if e.kind == "key" {
                    assert_eq!(e.key, "Enter");
                    key = true;
                }
                if e.kind == "close" {
                    assert!(pointer && key);
                    host.close();
                    return;
                }
            }
            panic!("pointer event missing");
        }
    }
}

#[cfg(test)]
mod command_key_tests {
    use super::*;
    #[test]
    fn native_gui_command_keys_preserve_legacy_input_and_dispatch_shortcuts() {
        #[cfg(target_os = "linux")]
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        #[cfg(any(target_os = "linux", windows))]
        {
            let mut host = Host::prepare().unwrap();
            host.present(&Frame {
                title: "REWIND native command keys".into(),
                width: 240,
                height: 160,
                background: 0xffffff,
                items: vec![],
            })
            .unwrap();
            host.backend
                .inject_command_key(Some('o'), None, true, false, false);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            let mut legacy_observed = false;
            while std::time::Instant::now() < deadline {
                match host.poll().unwrap() {
                    Some(event) => {
                        assert_ne!(event.key, "Ctrl+O");
                        if event.kind == "text" && event.key == "o" {
                            legacy_observed = true;
                            break;
                        }
                    }
                    None => {
                        #[cfg(windows)]
                        break;
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
            }
            #[cfg(target_os = "linux")]
            assert!(legacy_observed);
            #[cfg(windows)]
            let _ = legacy_observed;
            host.close();
            let mut host = Host::prepare().unwrap();
            host.present(&Frame {
                title: "REWIND native enabled command keys".into(),
                width: 240,
                height: 160,
                background: 0xffffff,
                items: vec![],
            })
            .unwrap();
            host.configure_command_keys(true);
            host.configure_clipboard(true);
            for (letter, function, control, alt, shift, expected) in [
                (Some('o'), None, true, false, false, "Ctrl+O"),
                (Some('t'), None, true, false, true, "Ctrl+Shift+T"),
                (Some('f'), None, false, true, false, "Alt+F"),
                (None, Some(10), false, false, false, "F10"),
                (Some('c'), None, true, false, false, "Ctrl+C"),
            ] {
                host.backend
                    .inject_command_key(letter, function, control, alt, shift);
                let mut found = false;
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while std::time::Instant::now() < deadline {
                    if let Some(event) = host.poll().unwrap() {
                        if event.kind == "key" && event.key == expected {
                            found = true;
                            break;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                assert!(found, "native shortcut missing: {expected}");
            }
            host.close();
        }
    }
}
