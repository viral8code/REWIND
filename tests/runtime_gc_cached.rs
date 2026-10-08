use rewind::{map_storage::PersistentMap, storage::PagedValues, MapKey, Runtime, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "rewind-cached-gc-{}-{}",
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
fn scalar_list(n: usize) -> PagedValues {
    (0..n).map(|i| Value::Int(i as i64)).collect()
}

#[test]
fn scalar_subtrees_collect_with_work_independent_of_element_count() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_cached_heap_traversal();
    rt.set_global("list", Value::TypedList("Int".into(), scalar_list(65536)))
        .unwrap();
    let map = (0..8192)
        .map(|i| (MapKey::Int(i), Value::Int(i)))
        .collect::<PersistentMap>();
    rt.set_global("map", Value::Map(map)).unwrap();
    let dead = rt.alloc(Value::Int(7)).unwrap();
    let (reclaimed, work) = rt.collect_heap(&[], 16).unwrap();
    assert_eq!(reclaimed, 1);
    assert!(work <= 16);
    assert!(rt.heap_get(dead).is_none());
}
#[test]
fn sparse_edges_shared_between_many_roots_are_found_without_scanning_scalars() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_cached_heap_traversal();
    let target = rt.alloc(Value::Int(77)).unwrap();
    let dead = rt.alloc(Value::Int(-1)).unwrap();
    let mut list = scalar_list(65536);
    list.set(65000, Value::HeapRef(target));
    let map = (0..8192)
        .map(|i| {
            (
                MapKey::Int(i),
                if i == 7000 {
                    Value::CellRef(target)
                } else {
                    Value::Int(i)
                },
            )
        })
        .collect::<PersistentMap>();
    let roots = (0..128)
        .map(|_| Value::TypedList("Object".into(), list.clone()))
        .chain((0..128).map(|_| Value::Map(map.clone())))
        .collect::<Vec<_>>();
    let (reclaimed, work) = rt.collect_heap(&roots, 1024).unwrap();
    assert_eq!(reclaimed, 1);
    assert!(work < 1024);
    assert_eq!(rt.heap_get(target), Some(&Value::Int(77)));
    assert!(rt.heap_get(dead).is_none());
}
#[test]
fn changed_edges_cycles_captures_and_checkpoint_heaps_remain_independent() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_cached_heap_traversal();
    let a = rt.alloc(Value::Null).unwrap();
    let b = rt.alloc(Value::Null).unwrap();
    let dead = rt.alloc(Value::Null).unwrap();
    rt.heap_set(
        a,
        Value::Closure(
            "capture".into(),
            "Fn".into(),
            BTreeMap::from([("cell".into(), Value::CellRef(b))]),
        ),
    )
    .unwrap();
    rt.heap_set(
        b,
        Value::OrderedMap(
            "Object".into(),
            "Object".into(),
            vec![(
                Value::HeapRef(a),
                Value::Option(Some(Box::new(Value::CellRef(b)))),
            )],
        ),
    )
    .unwrap();
    rt.heap_set(dead, Value::List(vec![Value::HeapRef(dead)]))
        .unwrap();
    let mut values = scalar_list(65536);
    values.set(
        1234,
        Value::Enum(
            "Edge".into(),
            "Some".into(),
            vec![("v".into(), Value::Result(Ok(Box::new(Value::CellRef(a)))))],
        ),
    );
    rt.set_global("root", Value::TypedList("Object".into(), values.clone()))
        .unwrap();
    rt.commit("saved").unwrap();
    assert_eq!(rt.collect_heap(&[], 256).unwrap().0, 1);
    assert!(rt.heap_get(a).is_some());
    assert!(rt.heap_get(b).is_some());
    values.set(1234, Value::Null);
    rt.set_global("root", Value::TypedList("Object".into(), values))
        .unwrap();
    assert_eq!(rt.collect_heap(&[], 16).unwrap().0, 2);
    rt.revert("saved").unwrap();
    assert!(rt.heap_get(a).is_some());
    assert!(rt.heap_get(b).is_some());
    assert!(rt.heap_get(dead).is_some());
    assert_eq!(rt.collect_heap(&[], 256).unwrap().0, 1);
    rt.drop_checkpoint("saved").unwrap();
    rt.set_global("root", Value::Null).unwrap();
    assert_eq!(rt.collect_heap(&[], 16).unwrap().0, 2);
}
#[test]
fn each_collection_follows_current_heap_contents_and_failure_preserves_heap() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_cached_heap_traversal();
    let a = rt.alloc(Value::Int(0)).unwrap();
    let mut values = scalar_list(65536);
    values.set(17, Value::HeapRef(a));
    rt.set_global("root", Value::TypedList("Object".into(), values))
        .unwrap();
    rt.collect_heap(&[], 256).unwrap();
    let b = rt.alloc(Value::Int(3)).unwrap();
    rt.heap_set(a, Value::CellRef(b)).unwrap();
    assert!(rt.collect_heap(&[], 2).is_err());
    assert_eq!(rt.heap_get(a), Some(&Value::CellRef(b)));
    assert_eq!(rt.heap_get(b), Some(&Value::Int(3)));
    assert_eq!(rt.collect_heap(&[], 256).unwrap().0, 0);
    assert!(rt.heap_get(b).is_some());
}
#[test]
fn previous_runtime_traversal_keeps_its_existing_work_contract() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.set_global("root", Value::TypedList("Int".into(), scalar_list(65536)))
        .unwrap();
    let id = rt.alloc(Value::Int(1)).unwrap();
    assert!(rt.collect_heap(&[], 16).is_err());
    assert!(rt.heap_get(id).is_some());
    assert_eq!(rt.collect_heap(&[], 100000).unwrap().0, 1);
}
fn nested_edges(edges: Vec<Value>, kind: usize) -> Value {
    match kind % 7 {
        0 => Value::List(edges),
        1 => Value::TypedList("Object".into(), edges.into()),
        2 => Value::Map(
            edges
                .into_iter()
                .enumerate()
                .map(|(i, v)| (MapKey::Int(i as i64), v))
                .collect(),
        ),
        3 => Value::Struct(
            "Node".into(),
            edges
                .into_iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v))
                .collect(),
        ),
        4 => Value::Closure(
            "graph".into(),
            "Fn".into(),
            edges
                .into_iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v))
                .collect(),
        ),
        5 => Value::Enum(
            "Node".into(),
            "Edges".into(),
            edges
                .into_iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v))
                .collect(),
        ),
        _ => Value::OrderedMap(
            "Object".into(),
            "Object".into(),
            edges.into_iter().map(|v| (v, Value::Null)).collect(),
        ),
    }
}
#[test]
fn sparse_nested_graphs_match_independently_computed_reachability() {
    for seed in 0..12u64 {
        let d = Dir::new();
        let mut rt = Runtime::new(&d.0).unwrap();
        rt.enable_cached_heap_traversal();
        let ids = (0..48)
            .map(|_| rt.alloc(Value::Null).unwrap())
            .collect::<Vec<_>>();
        let mut random = seed + 1;
        let mut adjacency = vec![Vec::new(); ids.len()];
        for i in 0..ids.len() {
            for _ in 0..(i % 3) {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                adjacency[i].push((random as usize) % ids.len());
            }
            let edges = adjacency[i]
                .iter()
                .enumerate()
                .map(|(j, &k)| {
                    if j % 2 == 0 {
                        Value::HeapRef(ids[k])
                    } else {
                        Value::CellRef(ids[k])
                    }
                })
                .collect();
            rt.heap_set(ids[i], nested_edges(edges, i)).unwrap();
        }
        let mut list = scalar_list(4096);
        list.set(3999, Value::HeapRef(ids[seed as usize]));
        let root = Value::TypedList("Object".into(), list);
        let mut expected = BTreeSet::new();
        let mut pending = vec![seed as usize];
        while let Some(i) = pending.pop() {
            if expected.insert(i) {
                pending.extend(&adjacency[i]);
            }
        }
        let (reclaimed, _) = rt.collect_heap(&[root], 2048).unwrap();
        assert_eq!(reclaimed, ids.len() - expected.len());
        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(
                rt.heap_get(id).is_some(),
                expected.contains(&i),
                "seed={seed}, node={i}"
            );
        }
    }
}

