use std::{fs, path::Path, process::Command};

#[test]
fn authenticated_server_records_no_credentials_and_replays_without_a_peer() {
    let root = std::env::temp_dir().join(format!("rewind-v1930-{}", std::process::id()));
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(repo.join("scripts/smoke-http-server-auth-sdk.py"))
        .arg(env!("CARGO_BIN_EXE_rewind"))
        .arg(&root)
        .arg(repo.join("examples/http-server-auth/main.rw"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
