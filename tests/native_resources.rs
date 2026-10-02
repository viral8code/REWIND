use rewind::{Runtime, Value};
use std::collections::BTreeMap;
#[test]
fn move_invalidates_alias_and_lease_state_restores_without_reusing_generation() {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    let mut owner = rt.native_value("HttpDownload", 7, BTreeMap::new()).unwrap();
    let first = rt.claim_native(&mut owner).unwrap().unwrap();
    let alias = owner.clone();
    rt.set_global("owner", owner.clone()).unwrap();
    rt.commit("owned").unwrap();
    rt.move_native(&mut owner).unwrap();
    assert!(rt.check_native(&alias).is_err());
    let second = rt.claim_native(&mut owner).unwrap().unwrap();
    assert_ne!(first, second);
    assert_eq!(rt.check_native(&owner).unwrap(), 7);
    rt.revert("owned").unwrap();
    assert_eq!(rt.check_native(&alias).unwrap(), 7);
    assert!(rt.check_native(&owner).is_err());
    let mut moved = alias.clone();
    rt.move_native(&mut moved).unwrap();
    let third = rt.claim_native(&mut moved).unwrap().unwrap();
    assert!(third.1 > second.1);
    assert_eq!(rt.live_native_ids(&[owner]).len(), 0);
    assert_eq!(rt.live_native_ids(&[moved]).len(), 1);
}
#[test]
fn fabricated_resources_and_stale_tokens_are_rejected() {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    assert!(rt
        .check_native(&Value::Struct("HttpDownload".into(), BTreeMap::new()))
        .is_err());
    let owner = rt
        .native_value("HttpDownload", 99, BTreeMap::new())
        .unwrap();
    rt.forget_native_owner(99);
    assert!(rt.check_native(&owner).is_err());
}
