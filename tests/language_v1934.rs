use std::{fs, process::Command};
#[test]
fn real_http_client_cache_source_free_keepalive_eviction_and_replay() {
    let root = std::env::temp_dir().join(format!("rewind-v1934-http-{}", std::process::id()));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            repo.join("scripts/smoke-http-pool-sdk.py").as_os_str(),
            std::path::PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            root.join("workflow").as_os_str(),
            repo.join("examples/http-pool/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("Verified extracted bounded HTTP clients")
    );
    fs::remove_dir_all(root).unwrap();
}
