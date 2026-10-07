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
fn profile_exposes_native_resources_before_io_and_after_closed_database_scope() {
    let r = root();
    fs::write(r.join("main.rw"),r#"import std.db as db;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
{var opening:Option<Task<Result<DbConnection,DbError>>>=None;external{opening=Some(db.sqlite(":memory:",false,5000));}match opening{Some(work)=>{let connection=take(take(await work));},None=>{panic("missing");}}}
var cleaning:Option<Task<Result<Unit,DbError>>>=None;external {cleaning=Some(db.waitForCleanup(5000));}match cleaning{Some(work)=>{take(take(await work));},None=>{panic("missing");}}Out.println("clean");publish;"#).unwrap();
    for (name, args) in [
        ("empty", vec!["profile", "empty.rw"]),
        (
            "closed",
            vec!["profile", "main.rw", "--allow-effects", "external,db,tasks"],
        ),
    ] {
        fs::write(r.join("empty.rw"), "publish;").unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&r)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        let profile: serde_json::Value =
            serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap())
                .unwrap();
        assert_eq!(profile["runtime"]["native_resources"], 0, "{profile}");
    }
    fs::remove_dir_all(r).unwrap();
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
