use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1932-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
#[test]
fn native_gui_http_and_both_databases_complete_source_and_compiled_workflows() {
    if std::env::var_os("REWIND_TEST_PG_DSN").is_none() {
        assert!(
            std::env::var_os("REWIND_REQUIRE_PG").is_none(),
            "required PostgreSQL fixture is missing"
        );
        return;
    }
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return;
    }
    if !cfg!(any(target_os = "linux", target_os = "windows")) {
        return;
    }
    let r = root();
    let target = r.join("workflow");
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let python = if cfg!(windows) { "python" } else { "python3" };
    let out = Command::new(python)
        .args([
            repo.join("scripts/smoke-gui-data-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            target.as_os_str(),
            repo.join("examples/gui-data/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("Verified native GUI data workflow"));
    fs::remove_dir_all(r).unwrap();
}
