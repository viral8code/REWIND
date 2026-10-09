use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str, language: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v1955-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("main.rw"),
        source.replace("import std.", "import "),
    )
    .unwrap();
    for name in ["numeric", "numericReduceAsync", "task"] {
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("libraries/std")
                .join(format!("{name}.rw")),
            root.join(format!("{name}.rw")),
        )
        .unwrap();
    }
    fs::write(root.join("rewind.toml"), format!("language = \"{language}\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"tasks,output\"\n")).unwrap();
    ok(&call(&root, &["update", "--root", "."]));
    root
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reductions_handoff_checkpoint_and_source_free_recording() {
    let root = std::env::temp_dir().join(format!("rewind-reductions-smoke-{}", std::process::id()));
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            repo.join("scripts/smoke-reductions-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            root.as_os_str(),
            repo.join("examples/reductions-async/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn reduction_empty_ddof_scalar_and_raw_state_errors_are_typed() {
    let source = r#"import std.numeric as n;import std.numericReduceAsync as jobs;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(0);let empty=take(n.zerosFloat(&shape));
assert_eq(take(take(await spawn jobs.sum(empty))),0.0);
match take(await spawn jobs.mean(empty)){Err(e)=>{assert_eq(e.code,"NumericEmpty");},_=>{panic("empty mean");}}
match take(await spawn jobs.variance(empty,-1)){Err(e)=>{assert_eq(e.code,"NumericIndex");},_=>{panic("negative ddof");}}
let scalarShape=List<Int>();let values=List<Float>();values.add(7.0);let scalar=take(n.fromFloat(&scalarShape,&values));
assert_eq(take(take(await spawn jobs.sum(scalar))),7.0);assert_eq(take(take(await spawn jobs.mean(scalar))),7.0);assert_eq(take(take(await spawn jobs.variance(scalar,0))),0.0);
match take(await spawn jobs.variance(scalar,1)){Err(e)=>{assert_eq(e.code,"NumericEmpty");},_=>{panic("ddof count");}}
match stdNumericSumStep(&scalar,-1,0.0,0.0){Err(e)=>{assert_eq(e.code,"NumericIndex");},_=>{panic("negative cursor");}}
match stdNumericMomentsStep(&scalar,0,2,0.0,0.0){Err(e)=>{assert_eq(e.code,"NumericIndex");},_=>{panic("late cursor");}}
Out.println(true);publish;"#;
    let root = root(source, "1.9.55");
    let result = call(
        &root,
        &[
            "run",
            "--root",
            ".",
            "--steps",
            "20000000",
            "--native-work",
            "100000000",
        ],
    );
    ok(&result);
    assert_eq!(result.stdout, b"true\n");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn previous_selected_language_rejects_new_reduction_primitives() {
    let root = root(
        r#"fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}let shape=List<Int>();shape.add(0);let x=take(stdNumericZerosFloat(&shape));stdNumericSumStep(&x,0,0.0,0.0);publish;"#,
        "1.9.54",
    );
    let result = call(&root, &["check", "--root", "."]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("require language 1.9.55"));
    assert!(result.stdout.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn repeated_cancel_reclaims_shared_inputs_and_completed_task_metadata() {
    for count in [512, 2048] {
        let source = format!(
            r#"import std.numeric as n;import std.numericReduceAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {{}} {{match move r{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
let shape=List<Int>();shape.add(16384);let zero=take(n.zerosFloat(&shape));var done=0;
for i in 0..{count}{{let work=spawn jobs.sum(zero);task.yieldNow();assert(!work.isDone());work.cancel();match await work{{Err(TaskError::Cancelled)=>{{done+=1;}},_=>{{panic("cancel");}}}}}}
Out.println(done);publish;"#
        );
        let root = root(&source, "1.9.55");
        let result = call(
            &root,
            &[
                "profile",
                "--root",
                ".",
                "--history-memory",
                "8MiB",
                "--steps",
                "30000000",
                "--native-work",
                "1000000000",
            ],
        );
        ok(&result);
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).trim(),
            count.to_string()
        );
        let stderr = String::from_utf8(result.stderr).unwrap();
        let profile: serde_json::Value = serde_json::from_str(
            stderr
                .lines()
                .rev()
                .find(|line| line.starts_with('{'))
                .unwrap(),
        )
        .unwrap();
        assert!(profile["gc"]["completed"].as_u64().unwrap() > 0);
        assert!(profile["numeric_pages"]["live_bytes"].as_u64().unwrap() < 4 * 1024 * 1024);
        fs::remove_dir_all(root).unwrap();
    }
}
