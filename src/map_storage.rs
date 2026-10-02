//! Persistent AVL storage for primitive Map keys; wire format remains ordered pairs.
use crate::{MapKey, Runtime, Value};
use std::{cmp::Ordering, sync::Arc};
type Link = Option<Arc<Node>>;
struct Entry {
    key: Arc<MapKey>,
    value: Arc<Value>,
    bytes: usize,
    allocation_bytes: usize,
}
impl Entry {
    fn new(key: Arc<MapKey>, value: Arc<Value>) -> Arc<Self> {
        let key_bytes = match key.as_ref() {
            MapKey::BigInt(v) => v.retained_bytes(),
            MapKey::Decimal(v) => v.retained_bytes(),
            MapKey::Text(s) => s.len(),
            MapKey::Bytes(b) => b.len(),
            MapKey::Bool(_) => 1,
            _ => 8,
        };
        let bytes = key_bytes + Runtime::value_bytes(&value);
        let allocation_bytes = key_bytes
            .saturating_add(Runtime::allocation_bytes(&value))
            .saturating_add(192);
        Arc::new(Self {
            key,
            value,
            bytes,
            allocation_bytes,
        })
    }
}
struct Node {
    entry: Arc<Entry>,
    left: Link,
    right: Link,
    height: usize,
    size: usize,
    bytes: usize,
    allocation_bytes: usize,
}
impl std::ops::Deref for Node {
    type Target = Entry;
    fn deref(&self) -> &Entry {
        &self.entry
    }
}
thread_local! { static NODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
pub fn map_nodes_created() -> usize {
    NODES.with(std::cell::Cell::get)
}
fn height(n: &Link) -> usize {
    n.as_ref().map_or(0, |n| n.height)
}
fn size(n: &Link) -> usize {
    n.as_ref().map_or(0, |n| n.size)
}
fn bytes(n: &Link) -> usize {
    n.as_ref().map_or(0, |n| n.bytes)
}
fn node(entry: Arc<Entry>, left: Link, right: Link) -> Arc<Node> {
    NODES.with(|n| n.set(n.get() + 1));
    Arc::new(Node {
        height: 1 + height(&left).max(height(&right)),
        size: 1 + size(&left) + size(&right),
        bytes: entry.bytes + bytes(&left) + bytes(&right),
        allocation_bytes: entry
            .allocation_bytes
            .saturating_add(left.as_ref().map_or(0, |n| n.allocation_bytes))
            .saturating_add(right.as_ref().map_or(0, |n| n.allocation_bytes)),
        entry,
        left,
        right,
    })
}

fn rotate_left(n: Arc<Node>) -> Arc<Node> {
    let right = n.right.as_ref().unwrap();
    node(
        right.entry.clone(),
        Some(node(n.entry.clone(), n.left.clone(), right.left.clone())),
        right.right.clone(),
    )
}
fn rotate_right(n: Arc<Node>) -> Arc<Node> {
    let left = n.left.as_ref().unwrap();
    node(
        left.entry.clone(),
        left.left.clone(),
        Some(node(n.entry.clone(), left.right.clone(), n.right.clone())),
    )
}
fn balanced(mut n: Arc<Node>) -> Arc<Node> {
    if height(&n.left) > height(&n.right) + 1 {
        let left = n.left.as_ref().unwrap();
        if height(&left.right) > height(&left.left) {
            n = node(
                n.entry.clone(),
                Some(rotate_left(left.clone())),
                n.right.clone(),
            );
        }
        rotate_right(n)
    } else if height(&n.right) > height(&n.left) + 1 {
        let right = n.right.as_ref().unwrap();
        if height(&right.left) > height(&right.right) {
            n = node(
                n.entry.clone(),
                n.left.clone(),
                Some(rotate_right(right.clone())),
            );
        }
        rotate_left(n)
    } else {
        n
    }
}
fn insert(root: &Link, key: Arc<MapKey>, value: Arc<Value>) -> Arc<Node> {
    let Some(n) = root else {
        return node(Entry::new(key, value), None, None);
    };
    match key.cmp(&n.key) {
        Ordering::Less => balanced(node(
            n.entry.clone(),
            Some(insert(&n.left, key, value)),
            n.right.clone(),
        )),
        Ordering::Greater => balanced(node(
            n.entry.clone(),
            n.left.clone(),
            Some(insert(&n.right, key, value)),
        )),
        Ordering::Equal => node(
            Entry::new(n.key.clone(), value),
            n.left.clone(),
            n.right.clone(),
        ),
    }
}
fn remove(root: &Link, key: &MapKey) -> Link {
    let n = root.as_ref()?;
    match key.cmp(&n.key) {
        Ordering::Less => Some(balanced(node(
            n.entry.clone(),
            remove(&n.left, key),
            n.right.clone(),
        ))),
        Ordering::Greater => Some(balanced(node(
            n.entry.clone(),
            n.left.clone(),
            remove(&n.right, key),
        ))),
        Ordering::Equal => match (&n.left, &n.right) {
            (None, _) => n.right.clone(),
            (_, None) => n.left.clone(),
            (Some(_), Some(right)) => {
                let mut next = right.as_ref();
                while let Some(left) = &next.left {
                    next = left;
                }
                Some(balanced(node(
                    next.entry.clone(),
                    n.left.clone(),
                    remove(&n.right, &next.key),
                )))
            }
        },
    }
}
#[derive(Clone, Default)]
pub struct PersistentMap {
    root: Link,
}
impl PersistentMap {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        size(&self.root)
    }
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }
    pub fn allocation_bytes(&self) -> usize {
        self.root.as_ref().map_or(0, |n| n.allocation_bytes)
    }
    pub fn logical_bytes(&self) -> usize {
        bytes(&self.root)
    }
    pub fn get(&self, key: &MapKey) -> Option<&Value> {
        let mut cursor = self.root.as_deref();
        while let Some(n) = cursor {
            match key.cmp(&n.key) {
                Ordering::Less => cursor = n.left.as_deref(),
                Ordering::Greater => cursor = n.right.as_deref(),
                Ordering::Equal => return Some(&n.value),
            }
        }
        None
    }
    pub fn contains_key(&self, key: &MapKey) -> bool {
        self.get(key).is_some()
    }
    pub fn insert(&mut self, key: MapKey, value: Value) -> Option<Value> {
        let previous = self.get(&key).cloned();
        self.set(key, value);
        previous
    }
    pub fn set(&mut self, key: MapKey, value: Value) {
        self.root = Some(insert(&self.root, Arc::new(key), Arc::new(value)));
    }
    pub fn remove(&mut self, key: &MapKey) -> Option<Value> {
        let previous = self.get(key).cloned();
        if previous.is_some() {
            self.root = remove(&self.root, key);
        }
        previous
    }
    pub fn delete(&mut self, key: &MapKey) {
        if self.contains_key(key) {
            self.root = remove(&self.root, key);
        }
    }
    pub fn iter(&self) -> Iter<'_> {
        let mut it = Iter { stack: Vec::new() };
        it.descend(self.root.as_deref());
        it
    }
    pub fn keys(&self) -> impl Iterator<Item = &MapKey> {
        self.iter().map(|(k, _)| k)
    }
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.iter().map(|(_, v)| v)
    }
}
pub struct Iter<'a> {
    stack: Vec<&'a Node>,
}
impl<'a> Iter<'a> {
    fn descend(&mut self, mut n: Option<&'a Node>) {
        while let Some(current) = n {
            self.stack.push(current);
            n = current.left.as_deref();
        }
    }
}
impl<'a> Iterator for Iter<'a> {
    type Item = (&'a MapKey, &'a Value);
    fn next(&mut self) -> Option<Self::Item> {
        let n = self.stack.pop()?;
        self.descend(n.right.as_deref());
        Some((&n.key, &n.value))
    }
}
impl<'a> IntoIterator for &'a PersistentMap {
    type Item = (&'a MapKey, &'a Value);
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl IntoIterator for PersistentMap {
    type Item = (MapKey, Value);
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>()
            .into_iter()
    }
}
impl FromIterator<(MapKey, Value)> for PersistentMap {
    fn from_iter<I: IntoIterator<Item = (MapKey, Value)>>(items: I) -> Self {
        let mut out = Self::new();
        for (k, v) in items {
            out.insert(k, v);
        }
        out
    }
}
impl From<std::collections::BTreeMap<MapKey, Value>> for PersistentMap {
    fn from(value: std::collections::BTreeMap<MapKey, Value>) -> Self {
        value.into_iter().collect()
    }
}
impl std::fmt::Debug for PersistentMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
impl PartialEq for PersistentMap {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl Eq for PersistentMap {}
#[cfg(test)]
mod tests {
    use super::*;
    fn verify(n: &Link) -> usize {
        let Some(n) = n else {
            return 0;
        };
        let a = verify(&n.left);
        let b = verify(&n.right);
        assert!(a.abs_diff(b) <= 1);
        assert_eq!(n.height, 1 + a.max(b));
        n.height
    }
    #[test]
    fn updates_copy_a_logarithmic_path_at_three_sizes() {
        for count in [4096usize, 8192, 16384] {
            let mut map = PersistentMap::new();
            for i in 0..count {
                map.set(MapKey::Int(i as i64), Value::Int(i as i64));
            }
            let snapshot = map.clone();
            assert!(Arc::ptr_eq(
                map.root.as_ref().unwrap(),
                snapshot.root.as_ref().unwrap()
            ));
            let before = map_nodes_created();
            for i in 0..count {
                map.set(MapKey::Int(i as i64), Value::Int(-1));
            }
            assert!(map_nodes_created() - before <= count * 20);
            assert_eq!(snapshot.get(&MapKey::Int(123)), Some(&Value::Int(123)));
            assert_eq!(map.get(&MapKey::Int(123)), Some(&Value::Int(-1)));
        }
    }
    #[test]
    fn persistent_avl_matches_reference_and_keeps_snapshots() {
        let mut map = PersistentMap::new();
        let mut reference = std::collections::BTreeMap::new();
        let mut state = 7u64;
        for _ in 0..4096 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let k = MapKey::Int((state % 1024) as i64);
            let v = Value::Int(state as i64);
            map.insert(k.clone(), v.clone());
            reference.insert(k, v);
            verify(&map.root);
        }
        let snapshot = map.clone();
        for i in (0..1024).step_by(2) {
            assert_eq!(
                map.remove(&MapKey::Int(i)),
                reference.remove(&MapKey::Int(i))
            );
            verify(&map.root);
        }
        assert_eq!(
            map.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<std::collections::BTreeMap<_, _>>(),
            reference
        );
        assert!(snapshot.len() > map.len());
        assert!(map.root.as_ref().unwrap().height <= 16);
    }
}
