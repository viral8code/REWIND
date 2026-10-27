use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};
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
fn unfinished_reverse_tasks_restore_and_replay_without_source_or_cache() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let r = std::env::temp_dir().join(format!("rewind-backward-smoke-{}", std::process::id()));
    let result = Command::new(python())
        .args([
            repo.join("scripts/smoke-autodiff-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            r.as_os_str(),
            repo.join("examples/autodiff-async/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn native_pointer_cancels_materialized_backward_and_replays_disconnected() {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let r = std::env::temp_dir().join(format!("rewind-backward-native-gui-{}", std::process::id()));
    let result = Command::new(python())
        .args([
            repo.join("scripts/smoke-backward-native-gui-sdk.py")
                .as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            r.as_os_str(),
            repo.join("examples/backward-gui/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(r).unwrap();
}
