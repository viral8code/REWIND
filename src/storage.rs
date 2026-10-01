//! Persistent paged values. Cloning a root is O(1); writes copy a path and at
//! most 64 payload values. Serialization keeps the previous flat array format.
use crate::{Runtime, Value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{cell::Cell, ops::Index, sync::Arc};
thread_local! {static WORK:Cell<(usize,usize)>=const {Cell::new((0,0))};}
pub fn storage_work() -> (usize, usize) {
    WORK.with(Cell::get)
}
#[cfg(test)]
fn reset_work() {
    WORK.with(|w| w.set((0, 0)));
}
#[derive(Debug)]
enum Kind {
    Leaf(Vec<Arc<Value>>),
    Branch(Option<Arc<Node>>, Option<Arc<Node>>),
}
#[derive(Debug)]
struct Node {
    kind: Kind,
    bytes: usize,
}
impl Clone for Node {
    fn clone(&self) -> Self {
        WORK.with(|w| {
            let (n, v) = w.get();
            w.set((
                n + 1,
                v + if let Kind::Leaf(xs) = &self.kind {
                    xs.len()
                } else {
                    0
                },
            ));
        });
        Self {
            bytes: self.bytes,
            kind: match &self.kind {
                Kind::Leaf(xs) => Kind::Leaf(xs.clone()),
                Kind::Branch(a, b) => Kind::Branch(a.clone(), b.clone()),
            },
        }
    }
}
#[derive(Clone, Default)]
pub struct PagedValues {
    root: Option<Arc<Node>>,
    len: usize,
    height: usize,
}
impl PagedValues {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn logical_bytes(&self) -> usize {
        self.root.as_ref().map_or(0, |n| n.bytes)
    }
    pub fn get(&self, index: usize) -> Option<&Value> {
        if index >= self.len {
            return None;
        }
        let mut n = self.root.as_deref()?;
        let mut h = self.height;
        let mut i = index;
        loop {
            match &n.kind {
                Kind::Leaf(xs) => return xs.get(i).map(Arc::as_ref),
                Kind::Branch(a, b) => {
                    let half = 64usize << (h - 1);
                    n = if i < half {
                        a.as_deref()?
                    } else {
                        i -= half;
                        b.as_deref()?
                    };
                    h -= 1;
                }
            }
        }
    }
    fn write(
        node: &mut Option<Arc<Node>>,
        height: usize,
        index: usize,
        value: Option<Value>,
    ) -> Option<Value> {
        let n = Arc::make_mut(node.get_or_insert_with(|| {
            Arc::new(Node {
                kind: if height == 0 {
                    Kind::Leaf(Vec::new())
                } else {
                    Kind::Branch(None, None)
                },
                bytes: 0,
            })
        }));
        let old = match &mut n.kind {
            Kind::Leaf(xs) => {
                let old = if index < xs.len() {
                    Some(xs[index].as_ref().clone())
                } else {
                    None
                };
                match value {
                    Some(v) => {
                        if index == xs.len() {
                            xs.push(Arc::new(v));
                        } else {
                            xs[index] = Arc::new(v);
                        }
                    }
                    None => {
                        xs.pop();
                    }
                };
                n.bytes = xs.iter().map(|v| Runtime::value_bytes(v)).sum();
                old
            }
            Kind::Branch(a, b) => {
                let half = 64usize << (height - 1);
                let old = if index < half {
                    Self::write(a, height - 1, index, value)
                } else {
                    Self::write(b, height - 1, index - half, value)
                };
                n.bytes = a
                    .as_ref()
                    .map_or(0, |n| n.bytes)
                    .saturating_add(b.as_ref().map_or(0, |n| n.bytes));
                old
            }
        };
        old
    }
    pub fn push(&mut self, value: Value) {
        if self.len == 64usize << self.height {
            self.root = Some(Arc::new(Node {
                bytes: self.logical_bytes(),
                kind: Kind::Branch(self.root.take(), None),
            }));
            self.height += 1;
        }
        Self::write(&mut self.root, self.height, self.len, Some(value));
        self.len += 1;
    }
    pub fn set(&mut self, index: usize, value: Value) -> Value {
        assert!(index < self.len);
        Self::write(&mut self.root, self.height, index, Some(value)).unwrap()
    }
    pub fn pop(&mut self) -> Option<Value> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Self::write(&mut self.root, self.height, self.len, None)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Value> + DoubleEndedIterator {
        (0..self.len).map(|i| self.get(i).unwrap())
    }
}
impl Index<usize> for PagedValues {
    type Output = Value;
    fn index(&self, i: usize) -> &Value {
        self.get(i).expect("paged index")
    }
}
impl From<Vec<Value>> for PagedValues {
    fn from(xs: Vec<Value>) -> Self {
        xs.into_iter().collect()
    }
}
impl FromIterator<Value> for PagedValues {
    fn from_iter<T: IntoIterator<Item = Value>>(xs: T) -> Self {
        let mut out = Self::default();
        for x in xs {
            out.push(x);
        }
        out
    }
}
impl IntoIterator for PagedValues {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter().cloned().collect::<Vec<_>>().into_iter()
    }
}
impl PartialEq for PagedValues {
    fn eq(&self, b: &Self) -> bool {
        self.len == b.len && self.iter().eq(b.iter())
    }
}
impl Eq for PagedValues {}
impl Serialize for PagedValues {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}
impl<'de> Deserialize<'de> for PagedValues {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Vec::<Value>::deserialize(d)?.into())
    }
}
/// Sparse persistent heap; collection removes unreachable IDs without renumbering.
#[derive(Clone, Default)]
pub struct HeapStore(crate::PersistentMap);
impl HeapStore {
    fn key(id: u64) -> crate::MapKey {
        crate::MapKey::Bytes(id.to_be_bytes().to_vec())
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn logical_bytes(&self) -> usize {
        self.0
            .logical_bytes()
            .saturating_sub(self.len().saturating_mul(8))
    }
    pub fn get(&self, id: &u64) -> Option<&Value> {
        self.0.get(&Self::key(*id))
    }
    pub fn contains_key(&self, id: &u64) -> bool {
        self.get(id).is_some()
    }
    pub fn insert(&mut self, id: u64, value: Value) {
        assert!(id != 0);
        self.0.set(Self::key(id), value);
    }
    pub fn remove(&mut self, id: u64) {
        self.0.delete(&Self::key(id));
    }
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.0.values()
    }
    pub fn iter(&self) -> impl Iterator<Item = (u64, &Value)> {
        self.0.iter().map(|(key, value)| {
            let crate::MapKey::Bytes(bytes) = key else {
                unreachable!()
            };
            (
                u64::from_be_bytes(bytes.as_slice().try_into().unwrap()),
                value,
            )
        })
    }
}
impl Serialize for HeapStore {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.iter())
    }
}
impl<'de> Deserialize<'de> for HeapStore {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let xs = std::collections::BTreeMap::<u64, Value>::deserialize(d)?;
        let mut out = Self::default();
        for (id, v) in xs {
            if id == 0 {
                return Err(serde::de::Error::custom("zero heap ID"));
            }
            out.insert(id, v);
        }
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paged_updates_share_history_and_have_bounded_copy_cost() {
        for n in [4096, 8192, 16384] {
            let mut xs: PagedValues = (0..n).map(|i| Value::Int(i as i64)).collect();
            let saved = xs.clone();
            reset_work();
            for i in 0..n {
                xs.set(i, Value::Int(-1));
            }
            let (nodes, values) = storage_work();
            assert!(nodes <= n * 10, "{n}: {nodes}");
            assert!(values <= n * 64, "{n}: {values}");
            assert_eq!(saved[n - 1], Value::Int(n as i64 - 1));
            assert_eq!(xs[n - 1], Value::Int(-1));
            assert_eq!(xs.logical_bytes(), n * 8);
            assert_eq!(
                serde_json::from_str::<PagedValues>(&serde_json::to_string(&xs).unwrap()).unwrap(),
                xs
            );
        }
    }
    #[test]
    fn debug_format_uses_logical_values_after_shrinking() {
        let mut xs = PagedValues::default();
        for i in 0..65 {
            xs.push(Value::Int(i));
        }
        for _ in 0..65 {
            xs.pop();
        }
        assert_eq!(format!("{xs:?}"), "[]");
        let mut h = HeapStore::default();
        h.insert(1, Value::TypedList("Int".into(), xs));
        assert_eq!(format!("{h:?}"), "{1: TypedList(\"Int\", [])}");
    }
    #[test]
    fn heap_roots_and_pop_keep_old_payloads() {
        let mut h = HeapStore::default();
        for i in 1..=200 {
            h.insert(i, Value::Int(i as i64));
        }
        let saved = h.clone();
        h.insert(100, Value::Int(-1));
        assert_eq!(saved.get(&100), Some(&Value::Int(100)));
        assert_eq!(h.get(&100), Some(&Value::Int(-1)));
        let mut xs = PagedValues::default();
        for i in 0..130 {
            xs.push(Value::Int(i));
        }
        let saved = xs.clone();
        for i in (0..130).rev() {
            assert_eq!(xs.pop(), Some(Value::Int(i)));
        }
        assert_eq!(xs.pop(), None);
        assert_eq!(saved.len(), 130);
    }
}

impl std::fmt::Debug for PagedValues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl std::fmt::Debug for HeapStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
