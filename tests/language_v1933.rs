use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1933-{}-{}",
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
const PRELUDE: &str = r#"import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
"#;
#[test]
fn old_work_type_name_remains_user_defined_and_new_work_cannot_be_forged() {
    let r = root();
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.32\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(
        r.join("main.rw"),
        "record SolveWork{code:Int}let x=SolveWork(7);assert_eq(x.code,7);publish;",
    )
    .unwrap();
    ok(&call(&r, &["update"]));
    ok(&call(&r, &["run", "main.rw"]));
    fs::remove_dir_all(r).unwrap();
    for source in [
        "let forged=SolveWork();publish;",
        "record SolveWork{code:Int}publish;",
    ] {
        let r = root();
        fs::write(r.join("main.rw"), source).unwrap();
        assert!(!call(&r, &["compile", "main.rw"]).status.success());
        fs::remove_dir_all(r).unwrap();
    }
}
#[test]
fn fatal_native_work_budget_is_not_converted_to_a_numeric_result() {
    let r = root();
    let source = format!(
        "{PRELUDE}{}",
        r#"
let s=List<Int>();s.add(65);s.add(65);let a=take(n.zerosFloat(&s));let rs=List<Int>();rs.add(65);let b=take(n.zerosFloat(&rs));
stdNumericSolveInit(&a,&b,0.0);Out.println("unreachable");publish;
"#
    );
    let out = run(&r, &source, &["--native-work", "4000"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("NativeWork"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty());
    let child=source.replace("stdNumericSolveInit(&a,&b,0.0);Out.println(\"unreachable\");", "let work=spawn jobs.solve(a,b,0.0);match await work{Err(TaskError::BudgetExceeded(BudgetKind::NativeWork))=>{},_=>{panic(\"numeric result escaped\");}}Out.println(\"task failed\");");
    let out = run(&r, &child, &["--native-work", "4000"]);
    ok(&out);
    assert_eq!(out.stdout, b"task failed\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn lu_task_checkpoint_pivot_handoff_and_source_free_replay() {
    let r = root();
    fs::write(
        r.join("main.rw"),
        include_str!("../examples/solve-async/main.rw"),
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
            "solved\nrestored\nsolve done\n"
        );
        let trace: serde_json::Value =
            serde_json::from_slice(&fs::read(r.join("trace.json")).unwrap()).unwrap();
        assert!(trace["schedule_choices"].as_array().unwrap().len() > 30);
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(replay.stdout, out.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn checkpoint_inside_elimination_restores_running_task_without_changing_inputs() {
    let r = root();
    let setup = include_str!("../examples/solve-async/main.rw")
        .split("let work=spawn")
        .next()
        .unwrap();
    let source = format!(
        "{setup}{}",
        r#"
let work=spawn jobs.solve(matrix,rhs,1e-13);for turn in 0..6{task.yieldNow();}assert(!work.isDone());commit elimination;
{let first=take(take(await work));assert_eq(first,take(numeric.solve(&matrix,&rhs,1e-13)));}
revert elimination;assert(!work.isDone());
{let restored=take(take(await work));assert_eq(restored,take(numeric.solve(&matrix,&rhs,1e-13)));}drop elimination;
let index=List<Int>();index.add(0);index.add(1);assert_eq(take(numeric.getFloat(&matrix,&index)),100.0);
Out.println("elimination restored");publish;
"#
    );
    let out = run(&r, &source, &[]);
    ok(&out);
    assert_eq!(out.stdout, b"elimination restored\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn lu_empty_singular_shape_and_tolerance_failures_are_typed() {
    let r = root();
    let source = format!(
        "{PRELUDE}{}",
        r#"
let matrixShape=List<Int>();matrixShape.add(0);matrixShape.add(0);let vectorShape=List<Int>();vectorShape.add(0);
let empty=take(n.zerosFloat(&matrixShape));let emptyRhs=take(n.zerosFloat(&vectorShape));
assert_eq(take(take(await spawn jobs.solve(empty,emptyRhs,0.0))),emptyRhs);
matrixShape.set(0,2);matrixShape.set(1,2);vectorShape.set(0,2);
let singular=take(n.zerosFloat(&matrixShape));let rhs=take(n.zerosFloat(&vectorShape));
match take(await spawn jobs.solve(singular,rhs,0.0)){Err(e)=>{assert_eq(e.code,"NumericSingular");},_=>{panic("singular escaped");}}
match take(await spawn jobs.solve(singular,rhs,-1.0)){Err(e)=>{assert_eq(e.code,"NumericShape");},_=>{panic("tolerance escaped");}}
match take(await spawn jobs.solve(rhs,rhs,0.0)){Err(e)=>{assert_eq(e.code,"NumericShape");},_=>{panic("shape escaped");}}
assert_eq(take(n.sum(&singular)),0.0);assert_eq(take(n.sum(&rhs)),0.0);Out.println("checked");publish;
"#
    );
    let out = run(&r, &source, &[]);
    ok(&out);
    assert_eq!(out.stdout, b"checked\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn cancelled_lu_tasks_release_private_pages_and_quota_metadata() {
    let r = root();
    let source = format!(
        "{PRELUDE}{}",
        r#"
let shape=List<Int>();shape.add(65);shape.add(65);let matrix=take(n.zerosFloat(&shape));
let rhsShape=List<Int>();rhsShape.add(65);let rhs=take(n.zerosFloat(&rhsShape));var done=0;
for i in 0..128{let work=spawn jobs.solve(matrix,rhs,0.0);task.yieldNow();assert(!work.isDone());work.cancel();
match await work{Err(TaskError::Cancelled)=>{done+=1;},_=>{panic("cancel");}}}
Out.println(done);publish;
"#
    );
    fs::write(r.join("main.rw"), source).unwrap();
    let out = call(
        &r,
        &[
            "profile",
            "main.rw",
            "--native-work",
            "100000000",
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
