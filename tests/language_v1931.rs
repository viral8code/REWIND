use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1931-{}-{}",
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
fn run(root: &PathBuf, source: &str, extras: &[&str]) -> Output {
    fs::write(root.join("main.rw"), source).unwrap();
    let mut args = vec![
        "run",
        "main.rw",
        "--steps",
        "20000000",
        "--task-steps",
        "2000000",
        "--native-work",
        "100000000",
    ];
    args.extend_from_slice(extras);
    call(root, &args)
}
#[test]
fn vector_views_handoff_checkpoint_and_source_free_replay() {
    let r = root();
    fs::write(
        r.join("main.rw"),
        include_str!("../examples/vector-async/main.rw"),
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
                "100000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "98304\n98304\nvector done\n"
        );
        let trace: serde_json::Value =
            serde_json::from_slice(&fs::read(r.join("trace.json")).unwrap()).unwrap();
        assert!(trace["schedule_choices"].as_array().unwrap().len() > 10);
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(replay.stdout, out.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn stable_norm_empty_and_late_division_failure_keep_inputs() {
    let r = root();
    let out = run(
        &r,
        r#"import std.numeric as n;import std.numericAsync as jobs;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let small=List<Int>();small.add(1);let values=List<Float>();values.add(1e200);let large=take(n.fromFloat(&small,&values));
let shape=List<Int>();shape.add(8193);let broadcast=take(n.broadcastFloat(&large,&shape));
assert_eq(take(take(await spawn jobs.norm2(broadcast))),take(n.norm2(&broadcast)));
let emptyShape=List<Int>();emptyShape.add(0);let empty=take(n.zerosFloat(&emptyShape));
assert_eq(take(take(await spawn jobs.norm2(empty))),0.0);assert_eq(take(take(await spawn jobs.scale(empty,7.0))),empty);
let oneValues=List<Float>();oneValues.add(1.0);let one=take(n.fromFloat(&small,&oneValues));let ones=take(n.broadcastFloat(&one,&shape));
let writable=take(n.materializeFloat(&ones));let index=List<Int>();index.add(8192);let denominator=take(n.withFloat(&writable,&index,0.0));
match take(await spawn jobs.zipFloat("div",ones,denominator)){Err(e)=>{assert_eq(e.code,"NumericDomain");},_=>{panic("late error escaped");}}
assert_eq(take(n.getFloat(&ones,&index)),1.0);assert_eq(take(n.getFloat(&denominator,&index)),0.0);
match take(await spawn jobs.zipFloat("invalid",ones,ones)){Err(e)=>{assert_eq(e.code,"NumericDomain");},_=>{panic("invalid operation accepted");}}
Out.println("checked");publish;"#,
        &[],
    );
    ok(&out);
    assert_eq!(out.stdout, b"checked\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn virtual_zero_and_matmul_init_admit_physical_storage_not_logical_size() {
    let r = root();
    let out = run(
        &r,
        r#"import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let huge=List<Int>();huge.add(16777216);let zero=take(n.zerosFloat(&huge));let izero=take(n.zerosInt(&huge));let last=List<Int>();last.add(16777215);
assert_eq(take(n.getFloat(&zero,&last)),0.0);assert_eq(take(n.getInt(&izero,&last)),0);
let ls=List<Int>();ls.add(4096);ls.add(1);let rs=List<Int>();rs.add(1);rs.add(4096);
let left=take(n.zerosFloat(&ls));let right=take(n.zerosFloat(&rs));let work=spawn jobs.matmul(left,right);task.yieldNow();assert(!work.isDone());work.cancel();
match await work{Err(TaskError::Cancelled)=>{},_=>{panic("cancel");}}Out.println("admitted");publish;"#,
        &["--history-memory", "2MiB", "--native-work", "300000"],
    );
    ok(&out);
    assert_eq!(out.stdout, b"admitted\n");
    let failed = run(
        &r,
        "import std.numeric as n;let s=List<Int>();s.add(16777216);n.zerosFloat(&s);publish;",
        &["--history-memory", "16KiB"],
    );
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("HistoryMemory"));
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn large_cg_checkpoint_restores_result_and_cancel_releases_private_solver_state() {
    let r = root();
    let out = run(
        &r,
        r#"import std.numeric as n;import std.sparse as sparse;import std.sparseAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let size=8193;let shape=List<Int>();shape.add(size);let os=List<Int>();os.add(size+1);
let offsets=List<Int>();let indices=List<Int>();for i in 0..size{offsets.add(i);indices.add(i);}offsets.add(size);
let small=List<Int>();small.add(1);let v=List<Float>();v.add(1.0);let scalar=take(n.fromFloat(&small,&v));let ones=take(n.broadcastFloat(&scalar,&shape));
let matrix=sparse.Matrix(size,size,take(n.fromInt(&os,&offsets)),take(n.fromInt(&shape,&indices)),ones);
let work=spawn jobs.conjugateGradient(matrix,ones,1e-12,8);
for turn in 0..14{task.yieldNow();}assert(!work.isDone());commit solver;
{let first=take(take(await work));assert(first.converged);assert_eq(first.iterations,1);assert_eq(first.solution,take(n.materializeFloat(&ones)));assert_eq(first.residual,0.0);}
revert solver;
{let restored=take(take(await work));assert(restored.converged);assert_eq(restored.solution,take(n.materializeFloat(&ones)));}drop solver;
let cancelled=spawn jobs.conjugateGradient(matrix,ones,1e-12,8);for turn in 0..14{task.yieldNow();}assert(!cancelled.isDone());cancelled.cancel();
match await cancelled{Err(TaskError::Cancelled)=>{},_=>{panic("solver cancel");}}Out.println("solved");publish;"#,
        &[],
    );
    ok(&out);
    assert_eq!(out.stdout, b"solved\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn cancelled_vector_and_norm_tasks_release_pages_without_unbounded_quota_metadata() {
    for count in [128, 512] {
        let r = root();
        let source = format!(
            r#"import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {{}} {{match move r{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
let small=List<Int>();small.add(1);let v=List<Float>();v.add(2.0);let scalar=take(n.fromFloat(&small,&v));let shape=List<Int>();shape.add(16384);let repeated=take(n.broadcastFloat(&scalar,&shape));var done=0;
for i in 0..{count}{{let work=spawn jobs.scale(repeated,3.0);task.yieldNow();assert(!work.isDone());work.cancel();match await work{{Err(TaskError::Cancelled)=>{{done+=1;}},_=>{{panic("cancel");}}}}
let norm=spawn jobs.norm2(repeated);task.yieldNow();assert(!norm.isDone());norm.cancel();match await norm{{Err(TaskError::Cancelled)=>{{}},_=>{{panic("norm cancel");}}}}}}
Out.println(done);publish;"#
        );
        fs::write(r.join("main.rw"), source).unwrap();
        let out = call(
            &r,
            &[
                "profile",
                "main.rw",
                "--native-work",
                "500000000",
                "--steps",
                "20000000",
                "--history-memory",
                "8MiB",
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            count.to_string()
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        let profile: serde_json::Value =
            serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap())
                .unwrap();
        assert!(
            profile["numeric_pages"]["live_bytes"].as_u64().unwrap() < 1024 * 1024,
            "{profile}"
        );
        let quotas = profile["task_instructions"].as_object().unwrap();
        assert!(quotas.len() < 64, "{profile}");
        assert_eq!(
            quotas.len(),
            profile["scheduler"]["tasks"].as_object().unwrap().len()
        );
        assert!(profile["gc"]["completed"].as_u64().unwrap() > 0);
        fs::remove_dir_all(r).unwrap();
    }
}
