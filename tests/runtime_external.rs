use rewind::{ResourceBudget, Runtime};
use serde_json::json;
use std::cell::Cell;
fn runtime() -> Runtime {
    Runtime::new(std::env::temp_dir()).unwrap()
}
#[test]
fn restore_reuses_results_and_fresh_appends_without_repeating_effects() {
    let mut r = runtime();
    let calls = Cell::new(0);
    r.commit("a").unwrap();
    for _ in 0..2 {
        r.enter_external(false).unwrap();
        assert_eq!(
            r.external_operation("send", b"a", 128, || {
                calls.set(calls.get() + 1);
                Ok(json!(calls.get()))
            })
            .unwrap(),
            Ok(json!(1))
        );
        r.exit_external().unwrap();
        r.revert("a").unwrap();
    }
    assert_eq!(calls.get(), 1);
    r.enter_external(false).unwrap();
    assert!(r
        .external_operation("send", b"changed", 128, || panic!("must not send"))
        .unwrap_err()
        .to_string()
        .contains("ExternalRequestMismatch"));
    r.exit_external().unwrap();
    r.enter_external(true).unwrap();
    assert_eq!(
        r.external_operation("send", b"changed", 128, || {
            calls.set(2);
            Ok(json!(2))
        })
        .unwrap(),
        Ok(json!(2))
    );
    r.exit_external().unwrap();
    let tape = r.export_observations().unwrap();
    let mut replay = runtime();
    replay.import_observations(&tape).unwrap();
    replay.enter_external(true).unwrap();
    assert_eq!(
        replay
            .external_operation("send", b"a", 128, || panic!("no host access during replay"))
            .unwrap(),
        Ok(json!(1))
    );
    replay.exit_external().unwrap();
    replay.enter_external(true).unwrap();
    assert_eq!(
        replay
            .external_operation("send", b"changed", 128, || panic!(
                "no host access during replay"
            ))
            .unwrap(),
        Ok(json!(2))
    );
    replay.exit_external().unwrap();
}
#[test]
fn unknown_outcome_and_host_failure_are_not_retried() {
    let mut r = runtime();
    r.commit("a").unwrap();
    r.enter_external(false).unwrap();
    assert!(r
        .external_operation("write", b"x", 8, || Ok(json!("too large to record")))
        .unwrap_err()
        .to_string()
        .contains("ExternalOutcomeUnknown"));
    r.exit_external().unwrap();
    r.revert("a").unwrap();
    r.enter_external(false).unwrap();
    assert!(r
        .external_operation("write", b"x", 8, || panic!("must not repeat"))
        .unwrap_err()
        .to_string()
        .contains("ExternalOutcomeUnknown"));
    r.exit_external().unwrap();
    let mut r = runtime();
    r.commit("a").unwrap();
    r.enter_external(false).unwrap();
    let failure = Err("OutcomeUncertain".into());
    assert_eq!(
        r.external_operation("send", b"", 128, || failure.clone())
            .unwrap(),
        failure
    );
    r.exit_external().unwrap();
    r.revert("a").unwrap();
    r.enter_external(false).unwrap();
    assert_eq!(
        r.external_operation("send", b"", 128, || panic!("no retry"))
            .unwrap(),
        failure
    );
}
#[test]
fn boundary_and_budget_fail_before_host_effect() {
    let mut r = runtime();
    assert!(r
        .external_operation("x", b"", 128, || panic!("outside region"))
        .is_err());
    r.enter_external(false).unwrap();
    assert!(r.commit("a").is_err());
    assert!(r.publish(false, &mut Vec::new(), &mut Vec::new()).is_err());
    assert!(r.enter_external(false).is_err());
    r.exit_external().unwrap();
    r.set_budget(ResourceBudget {
        history_memory: 1,
        history_storage: 1,
        spill_threshold: 1,
    })
    .unwrap_or(());
    r.enter_external(false).unwrap();
    assert!(r
        .external_operation("x", b"", 128, || panic!("budget before effects"))
        .is_err());
}

#[test]
fn resource_tokens_do_not_revive_or_alias_after_close() {
    use rewind::external::Resources;
    use std::rc::Rc;
    struct Owner(Rc<Cell<usize>>);
    impl Drop for Owner {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let mut a = Resources::new(1).unwrap();
    let first = a.insert(Owner(drops.clone())).unwrap();
    assert!(a.insert(Owner(drops.clone())).is_err());
    assert_eq!(drops.get(), 1);
    a.close(first).unwrap();
    assert_eq!(drops.get(), 2);
    let second = a.insert(Owner(drops.clone())).unwrap();
    assert_ne!(first, second);
    assert!(a.get(first).is_err());
    let b: Resources<Owner> = Resources::new(1).unwrap();
    assert!(b.get(second).is_err());
    drop(a);
    assert_eq!(drops.get(), 3);
}

#[test]
fn secret_payloads_are_not_written_into_observation_tapes() {
    let mut r = runtime();
    r.register_secret_value(&rewind::Value::Text("private-token".into()));
    r.enter_external(false).unwrap();
    assert!(r
        .external_operation("credential", b"private-token", 128, || panic!(
            "secret in request"
        ))
        .is_err());
    let _ = r
        .external_operation("credential", b"credential-alias", 128, || {
            Ok(json!({"token":"private-token"}))
        })
        .unwrap();
    r.exit_external().unwrap();
    assert!(r
        .export_observations()
        .unwrap_err()
        .to_string()
        .contains("SecretObservationUnrecordable"));
}
