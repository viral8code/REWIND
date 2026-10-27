//! PERF-01 (deterministic part): the logical work of small reference programs is recorded in
//! `tests/fixtures/logical-cost.json` and may not grow by more than 20 %. Logical work is the VM instruction count
//! and the storage-work counters from `rewind profile`; it does not depend on the machine. Wall time, RSS and GC
//! time are NOT measured here (see docs/archive/v2-next-performance.md); their absence is not a zero.
//! Refresh the budgets after an intended change with `REWIND_UPDATE_LOGICAL_COST=1 cargo test --test logical_cost`.
use std::{fs, path::Path, process::Command};

const CASES: &[&str] = &["loop_sum", "fib", "list_sort", "map_text", "json_parse"];
const BUDGET_FILE: &str = "tests/fixtures/logical-cost.json";

fn measure(case: &str) -> (u64, Vec<u64>, String) {
    let source = format!("tests/fixtures/logical/{case}.rw");
    let run = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["run", &source])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{case}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let profile = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["profile", &source, "--profile-format", "json"])
        .output()
        .unwrap();
    assert!(profile.status.success(), "{case}: profile failed");
    // The profile document is written to stderr, the program's own output to stdout.
    let document: serde_json::Value = serde_json::from_slice(&profile.stderr).unwrap();
    let instructions = document["task_instruction_total"].as_u64().unwrap();
    let storage = document["storage_work"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    (
        instructions,
        storage,
        String::from_utf8_lossy(&run.stdout).into_owned(),
    )
}

#[test]
fn logical_work_of_reference_programs_stays_within_budget() {
    let update = std::env::var_os("REWIND_UPDATE_LOGICAL_COST").is_some();
    let mut recorded = serde_json::Map::new();
    let stored: serde_json::Value = if Path::new(BUDGET_FILE).exists() {
        serde_json::from_str(&fs::read_to_string(BUDGET_FILE).unwrap()).unwrap()
    } else {
        serde_json::json!({})
    };
    for case in CASES {
        let (instructions, storage, stdout) = measure(case);
        recorded.insert(
            (*case).into(),
            serde_json::json!({"instructions": instructions, "storage_work": storage, "stdout": stdout}),
        );
        if update {
            continue;
        }
        let budget = &stored[*case];
        assert!(
            !budget.is_null(),
            "{case} has no budget; run with REWIND_UPDATE_LOGICAL_COST=1"
        );
        assert_eq!(
            budget["stdout"].as_str().unwrap(),
            stdout,
            "{case}: output changed"
        );
        let limit = budget["instructions"].as_u64().unwrap() * 12 / 10;
        assert!(
            instructions <= limit,
            "{case}: {instructions} VM instructions exceed the budget {} by more than 20 %",
            budget["instructions"]
        );
        let stored_storage: u64 = budget["storage_work"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .sum();
        let now_storage: u64 = storage.iter().sum();
        assert!(
            now_storage <= stored_storage * 12 / 10 + 16,
            "{case}: storage work {now_storage} exceeds the budget {stored_storage} by more than 20 %"
        );
    }
    if update {
        fs::write(
            BUDGET_FILE,
            serde_json::to_string_pretty(&serde_json::Value::Object(recorded)).unwrap() + "\n",
        )
        .unwrap();
    }
}
