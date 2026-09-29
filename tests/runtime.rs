use rewind::{Error, ResourceBudget, Runtime, Value};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rewind-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn revert_restores_compute_files_handles_and_output() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.set_global("a", Value::Int(10)).unwrap();
    let object = runtime.alloc(Value::List(vec![Value::Int(10)])).unwrap();
    runtime.write_file("data.txt", "A").unwrap();
    let handle = runtime.open_file("data.txt").unwrap();
    runtime.seek(handle, 100).unwrap();
    runtime.print_out("A\n").unwrap();
    runtime.commit("base").unwrap();

    runtime.set_global("a", Value::Int(20)).unwrap();
    runtime
        .heap_set(object, Value::List(vec![Value::Int(10), Value::Int(20)]))
        .unwrap();
    runtime.write_file("data.txt", "B").unwrap();
    runtime.seek(handle, 500).unwrap();
    runtime.print_out("B\n").unwrap();
    runtime.revert("base").unwrap();

    assert_eq!(runtime.global("a"), Some(&Value::Int(10)));
    assert_eq!(
        runtime.heap_get(object),
        Some(&Value::List(vec![Value::Int(10)]))
    );
    assert_eq!(runtime.read_file("data.txt").unwrap(), b"A");
    assert_eq!(runtime.handle(handle).unwrap().position, 100);
    assert_eq!(runtime.state().stdout.bytes().unwrap(), b"A\n");
    assert!(!dir.0.join("data.txt").exists());
}

#[test]
fn publication_applies_only_selected_history() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.write_file("result.txt", "initial").unwrap();
    runtime.print_out("start\n").unwrap();
    runtime.commit("base").unwrap();
    runtime.write_file("result.txt", "discarded").unwrap();
    runtime.print_out("discarded\n").unwrap();
    runtime.revert("base").unwrap();
    runtime.append_file("result.txt", " selected").unwrap();
    runtime.print_out("selected\n").unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    runtime.publish(false, &mut out, &mut err).unwrap();
    assert_eq!(
        fs::read(dir.0.join("result.txt")).unwrap(),
        b"initial selected"
    );
    assert_eq!(out, b"start\nselected\n");
    assert!(err.is_empty());
    runtime.publish(false, &mut out, &mut err).unwrap();
    assert_eq!(out, b"start\nselected\n");
}

#[test]
fn input_and_time_observations_replay_after_revert() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    let mut input = Cursor::new(b"Alice\nBob\n".to_vec());
    assert_eq!(
        runtime.input_line(&mut input).unwrap().as_deref(),
        Some("Alice")
    );
    runtime.commit("after_alice").unwrap();
    assert_eq!(
        runtime.input_line(&mut input).unwrap().as_deref(),
        Some("Bob")
    );
    let first_time = runtime.now_millis().unwrap();
    let first_random = runtime.random_u64();
    runtime.revert("after_alice").unwrap();
    assert_eq!(
        runtime.input_line(&mut input).unwrap().as_deref(),
        Some("Bob")
    );
    assert_eq!(runtime.now_millis().unwrap(), first_time);
    assert_eq!(runtime.random_u64(), first_random);
}

#[test]
fn changed_host_file_causes_conflict_before_output_or_write() {
    let dir = TestDir::new();
    fs::write(dir.0.join("data.txt"), "original").unwrap();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.write_file("data.txt", "virtual").unwrap();
    runtime.print_out("would publish\n").unwrap();
    fs::write(dir.0.join("data.txt"), "external").unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert!(matches!(
        runtime.publish(false, &mut out, &mut err),
        Err(Error::ExternalStateConflict(_))
    ));
    assert_eq!(fs::read(dir.0.join("data.txt")).unwrap(), b"external");
    assert!(out.is_empty());
    runtime.publish(true, &mut out, &mut err).unwrap();
    assert_eq!(fs::read(dir.0.join("data.txt")).unwrap(), b"virtual");
}

#[test]
fn paths_cannot_escape_root() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    for path in ["../escape.txt", "/absolute.txt", "C:\\absolute.txt"] {
        assert!(matches!(
            runtime.write_file(path, "x"),
            Err(Error::InvalidPath(_))
        ));
    }
}

#[test]
fn lazy_handle_replays_captured_block_and_rejects_changed_unread_block() {
    let dir = TestDir::new();
    let path = dir.0.join("large.bin");
    fs::write(&path, vec![b'A'; 8192]).unwrap();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    let handle = runtime.open_file("large.bin").unwrap();
    assert_eq!(runtime.read_handle(handle, 4).unwrap(), b"AAAA");
    runtime.commit("read_first").unwrap();
    fs::write(&path, vec![b'B'; 8193]).unwrap();
    runtime.revert("read_first").unwrap();
    runtime.seek(handle, 0).unwrap();
    assert_eq!(runtime.read_handle(handle, 4).unwrap(), b"AAAA");
    runtime.seek(handle, 4096).unwrap();
    assert!(matches!(
        runtime.read_handle(handle, 4),
        Err(Error::ExternalStateConflict(_))
    ));
}

#[test]
fn strict_snapshot_and_prior_epoch_remain_stable_after_publish() {
    let dir = TestDir::new();
    let path = dir.0.join("data.txt");
    fs::write(&path, "old").unwrap();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    let handle = runtime.open_snapshot("data.txt").unwrap();
    runtime.commit("before_publish").unwrap();
    runtime.write_file("data.txt", "new").unwrap();
    runtime
        .publish(false, &mut Vec::new(), &mut Vec::new())
        .unwrap();
    runtime.revert("before_publish").unwrap();
    assert_eq!(runtime.read_handle(handle, 3).unwrap(), b"old");
    assert_eq!(runtime.read_file("data.txt").unwrap(), b"old");
    assert_eq!(fs::read(path).unwrap(), b"new");
}

