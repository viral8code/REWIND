use std::{fs, process::Command};
#[test]
fn real_http_client_cache_source_free_keepalive_eviction_and_replay() {
    let root = std::env::temp_dir().join(format!("rewind-v1934-http-{}", std::process::id()));
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
#[test]
fn huge_virtual_lu_input_can_handoff_checkpoint_and_cancel_with_bounded_copy_admission() {
    let root = std::env::temp_dir().join(format!("rewind-v1934-lu-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"),r#"
import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let ms=List<Int>();ms.add(4096);ms.add(4096);let matrix=take(n.zerosFloat(&ms));
let vs=List<Int>();vs.add(4096);let rhs=take(n.zerosFloat(&vs));
let work=spawn jobs.solve(matrix,rhs,0.0);task.yieldNow();assert(!work.isDone());commit copying;
work.cancel();match await work{Err(TaskError::Cancelled)=>{},_=>{panic("cancel");}}
revert copying;assert(!work.isDone());task.yieldNow();work.cancel();
match await work{Err(TaskError::Cancelled)=>{},_=>{panic("restore cancel");}}drop copying;
Out.println("copy cancelled");publish;
"#).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&root)
        .args([
            "run",
            "main.rw",
            "--history-memory",
            "2MiB",
            "--native-work",
            "2000000",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"copy cancelled\n");
    fs::remove_dir_all(root).unwrap();
}
