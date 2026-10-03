use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v196-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn compact_records_long_execution_without_step_history_and_detects_tampering() {
    let root = root();
    fs::write(root.join("main.rw"),"var sum=0;for i in 0..30000 {sum+=i;}commit saved;sum=99;revert saved;assert_eq(sum,449985000);drop saved;Out.println(sum);publish;").unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
            "--steps",
            "1000000",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"449985000\n");
    let encoded = fs::read(root.join("trace.json")).unwrap();
    assert!(encoded.len() < 16384);
    let trace: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(trace["record_mode"], "compact");
    assert!(trace["debug"].is_null());
    assert_eq!(trace["events"].as_array().unwrap().len(), 0);
    assert!(trace["execution"]["steps"].as_u64().unwrap() > 300000);
    let replay = call(&root, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    for field in ["steps", "sha256"] {
        let mut modified = trace.clone();
        modified["execution"][field] = if field == "steps" {
            serde_json::json!(1)
        } else {
            serde_json::json!("bad")
        };
        fs::write(
            root.join("tampered.json"),
            serde_json::to_vec(&modified).unwrap(),
        )
        .unwrap();
        let bad = call(&root, &["replay", "tampered.json"]);
        assert!(!bad.status.success());
        assert!(String::from_utf8_lossy(&bad.stderr).contains("ReplayMismatch"));
    }
    let mismatch = call(&root, &["replay", "trace.json", "--record-mode", "debug"]);
    assert!(!mismatch.status.success());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("ReplayMismatch: record mode"));
    let inspect = call(&root, &["debug", "main.rwc", "--record-mode", "compact"]);
    assert!(!inspect.status.success());
    assert!(String::from_utf8_lossy(&inspect.stderr).contains("CompactTraceNoDebugHistory"));
    let view = call(&root, &["debug", "trace.json"]);
    assert!(!view.status.success());
    assert!(String::from_utf8_lossy(&view.stderr).contains("CompactTraceNoDebugHistory"));
    let inspected = call(&root, &["debug", "main.rwc"]);
    ok(&inspected);
    assert!(String::from_utf8_lossy(&inspected.stderr).contains("\"checkpoints\""));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn compact_preserves_task_scheduling_failure_and_source_free_replay() {
    let root = root();
    fs::write(
        root.join("main.rw"),
        r#"
async fn worker(n:Int)->Int effects {} {return n+1;}
let channel=Channel<Int>(1);let first=spawn worker(1);let second=spawn worker(2);
assert_eq(await first,Ok(2));assert_eq(await second,Ok(3));
assert_eq(await channel.send(7),Ok(()));assert_eq(await channel.receive(),Ok(7));channel.close();
for i in 0..100 {assert_eq(await worker(i),Ok(i+1));}
Out.println(1);publish;
"#,
    )
    .unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
            "--native-work",
            "5000000",
        ],
    );
    ok(&out);
    let replay = call(&root, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::write(
        root.join("failure.rw"),
        "Out.println(9);panic(\"expected failure\");",
    )
    .unwrap();
    let failed = call(
        &root,
        &[
            "run",
            "failure.rw",
            "--record",
            "failure.json",
            "--record-mode",
            "compact",
        ],
    );
    assert!(!failed.status.success());
    let failed_replay = call(&root, &["replay", "failure.json"]);
    assert!(!failed_replay.status.success());
    assert!(String::from_utf8_lossy(&failed_replay.stderr).contains("expected failure"));
    assert!(!String::from_utf8_lossy(&failed_replay.stderr).contains("ReplayMismatch"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compact_replay_uses_observations_and_preserves_published_host_file() {
    let root = root();
    fs::write(root.join("input.txt"), "captured input").unwrap();
    fs::write(
        root.join("main.rw"),
        r#"
let text=File.readText("input.txt")?;File.writeText("output.txt",text);Out.println(text);publish;
"#,
    )
    .unwrap();
    ok(&call(
        &root,
        &[
            "compile",
            "main.rw",
            "--allow-effects",
            "fileRead,fileWrite",
        ],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            "fileRead,fileWrite",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
        ],
    );
    ok(&out);
    fs::remove_file(root.join("input.txt")).unwrap();
    fs::write(root.join("output.txt"), "host file changed after recording").unwrap();
    let replay = call(
        &root,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            "fileRead,fileWrite",
        ],
    );
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    assert_eq!(
        fs::read_to_string(root.join("output.txt")).unwrap(),
        "host file changed after recording"
    );
    fs::remove_dir_all(root).unwrap();
}
