use rewind::{Runtime, Value};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn runtime() -> (std::path::PathBuf, Runtime) {
    let p = std::env::temp_dir().join(format!(
        "rewind-v099-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    let rt = Runtime::new(&p).unwrap();
    (p, rt)
}
#[test]
fn collector_reclaims_cycles_keeps_external_roots_and_restores_checkpoints() {
    let (p, mut rt) = runtime();
    let root = rt.alloc(Value::Int(10)).unwrap();
    rt.set_global("root", Value::HeapRef(root)).unwrap();
    let a = rt.alloc(Value::Null).unwrap();
    let b = rt.alloc(Value::HeapRef(a)).unwrap();
    rt.heap_set(a, Value::HeapRef(b)).unwrap();
    rt.commit("before").unwrap();
    let external = rt.alloc(Value::Int(20)).unwrap();
    assert_eq!(
        rt.collect_heap(&[Value::HeapRef(external)], 100).unwrap().0,
        2
    );
    assert_eq!(rt.heap_get(root), Some(&Value::Int(10)));
    assert_eq!(rt.heap_get(external), Some(&Value::Int(20)));
    assert!(rt.heap_get(a).is_none());
    rt.revert("before").unwrap();
    assert!(rt.heap_get(a).is_some());
    assert!(rt.heap_get(external).is_none());
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn collection_work_failure_is_atomic_and_sparse_wire_roundtrips() {
    let (p, mut rt) = runtime();
    let a = rt.alloc(Value::Text("a".into())).unwrap();
    let b = rt.alloc(Value::Text("b".into())).unwrap();
    let before = rt.state_digest().unwrap();
    assert!(rt.collect_heap(&[Value::HeapRef(b)], 0).is_err());
    assert_eq!(before, rt.state_digest().unwrap());
    rt.collect_heap(&[Value::HeapRef(b)], 100).unwrap();
    assert!(rt.heap_get(a).is_none());
    let encoded = serde_json::to_vec(rt.state().heap.as_ref()).unwrap();
    let decoded: rewind::storage::HeapStore = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded.get(&b), Some(&Value::Text("b".into())));
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn collection_releases_all_unrooted_storage_at_multiple_sizes() {
    for n in [4096, 8192, 16384] {
        let (p, mut rt) = runtime();
        for _ in 0..n {
            rt.alloc(Value::Text("payload".repeat(8))).unwrap();
        }
        assert_eq!(rt.collect_heap(&[], n).unwrap().0, n);
        assert_eq!(rt.state().heap.len(), 0);
        assert_eq!(rt.state().heap.logical_bytes(), 0);
        fs::remove_dir_all(p).unwrap();
    }
}
