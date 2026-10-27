use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "rewind-v14-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&path);
    fs::create_dir(&path).unwrap();
    path
}
fn invoke(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(args)
        .output()
        .unwrap()
}
fn success(out: Output, expected: &[u8]) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected);
}
fn source(path: &Path, text: &str) -> PathBuf {
    let file = path.join("main.rw");
    fs::write(&file, text).unwrap();
    file
}
#[test]
fn notes_edit_unicode_resize_save_and_replay_without_host_changes() {
    let path = root();
    let file = source(&path, include_str!("../examples/notes/main.rw"));
    let events = path.join("events.json");
    fs::write(&events, include_str!("../examples/notes/events.json")).unwrap();
    let trace = path.join("trace.json");
    success(
        invoke(&[
            "run",
            file.to_str().unwrap(),
            "--allow-effects",
            "gui,fileRead,fileWrite",
            "--gui-events",
            events.to_str().unwrap(),
            "--record",
            trace.to_str().unwrap(),
        ]),
        b"",
    );
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        "日本語\nnotes"
    );
    fs::write(path.join("notes.txt"), "sentinel").unwrap();
    success(
        invoke(&[
            "replay",
            trace.to_str().unwrap(),
            "--root",
            path.to_str().unwrap(),
            "--allow-effects",
            "gui,fileRead,fileWrite",
        ]),
        b"",
    );
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        "sentinel"
    );
    success(
        invoke(&[
            "compile",
            file.to_str().unwrap(),
            "--allow-effects",
            "gui,fileRead,fileWrite",
        ]),
        b"",
    );
    fs::remove_file(file).unwrap();
    fs::remove_file(path.join("notes.txt")).unwrap();
    fs::remove_dir_all(path.join(".rewind")).unwrap();
    success(
        invoke(&[
            "run",
            path.join("main.rwc").to_str().unwrap(),
            "--allow-effects",
            "gui,fileRead,fileWrite",
            "--gui-events",
            events.to_str().unwrap(),
        ]),
        b"",
    );
    assert_eq!(
        fs::read_to_string(path.join("notes.txt")).unwrap(),
        "日本語\nnotes"
    );
    fs::remove_dir_all(path).unwrap();
}
