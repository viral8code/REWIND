use rewind::{ResourceBudget, Runtime, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rewind-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn ephemeral_bytes_are_pruned_before_budget_denial_and_checkpoint_owners_survive() {
    let dir = TestDir::new();
    let mut r = Runtime::new(&dir.0).unwrap();
    r.enable_allocation_accounting();
    r.enable_byte_payload_accounting();
    r.set_budget(ResourceBudget {
        history_memory: 512 * 1024,
        ..Default::default()
    })
    .unwrap();
    for _ in 0..128 {
        r.set_global(
            "data",
            Value::Bytes(std::sync::Arc::new(vec![1; 64 * 1024])),
        )
        .unwrap();
        r.set_global("data", Value::Int(0)).unwrap();
    }
    assert_eq!(r.shared_payload_metrics().unwrap()["live_bytes"], 0);
    let bytes = std::sync::Arc::new(vec![2; 64 * 1024]);
    let weak = std::sync::Arc::downgrade(&bytes);
    r.set_global("data", Value::Bytes(bytes)).unwrap();
    r.commit("saved").unwrap();
    r.set_global("data", Value::Int(0)).unwrap();
    r.collect_heap(&[], 100000).unwrap();
    assert!(weak.upgrade().is_some());
    assert_eq!(
        r.shared_payload_metrics().unwrap()["live_bytes"],
        64 * 1024 + 256
    );
    r.drop_checkpoint("saved").unwrap();
    r.collect_heap(&[], 100000).unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(r.shared_payload_metrics().unwrap()["live_bytes"], 0);
}

#[test]
fn independent_equal_bytes_cannot_evade_a_budget_by_replacing_checkpoint_data() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.enable_allocation_accounting();
    runtime.enable_byte_payload_accounting();
    runtime
        .set_budget(ResourceBudget {
            history_memory: 80 * 1024,
            ..Default::default()
        })
        .unwrap();
    let original = std::sync::Arc::new(vec![9; 48 * 1024]);
    runtime
        .set_global("data", Value::Bytes(original.clone()))
        .unwrap();
    runtime.commit("saved").unwrap();
    let rejected = std::sync::Arc::new(original.as_ref().clone());
    let weak = std::sync::Arc::downgrade(&rejected);
    assert!(runtime.set_global("data", Value::Bytes(rejected)).is_err());
    assert!(weak.upgrade().is_none());
    assert_eq!(
        runtime.global("data"),
        Some(&Value::Bytes(original.clone()))
    );
    assert_eq!(
        runtime.shared_payload_metrics().unwrap()["live_bytes"],
        48 * 1024 + 256
    );
    runtime.revert("saved").unwrap();
    assert_eq!(runtime.global("data"), Some(&Value::Bytes(original)));
}
