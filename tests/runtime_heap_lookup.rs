use rewind::{storage::HeapStore, Runtime, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "rewind-heap-lookup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn unsigned_heap_ids_misses_updates_and_wire_order_match_independent_map() {
    let ids = [
        1,
        17,
        256,
        65536,
        u64::MAX / 2,
        u64::MAX / 2 + 1,
        u64::MAX - 1,
        u64::MAX,
    ];
    let mut heap = HeapStore::default();
    let mut expected = BTreeMap::new();
    for &id in ids.iter().rev() {
        heap.insert(id, Value::Int(id as i64));
        expected.insert(id, Value::Int(id as i64));
    }
    for id in [0, 2, 16, 255, 65535, u64::MAX - 2] {
        assert!(heap.get(&id).is_none());
        assert!(!heap.contains_key(&id));
    }
    for &id in &ids {
        assert_eq!(heap.get(&id), expected.get(&id));
        assert!(heap.contains_key(&id));
    }
    assert_eq!(
        heap.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        expected.keys().copied().collect::<Vec<_>>()
    );
    let saved = heap.clone();
    heap.insert(u64::MAX, Value::Int(9));
    heap.remove(17);
    expected.insert(u64::MAX, Value::Int(9));
    expected.remove(&17);
    assert_eq!(saved.get(&u64::MAX), Some(&Value::Int(-1)));
    assert!(saved.get(&17).is_some());
    let wire = serde_json::to_string(&heap).unwrap();
    assert_eq!(wire, serde_json::to_string(&expected).unwrap());
    let decoded: HeapStore = serde_json::from_str(&wire).unwrap();
    for &id in &ids {
        assert_eq!(decoded.get(&id), expected.get(&id));
    }
    let zero = serde_json::to_string(&BTreeMap::from([(0u64, Value::Null)])).unwrap();
    assert!(serde_json::from_str::<HeapStore>(&zero).is_err());
}
#[test]
fn heap_owned_large_inline_scalars_are_kept_without_traversing_each_element() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_cached_heap_entries();
    let a = rt
        .alloc(Value::List((0..65536).map(Value::Int).collect()))
        .unwrap();
    let b = rt
        .alloc(Value::OrderedMap(
            "Int".into(),
            "Int".into(),
            (0..65536)
                .map(|i| (Value::Int(i), Value::Int(-i)))
                .collect(),
        ))
        .unwrap();
    let dead = rt.alloc(Value::Int(0)).unwrap();
    rt.set_global("a", Value::HeapRef(a)).unwrap();
    rt.set_global("b", Value::CellRef(b)).unwrap();
    let (reclaimed, work) = rt.collect_heap(&[], 16).unwrap();
    assert_eq!(reclaimed, 1);
    assert!(work <= 16);
    assert!(rt.heap_get(a).is_some());
    assert!(rt.heap_get(b).is_some());
    assert!(rt.heap_get(dead).is_none());
    rt.set_global("a", Value::Null).unwrap();
    rt.set_global("b", Value::Null).unwrap();
    assert_eq!(rt.collect_heap(&[], 16).unwrap().0, 2);
}
#[test]
fn cached_entry_presence_follows_current_heap_mutations_and_restored_snapshots() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_cached_heap_entries();
    let a = rt.alloc(Value::Null).unwrap();
    rt.set_global("a", Value::HeapRef(a)).unwrap();
    rt.commit("before").unwrap();
    let b = rt.alloc(Value::Int(4)).unwrap();
    rt.heap_set(
        a,
        Value::Closure(
            "f".into(),
            "Fn".into(),
            BTreeMap::from([("cell".into(), Value::CellRef(b))]),
        ),
    )
    .unwrap();
    assert_eq!(rt.collect_heap(&[], 32).unwrap().0, 0);
    assert_eq!(rt.heap_get(b), Some(&Value::Int(4)));
    rt.heap_set(a, Value::Int(1)).unwrap();
    assert_eq!(rt.collect_heap(&[], 16).unwrap().0, 1);
    assert!(rt.heap_get(a).is_some());
    rt.revert("before").unwrap();
    assert_eq!(rt.heap_get(a), Some(&Value::Null));
    assert!(rt.heap_get(b).is_none());
    assert_eq!(rt.collect_heap(&[], 16).unwrap().0, 0);
}
#[test]
fn prior_cached_container_mode_retains_its_inline_value_work_contract() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_cached_heap_traversal();
    let a = rt
        .alloc(Value::List((0..65536).map(Value::Int).collect()))
        .unwrap();
    rt.set_global("a", Value::HeapRef(a)).unwrap();
    let before = rt.state_digest().unwrap();
    assert!(rt.collect_heap(&[], 16).is_err());
    assert_eq!(rt.state_digest().unwrap(), before);
    assert!(rt.heap_get(a).is_some());
    assert_eq!(rt.collect_heap(&[], 100000).unwrap().0, 0);
}
