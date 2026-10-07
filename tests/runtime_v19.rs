use rewind::{ResourceBudget, Runtime, Value};
#[test]
fn large_allocations_and_mutation_trigger_collection_and_preserve_history() {
    let root = std::env::temp_dir().join(format!("rewind-v19-gc-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rt = Runtime::new(&root).unwrap();
    let id = rt
        .alloc(Value::Text("x".repeat(4 * 1024 * 1024).into()))
        .unwrap();
    assert!(rt.collection_due());
    rt.commit("keep").unwrap();
    assert_eq!(rt.collect_heap(&[], 100).unwrap().0, 1);
    assert!(!rt.collection_due());
    assert!(rt.heap_get(id).is_none());
    rt.revert("keep").unwrap();
    assert_eq!(
        Runtime::value_bytes(rt.heap_get(id).unwrap()),
        4 * 1024 * 1024
    );
    rt.drop_checkpoint("keep").unwrap();
    rt.collect_heap(&[], 100).unwrap();
    let id = rt.alloc(Value::Text(String::new().into())).unwrap();
    rt.set_global("root", Value::HeapRef(id)).unwrap();
    rt.heap_set(id, Value::Text("y".repeat(4 * 1024 * 1024).into()))
        .unwrap();
    assert!(rt.collection_due());
    assert_eq!(rt.collect_heap(&[], 100).unwrap().0, 0);
    assert!(!rt.collection_due());
    let before = rt.state_digest().unwrap();
    rt.set_budget(ResourceBudget {
        history_memory: 5 * 1024 * 1024,
        ..ResourceBudget::default()
    })
    .unwrap();
    assert!(rt
        .heap_set(id, Value::Text("z".repeat(6 * 1024 * 1024).into()))
        .is_err());
    assert_eq!(before, rt.state_digest().unwrap());
    assert!(!rt.collection_due());
    drop(rt);
    std::fs::remove_dir_all(root).unwrap();
}
