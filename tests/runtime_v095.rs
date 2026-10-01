use rewind::{Error, ResourceBudget, Runtime};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn runtime() -> (std::path::PathBuf, Runtime) {
    let p = std::env::temp_dir().join(format!(
        "rewind-v095-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    let mut rt = Runtime::new(&p).unwrap();
    rt.enable_incremental_publish();
    (p, rt)
}
#[test]
fn file_and_directory_metadata_budget_failure_restores_all_pending_state() {
    let (p, mut rt) = runtime();
    rt.set_budget(ResourceBudget {
        history_memory: 1,
        ..ResourceBudget::default()
    })
    .unwrap();
    let before = rt.state_digest().unwrap();
    assert!(matches!(
        rt.write_file("file", b""),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert_eq!(rt.state_digest().unwrap(), before);
    assert!(matches!(
        rt.create_directory("dir"),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert_eq!(rt.state_digest().unwrap(), before);
    assert!(!p.join("dir").exists());
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn virtual_directory_tree_can_move_and_delete_after_publication() {
    let (p, mut rt) = runtime();
    rt.enable_virtual_publish();
    rt.create_directory("a").unwrap();
    rt.create_directory("a/b").unwrap();
    rt.write_file("a/b/c", b"content").unwrap();
    rt.publish(false, &mut vec![], &mut vec![]).unwrap();
    rt.move_directory("a", "x").unwrap();
    rt.publish(false, &mut vec![], &mut vec![]).unwrap();
    assert_eq!(rt.read_file("x/b/c").unwrap(), b"content");
    assert!(rt.read_file("a/b/c").is_err());
    rt.delete_file("x/b/c").unwrap();
    rt.delete_directory("x/b").unwrap();
    rt.delete_directory("x").unwrap();
    rt.publish(false, &mut vec![], &mut vec![]).unwrap();
    assert!(!p.join("a").exists());
    assert!(!p.join("x").exists());
    fs::remove_dir_all(p).unwrap();
}
