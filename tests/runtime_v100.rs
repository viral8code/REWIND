use rewind::{Error, ResourceBudget, Runtime, Value};
use std::{
    fs,
    io::{self, Write},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn runtime() -> (std::path::PathBuf, Runtime) {
    let path = std::env::temp_dir().join(format!(
        "rewind-v100-runtime-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    let mut rt = Runtime::new(&path).unwrap();
    rt.enable_incremental_publish();
    (path, rt)
}
#[test]
fn ledger_reclaims_only_after_the_last_external_branch_anchor_dies() {
    let (p, mut rt) = runtime();
    rt.print_out("one\n").unwrap();
    let anchor = rt.begin_branch();
    let clone = anchor.clone();
    let mut output = vec![];
    rt.publish(false, &mut output, &mut vec![]).unwrap();
    assert_eq!(rt.debug_state()["published_operation_count"], 1);
    rt.print_out("two\n").unwrap();
    rt.end_branch("child", anchor).unwrap();
    rt.publish(false, &mut output, &mut vec![]).unwrap();
    assert_eq!(output, b"one\n");
    drop(clone);
    rt.drop_checkpoint("child").unwrap();
    rt.publish(false, &mut output, &mut vec![]).unwrap();
    assert_eq!(rt.debug_state()["published_operation_count"], 0);
    assert_eq!(output, b"one\n");
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn byte_observations_spill_and_roundtrip_revert_and_replay() {
    let (p, mut rt) = runtime();
    rt.set_budget(ResourceBudget {
        history_memory: 128,
        history_storage: 16384,
        spill_threshold: 0,
    })
    .unwrap();
    rt.commit("before").unwrap();
    let input = vec![b'x'; 4096];
    assert_eq!(
        rt.input_chunk(&mut io::Cursor::new(&input), 4096).unwrap(),
        Some(input.clone())
    );
    let tape = rt.export_observations().unwrap();
    rt.revert("before").unwrap();
    assert_eq!(
        rt.input_chunk(&mut io::Cursor::new(b"different"), 4096)
            .unwrap(),
        Some(input.clone())
    );
    let mut replay = Runtime::new(&p).unwrap();
    replay.import_observations(&tape).unwrap();
    assert_eq!(
        replay.input_chunk(&mut io::Cursor::new(b""), 4096).unwrap(),
        Some(input)
    );
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn native_gc_limit_preflights_wide_children_and_preserves_heap() {
    let (p, mut rt) = runtime();
    let id = rt.alloc(Value::List(vec![Value::Null; 10000])).unwrap();
    rt.set_global("keep", Value::HeapRef(id)).unwrap();
    rt.configure_native_work(4);
    let before = rt.state_digest().unwrap();
    assert!(rt.collect_heap(&[], 100000).is_err());
    assert_eq!(before, rt.state_digest().unwrap());
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn zero_payload_collections_still_pay_allocation_admission() {
    let (p, mut rt) = runtime();
    rt.enable_allocation_accounting();
    rt.set_budget(ResourceBudget {
        history_memory: 2048,
        ..ResourceBudget::default()
    })
    .unwrap();
    let id = rt.alloc(Value::List(vec![])).unwrap();
    let before = rt.state_digest().unwrap();
    assert!(matches!(
        rt.heap_set(id, Value::List(vec![Value::Null; 100])),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert_eq!(before, rt.state_digest().unwrap());
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn directory_and_rename_failures_report_applied_paths_and_poison_retry() {
    let (p, mut rt) = runtime();
    rt.create_directory("a").unwrap();
    rt.create_directory("z").unwrap();
    fs::write(p.join("z"), b"block").unwrap();
    assert!(matches!(
        rt.publish(true, &mut vec![], &mut vec![]),
        Err(Error::PublishPartiallyApplied(_))
    ));
    let failure = rt.publish_failure().unwrap();
    assert_eq!(failure.phase, "directory-create");
    assert_eq!(failure.applied.len(), 1);
    assert!(!failure.retryable);
    assert!(p.join("a").is_dir());
    assert!(rt.publish(true, &mut vec![], &mut vec![]).is_err());
    fs::remove_dir_all(p).unwrap();
    let (p, mut rt) = runtime();
    rt.write_file("a", b"applied").unwrap();
    rt.write_file("z", b"blocked").unwrap();
    rt.commit("pending").unwrap();
    fs::create_dir(p.join("z")).unwrap();
    assert!(rt.publish(true, &mut vec![], &mut vec![]).is_err());
    assert_eq!(rt.publish_failure().unwrap().phase, "rename");
    assert_eq!(rt.publish_failure().unwrap().applied.len(), 1);
    assert_eq!(fs::read(p.join("a")).unwrap(), b"applied");
    rt.revert("pending").unwrap();
    assert!(rt.publish(true, &mut vec![], &mut vec![]).is_err());
    fs::remove_dir_all(p).unwrap();
}
struct Short {
    bytes: Vec<u8>,
}
impl Write for Short {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.bytes.is_empty() {
            return Err(io::Error::other("injected"));
        }
        self.bytes.push(bytes[0]);
        Ok(1)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn replay_stream_short_write_is_terminal_too() {
    let (p, mut rt) = runtime();
    rt.import_observations(&rt.export_observations().unwrap())
        .unwrap();
    rt.print_out("hello").unwrap();
    let mut out = Short { bytes: vec![] };
    assert!(rt.publish(false, &mut out, &mut vec![]).is_err());
    assert_eq!(out.bytes, b"h");
    assert_eq!(rt.publish_failure().unwrap().phase, "stdout");
    assert!(rt.publish(false, &mut vec![], &mut vec![]).is_err());
    fs::remove_dir_all(p).unwrap();
}
