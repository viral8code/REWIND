use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1911-{}-{}",
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
fn numeric_cold_task_arguments_use_shared_page_accounting_and_source_free_replay() {
    let r = root();
    fs::write(r.join("main.rw"),r#"import std.numeric as numeric;import std.numericAsync as jobs;import std.task as task;
 fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
 let shape=List<Int>();shape.add(1000000);let zeros=take(numeric.zerosFloat(&shape));let ones=take(numeric.mapFloat("exp",&zeros));
 let work=spawn jobs.dot(ones,ones);task.yieldNow();assert(!work.isDone());Out.println(take(take(await work)));publish;"#).unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    let out = call(
        &r,
        &[
            "run",
            "main.rwc",
            "--history-memory",
            "64MiB",
            "--native-work",
            "100000000",
            "--steps",
            "10000000",
            "--task-steps",
            "10000000",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"1000000\n");
    let replay = call(&r, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn numeric_checkpoint_versions_fit_shared_page_budget_and_restore_values() {
    let r = root();
    let mut source = String::from(
        r#"import std.numeric as numeric;
 fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
 let shape=List<Int>();shape.add(1000000);var array=take(numeric.zerosFloat(&shape));let index=List<Int>();index.add(0);
 "#,
    );
    for i in 0..64 {
        source.push_str(&format!(
            "commit c{i};array=take(numeric.withFloat(&array,&index,{}.0));\n",
            i + 1
        ));
    }
    source.push_str("Out.println(take(numeric.getFloat(&array,&index)));publish;revert c0;Out.println(take(numeric.getFloat(&array,&index)));publish;\n");
    for i in 0..64 {
        source.push_str(&format!("drop c{i};\n"));
    }
    fs::write(r.join("main.rw"), source).unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    let out = call(
        &r,
        &[
            "run",
            "main.rwc",
            "--history-memory",
            "16MiB",
            "--native-work",
            "100000000",
            "--steps",
            "10000000",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"64\n0\n");
    let replay = call(&r, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    let too_small = call(
        &r,
        &[
            "run",
            "main.rwc",
            "--history-memory",
            "1MiB",
            "--native-work",
            "100000000",
        ],
    );
    assert!(!too_small.status.success());
    assert!(String::from_utf8_lossy(&too_small.stderr).contains("HistoryMemory"));
    fs::remove_dir_all(r).unwrap();
}
