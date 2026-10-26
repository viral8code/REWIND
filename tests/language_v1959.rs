use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1959-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&r).unwrap();
    fs::write(r.join("main.rw"), source).unwrap();
    r
}
fn call(r: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(r)
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn python() -> &'static str {
    if cfg!(windows) {
        "python"
    } else {
        "python3"
    }
}
#[test]
fn menu_fixture_checkpoint_is_source_free_in_both_trace_modes() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for mode in ["debug", "compact"] {
        let r = root(&fs::read_to_string(repo.join("examples/gui-menu/main.rw")).unwrap());
        fs::write(r.join("events.json"),r#"[{"window":"main","event":{"kind":"key","x":0,"y":0,"key":"Ctrl+T","width":0,"height":0}},{"window":"main","event":{"kind":"key","x":0,"y":0,"key":"Ctrl+T","width":0,"height":0}},{"window":"main","event":{"kind":"key","x":0,"y":0,"key":"Ctrl+O","width":0,"height":0}}]"#).unwrap();
        ok(&call(&r, &["compile", "main.rw", "--allow-effects", "gui"]));
        fs::remove_file(r.join("main.rw")).unwrap();
        fs::remove_dir_all(r.join(".rewind")).unwrap();
        let o = call(
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
        ok(&o);
        assert_eq!(o.stdout, b"ready\ntoggle\nundo\ntoggle\nopen\n");
        fs::remove_file(r.join("events.json")).unwrap();
        let replay = call(&r, &["replay", "trace.json", "--allow-effects", "gui"]);
        ok(&replay);
        assert_eq!(replay.stdout, o.stdout);
        fs::remove_dir_all(r).unwrap();
    }
}
#[test]
fn native_menu_commands_restore_and_replay_without_a_display() {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let r = std::env::temp_dir().join(format!("rewind-native-menu-{}", std::process::id()));
    let result = Command::new(python())
        .args([
            repo.join("scripts/smoke-gui-menu-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            r.as_os_str(),
            repo.join("examples/gui-menu/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(r).unwrap();
}
