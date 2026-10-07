//! Shared immutable text with copy-on-write for Rust-side string builders.
use std::{
    borrow::Borrow,
    fmt,
    ops::{Deref, DerefMut},
    sync::Arc,
};

// Short strings keep the previous allocation layout; do not add an Arc allocation
// to every tiny diagnostic, label, field name or scalar conversion.
const SHARED_THRESHOLD: usize = 256;
#[derive(Clone)]
enum Storage {
    Owned(String),
    Shared(Arc<String>),
}
#[derive(Clone)]
pub struct Text(Storage);
impl Text {
    /// Work needed to clone the payload, excluding the Value/frame metadata.
    pub fn clone_work(&self) -> usize {
        match &self.0 {
            Storage::Owned(value) => value.len().saturating_add(1),
            Storage::Shared(_) => 1,
        }
    }
    pub fn into_owned(self) -> String {
        match self.0 {
            Storage::Owned(value) => value,
            Storage::Shared(value) => {
                Arc::try_unwrap(value).unwrap_or_else(|shared| shared.as_ref().clone())
            }
        }
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.into_owned().into_bytes()
    }
    pub fn capacity(&self) -> usize {
        self.deref().capacity()
    }
}
impl From<String> for Text {
    fn from(value: String) -> Self {
        if value.len() <= SHARED_THRESHOLD {
            Self(Storage::Owned(value))
        } else {
            Self(Storage::Shared(Arc::new(value)))
        }
    }
}
impl Default for Text {
    fn default() -> Self {
        String::new().into()
    }
}
impl Eq for Text {}
impl Ord for Text {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl PartialOrd for Text {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl std::hash::Hash for Text {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}
impl serde::Serialize for Text {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de> serde::Deserialize<'de> for Text {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <String as serde::Deserialize>::deserialize(deserializer).map(Into::into)
    }
}
impl From<&str> for Text {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}
impl From<Text> for String {
    fn from(value: Text) -> Self {
        value.into_owned()
    }
}
impl<T: AsRef<str> + ?Sized> PartialEq<T> for Text {
    fn eq(&self, other: &T) -> bool {
        self.as_str() == other.as_ref()
    }
}
impl PartialEq<Text> for String {
    fn eq(&self, other: &Text) -> bool {
        self.as_str() == other.as_str()
    }
}
impl PartialEq<Text> for str {
    fn eq(&self, other: &Text) -> bool {
        self == other.as_str()
    }
}
impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<std::ffi::OsStr> for Text {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.as_str().as_ref()
    }
}
impl Borrow<str> for Text {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl Deref for Text {
    type Target = String;
    fn deref(&self) -> &String {
        match &self.0 {
            Storage::Owned(value) => value,
            Storage::Shared(value) => value,
        }
    }
}
impl DerefMut for Text {
    fn deref_mut(&mut self) -> &mut String {
        match &mut self.0 {
            Storage::Owned(value) => value,
            Storage::Shared(value) => Arc::make_mut(value),
        }
    }
}
impl<T: AsRef<str> + ?Sized> std::ops::Add<&T> for Text {
    type Output = Self;
    fn add(mut self, other: &T) -> Self {
        self.push_str(other.as_ref());
        // Promote a short builder after it crosses the threshold, moving its buffer.
        if matches!(&self.0, Storage::Owned(value) if value.len() > SHARED_THRESHOLD) {
            Self::from(self.into_owned())
        } else {
            self
        }
    }
}
impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}
impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_clones_share_until_mutation_and_last_owner_releases_storage() {
        let original = Text::from("日本語\n".repeat(1024));
        let Storage::Shared(storage) = &original.0 else {
            panic!("long text not shared")
        };
        let weak = Arc::downgrade(storage);
        let mut copy = original.clone();
        let Storage::Shared(shared) = &copy.0 else {
            panic!("clone lost shared storage")
        };
        assert!(Arc::ptr_eq(storage, shared));
        assert_eq!(original.clone_work(), 1);
        copy.push_str("edited");
        assert!(!original.ends_with("edited"));
        assert!(copy.ends_with("edited"));
        drop(original);
        assert!(weak.upgrade().is_none(), "modified copy retained old text");
    }
    #[test]
    fn small_values_avoid_extra_arc_and_concat_promotes_without_changing_order_or_wire() {
        let small = Text::from("a".repeat(SHARED_THRESHOLD));
        assert!(matches!(small.0, Storage::Owned(_)));
        assert_eq!(small.clone_work(), SHARED_THRESHOLD + 1);
        let large = small.clone() + &"b";
        assert!(matches!(large.0, Storage::Shared(_)));
        assert!(small < large);
        assert!(Text::from("z") > Text::from("a".repeat(SHARED_THRESHOLD + 1)));
        for text in [Text::default(), "日本語".into(), small, large] {
            let raw = text.to_string();
            assert_eq!(
                serde_json::to_string(&text).unwrap(),
                serde_json::to_string(&raw).unwrap()
            );
            let restored: Text =
                serde_json::from_str(&serde_json::to_string(&raw).unwrap()).unwrap();
            assert_eq!(text, restored);
            assert_eq!(text.clone().into_bytes(), raw.as_bytes());
            let mut a = std::collections::hash_map::DefaultHasher::new();
            let mut b = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&text, &mut a);
            std::hash::Hash::hash(&raw, &mut b);
            assert_eq!(std::hash::Hasher::finish(&a), std::hash::Hasher::finish(&b));
        }
        let value = crate::Value::Text("unchanged".into());
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            r#"{"Text":"unchanged"}"#
        );
    }
}
