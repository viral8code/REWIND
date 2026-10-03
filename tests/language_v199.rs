use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v199-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn named_gui_source_free_undo_and_strict_replay_without_event_fixture() {
    let root = root();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/gui-windows/main.rw"),
    )
    .unwrap();
    ok(&call(
        &root,
        &["compile", "main.rw", "--allow-effects", "gui"],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    for mode in ["debug", "compact"] {
        fs::write(
            root.join("events.json"),
            include_str!("../examples/gui-windows/events.json"),
        )
        .unwrap();
        let out = call(
            &root,
            &[
                "run",
                "main.rwc",
                "--allow-effects",
                "gui",
                "--gui-window-events",
                "events.json",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(out.stdout, b"1\n0\n");
        fs::remove_file(root.join("events.json")).unwrap();
        let replay = call(&root, &["replay", "trace.json", "--allow-effects", "gui"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
        let mut trace: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("trace.json")).unwrap()).unwrap();
        assert_eq!(
            trace["observations"]["gui_window_events"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        trace["observations"]["gui_window_events"][0]["poll"] = serde_json::json!(true);
        fs::write(
            root.join("tampered.json"),
            serde_json::to_vec(&trace).unwrap(),
        )
        .unwrap();
        let bad = call(
            &root,
            &["replay", "tampered.json", "--allow-effects", "gui"],
        );
        assert!(!bad.status.success());
        assert!(String::from_utf8_lossy(&bad.stderr).contains("ReplayMismatch"));
    }
    fs::remove_dir_all(root).unwrap();
}
