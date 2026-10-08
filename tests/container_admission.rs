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
fn many_scalar_list_checkpoints_admit_unique_native_owners_and_restore_values() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.enable_container_accounting();
    runtime
        .set_budget(ResourceBudget {
            history_memory: 32 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let mut values = rewind::storage::PagedValues::default();
    for i in 0..65536 {
        values.push(Value::Int(i));
    }
    runtime
        .set_global("items", Value::TypedList("Int".into(), values.clone()))
        .unwrap();
    runtime.commit("original").unwrap();
    for i in 0..128 {
        values.set(0, Value::Int(-i));
        runtime
            .set_global("items", Value::TypedList("Int".into(), values.clone()))
            .unwrap();
        runtime.commit(format!("snapshot{i}")).unwrap();
    }
    let bytes = runtime.shared_payload_metrics().unwrap()["live_bytes"]
        .as_u64()
        .unwrap();
    assert!(bytes < 32 * 1024 * 1024);
    runtime.revert("original").unwrap();
    match runtime.global("items").unwrap() {
        Value::TypedList(_, v) => assert_eq!(v.get(0), Some(&Value::Int(0))),
        _ => panic!("list"),
    }
    for i in 0..128 {
        runtime.drop_checkpoint(&format!("snapshot{i}")).unwrap();
    }
    runtime.drop_checkpoint("original").unwrap();
    runtime.set_global("items", Value::Int(0)).unwrap();
    drop(values);
    runtime.collect_heap(&[], 1000000).unwrap();
    assert_eq!(runtime.shared_payload_metrics().unwrap()["live_bytes"], 0);
}
#[test]
fn many_map_checkpoints_share_keys_entries_and_native_value_owners() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.enable_container_accounting();
    runtime
        .set_budget(ResourceBudget {
            history_memory: 16 * 1024 * 1024,
            ..Default::default()
        })
        .unwrap();
    let mut map = rewind::map_storage::PersistentMap::default();
    for i in 0..8192 {
        map.insert(rewind::MapKey::Int(i), Value::Int(i));
    }
    for i in 0..64 {
        map.insert(rewind::MapKey::Int(0), Value::Int(-i));
        runtime.set_global("map", Value::Map(map.clone())).unwrap();
        runtime.commit(format!("map{i}")).unwrap();
    }
    runtime.revert("map0").unwrap();
    match runtime.global("map").unwrap() {
        Value::Map(v) => assert_eq!(v.get(&rewind::MapKey::Int(0)), Some(&Value::Int(0))),
        _ => panic!("map"),
    }
    for i in 0..64 {
        runtime.drop_checkpoint(&format!("map{i}")).unwrap();
    }
    runtime.set_global("map", Value::Int(0)).unwrap();
    drop(map);
    runtime.collect_heap(&[], 1000000).unwrap();
    assert_eq!(runtime.shared_payload_metrics().unwrap()["live_bytes"], 0);
}
