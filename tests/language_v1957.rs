use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};
fn ok(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn contraction_and_logical_copy_restore_unfinished_tasks_and_source_free_replay() {
    let root = std::env::temp_dir().join(format!("rewind-shape-smoke-{}", std::process::id()));
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            repo.join("scripts/smoke-shape-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            root.as_os_str(),
            repo.join("examples/shape-async/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(root).unwrap();
}
