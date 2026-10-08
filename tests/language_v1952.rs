use std::{fs, path::PathBuf, process::Command};

#[test]
fn native_clipboard_undo_and_disconnected_recording_work_without_sources() {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return;
    }
    if !cfg!(any(target_os = "linux", windows)) {
        return;
    }
    let root = std::env::temp_dir().join(format!("rewind-v1952-{}", std::process::id()));
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let python = if cfg!(windows) { "python" } else { "python3" };
    let result = Command::new(python)
        .args([
            repo.join("scripts/smoke-gui-clipboard-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            root.as_os_str(),
            repo.join("examples/gui-clipboard/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("Verified native clipboard SDK"));
    fs::remove_dir_all(root).unwrap();
}
