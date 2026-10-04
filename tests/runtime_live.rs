use rewind::{ResourceBudget, Runtime};
use serde_json::json;
use std::cell::Cell;
#[test]
fn live_reexecution_does_not_advance_recorded_cursors_and_cannot_export_a_tape() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.configure_live_external(true);
    runtime.commit("start").unwrap();
    let calls = Cell::new(0);
    for expected in 1..=2 {
        runtime.enter_external_live_task(0).unwrap();
        assert_eq!(
            runtime
                .external_operation("clock", b"same", 128, || {
                    calls.set(calls.get() + 1);
                    Ok(json!(calls.get()))
                })
                .unwrap(),
            Ok(json!(expected))
        );
        runtime.exit_external().unwrap();
        runtime.revert("start").unwrap();
    }
    assert!(runtime
        .export_observations()
        .unwrap_err()
        .to_string()
        .contains("ExternalLiveRecordingUnsupported"));
    let recorded = Cell::new(0);
    for _ in 0..2 {
        runtime.enter_external(false).unwrap();
        assert_eq!(
            runtime
                .external_operation("recorded", b"request", 128, || {
                    recorded.set(recorded.get() + 1);
                    Ok(json!(99))
                })
                .unwrap(),
            Ok(json!(99))
        );
        runtime.exit_external().unwrap();
        runtime.revert("start").unwrap();
    }
    assert_eq!(calls.get(), 2);
    assert_eq!(recorded.get(), 1);
}
#[test]
fn denied_live_and_exhausted_work_or_memory_do_not_call_the_host() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    assert!(runtime.enter_external_live_task(0).is_err());
    runtime.configure_live_external(true);
    runtime.configure_native_work(1);
    runtime.enter_external_live_task(0).unwrap();
    let called = Cell::new(false);
    assert!(runtime
        .external_operation("clock", b"", 128, || {
            called.set(true);
            Ok(json!(1))
        })
        .is_err());
    assert!(!called.get());
    runtime.exit_external().unwrap();
    runtime.configure_native_work(1_000_000);
    runtime
        .set_budget(ResourceBudget {
            history_memory: 16384,
            history_storage: 16384,
            spill_threshold: 16384,
        })
        .unwrap();
    runtime.enter_external_live_task(0).unwrap();
    assert!(runtime
        .external_operation("clock", b"", 8192, || {
            called.set(true);
            Ok(json!(1))
        })
        .is_err());
    assert!(!called.get());
    runtime.exit_external().unwrap();
}
