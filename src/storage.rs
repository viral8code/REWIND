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
enum Kind {
    Leaf(Vec<Arc<Value>>),
    Branch(Option<Arc<Node>>, Option<Arc<Node>>),
}
struct Node {
    kind: Kind,
    bytes: usize,
    allocation_bytes: usize,
    numeric_bytes: usize,
    shared_bytes: usize,
    byte_payload: (usize, usize),
    shared_registered: crate::shared_payload::Registration,
    numeric_registered: crate::numeric::RegistrationMemo,
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
            allocation_bytes: self.allocation_bytes,
            numeric_bytes: self.numeric_bytes,
            shared_bytes: self.shared_bytes,
            byte_payload: self.byte_payload,
            shared_registered: Default::default(),
            numeric_registered: Default::default(),
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
    pub fn allocation_bytes(&self) -> usize {
        self.root.as_ref().map_or(0, |n| n.allocation_bytes)
    }
    pub fn logical_bytes(&self) -> usize {
        self.root.as_ref().map_or(0, |n| n.bytes)
    }
    pub(crate) fn numeric_bytes(&self) -> usize {
        self.root.as_ref().map_or(0, |n| n.numeric_bytes)
    }
    pub(crate) fn register_numerics(&self, accounting: &crate::numeric::Accounting) {
        fn visit(node: &Node, accounting: &crate::numeric::Accounting) {
            if node.numeric_bytes == 0 || !node.numeric_registered.mark(accounting) {
                return;
            }
            match &node.kind {
                Kind::Leaf(values) => {
                    for value in values {
                        Runtime::register_numeric_value(value, accounting);
                    }
                }
                Kind::Branch(left, right) => {
                    for child in [left, right].into_iter().flatten() {
                        visit(child, accounting);
                    }
                }
            }
        }
        if let Some(root) = &self.root {
            visit(root, accounting);
        }
    }
    pub(crate) fn byte_payload_info(&self) -> (usize, usize) {
        self.root.as_ref().map_or((0, 0), |n| n.byte_payload)
    }
    pub(crate) fn shared_bytes(&self) -> usize {
        self.root.as_ref().map_or(0, |n| n.shared_bytes)
    }
    pub(crate) fn register_shared_payloads(&self, accounting: &crate::shared_payload::Accounting) {
        fn visit(node: &Node, accounting: &crate::shared_payload::Accounting) {
            if (!accounting.includes_containers()
                && node.shared_bytes == 0
                && (!accounting.includes_bytes() || node.byte_payload.1 == 0))
                || !node.shared_registered.register(
                    accounting,
                    if accounting.includes_containers() {
                        std::mem::size_of::<Node>()
                            + 256
                            + match &node.kind {
                                Kind::Leaf(v) => v.capacity() * std::mem::size_of::<Arc<Value>>(),
                                _ => 0,
                            }
                    } else {
                        0
                    },
                )
            {
                return;
            }
            match &node.kind {
                Kind::Leaf(values) => {
                    for value in values {
                        Runtime::register_owned_value(value, accounting);
                    }
                }
                Kind::Branch(left, right) => {
                    for child in [left, right].into_iter().flatten() {
                        visit(child, accounting);
                    }
                }
            }
        }
        if let Some(root) = &self.root {
            visit(root, accounting);
        }
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
                allocation_bytes: std::mem::size_of::<Node>() + 256,
                numeric_bytes: 0,
                shared_bytes: 0,
                byte_payload: (0, 0),
                shared_registered: Default::default(),
                numeric_registered: Default::default(),
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
                n.numeric_bytes = xs.iter().map(|v| Runtime::numeric_payload_bytes(v)).sum();
                n.shared_bytes = xs.iter().map(|v| Runtime::shared_payload_bytes(v)).sum();
                n.byte_payload = xs.iter().fold((0usize, 0usize), |n, v| {
                    let b = Runtime::byte_payload_info(v);
                    (n.0.saturating_add(b.0), n.1.saturating_add(b.1))
                });
                n.allocation_bytes = xs.iter().fold(std::mem::size_of::<Node>() + 256, |n, v| {
                    n.saturating_add(Runtime::allocation_bytes(v))
                        .saturating_add(32)
                });
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
                n.numeric_bytes = a
                    .as_ref()
                    .map_or(0, |n| n.numeric_bytes)
                    .saturating_add(b.as_ref().map_or(0, |n| n.numeric_bytes));
                n.shared_bytes = a
                    .as_ref()
                    .map_or(0, |n| n.shared_bytes)
                    .saturating_add(b.as_ref().map_or(0, |n| n.shared_bytes));
                let av = a.as_ref().map_or((0usize, 0usize), |n| n.byte_payload);
                let bv = b.as_ref().map_or((0usize, 0usize), |n| n.byte_payload);
                n.byte_payload = (av.0.saturating_add(bv.0), av.1.saturating_add(bv.1));
                n.allocation_bytes = (std::mem::size_of::<Node>() + 256)
                    .saturating_add(a.as_ref().map_or(0, |n| n.allocation_bytes))
                    .saturating_add(b.as_ref().map_or(0, |n| n.allocation_bytes));
                old
            }
        };
        n.numeric_registered = Default::default();
        n.shared_registered = Default::default();
        old
    }
    pub fn push(&mut self, value: Value) {
        if self.len == 64usize << self.height {
            self.root = Some(Arc::new(Node {
                bytes: self.logical_bytes(),
                allocation_bytes: self
                    .allocation_bytes()
                    .saturating_add(std::mem::size_of::<Node>() + 256),
                numeric_bytes: self.numeric_bytes(),
                shared_bytes: self.shared_bytes(),
                byte_payload: self.byte_payload_info(),
                shared_registered: Default::default(),
                numeric_registered: Default::default(),
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
    pub fn allocation_bytes(&self) -> usize {
        self.0.allocation_bytes()
    }
    pub(crate) fn numeric_bytes(&self) -> usize {
        self.0.numeric_bytes()
    }
    pub(crate) fn byte_payload_info(&self) -> (usize, usize) {
        self.0.byte_payload_info()
    }
    pub(crate) fn shared_bytes(&self) -> usize {
        self.0.shared_bytes()
    }
    pub(crate) fn register_shared_payloads(&self, a: &crate::shared_payload::Accounting) {
        self.0.register_shared_payloads(a);
    }
    pub(crate) fn register_numerics(&self, accounting: &crate::numeric::Accounting) {
        self.0.register_numerics(accounting);
    }
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
    #[test]
    fn shared_text_registration_reuses_unchanged_list_pages() {
        let ledger = crate::shared_payload::Accounting::default();
        let text = crate::text_storage::Text::from("a".repeat(4096));
        let mut xs = PagedValues::default();
        for _ in 0..65536 {
            xs.push(Value::Text(text.clone()));
        }
        xs.register_shared_payloads(&ledger);
        let visits = ledger.visits();
        assert_eq!(ledger.bytes(), text.capacity() + 256);
        xs.register_shared_payloads(&ledger);
        assert_eq!(ledger.visits(), visits);
        let old = xs.clone();
        xs.set(32000, Value::Text("small".into()));
        xs.register_shared_payloads(&ledger);
        assert!(ledger.visits() - visits < 64);
        assert_eq!(ledger.bytes(), text.capacity() + 256);
        assert_eq!(old.shared_bytes(), 65536 * 4096);
        assert_eq!(xs.shared_bytes(), 65535 * 4096);
        drop(text);
        drop(xs);
        assert!(ledger.bytes() > 0);
        drop(old);
        assert_eq!(ledger.bytes(), 0);
    }
    #[test]
    fn byte_cache_tracks_empty_payloads_and_changed_pages_without_rescan() {
        let ledger = crate::shared_payload::Accounting::with_bytes();
        let payload = Arc::new(vec![1; 4096]);
        let mut xs = PagedValues::default();
        for _ in 0..65536 {
            xs.push(Value::Bytes(payload.clone()));
        }
        xs.register_shared_payloads(&ledger);
        let visits = ledger.visits();
        assert_eq!(ledger.bytes(), 4096 + 256);
        let saved = xs.clone();
        xs.set(32000, Value::Bytes(Arc::new(Vec::new())));
        xs.register_shared_payloads(&ledger);
        assert!(ledger.visits() - visits < 64);
        assert_eq!(xs.byte_payload_info(), (65535 * 4096, 65536));
        assert_eq!(saved.byte_payload_info(), (65536 * 4096, 65536));
        assert_eq!(ledger.bytes(), 4096 + 512);
        drop(payload);
        drop(xs);
        ledger.prune_bytes();
        assert_eq!(ledger.bytes(), 4096 + 256);
        drop(saved);
        ledger.prune_bytes();
        assert_eq!(ledger.bytes(), 0);
    }
    #[test]
    fn shared_list_native_owners_match_independent_counts_and_preserve_old_pages() {
        let a = crate::shared_payload::Accounting::with_containers();
        let mut xs = PagedValues::default();
        for i in 0..32 {
            xs.push(Value::Int(i));
        }
        xs.register_shared_payloads(&a);
        let node = std::mem::size_of::<Node>() + 256 + 32 * std::mem::size_of::<Arc<Value>>();
        let value = std::mem::size_of::<Value>() + 32 + 8 + 128;
        assert_eq!(a.bytes(), node + 32 * value);
        let saved = xs.clone();
        let removed = xs.set(0, Value::Int(-1));
        drop(removed);
        xs.register_shared_payloads(&a);
        assert_eq!(a.bytes(), 2 * node + 33 * value);
        assert_eq!(saved.get(0), Some(&Value::Int(0)));
        drop(xs);
        a.prune_bytes();
        assert_eq!(a.bytes(), node + 32 * value);
        drop(saved);
        a.prune_bytes();
        assert_eq!(a.bytes(), 0);
    }
    #[test]
    fn scalar_lists_register_only_changed_paths_in_unique_owner_mode() {
        let a = crate::shared_payload::Accounting::with_containers();
        let mut xs = PagedValues::default();
        for i in 0..65536 {
            xs.push(Value::Int(i));
        }
        xs.register_shared_payloads(&a);
        let before = a.visits();
        let bytes = a.bytes();
        let saved = xs.clone();
        xs.set(32768, Value::Int(-1));
        xs.register_shared_payloads(&a);
        assert!(a.visits() - before < 64);
        assert!(a.bytes() - bytes < 32768);
        drop(xs);
        drop(saved);
        a.prune_bytes();
        assert_eq!(a.bytes(), 0);
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
