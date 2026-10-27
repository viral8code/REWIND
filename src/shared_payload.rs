//! Runtime-scoped admission for immutable shared payload owners.
//! Registrations retain only weak ledger references, never payload owners.
use std::collections::HashMap;
/// Owner keys are `Arc` addresses, so a multiplicative mix of the address is enough; SipHash costs
/// more than the rest of a registration. Iteration order is never observed.
#[derive(Default, Clone, Copy)]
struct PtrHasher(u64);
impl std::hash::Hasher for PtrHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_usize(&mut self, i: usize) {
        let x = ((i as u64) >> 4).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 = x ^ (x >> 32);
    }
}
type OwnerMap<T> = HashMap<usize, OwnerRecord<T>, std::hash::BuildHasherDefault<PtrHasher>>;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex, Weak,
};
/// Index tables at or below this capacity are kept after a prune; shrinking them costs more than it saves.
const SHRINK_FLOOR: usize = 8192;
static NEXT: AtomicUsize = AtomicUsize::new(1);
struct OwnerRecord<T> {
    owner: Weak<T>,
    charge: usize,
}
struct Ledger {
    includes_bytes: bool,
    includes_containers: bool,
    value_owners: Mutex<OwnerMap<crate::Value>>,
    key_owners: Mutex<OwnerMap<crate::MapKey>>,
    byte_owners: Mutex<OwnerMap<Vec<u8>>>,
    id: usize,
    bytes: AtomicUsize,
    allocated: AtomicUsize,
    generation: AtomicUsize,
    visits: AtomicUsize,
}
impl Default for Ledger {
    fn default() -> Self {
        Self {
            includes_bytes: false,
            includes_containers: false,
            value_owners: Mutex::new(HashMap::default()),
            key_owners: Mutex::new(HashMap::default()),
            byte_owners: Mutex::new(HashMap::default()),
            id: NEXT
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
                .expect("shared payload ledger ID exhausted"),
            bytes: AtomicUsize::new(0),
            allocated: AtomicUsize::new(0),
            generation: AtomicUsize::new(0),
            visits: AtomicUsize::new(0),
        }
    }
}
#[derive(Clone, Default)]
pub(crate) struct Accounting(Arc<Ledger>);
impl Accounting {
    pub fn with_containers() -> Self {
        Self(Arc::new(Ledger {
            includes_bytes: true,
            includes_containers: true,
            ..Ledger::default()
        }))
    }
    pub fn identity(&self) -> usize {
        self.0.id
    }
    pub fn includes_containers(&self) -> bool {
        self.0.includes_containers
    }
    pub fn register_value(
        &self,
        value: &Arc<crate::Value>,
        charge: impl FnOnce() -> usize,
    ) -> bool {
        self.register_owner(&self.0.value_owners, value, charge)
    }
    pub fn register_key(&self, value: &Arc<crate::MapKey>, charge: impl FnOnce() -> usize) -> bool {
        self.register_owner(&self.0.key_owners, value, charge)
    }
    fn register_owner<T>(
        &self,
        index: &Mutex<OwnerMap<T>>,
        value: &Arc<T>,
        charge: impl FnOnce() -> usize,
    ) -> bool {
        if !self.includes_containers() {
            return false;
        }
        let key = Arc::as_ptr(value) as usize;
        let owner = Arc::downgrade(value);
        let mut entries = index.lock().unwrap();
        if entries
            .get(&key)
            .is_some_and(|v| Weak::ptr_eq(&v.owner, &owner))
        {
            return false;
        }
        if let Some(old) = entries.remove(&key) {
            self.0.bytes.fetch_sub(old.charge, Ordering::Relaxed);
        }
        let charge = charge();
        entries.insert(key, OwnerRecord { owner, charge });
        self.0.bytes.fetch_add(charge, Ordering::Relaxed);
        self.0
            .allocated
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_add(charge))
            })
            .unwrap();
        self.0.generation.fetch_add(1, Ordering::Relaxed);
        self.0.visits.fetch_add(1, Ordering::Relaxed);
        true
    }
    fn prune_index<T>(&self, index: &Mutex<OwnerMap<T>>) -> usize {
        let mut entries = index.lock().unwrap();
        let mut removed = 0usize;
        entries.retain(|_, v| {
            if v.owner.strong_count() == 0 {
                removed = removed.saturating_add(v.charge);
                false
            } else {
                true
            }
        });
        if entries.capacity() > SHRINK_FLOOR && entries.len() < entries.capacity() / 4 {
            entries.shrink_to_fit();
        }
        if removed != 0 {
            self.0.bytes.fetch_sub(removed, Ordering::Relaxed);
        }
        removed
    }
    pub fn with_bytes() -> Self {
        Self(Arc::new(Ledger {
            includes_bytes: true,
            ..Ledger::default()
        }))
    }
    pub fn includes_bytes(&self) -> bool {
        self.0.includes_bytes
    }
    /// Weak entries do not retain the buffer. Safe Arc mutation dissociates a
    /// weak owner, so a registered identity cannot silently grow its capacity.
    pub fn register_bytes(&self, value: &Arc<Vec<u8>>) {
        if !self.includes_bytes() {
            return;
        }
        let key = Arc::as_ptr(value) as usize;
        let owner = Arc::downgrade(value);
        let mut entries = self.0.byte_owners.lock().unwrap();
        if entries
            .get(&key)
            .is_some_and(|entry| Weak::ptr_eq(&entry.owner, &owner))
        {
            return;
        }
        // Address reuse is impossible while the old Weak exists, but handle
        // replacement conservatively rather than relying on allocator details.
        if let Some(old) = entries.remove(&key) {
            self.0.bytes.fetch_sub(old.charge, Ordering::Relaxed);
        }
        let charge = value.capacity().saturating_add(256);
        entries.insert(key, OwnerRecord { owner, charge });
        self.0.bytes.fetch_add(charge, Ordering::Relaxed);
        self.0
            .allocated
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_add(charge))
            })
            .unwrap();
        self.0.generation.fetch_add(1, Ordering::Relaxed);
        self.0.visits.fetch_add(1, Ordering::Relaxed);
    }
    /// Called at collection, observation, or budget pressure, not every opcode.
    pub fn prune_bytes(&self) -> usize {
        if !self.includes_bytes() {
            return 0;
        }
        let mut entries = self.0.byte_owners.lock().unwrap();
        let mut removed = 0usize;
        entries.retain(|_, entry| {
            if entry.owner.strong_count() == 0 {
                removed = removed.saturating_add(entry.charge);
                false
            } else {
                true
            }
        });
        if entries.capacity() > SHRINK_FLOOR && entries.len() < entries.capacity() / 4 {
            entries.shrink_to_fit();
        }
        if removed != 0 {
            self.0.bytes.fetch_sub(removed, Ordering::Relaxed);
        }
        drop(entries);
        removed
            .saturating_add(self.prune_index(&self.0.value_owners))
            .saturating_add(self.prune_index(&self.0.key_owners))
    }

    pub fn bytes(&self) -> usize {
        self.0.bytes.load(Ordering::Relaxed)
    }
    pub fn allocated_bytes(&self) -> usize {
        self.0.allocated.load(Ordering::Relaxed)
    }
    pub fn generation(&self) -> usize {
        self.0.generation.load(Ordering::Relaxed)
    }
    pub fn visits(&self) -> usize {
        self.0.visits.load(Ordering::Relaxed)
    }
}
type LedgerRef = (Weak<Ledger>, usize);
/// The first registration is stored inline; most payloads are registered with one ledger only.
/// Order is preserved: `first` is the oldest live registration.
#[derive(Default)]
struct Owners {
    first: Option<LedgerRef>,
    rest: Option<Box<Vec<LedgerRef>>>,
}
impl Owners {
    fn retain(&mut self, f: impl Fn(&LedgerRef) -> bool) {
        if self.first.as_ref().is_some_and(|x| !f(x)) {
            self.first = None;
        }
        if let Some(rest) = self.rest.as_mut() {
            rest.retain(|x| f(x));
            if self.first.is_none() && !rest.is_empty() {
                self.first = Some(rest.remove(0));
            }
            if rest.is_empty() {
                self.rest = None;
            }
        }
    }
    fn iter(&self) -> impl Iterator<Item = &LedgerRef> {
        self.first
            .iter()
            .chain(self.rest.iter().flat_map(|r| r.iter()))
    }
    fn push(&mut self, x: LedgerRef) {
        if self.first.is_none() {
            self.first = Some(x);
        } else {
            self.rest.get_or_insert_with(Default::default).push(x);
        }
    }
    #[cfg(test)]
    fn len(&self) -> usize {
        usize::from(self.first.is_some()) + self.rest.as_ref().map_or(0, |r| r.len())
    }
    fn take_all(&mut self) -> impl Iterator<Item = LedgerRef> {
        self.first
            .take()
            .into_iter()
            .chain(self.rest.take().into_iter().flat_map(|r| (*r).into_iter()))
    }
}
#[derive(Default)]
pub(crate) struct Registration {
    first: AtomicUsize,
    ledgers: Mutex<Owners>,
}
const _: () = assert!(std::mem::size_of::<Registration>() == 40);
impl Registration {
    /// Zero-byte registrations memoize a persistent container subtree visit.
    pub fn register(&self, accounting: &Accounting, bytes: usize) -> bool {
        if self.first.load(Ordering::Acquire) == accounting.0.id {
            return false;
        }
        let mut owners = self.ledgers.lock().unwrap();
        owners.retain(|(v, _)| v.strong_count() > 0);
        let candidate = Arc::downgrade(&accounting.0);
        if owners.iter().any(|(v, _)| Weak::ptr_eq(v, &candidate)) {
            return false;
        }
        accounting.0.visits.fetch_add(1, Ordering::Relaxed);
        owners.push((candidate, bytes));
        if bytes > 0 {
            accounting.0.bytes.fetch_add(bytes, Ordering::Relaxed);
            accounting
                .0
                .allocated
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                    Some(v.saturating_add(bytes))
                })
                .unwrap();
            accounting.0.generation.fetch_add(1, Ordering::Relaxed);
        }
        self.first.store(
            owners.iter().find_map(|(v, _)| v.upgrade()).unwrap().id,
            Ordering::Release,
        );
        true
    }
    pub fn clear(&mut self) {
        for (ledger, bytes) in self.ledgers.get_mut().unwrap().take_all() {
            if let Some(ledger) = ledger.upgrade() {
                let old = ledger.bytes.fetch_sub(bytes, Ordering::Relaxed);
                debug_assert!(old >= bytes);
            }
        }
        self.first.store(0, Ordering::Release);
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owners_are_charged_once_per_runtime_and_weak_registrations_release() {
        let a = Accounting::default();
        let b = Accounting::default();
        let mut r = Registration::default();
        assert!(r.register(&a, 123));
        assert!(!r.register(&a, 123));
        assert!(r.register(&b, 123));
        assert_eq!(a.bytes(), 123);
        assert_eq!(b.bytes(), 123);
        assert_eq!(a.visits(), 1);
        r.clear();
        assert_eq!(a.bytes(), 0);
        assert_eq!(b.bytes(), 0);
        assert!(r.register(&a, 456));
        assert_eq!(a.allocated_bytes(), 579);
        drop(a);
        assert!(r.register(&b, 456));
        assert_eq!(b.bytes(), 456);
        assert_eq!(r.ledgers.lock().unwrap().len(), 1);
        drop(r);
        assert_eq!(b.bytes(), 0);
    }
    #[test]
    fn byte_aliases_charge_capacity_once_and_weak_entries_do_not_retain_payload() {
        let a = Accounting::with_bytes();
        let b = Accounting::with_bytes();
        let mut data = Vec::with_capacity(8192);
        data.extend_from_slice(&[7; 16]);
        let owner = Arc::new(data);
        let weak = Arc::downgrade(&owner);
        a.register_bytes(&owner);
        a.register_bytes(&owner.clone());
        b.register_bytes(&owner);
        assert_eq!(a.bytes(), 8192 + 256);
        assert_eq!(a.visits(), 1);
        assert_eq!(b.bytes(), a.bytes());
        drop(owner);
        assert!(weak.upgrade().is_none());
        assert_eq!(a.prune_bytes(), 8192 + 256);
        assert_eq!(a.bytes(), 0);
        assert_eq!(b.prune_bytes(), 8192 + 256);
        assert!(a.0.byte_owners.lock().unwrap().is_empty());
    }
    #[test]
    fn prune_shrinks_only_large_sparse_indices() {
        let a = Accounting::with_bytes();
        let owners: Vec<_> = (0..20_000).map(|i| Arc::new(vec![i as u8; 1])).collect();
        for o in &owners {
            a.register_bytes(o);
        }
        let big = a.0.byte_owners.lock().unwrap().capacity();
        assert!(big > SHRINK_FLOOR);
        let keep = owners[0].clone();
        drop(owners);
        a.prune_bytes();
        let map = a.0.byte_owners.lock().unwrap();
        assert_eq!(map.len(), 1);
        assert!(map.capacity() < big);
        drop(map);
        // A small table is left alone, so the next registrations do not reallocate it.
        let small = Accounting::with_bytes();
        let tmp: Vec<_> = (0..64).map(|i| Arc::new(vec![i as u8; 1])).collect();
        for o in &tmp {
            small.register_bytes(o);
        }
        let cap = small.0.byte_owners.lock().unwrap().capacity();
        drop(tmp);
        small.prune_bytes();
        assert_eq!(small.0.byte_owners.lock().unwrap().capacity(), cap);
        drop(keep);
    }
    #[test]
    fn registration_keeps_order_across_the_inline_first_slot() {
        let r = Registration::default();
        let (a, b, c) = (
            Accounting::with_bytes(),
            Accounting::with_bytes(),
            Accounting::with_bytes(),
        );
        assert!(r.register(&a, 10));
        assert!(r.register(&b, 20));
        assert!(r.register(&c, 30));
        assert_eq!(r.ledgers.lock().unwrap().len(), 3);
        assert_eq!(r.first.load(Ordering::Acquire), a.0.id);
        assert!(!r.register(&a, 10));
        let b_id = b.0.id;
        drop(a);
        assert!(!r.register(&b, 20));
        assert_eq!(r.ledgers.lock().unwrap().len(), 2);
        // `a` died, so `b` moved from `rest` into the inline slot and is now the oldest.
        assert!(r
            .ledgers
            .lock()
            .unwrap()
            .first
            .as_ref()
            .unwrap()
            .0
            .upgrade()
            .is_some_and(|l| l.id == b_id));
        drop(r);
        assert_eq!(b.bytes(), 0);
        assert_eq!(c.bytes(), 0);
    }
    #[test]
    fn byte_cow_and_independent_equal_buffers_have_distinct_admission() {
        let a = Accounting::with_bytes();
        let mut value = Arc::new(vec![1; 4096]);
        a.register_bytes(&value);
        let old = value.clone();
        Arc::make_mut(&mut value).resize(8192, 2);
        a.register_bytes(&value);
        let equal = Arc::new(old.as_ref().clone());
        a.register_bytes(&equal);
        assert_eq!(
            a.bytes(),
            old.capacity() + value.capacity() + equal.capacity() + 3 * 256
        );
        drop(old);
        drop(equal);
        a.prune_bytes();
        assert_eq!(a.bytes(), value.capacity() + 256);
        drop(value);
        a.prune_bytes();
        assert_eq!(a.bytes(), 0);
    }
    #[test]
    fn empty_bytes_charge_metadata_and_text_only_mode_is_unchanged() {
        let a = Accounting::with_bytes();
        let old = Accounting::default();
        let bytes = Arc::new(Vec::new());
        a.register_bytes(&bytes);
        old.register_bytes(&bytes);
        assert_eq!(a.bytes(), 256);
        assert_eq!(old.bytes(), 0);
    }
    #[test]
    fn container_indices_are_lazy_distinct_and_do_not_retain_values_or_keys() {
        let a = Accounting::with_containers();
        let b = Accounting::with_containers();
        let v = Arc::new(crate::Value::Int(7));
        let k = Arc::new(crate::MapKey::Text("key".into()));
        let weak = Arc::downgrade(&v);
        assert!(a.register_value(&v, || 123));
        assert!(!a.register_value(&v, || panic!("duplicate must not compute charge")));
        assert!(b.register_value(&v, || 123));
        assert!(a.register_key(&k, || 456));
        assert_eq!(a.bytes(), 579);
        assert_eq!(b.bytes(), 123);
        drop(v);
        drop(k);
        assert!(weak.upgrade().is_none());
        assert_eq!(a.prune_bytes(), 579);
        assert_eq!(b.prune_bytes(), 123);
        assert_eq!(a.bytes(), 0);
        assert!(a.0.value_owners.lock().unwrap().is_empty());
        assert!(a.0.key_owners.lock().unwrap().is_empty());
    }
    #[test]
    fn inline_names_short_text_and_unused_vector_capacity_are_admitted() {
        let a = Accounting::with_containers();
        let mut name = String::with_capacity(8192);
        name.push('T');
        let named = Arc::new(crate::Value::TypedList(
            name,
            crate::storage::PagedValues::default(),
        ));
        crate::Runtime::register_owned_value(&named, &a);
        assert!(a.bytes() >= 8192 + std::mem::size_of::<crate::Value>() + 128);
        let mut text = String::with_capacity(4096);
        text.push('x');
        let text = Arc::new(crate::Value::Text(text.into()));
        crate::Runtime::register_owned_value(&text, &a);
        let empty = Arc::new(crate::Value::List(Vec::with_capacity(4096)));
        crate::Runtime::register_owned_value(&empty, &a);
        assert!(a.bytes() >= 8192 + 4096 + 4096 * std::mem::size_of::<crate::Value>());
        drop(named);
        drop(text);
        drop(empty);
        a.prune_bytes();
        assert_eq!(a.bytes(), 0);
    }
}
