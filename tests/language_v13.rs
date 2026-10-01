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
fn initial_checkpoint_resets_values_heap_pending_io_and_user_labels() {
    let path = root();
    let file = source(
        &path,
        r#"
 var n=13; let values=List<Int>();values.add(99);commit saved;
 Out.println("discarded");revert begin;
 var n=7;let values=List<Int>();values.add(n);
 assert_eq(values.len(),1);assert_eq(values.get(0),7);
 commit saved;Out.println("reset");publish;
 revert begin;commit saved;Out.println("again");publish;
 "#,
    );
    success(invoke(&["run", file.to_str().unwrap()]), b"reset\nagain\n");
    let d = failure(&path, "var n=1;revert begin;Out.println(n);publish;");
    assert!(d.to_string().contains("n"));
    fs::remove_dir_all(path).unwrap();
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
fn begin_reset_retains_nested_continuations_and_replays_input() {
    let path = root();
    let file = source(
        &path,
        r#"
 fn reset()->Unit effects {output} {var local=9;revert begin;Out.println("function");}
 reset();publish;if true {let x=99;revert begin;Out.println("block");}
 publish;
 "#,
    );
    success(
        invoke(&["run", file.to_str().unwrap()]),
        b"function\nblock\n",
    );
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
#[test]
fn gui_requires_permission_and_published_scene_and_valid_script() {
    let path = root();
    let file=source(&path,"import std.gui as gui;match gui.nextEvent(){Err(e)=>{assert_eq(e.code,\"GuiNotPublished\");},_=>{panic(\"event\");}}");
    let out = invoke(&["run", file.to_str().unwrap()]);
    assert!(!out.status.success());
    success(
        invoke(&["run", file.to_str().unwrap(), "--allow-effects", "gui"]),
        b"",
    );
    let events = path.join("bad.json");
    fs::write(
        &events,
        "[{\"kind\":\"invalid\",\"x\":0,\"y\":0,\"key\":\"\",\"width\":0,\"height\":0}]",
    )
    .unwrap();
    let out = invoke(&[
        "run",
        file.to_str().unwrap(),
        "--allow-effects",
        "gui",
        "--gui-events",
        events.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    let file=source(&path,"import std.gui as gui;async fn bad()->Unit effects {gui} {gui.close();}let task=spawn bad();");
    let out = invoke(&["run", file.to_str().unwrap(), "--allow-effects", "gui"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("GuiMainTaskOnly"));
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn map_fields_preserve_value_types_and_reject_wrong_arguments() {
    let path = root();
    let file=source(&path,"struct Store {values:Map<String,Int>}let s=Store(Map<String,Int>());s.values.set(\"x\",7);assert_eq(s.values.get(\"x\"),Some(7));assert_eq(s.values.keys().get(0),\"x\");s.values.remove(\"x\");assert_eq(s.values.len(),0);");
    success(invoke(&["run", file.to_str().unwrap()]), b"");
    let d=failure(&path,"struct Store {values:Map<String,Int>}let s=Store(Map<String,Int>());s.values.set(\"x\",\"bad\");");
    assert!(d.to_string().contains("InvalidArguments"));
    fs::remove_dir_all(path).unwrap();
}
