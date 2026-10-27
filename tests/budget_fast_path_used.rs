// 速い経路が実際に使われていることを確かめる。ベンチ相当のループ（スカラの push/pop と局所変数）で、
// 予算検査のうち速い経路が受理した回数が大半を占めることを見る。回数は壁時計ではなく数える。
// BUDGET_EPOCH は全体で 1 つなので、並列のテストで数がぶれないよう、このファイルにはテストを 1 本だけ置く。
// budget-verify では確認のために全体検査も走るので、数えない。
#![cfg(not(feature = "budget-verify"))]
use rewind::{budget_check_counts, ResourceBudget, Runtime, Value};

#[test]
fn scalar_loop_is_mostly_accepted_by_the_fast_path() {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    rt.set_budget(ResourceBudget {
        history_memory: 64 * 1024 * 1024,
        history_storage: 64 * 1024 * 1024,
        spill_threshold: 32 * 1024 * 1024,
    })
    .unwrap();
    rt.push_frame(0).unwrap();
    let (fast0, full0) = budget_check_counts();
    for i in 0..5000i64 {
        rt.push_stack(Value::Int(i)).unwrap();
        rt.set_local(format!("x{}", i % 7), Value::Int(i)).unwrap();
        if i % 2 == 1 {
            rt.pop_stack().unwrap();
        }
    }
    let (fast1, full1) = budget_check_counts();
    let (fast, full) = (fast1 - fast0, full1 - full0);
    assert!(fast + full >= 10000, "fast={fast} full={full}");
    // 下限は緩く取る。ほぼ全部が速い経路のはずだが、割合だけを見る。
    assert!(fast * 10 >= (fast + full) * 8, "fast={fast} full={full}");
}
