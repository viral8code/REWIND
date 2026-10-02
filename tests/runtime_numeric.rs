use rewind::{
    numeric::{Array, DType},
    Error, ResourceBudget, Runtime, Value,
};
#[test]
fn numeric_admission_counts_native_storage_and_restores_failed_temporary_reservations() {
    let root = std::env::temp_dir().join(format!("rewind-numeric-budget-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut rt = Runtime::new(&root).unwrap();
    rt.enable_allocation_accounting();
    rt.set_budget(ResourceBudget {
        history_memory: 4096,
        ..ResourceBudget::default()
    })
    .unwrap();
    let value = Value::NumericArray(Array::zeros(DType::Int64, vec![32]).unwrap());
    assert!(Runtime::value_bytes(&value) >= 32 * 8);
    let id = rt.alloc(value).unwrap();
    let before = rt.state_digest().unwrap();
    assert!(matches!(
        rt.check_native_allocation(8192),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert!(matches!(
        rt.check_native_allocation(usize::MAX),
        Err(Error::HistoryBudgetExceeded)
    ));
    rt.check_native_allocation(0).unwrap();
    assert_eq!(before, rt.state_digest().unwrap());
    assert!(matches!(
        rt.heap_set(
            id,
            Value::NumericArray(Array::zeros(DType::Int64, vec![8192]).unwrap())
        ),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert_eq!(before, rt.state_digest().unwrap());
    drop(rt);
    std::fs::remove_dir_all(root).unwrap();
}
