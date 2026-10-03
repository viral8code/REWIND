use rewind::{ResourceBudget, Runtime};
#[test]
fn virtual_snapshot_shares_pages_and_keeps_old_contents_after_writes_and_restore() {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    rt.set_budget(ResourceBudget {
        spill_threshold: 0,
        ..ResourceBudget::default()
    })
    .unwrap();
    let mut old = vec![b'a'; 8192];
    old.extend(b"tail");
    rt.write_file("virtual-v192.bin", &old).unwrap();
    let snapshot = rt.open_snapshot("virtual-v192.bin").unwrap();
    rt.seek(snapshot, 4094).unwrap();
    rt.commit("before").unwrap();
    rt.write_file("virtual-v192.bin", b"changed").unwrap();
    assert_eq!(rt.read_handle(snapshot, 5).unwrap(), b"aaaaa");
    rt.revert("before").unwrap();
    assert_eq!(rt.read_handle(snapshot, 5).unwrap(), b"aaaaa");
    rt.seek(snapshot, 8192).unwrap();
    assert_eq!(rt.read_handle(snapshot, 65536).unwrap(), b"tail");
    assert!(rt.read_handle(snapshot, 1).unwrap().is_empty());
    let handle = rt.open_file("virtual-v192.bin").unwrap();
    rt.seek(handle, 8190).unwrap();
    assert_eq!(rt.read_handle(handle, 6).unwrap(), b"aatail");
    rt.close_handle(handle).unwrap();
    rt.close_handle(snapshot).unwrap();
}
