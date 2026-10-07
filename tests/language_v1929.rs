use std::{fs, path::Path, process::Command};

#[test]
fn source_free_https_server_keeps_keys_private_and_replays_disconnected() {
    let root = std::env::temp_dir().join(format!("rewind-v1929-{}", std::process::id()));
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(repo.join("scripts/smoke-http-server-tls-sdk.py"))
        .arg(env!("CARGO_BIN_EXE_rewind"))
        .arg(&root)
        .arg(repo.join("examples/http-server-tls/main.rw"))
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
