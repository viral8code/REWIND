use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v17-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    root
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn compiled_sqlite_example_and_nested_frozen_rows_replay_without_source_or_database() {
    let root = dir();
    let source =
        include_str!("../examples/database/main.rw").replace("\":memory:\"", "\"data.sqlite\"");
    fs::write(root.join("main.rw"), source).unwrap();
    success(&call(
        &root,
        &["compile", "main.rw", "--allow-effects", "external,db,tasks"],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            "external,db,tasks",
            "--record",
            "trace.json",
        ],
    );
    success(&output);
    assert_eq!(output.stdout, b"1\nAlice\n42\n");
    fs::remove_file(root.join("data.sqlite")).unwrap();
    let replay = call(
        &root,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            "external,db,tasks",
        ],
    );
    success(&replay);
    assert_eq!(output.stdout, replay.stdout);
    assert!(!root.join("data.sqlite").exists());
    fs::remove_dir_all(root).unwrap();
}

