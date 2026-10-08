use rewind::{ResourceBudget, Runtime, Value};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "rewind-sensitive-accounting-{}-{}",
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
fn metric(rt: &Runtime, key: &str) -> u64 {
    rt.sensitive_registry_metrics().unwrap()[key]
        .as_u64()
        .unwrap()
}
#[test]
fn distinct_text_capacity_is_admitted_once_and_duplicates_do_not_expand_registry() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_sensitive_accounting();
    let text = "classified-capacity".repeat(1024);
    let value = Value::Text(text.clone().into());
    rt.register_secret_value(&value);
    let charged = metric(&rt, "text_bytes");
    assert!(charged >= text.len() as u64);
    assert_eq!(metric(&rt, "text_patterns"), 1);
    for _ in 0..100 {
        rt.register_secret_value(&value);
    }
    assert_eq!(metric(&rt, "text_bytes"), charged);
    rt.register_secret_value(&Value::Text((text.clone() + "other").into()));
    assert_eq!(metric(&rt, "text_patterns"), 2);
    assert!(metric(&rt, "text_bytes") >= 2 * text.len() as u64);
}
#[test]
fn registration_before_enabling_is_counted_and_old_mode_preserves_its_budget_contract() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    let text = "sensitive-but-unrooted".repeat(8192);
    rt.register_secret_value(&Value::Text(text.clone().into()));
    rt.set_budget(ResourceBudget {
        history_memory: 64 * 1024,
        ..ResourceBudget::default()
    })
    .unwrap();
    assert!(rt.sensitive_registry_metrics().is_none());
    rt.check_sensitive_accounting().unwrap();
    rt.enable_sensitive_accounting();
    assert!(metric(&rt, "text_bytes") >= text.len() as u64);
    assert!(rt.check_sensitive_accounting().is_err());
    assert_ne!(
        rt.mask_debug_json(&serde_json::json!(text)),
        serde_json::json!(text)
    );
}
#[test]
fn registry_remains_a_security_root_after_checkpoint_drop_revert_and_collection() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_sensitive_accounting();
    rt.commit("before").unwrap();
    let text = "retained-pattern".repeat(1024);
    rt.register_secret_value(&Value::Text(text.clone().into()));
    let charge = metric(&rt, "text_bytes");
    rt.revert("before").unwrap();
    rt.drop_checkpoint("before").unwrap();
    rt.collect_heap(&[], 10000).unwrap();
    assert_eq!(metric(&rt, "text_bytes"), charge);
    assert_ne!(
        rt.mask_debug_json(&serde_json::json!(text)),
        serde_json::json!(text)
    );
}
#[test]
fn shared_binary_owner_is_not_charged_twice_and_survives_loss_of_ordinary_roots() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_sensitive_accounting();
    let bytes = Arc::new(vec![255; 65536]);
    rt.set_global("bytes", Value::Bytes(bytes.clone())).unwrap();
    let before = rt.shared_payload_metrics().unwrap()["live_bytes"]
        .as_u64()
        .unwrap();
    rt.register_secret_value(&Value::Bytes(bytes.clone()));
    let after = rt.shared_payload_metrics().unwrap()["live_bytes"]
        .as_u64()
        .unwrap();
    assert_eq!(after, before);
    assert_eq!(metric(&rt, "binary_patterns"), 1);
    assert_eq!(metric(&rt, "binary_entry_bytes"), 96);
    rt.register_secret_value(&Value::Bytes(Arc::new(vec![255; 65536])));
    assert_eq!(metric(&rt, "binary_patterns"), 1);
    assert_eq!(
        rt.shared_payload_metrics().unwrap()["live_bytes"]
            .as_u64()
            .unwrap(),
        after
    );
    drop(bytes);
    rt.remove_global("bytes");
    rt.collect_heap(&[], 10000).unwrap();
    assert!(
        rt.shared_payload_metrics().unwrap()["live_bytes"]
            .as_u64()
            .unwrap()
            >= 65536
    );
}
#[test]
fn budget_failure_keeps_current_pattern_masked_and_different_runtimes_have_independent_ledgers() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_sensitive_accounting();
    rt.set_budget(ResourceBudget {
        history_memory: 16 * 1024,
        ..ResourceBudget::default()
    })
    .unwrap();
    let text = "new-sensitive-value".repeat(2048);
    rt.register_secret_value(&Value::Text(text.clone().into()));
    assert!(rt.check_sensitive_accounting().is_err());
    assert_ne!(
        rt.mask_debug_json(&serde_json::json!(text)),
        serde_json::json!(text)
    );
    let mut other = Runtime::new(&d.0).unwrap();
    other.enable_sensitive_accounting();
    assert_eq!(metric(&other, "text_bytes"), 0);
    other.check_sensitive_accounting().unwrap();
}
#[test]
fn heap_cycles_and_nested_values_register_finite_patterns_without_retaining_heap_objects() {
    let d = Dir::new();
    let mut rt = Runtime::new(&d.0).unwrap();
    rt.enable_sensitive_accounting();
    let id = rt.alloc(Value::Null).unwrap();
    let text = "nested-sensitive-pattern".repeat(256);
    rt.heap_set(
        id,
        Value::List(vec![Value::HeapRef(id), Value::Text(text.clone().into())]),
    )
    .unwrap();
    rt.register_secret_value(&Value::HeapRef(id));
    assert_eq!(metric(&rt, "text_patterns"), 1);
    let (reclaimed, _) = rt.collect_heap(&[], 10000).unwrap();
    assert_eq!(reclaimed, 1);
    assert!(rt.heap_get(id).is_none());
    assert_ne!(
        rt.mask_debug_json(&serde_json::json!(text)),
        serde_json::json!(text)
    );
}