#[test]
fn directory_move_is_virtual_until_publish_and_revertable() {
    let dir = TestDir::new();
    fs::create_dir(dir.0.join("old")).unwrap();
    fs::write(dir.0.join("old/data.txt"), "hello").unwrap();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.commit("base").unwrap();
    runtime.move_directory("old", "new").unwrap();
    assert_eq!(runtime.read_file("new/data.txt").unwrap(), b"hello");
    assert!(dir.0.join("old/data.txt").exists());
    assert!(!dir.0.join("new").exists());
    runtime.revert("base").unwrap();
    assert!(matches!(
        runtime.read_file("new/data.txt"),
        Err(Error::MissingFile(_))
    ));
    runtime.move_directory("old", "new").unwrap();
    runtime
        .publish(false, &mut Vec::new(), &mut Vec::new())
        .unwrap();
    assert_eq!(fs::read(dir.0.join("new/data.txt")).unwrap(), b"hello");
    assert!(!dir.0.join("old").exists());
}

#[test]
fn output_journal_spills_and_budget_errors_preserve_state() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime
        .set_budget(ResourceBudget {
            history_memory: 5,
            history_storage: 20,
            spill_threshold: 5,
        })
        .unwrap();
    runtime.print_out("hello").unwrap();
    runtime.commit("base").unwrap();
    runtime.print_out("world").unwrap();
    assert!(runtime.state().stdout.storage_bytes() >= 5);
    runtime.revert("base").unwrap();
    assert_eq!(runtime.state().stdout.bytes().unwrap(), b"hello");
    runtime
        .set_budget(ResourceBudget {
            history_memory: 5,
            history_storage: 5,
            spill_threshold: 5,
        })
        .unwrap();
    assert!(matches!(
        runtime.print_out("world!"),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert_eq!(runtime.state().stdout.bytes().unwrap(), b"hello");
}

#[test]
fn file_page_budget_rejects_large_delta() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime
        .set_budget(ResourceBudget {
            history_memory: 4096,
            history_storage: 0,
            spill_threshold: 0,
        })
        .unwrap();
    assert!(matches!(
        runtime.write_file("big.bin", vec![0; 4097]),
        Err(Error::HistoryBudgetExceeded)
    ));
    assert!(!dir.0.join("big.bin").exists());
    assert!(matches!(
        runtime.read_file("big.bin"),
        Err(Error::MissingFile(_))
    ));
}

#[test]
fn stack_frames_locals_and_program_counter_revert() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.set_program_counter(12);
    runtime.push_stack(Value::Int(1)).unwrap();
    runtime.push_frame(99).unwrap();
    runtime
        .set_local("x", Value::Text("before".into()))
        .unwrap();
    runtime.commit("frame").unwrap();
    runtime.set_program_counter(50);
    runtime.push_stack(Value::Int(2)).unwrap();
    runtime.set_local("x", Value::Text("after".into())).unwrap();
    runtime.pop_frame().unwrap();
    runtime.revert("frame").unwrap();
    assert_eq!(runtime.state().program_counter, 12);
    assert_eq!(runtime.state().stack.as_slice(), &[Value::Int(1)]);
    assert_eq!(runtime.local("x"), Some(&Value::Text("before".into())));
    assert_eq!(runtime.pop_frame().unwrap().return_pc, 99);
}

#[test]
fn heap_and_handle_ids_replay_after_revert() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.write_file("data.txt", "x").unwrap();
    runtime.commit("base").unwrap();
    let first_heap = runtime.alloc(Value::Int(1)).unwrap();
    let first_handle = runtime.open_file("data.txt").unwrap();
    runtime.revert("base").unwrap();
    assert_eq!(runtime.alloc(Value::Int(1)).unwrap(), first_heap);
    assert_eq!(runtime.open_file("data.txt").unwrap(), first_handle);
}

#[test]
fn handle_partial_write_restores_file_and_cursor() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.write_file("data.bin", vec![b'A'; 8192]).unwrap();
    let handle = runtime.open_file("data.bin").unwrap();
    runtime.seek(handle, 4096).unwrap();
    runtime.commit("before_write").unwrap();
    runtime.write_handle(handle, b"XYZ").unwrap();
    assert_eq!(&runtime.read_file("data.bin").unwrap()[4096..4099], b"XYZ");
    assert_eq!(runtime.handle(handle).unwrap().position, 4099);
    runtime.revert("before_write").unwrap();
    assert_eq!(&runtime.read_file("data.bin").unwrap()[4096..4099], b"AAA");
    assert_eq!(runtime.handle(handle).unwrap().position, 4096);
}

#[test]
fn strict_snapshot_ignores_later_virtual_writes() {
    let dir = TestDir::new();
    let mut runtime = Runtime::new(&dir.0).unwrap();
    runtime.write_file("data.txt", "old").unwrap();
    let handle = runtime.open_snapshot("data.txt").unwrap();
    runtime.write_file("data.txt", "new").unwrap();
    assert_eq!(runtime.read_handle(handle, 3).unwrap(), b"old");
    assert!(matches!(
        runtime.write_handle(handle, b"x"),
        Err(Error::InvalidOperation(_))
    ));
}