#[test]
fn stack_frame_and_serialized_sparse_roots_keep_their_heap_edges() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_cached_heap_traversal();
    let stack = rt.alloc(Value::Int(1)).unwrap();
    let frame = rt.alloc(Value::Int(2)).unwrap();
    let external = rt.alloc(Value::Int(3)).unwrap();
    let mut values = scalar_list(4096);
    values.set(
        4000,
        Value::Option(Some(Box::new(Value::CellRef(external)))),
    );
    let root = Value::TypedList("Object".into(), values);
    let wire = serde_json::to_vec(&root).unwrap();
    let decoded: Value = serde_json::from_slice(&wire).unwrap();
    assert_eq!(decoded, root);
    rt.push_stack(Value::HeapRef(stack)).unwrap();
    rt.push_frame(5).unwrap();
    rt.set_local(
        "captured",
        Value::Result(Err(Box::new(Value::CellRef(frame)))),
    )
    .unwrap();
    assert_eq!(rt.collect_heap_refs(&[&decoded], 256).unwrap().0, 0);
    rt.pop_stack().unwrap();
    rt.pop_frame().unwrap();
    assert_eq!(rt.collect_heap_refs(&[&decoded], 256).unwrap().0, 2);
    assert!(rt.heap_get(external).is_some());
}

#[test]
fn map_delete_and_leaf_pop_update_only_new_snapshot_edges() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_cached_heap_traversal();
    let id = rt.alloc(Value::Int(99)).unwrap();
    let mut list = scalar_list(64);
    list.push(Value::HeapRef(id));
    let mut map = (0..8192)
        .map(|i| (MapKey::Int(i), Value::Int(i)))
        .collect::<PersistentMap>();
    map.insert(MapKey::Int(7000), Value::CellRef(id));
    rt.set_global("list", Value::TypedList("Object".into(), list.clone()))
        .unwrap();
    rt.set_global("map", Value::Map(map.clone())).unwrap();
    rt.commit("old").unwrap();
    list.pop();
    map.delete(&MapKey::Int(7000));
    rt.set_global("list", Value::TypedList("Object".into(), list))
        .unwrap();
    rt.set_global("map", Value::Map(map)).unwrap();
    assert_eq!(rt.collect_heap(&[], 16).unwrap().0, 1);
    assert!(rt.heap_get(id).is_none());
    rt.revert("old").unwrap();
    assert_eq!(rt.heap_get(id), Some(&Value::Int(99)));
    assert_eq!(rt.collect_heap(&[], 256).unwrap().0, 0);
}
