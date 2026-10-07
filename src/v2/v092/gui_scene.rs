//! Serialize logical widget records directly without constructing VM JSON maps.
use super::*;
use rewind::{map_storage::PersistentMap, storage::PagedValues, MapKey};
use serde::{
    ser::{Error as _, SerializeMap, SerializeSeq},
    Serialize, Serializer,
};
use std::io::{self, Write};
const MAX_SCENE: usize = 1024 * 1024;
fn target<'a>(value: &'a Value, rt: &'a Runtime) -> &'a Value {
    match value {
        Value::HeapRef(id) | Value::CellRef(id) => rt.heap_get(*id).unwrap_or(value),
        _ => value,
    }
}
struct Scene<'a> {
    title: &'a str,
    width: i64,
    height: i64,
    background: i64,
    items: &'a PagedValues,
    focus: i64,
    scroll: &'a PersistentMap,
}
fn scene<'a>(args: &'a [Value], rt: &'a Runtime) -> std::result::Result<Scene<'a>, &'static str> {
    let [Value::Text(title), Value::Int(width), Value::Int(height), Value::Int(background), items, Value::Int(focus), scroll] =
        args
    else {
        return Err("GuiInvalidScene");
    };
    let Value::TypedList(_, items) = target(items, rt) else {
        return Err("GuiInvalidScene");
    };
    let Value::TypedMap(key, value, scroll) = target(scroll, rt) else {
        return Err("GuiInvalidScene");
    };
    if key != "String" || value != "Int" || items.len() > 2048 {
        return Err("GuiInvalidScene");
    }
    Ok(Scene {
        title,
        width: *width,
        height: *height,
        background: *background,
        items,
        focus: *focus,
        scroll,
    })
}
fn text<'a>(
    fields: &'a BTreeMap<String, Value>,
    name: &str,
) -> std::result::Result<&'a str, &'static str> {
    match fields.get(name) {
        Some(Value::Text(s)) => Ok(s),
        _ => Err("GuiInvalidScene"),
    }
}
fn integer(fields: &BTreeMap<String, Value>, name: &str) -> std::result::Result<i64, &'static str> {
    match fields.get(name) {
        Some(Value::Int(n)) => Ok(*n),
        _ => Err("GuiInvalidScene"),
    }
}
fn boolean(
    fields: &BTreeMap<String, Value>,
    name: &str,
) -> std::result::Result<bool, &'static str> {
    match fields.get(name) {
        Some(Value::Bool(n)) => Ok(*n),
        _ => Err("GuiInvalidScene"),
    }
}
struct Item<'a> {
    fields: &'a BTreeMap<String, Value>,
    focused: bool,
    scroll: i64,
}
impl Serialize for Item<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let f = self.fields;
        let mut m = serializer.serialize_map(Some(15))?;
        macro_rules! int {
            ($name:literal) => {
                m.serialize_entry($name, &integer(f, $name).map_err(S::Error::custom)?)?;
            };
        }
        macro_rules! bool {
            ($name:literal) => {
                m.serialize_entry($name, &boolean(f, $name).map_err(S::Error::custom)?)?;
            };
        }
        macro_rules! text {
            ($name:literal) => {
                m.serialize_entry($name, &text(f, $name).map_err(S::Error::custom)?)?;
            };
        }
        int!("anchor");
        int!("background");
        bool!("checked");
        int!("cursor");
        bool!("enabled");
        m.serialize_entry("focused", &self.focused)?;
        int!("foreground");
        int!("height");
        text!("id");
        text!("kind");
        m.serialize_entry("scroll", &self.scroll)?;
        m.serialize_entry("text", &text(f, "caption").map_err(S::Error::custom)?)?;
        int!("width");
        int!("x");
        int!("y");
        m.end()
    }
}
struct Items<'a>(&'a Scene<'a>);
impl Serialize for Items<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let scene = self.0;
        let mut sequence = serializer.serialize_seq(Some(scene.items.len()))?;
        for (i, value) in scene.items.iter().enumerate() {
            let Value::Struct(_, fields) = value else {
                return Err(S::Error::custom("GuiInvalidScene"));
            };
            let id = text(fields, "id").map_err(S::Error::custom)?;
            let scroll = match scene.scroll.get(&MapKey::Text(id.into())) {
                Some(Value::Int(n)) => *n,
                None => 0,
                _ => return Err(S::Error::custom("GuiInvalidScene")),
            };
            sequence.serialize_element(&Item {
                fields,
                focused: scene.focus == i as i64,
                scroll,
            })?;
        }
        sequence.end()
    }
}
impl Serialize for Scene<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut m = serializer.serialize_map(Some(5))?;
        m.serialize_entry("background", &self.background)?;
        m.serialize_entry("height", &self.height)?;
        m.serialize_entry("items", &Items(self))?;
        m.serialize_entry("title", &self.title)?;
        m.serialize_entry("width", &self.width)?;
        m.end()
    }
}
struct Counter(usize);
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_SCENE - self.0 {
            return Err(io::Error::other("ByteLimit"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn parameter(name: &str, index: usize) -> Option<&'static str> {
    if name != "stdGuiScene" {
        return None;
    }
    match index {
        4 => Some("&List<Unknown>"),
        6 => Some("&Map<String,Int>"),
        _ => None,
    }
}
pub(super) fn work(name: &str, args: &[Value], rt: &Runtime) -> Option<usize> {
    if name != "stdGuiScene" {
        return None;
    }
    Some(match scene(args, rt) {
        Err(_) => 1,
        Ok(s) => s
            .items
            .iter()
            .fold(s.title.len().saturating_add(64), |sum, item| {
                let bytes = match item {
                    Value::Struct(_, fields) => fields
                        .values()
                        .map(|v| match v {
                            Value::Text(s) => s.len(),
                            _ => 8,
                        })
                        .sum::<usize>(),
                    _ => 1,
                };
                sum.saturating_add(bytes.saturating_mul(12).saturating_add(1024))
            }),
    })
}
pub(super) fn call(name: &str, args: &[Value], rt: &mut Runtime) -> Result<Option<Value>> {
    if name != "stdGuiScene" {
        return Ok(None);
    }
    let mut counter = Counter(0);
    let result = scene(args, rt).and_then(|s| {
        serde_json::to_writer(&mut counter, &s).map_err(|e| {
            if e.to_string().contains("ByteLimit") {
                "ByteLimit"
            } else {
                "GuiInvalidScene"
            }
        })
    });
    if let Err(code) = result {
        return Ok(Some(outcome(Err((code, 0)))));
    }
    rt.check_native_allocation(counter.0)?;
    let mut output = Vec::with_capacity(counter.0);
    let result = scene(args, rt)
        .and_then(|s| serde_json::to_writer(&mut output, &s).map_err(|_| "GuiInvalidScene"))
        .map(|_| {
            Value::Text(
                String::from_utf8(output)
                    .expect("JSON serialization is UTF-8")
                    .into(),
            )
        });
    Ok(Some(outcome(result.map_err(|e| (e, 0)))))
}
