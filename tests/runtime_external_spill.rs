use rewind::{ResourceBudget, Runtime};
use serde_json::json;
#[test]
fn completed_observations_spill_and_replay_without_repeating_host_calls() {
    let root = std::env::temp_dir();
    let mut rt = Runtime::new(&root).unwrap();
    rt.set_budget(ResourceBudget {
        history_memory: 128 * 1024,
        history_storage: 8 * 1024 * 1024,
        spill_threshold: 4096,
    })
    .unwrap();
    rt.commit("start").unwrap();
    let payload = "x".repeat(60 * 1024);
    for i in 0u64..64 {
        rt.enter_external(false).unwrap();
        assert_eq!(
            rt.external_operation("chunk", &i.to_le_bytes(), 64 * 1024, || Ok(json!(payload)))
                .unwrap(),
            Ok(json!(payload))
        );
        rt.exit_external().unwrap();
    }
    let tape = rt.export_observations().unwrap();
    rt.revert("start").unwrap();
    for i in 0u64..64 {
        rt.enter_external(false).unwrap();
        assert_eq!(
            rt.external_operation("chunk", &i.to_le_bytes(), 64 * 1024, || panic!(
                "must not resend after revert"
            ))
            .unwrap(),
            Ok(json!(payload))
        );
        rt.exit_external().unwrap();
    }
    let mut replay = Runtime::new(root).unwrap();
    replay
        .set_budget(ResourceBudget {
            history_memory: 128 * 1024,
            history_storage: 8 * 1024 * 1024,
            spill_threshold: 4096,
        })
        .unwrap();
    replay.import_observations(&tape).unwrap();
    for i in 0u64..64 {
        replay.enter_external(false).unwrap();
        assert_eq!(
            replay
                .external_operation("chunk", &i.to_le_bytes(), 64 * 1024, || panic!(
                    "must not contact host during replay"
                ))
                .unwrap(),
            Ok(json!(payload))
        );
        replay.exit_external().unwrap();
    }
}
#[test]
fn disk_capacity_failure_keeps_known_result_and_never_repeats_effect() {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    rt.set_budget(ResourceBudget {
        history_memory: 65536,
        history_storage: 1,
        spill_threshold: 0,
    })
    .unwrap();
    rt.commit("start").unwrap();
    rt.enter_external(false).unwrap();
    let result = rt.external_operation("send", b"request", 8192, || Ok(json!("x".repeat(4096))));
    assert!(result.is_err());
    rt.exit_external().unwrap();
    rt.set_budget(ResourceBudget {
        history_memory: 65536,
        history_storage: 16384,
        spill_threshold: 0,
    })
    .unwrap();
    rt.revert("start").unwrap();
    rt.enter_external(false).unwrap();
    assert_eq!(
        rt.external_operation("send", b"request", 8192, || panic!(
            "recorded result must prevent resend"
        ))
        .unwrap(),
        Ok(json!("x".repeat(4096)))
    );
}
#[test]
fn trace_limit_counts_spilled_payload_instead_of_resident_metadata() {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    rt.set_budget(ResourceBudget {
        history_memory: 3 * 1024 * 1024,
        history_storage: 32 * 1024 * 1024,
        spill_threshold: 0,
    })
    .unwrap();
    for i in 0u64..17 {
        rt.enter_external(false).unwrap();
        rt.external_operation("chunk", &i.to_le_bytes(), 2 * 1024 * 1024, || {
            Ok(json!("x".repeat(1024 * 1024)))
        })
        .unwrap()
        .unwrap();
        rt.exit_external().unwrap();
    }
    assert!(rt
        .export_observations()
        .unwrap_err()
        .to_string()
        .contains("TraceBudgetExceeded"));
}
