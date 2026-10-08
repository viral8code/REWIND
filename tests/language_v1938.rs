use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1938-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &PathBuf, args: &[&str]) -> Output {
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
fn eigen_checkpoint_and_source_free_debug_compact_replay() {
    let r = root();
    fs::write(
        r.join("main.rw"),
        include_str!("../examples/eigen-async/main.rw"),
    )
    .unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(r.join(".rewind"));
    for mode in ["debug", "compact"] {
        let out = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--steps",
                "20000000",
                "--task-steps",
                "2000000",
                "--native-work",
                "10000000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "diagonalized\nrestored\neigen done\n"
        );
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn cooperative_work_names_are_versioned_and_opaque() {
    let r = root();
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.37\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(
        r.join("main.rw"),
        "record EigenWork{code:Int}let x=EigenWork(7);assert_eq(x.code,7);publish;",
    )
    .unwrap();
    ok(&call(&r, &["update"]));
    ok(&call(&r, &["run", "main.rw"]));
    fs::remove_dir_all(r).unwrap();
    for source in [
        "let forged=EigenWork();publish;",
        "record EigenWork{code:Int}publish;",
    ] {
        let r = root();
        fs::write(r.join("main.rw"), source).unwrap();
        assert!(!call(&r, &["compile", "main.rw"]).status.success());
        fs::remove_dir_all(r).unwrap();
    }
}

#[test]
fn huge_shared_zero_input_can_copy_and_cancel_with_small_memory_budget() {
    let r = root();
    fs::write(r.join("main.rw"),r#"
import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(4096);shape.add(4096);let a=take(n.zerosFloat(&shape));
let work=spawn jobs.eigenSymmetric(a,0.0,0);for i in 0..4{task.yieldNow();}assert(!work.isDone());work.cancel();
match await work{Err(TaskError::Cancelled)=>{},_=>{panic("partial result");}}
Out.println("cancelled");publish;
"#).unwrap();
    let out = call(
        &r,
        &[
            "profile",
            "main.rw",
            "--native-work",
            "1000000000",
            "--history-memory",
            "2097152",
            "--task-steps",
            "1000000",
            "--steps",
            "10000000",
        ],
    );
    ok(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        "cancelled\n"
    );
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn invalid_input_is_typed_and_fatal_work_is_not_a_numeric_result() {
    let r = root();
    let prefix = r#"import std.numeric as n;import std.numericAsync as jobs;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(0);shape.add(0);let a=take(n.zerosFloat(&shape));
"#;
    fs::write(r.join("main.rw"),format!("{prefix}{}",r#"
let answer=take(take(await spawn jobs.eigenSymmetric(a,0.0,0)));assert_eq(answer.sweeps,0);
match take(await spawn jobs.eigenSymmetric(a,-1.0,0)){Err(e)=>{assert_eq(e.code,"NumericDomain");},_=>{panic("domain");}}
Out.println("checked");publish;
"#)).unwrap();
    ok(&call(&r, &["run", "main.rw"]));
    fs::write(
        r.join("main.rw"),
        format!("{prefix}stdNumericEigenInit(&a,0.0,0);Out.println(1);publish;"),
    )
    .unwrap();
    let out = call(&r, &["run", "main.rw", "--native-work", "4000"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("NativeWork"));
    assert!(out.stdout.is_empty());
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn repeated_cancel_reclaims_work_pages_and_task_quota_metadata() {
    let r = root();
    fs::write(r.join("main.rw"),r#"
import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(65);shape.add(65);let a=take(n.zerosFloat(&shape));var done=0;
for i in 0..128{let work=spawn jobs.eigenSymmetric(a,0.0,0);task.yieldNow();assert(!work.isDone());work.cancel();
match await work{Err(TaskError::Cancelled)=>{done+=1;},_=>{panic("cancel");}}}
Out.println(done);publish;
"#).unwrap();
    let out = call(
        &r,
        &[
            "profile",
            "main.rw",
            "--native-work",
            "10000000000",
            "--steps",
            "20000000",
            "--history-memory",
            "8MiB",
        ],
    );
    ok(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "128");
    let stderr = String::from_utf8(out.stderr).unwrap();
    let p: serde_json::Value =
        serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap()).unwrap();
    assert!(
        p["numeric_pages"]["live_bytes"].as_u64().unwrap() < 1024 * 1024,
        "{p}"
    );
    assert!(
        p["task_instructions"].as_object().unwrap().len() < 64,
        "{p}"
    );
    assert!(p["gc"]["completed"].as_u64().unwrap() > 0, "{p}");
    fs::remove_dir_all(r).unwrap();
}
