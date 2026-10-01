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
#[test]
fn polling_records_idle_and_replay_reproduces_readiness() {
    let path = root();
    let file=source(&path,"import std.gui as gui;match gui.window(\"poll\",240,160){Err(e)=>{panic(e.code);},Ok(view)=>{assert_eq(gui.present(&view),Ok(()));publish;assert_eq(gui.pollEvent(),Ok(None));match gui.pollEvent(){Ok(Some(e))=>{assert_eq(e.kind,\"close\");},_=>{panic(\"event\");}}assert_eq(gui.close(),Ok(()));publish;}}");
    let events = path.join("events.json");
    fs::write(&events,r#"[{"kind":"idle","x":0,"y":0,"key":"","width":0,"height":0},{"kind":"close","x":0,"y":0,"key":"","width":0,"height":0}]"#).unwrap();
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
        b"",
    );
    success(
        invoke(&[
            "replay",
            trace.to_str().unwrap(),
            "--root",
            path.to_str().unwrap(),
            "--allow-effects",
            "gui",
        ]),
        b"",
    );
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn begin_reset_rejects_pending_expression_operands() {
    let path = root();
    let d = failure(
        &path,
        "fn reset()->Int effects {} {revert begin;return 2;}let value=1+reset();",
    );
    assert!(d.to_string().contains("InvalidContinuation"), "{d}");
    fs::remove_dir_all(path).unwrap();
}
