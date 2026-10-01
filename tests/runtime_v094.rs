use rewind::{Error, ResourceBudget, Runtime};
use std::{
    fs,
    io::{self, Write},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn runtime() -> (std::path::PathBuf, Runtime) {
    let p = std::env::temp_dir().join(format!(
        "rewind-publish-v094-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    let mut rt = Runtime::new(&p).unwrap();
    rt.enable_incremental_publish();
    (p, rt)
}
#[test]
fn file_publish_is_not_undone_or_reapplied_by_restore() {
    let (p, mut rt) = runtime();
    let mut out = vec![];
    let mut err = vec![];
    rt.commit("empty").unwrap();
    rt.write_file("data", b"one").unwrap();
    rt.commit("pending").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("empty").unwrap();
    assert_eq!(fs::read(p.join("data")).unwrap(), b"one");
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("pending").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    assert_eq!(fs::read(p.join("data")).unwrap(), b"one");
    rt.write_file("data", b"two").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("pending").unwrap();
    rt.copy_file("data", "copy").unwrap();
    rt.commit("copy").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("copy").unwrap();
    rt.delete_file("data").unwrap();
    rt.commit("deleted").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("deleted").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    assert!(!p.join("data").exists());
    assert_eq!(fs::read(p.join("copy")).unwrap(), b"two");
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn unpublished_old_file_delta_and_external_edits_conflict() {
    let (p, mut rt) = runtime();
    let mut out = vec![];
    let mut err = vec![];
    rt.write_file("data", b"old").unwrap();
    rt.commit("old").unwrap();
    rt.write_file("data", b"new").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("old").unwrap();
    assert!(matches!(
        rt.publish(false, &mut out, &mut err),
        Err(Error::ExternalStateConflict(_))
    ));
    assert_eq!(fs::read(p.join("data")).unwrap(), b"new");
    rt.revert("old").unwrap();
    fs::write(p.join("data"), b"external").unwrap();
    assert!(matches!(
        rt.publish(false, &mut out, &mut err),
        Err(Error::ExternalStateConflict(_))
    ));
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn branches_keep_event_identity_and_do_not_deduplicate_text() {
    let (p, mut rt) = runtime();
    let mut out = vec![];
    let mut err = vec![];
    let anchor = rt.begin_branch();
    rt.print_out("same\n").unwrap();
    rt.end_branch("candidate", anchor).unwrap();
    rt.print_out("same\n").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("candidate").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("candidate").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    assert_eq!(out, b"same\nsame\n");
    fs::remove_dir_all(p).unwrap();
}
struct PartialWriter {
    count: usize,
}
impl Write for PartialWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.count == 0 {
            self.count += 1;
            Ok(b.len().min(1))
        } else {
            Err(io::Error::other("injected short write failure"))
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn partial_output_failure_survives_restore_and_blocks_retry() {
    let (p, mut rt) = runtime();
    rt.commit("base").unwrap();
    rt.print_out("abc").unwrap();
    let mut err = vec![];
    let mut broken = PartialWriter { count: 0 };
    assert!(matches!(
        rt.publish(false, &mut broken, &mut err),
        Err(Error::PublishPartiallyApplied(_))
    ));
    rt.revert("base").unwrap();
    rt.print_out("next").unwrap();
    let mut out = vec![];
    assert!(matches!(
        rt.publish(false, &mut out, &mut err),
        Err(Error::PublishPartiallyApplied(_))
    ));
    assert!(out.is_empty());
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn publication_ledger_is_charged_to_history_budget() {
    let (p, mut rt) = runtime();
    rt.set_budget(ResourceBudget {
        history_memory: 128,
        history_storage: 1024,
        spill_threshold: 128,
    })
    .unwrap();
    let mut out = vec![];
    let mut err = vec![];
    for i in 0..3 {
        rt.print_out("x").unwrap();
        rt.commit(format!("retained-{i}")).unwrap();
        rt.publish(false, &mut out, &mut err).unwrap();
    }
    let mut failed = false;
    for i in 0..4 {
        if rt.print_out("x").is_err() {
            failed = true;
            break;
        }
        rt.commit(format!("more-{i}")).unwrap();
        rt.publish(false, &mut out, &mut err).unwrap();
    }
    assert!(failed);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn virtual_publish_keeps_a_virtual_host_baseline_without_changing_disk() {
    let (p, mut rt) = runtime();
    rt.enable_virtual_publish();
    let mut out = vec![];
    let mut err = vec![];
    rt.commit("base").unwrap();
    rt.write_file("data", b"one").unwrap();
    rt.commit("saved").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("base").unwrap();
    assert_eq!(rt.read_file("data").unwrap(), b"one");
    rt.write_file("data", b"two").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    rt.revert("saved").unwrap();
    assert_eq!(rt.read_file("data").unwrap(), b"two");
    rt.create_directory("dir").unwrap();
    rt.write_file("dir/child", b"x").unwrap();
    rt.publish(false, &mut out, &mut err).unwrap();
    assert_eq!(rt.directory_entries("dir").unwrap(), vec!["child"]);
    assert!(!p.join("data").exists());
    assert!(!p.join("dir").exists());
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn byte_input_eof_and_budget_failures_cannot_be_retried_as_new_host_reads() {
    let (p, mut rt) = runtime();
    let mut input = io::Cursor::new(b"ab".to_vec());
    rt.commit("start").unwrap();
    assert_eq!(rt.input_chunk(&mut input, 2).unwrap(), Some(b"ab".to_vec()));
    assert_eq!(rt.input_chunk(&mut input, 2).unwrap(), None);
    input.get_mut().extend_from_slice(b"later");
    assert_eq!(rt.input_chunk(&mut input, 2).unwrap(), None);
    assert_eq!(input.position(), 2);
    rt.revert("start").unwrap();
    assert_eq!(rt.input_chunk(&mut input, 2).unwrap(), Some(b"ab".to_vec()));
    assert_eq!(rt.input_chunk(&mut input, 2).unwrap(), None);
    assert_eq!(input.position(), 2);
    fs::remove_dir_all(p).unwrap();

    let (p, mut rt) = runtime();
    rt.set_budget(ResourceBudget {
        history_memory: 1,
        ..ResourceBudget::default()
    })
    .unwrap();
    let mut input = io::Cursor::new(b"abcd".to_vec());
    assert!(matches!(
        rt.input_chunk(&mut input, 2),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert!(matches!(
        rt.input_chunk(&mut input, 2),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert_eq!(input.position(), 2);
    fs::remove_dir_all(p).unwrap();
}
