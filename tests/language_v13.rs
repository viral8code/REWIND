use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "rewind-v13-{}-{}",
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
fn failure(path: &Path, text: &str) -> serde_json::Value {
    let file = source(path, text);
    let out = invoke(&["run", file.to_str().unwrap(), "--diagnostic-format", "json"]);
    assert!(!out.status.success());
    serde_json::from_slice(&out.stderr).unwrap()
}

#[test]
fn begin_is_reserved_and_cannot_be_redefined_or_dropped() {
    let path = root();
    for text in [
        "var begin=1;",
        "commit begin;",
        "drop begin;",
        "fn begin()->Unit {}",
        "branch begin {}",
    ] {
        let d = failure(&path, text);
        assert!(d.to_string().contains("ReservedLabel"), "{text}: {d}");
    }
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn native_gui_counter_can_undo_and_replay_without_a_display() {
    let path = root();
    let file = source(&path, include_str!("../examples/gui/main.rw"));
    let events = path.join("events.json");
    fs::write(&events, include_str!("../examples/gui/events.json")).unwrap();
    let trace = path.join("trace.json");
    success(
        invoke(&[
            "run",
            file.to_str().unwrap(),
            "--allow-effects",
            "gui",
            "--gui-events",
            events.to_str().unwrap(),
            "--record",
            trace.to_str().unwrap(),
        ]),
        b"1\n",
    );
    success(
        invoke(&[
            "replay",
            trace.to_str().unwrap(),
            "--allow-effects",
            "gui",
            "--root",
            path.to_str().unwrap(),
        ]),
        b"1\n",
    );
    fs::remove_dir_all(path).unwrap();
}
