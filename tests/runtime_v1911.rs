use rewind::map_storage::PersistentMap;
use rewind::numeric::{Array, DType};
use rewind::storage::PagedValues;
use rewind::{Error, MapKey, ResourceBudget, Runtime, Value};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn runtime() -> (std::path::PathBuf, Runtime) {
    let path = std::env::temp_dir().join(format!(
        "rewind-numeric-accounting-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    let mut rt = Runtime::new(&path).unwrap();
    rt.enable_allocation_accounting();
    rt.enable_numeric_accounting();
    (path, rt)
}
#[test]
fn numeric_checkpoint_sharing_and_cow_are_charged_incrementally() {
    let (path, mut rt) = runtime();
    // This contract measures a materialized dense buffer; zeros now share pages.
    let initial = Array::from_bits(
        DType::Float64,
        vec![1_000_000],
        std::iter::repeat_n(0, 1_000_000),
    )
    .unwrap();
    rt.set_global("array", Value::NumericArray(initial.clone()))
        .unwrap();
    let base = rt.retained_numeric_bytes();
    rt.set_budget(ResourceBudget {
        history_memory: base + 1_000_000,
        ..ResourceBudget::default()
    })
    .unwrap();
    let mut current = initial.clone();
    for i in 0..128 {
        rt.commit(format!("c{i}")).unwrap();
        current.set_float(&[i * 256], i as f64 + 1.0).unwrap();
        rt.set_global("array", Value::NumericArray(current.clone()))
            .unwrap();
    }
    assert!(rt.retained_numeric_bytes() > base);
    assert!(rt.retained_numeric_bytes() < base + 800_000);
    drop(initial);
    drop(current);
    for i in 0..128 {
        rt.drop_checkpoint(&format!("c{i}")).unwrap();
    }
    assert_eq!(rt.retained_numeric_bytes(), base);
    rt.remove_global("array");
    assert_eq!(rt.retained_numeric_bytes(), 0);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn numeric_nested_roots_and_unique_container_mutations() {
    let (path, mut rt) = runtime();
    let a = Array::zeros(DType::Float64, vec![10000]).unwrap();
    let mut list = PagedValues::default();
    for _ in 0..128 {
        list.push(Value::Option(Some(Box::new(Value::NumericArray(
            a.clone(),
        )))));
    }
    let mut map = PersistentMap::default();
    map.insert(
        MapKey::Text("items".into()),
        Value::TypedList("Option<FloatArray>".into(), list.clone()),
    );
    rt.set_global(
        "root",
        Value::TypedMap(
            "String".into(),
            "List<Option<FloatArray>>".into(),
            map.clone(),
        ),
    )
    .unwrap();
    let base = rt.retained_numeric_bytes();
    rt.set_budget(ResourceBudget {
        history_memory: base + 300_000,
        ..ResourceBudget::default()
    })
    .unwrap();
    let b = Array::zeros(DType::Float64, vec![10000]).unwrap();
    list.set(0, Value::NumericArray(b.clone()));
    map.insert(
        MapKey::Text("items".into()),
        Value::TypedList("FloatArray".into(), list.clone()),
    );
    rt.set_global("root", Value::Map(map)).unwrap();
    assert_eq!(rt.retained_numeric_bytes(), base * 2);
    drop(a);
    drop(b);
    drop(list);
    rt.remove_global("root");
    assert_eq!(rt.retained_numeric_bytes(), 0);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn numeric_budget_failure_rolls_back_and_does_not_retain_output() {
    let (path, mut rt) = runtime();
    let a = Array::zeros(DType::Float64, vec![10000]).unwrap();
    rt.set_global("a", Value::NumericArray(a)).unwrap();
    let base = rt.retained_numeric_bytes();
    rt.set_budget(ResourceBudget {
        history_memory: base + 2048,
        ..ResourceBudget::default()
    })
    .unwrap();
    let before = rt.state_digest().unwrap();
    assert!(matches!(
        rt.set_global(
            "tooLarge",
            Value::NumericArray(Array::zeros(DType::Float64, vec![10000]).unwrap())
        ),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert_eq!(rt.state_digest().unwrap(), before);
    assert_eq!(rt.retained_numeric_bytes(), base);
    rt.remove_global("a");
    assert_eq!(rt.retained_numeric_bytes(), 0);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn numeric_heap_gc_releases_payload_but_checkpoint_keeps_it() {
    let (path, mut rt) = runtime();
    let a = Array::zeros(DType::Float64, vec![10000]).unwrap();
    let id = rt.alloc(Value::Int(0)).unwrap();
    rt.heap_set(
        id,
        Value::Struct(
            "Holder".into(),
            BTreeMap::from([
                ("array".into(), Value::NumericArray(a)),
                ("cycle".into(), Value::CellRef(id)),
            ]),
        ),
    )
    .unwrap();
    let base = rt.retained_numeric_bytes();
    assert!(base > 0);
    rt.set_global("root", Value::CellRef(id)).unwrap();
    rt.commit("keep").unwrap();
    rt.remove_global("root");
    rt.collect_heap(&[], 1_000_000).unwrap();
    assert_eq!(rt.retained_numeric_bytes(), base);
    rt.drop_checkpoint("keep").unwrap();
    rt.collect_heap(&[], 1_000_000).unwrap();
    assert_eq!(rt.retained_numeric_bytes(), 0);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn numeric_gc_trigger_counts_new_pages_instead_of_shared_descriptors() {
    let (path, mut rt) = runtime();
    // A materialized buffer must cross the collection threshold; shared zeros need not.
    let a = Array::from_bits(
        DType::Float64,
        vec![1000000],
        std::iter::repeat_n(0, 1000000),
    )
    .unwrap();
    rt.set_global("a", Value::NumericArray(a.clone())).unwrap();
    assert!(rt.collection_due());
    rt.collect_heap(&[], 1000000).unwrap();
    assert!(!rt.collection_due());
    for _ in 0..100 {
        rt.alloc(Value::Option(Some(Box::new(Value::NumericArray(
            a.clone(),
        )))))
        .unwrap();
    }
    assert!(!rt.collection_due());
    rt.alloc(Value::NumericArray(
        Array::from_bits(
            DType::Float64,
            vec![1000000],
            std::iter::repeat_n(0, 1000000),
        )
        .unwrap(),
    ))
    .unwrap();
    assert!(rt.collection_due());
    rt.collect_heap(&[], 1000000).unwrap();
    assert!(!rt.collection_due());
    drop(a);
    rt.remove_global("a");
    assert_eq!(rt.retained_numeric_bytes(), 0);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
