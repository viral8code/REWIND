// heap_set が予算超過で失敗したとき、ヒープが元の値に戻ることを固定する。
// tracked（ヒープの所有者が自分だけ）と、checkpoint が共有している場合の両方を見る。
use rewind::{Error, ResourceBudget, Runtime, Value};

fn runtime() -> (Runtime, Vec<u64>) {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    rt.set_budget(ResourceBudget {
        history_memory: 200_000,
        history_storage: 8 * 1024 * 1024,
        spill_threshold: 512,
    })
    .unwrap();
    rt.push_frame(0).unwrap();
    let ids = (0..5).map(|i| rt.alloc(Value::Int(i)).unwrap()).collect();
    (rt, ids)
}

fn snapshot(rt: &Runtime, ids: &[u64]) -> Vec<Value> {
    ids.iter().map(|id| rt.heap_get(*id).unwrap().clone()).collect()
}

fn failed_set_restores(checkpoint: bool) {
    let (mut rt, ids) = runtime();
    if checkpoint {
        rt.commit("c").unwrap();
    }
    rt.heap_set(ids[2], Value::Int(99)).unwrap();
    let before = snapshot(&rt, &ids);
    let huge = Value::Text("x".repeat(4 * 1024 * 1024).into());
    match rt.heap_set(ids[2], huge) {
        Err(Error::HistoryBudgetExceeded) => {}
        other => panic!("expected budget error, got {other:?}"),
    }
    assert_eq!(snapshot(&rt, &ids), before);
    assert_eq!(rt.heap_get(ids[2]), Some(&Value::Int(99)));
    // A failed write leaves the runtime usable and the next small write succeeds.
    rt.heap_set(ids[2], Value::Int(7)).unwrap();
    assert_eq!(rt.heap_get(ids[2]), Some(&Value::Int(7)));
    if checkpoint {
        rt.revert("c").unwrap();
        assert_eq!(rt.heap_get(ids[2]), Some(&Value::Int(2)));
    }
}

#[test]
fn failed_heap_set_restores_when_heap_is_uniquely_owned() {
    failed_set_restores(false);
}

#[test]
fn failed_heap_set_restores_when_checkpoint_shares_the_heap() {
    failed_set_restores(true);
}
