use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1912-{}-{}",
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
fn forms_and_tables_source_free_multi_window_input_undo_and_replay() {
    let r = root();
    fs::write(
        r.join("main.rw"),
        include_str!("../examples/gui-controls/main.rw"),
    )
    .unwrap();
    ok(&call(&r, &["compile", "main.rw", "--allow-effects", "gui"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    for mode in ["compact"] {
        fs::write(
            r.join("events.json"),
            include_str!("../examples/gui-controls/events.json"),
        )
        .unwrap();
        let out = call(
            &r,
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
        assert_eq!(
            out.stdout,
            "{\"active\":false,\"age\":13,\"name\":\"日本語\"}\n1\n日本\n".as_bytes()
        );
        fs::remove_file(r.join("events.json")).unwrap();
        let replay = call(&r, &["replay", "trace.json", "--allow-effects", "gui"]);
        ok(&replay);
        assert_eq!(replay.stdout, out.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
