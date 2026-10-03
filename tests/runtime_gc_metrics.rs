use rewind::{Runtime, Value};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
#[test]
fn optional_gc_metrics_preserve_failure_atomicity_and_are_not_rewound() {
    let root = std::env::temp_dir().join(format!(
        "rewind-gc-profile-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let mut runtime = Runtime::new(&root).unwrap();
    assert!(runtime.gc_metrics().is_none());
    let alive = runtime.alloc(Value::Text("alive".into())).unwrap();
    let dead = runtime.alloc(Value::Text("dead".into())).unwrap();
    runtime.commit("saved").unwrap();
    let digest = runtime.state_digest().unwrap();
    runtime.enable_gc_profiling();
    assert_eq!(runtime.state_digest().unwrap(), digest);
    let root_value = Value::HeapRef(alive);
    assert!(runtime.collect_heap_refs(&[&root_value], 0).is_err());
    assert!(runtime.heap_get(alive).is_some());
    assert!(runtime.heap_get(dead).is_some());
    assert_eq!(runtime.state_digest().unwrap(), digest);
    assert_eq!(runtime.gc_metrics().unwrap().failed, 1);
    let (reclaimed, _) = runtime.collect_heap_refs(&[&root_value], 100).unwrap();
    assert_eq!(reclaimed, 1);
    assert!(runtime.heap_get(alive).is_some());
    assert!(runtime.heap_get(dead).is_none());
    assert_eq!(runtime.gc_metrics().unwrap().completed, 1);
    runtime.revert("saved").unwrap();
    assert!(runtime.heap_get(dead).is_some());
    assert_eq!(runtime.gc_metrics().unwrap().attempts, 2);
    assert_eq!(runtime.gc_metrics().unwrap().reclaimed_objects, 1);
    let (reclaimed, _) = runtime.collect_heap_refs(&[&root_value], 100).unwrap();
    assert_eq!(reclaimed, 1);
    assert_eq!(runtime.gc_metrics().unwrap().completed, 2);
    assert_eq!(runtime.gc_metrics().unwrap().reclaimed_objects, 2);
    drop(runtime);
    fs::remove_dir_all(root).unwrap();
}
