//! Runtime-scoped admission for immutable shared payload owners.
//! Registrations retain only weak ledger references, never payload owners.
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex, Weak,
};
static NEXT: AtomicUsize = AtomicUsize::new(1);
struct Ledger {
    id: usize,
    bytes: AtomicUsize,
    allocated: AtomicUsize,
    generation: AtomicUsize,
    visits: AtomicUsize,
}
impl Default for Ledger {
    fn default() -> Self {
        Self {
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
#[derive(Default)]
pub(crate) struct Registration {
    first: AtomicUsize,
    ledgers: Mutex<Vec<(Weak<Ledger>, usize)>>,
}
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
        for (ledger, bytes) in self.ledgers.get_mut().unwrap().drain(..) {
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
}
