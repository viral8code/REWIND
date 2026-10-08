use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str, language: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v1953-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("main.rw"),
        source.replace("import std.", "import "),
    )
    .unwrap();
    for name in ["numeric", "numericTransformAsync", "task"] {
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
fn unary_views_checkpoint_handoff_and_source_free_recording() {
    let root = std::env::temp_dir().join(format!("rewind-unary-smoke-{}", std::process::id()));
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            repo.join("scripts/smoke-unary-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            root.as_os_str(),
            repo.join("examples/unary-async/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn transform_operation_names_empty_domains_and_late_errors_are_typed() {
    let source = r#"import std.numeric as n;import std.numericTransformAsync as jobs;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let oneShape=List<Int>();oneShape.add(1);let values=List<Float>();values.add(0.5);let scalar=take(n.fromFloat(&oneShape,&values));
let shape=List<Int>();shape.add(4097);let input=take(n.broadcastFloat(&scalar,&shape));
let names=List<String>();names.add("sqrt");names.add("exp");names.add("expM1");names.add("log");names.add("log1p");names.add("sin");names.add("cos");names.add("tan");names.add("asin");names.add("acos");names.add("atan");names.add("sinh");names.add("cosh");names.add("tanh");names.add("abs");names.add("floor");names.add("ceil");names.add("trunc");names.add("round");names.add("roundEven");
for name in names{assert_eq(take(take(await spawn jobs.mapFloat(name,input))),take(n.mapFloat(name,&input)));}
let activation=List<String>();activation.add("relu");activation.add("reluGrad");activation.add("sigmoid");activation.add("sigmoidGrad");activation.add("tanhGrad");activation.add("reciprocal");
for name in activation{assert_eq(take(take(await spawn jobs.activation(name,input))),take(stdNumericActivation(name,&input)));}
let emptyShape=List<Int>();emptyShape.add(0);let empty=take(n.zerosFloat(&emptyShape));
assert_eq(take(take(await spawn jobs.mapFloat("sqrt",empty))),empty);
match take(await spawn jobs.mapFloat("relu",empty)){Err(e)=>{assert_eq(e.code,"NumericDomain");},_=>{panic("wrong namespace");}}
let writable=take(n.materializeFloat(&input));let last=List<Int>();last.add(4096);let invalid=take(n.withFloat(&writable,&last,-1.0));
match take(await spawn jobs.mapFloat("log",invalid)){Err(e)=>{assert_eq(e.code,"NumericDomain");},_=>{panic("late error escaped");}}
assert_eq(take(n.getFloat(&invalid,&last)),-1.0);assert_eq(take(n.getFloat(&input,&last)),0.5);
Out.println(true);publish;"#;
    let root = root(source, "1.9.53");
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
fn previous_selected_language_rejects_new_unary_primitives() {
    let root = root(
        r#"fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("zeros");}}}
let s=List<Int>();s.add(0);let x=take(stdNumericZerosFloat(&s));stdNumericUnaryInit("map:exp",&x);publish;"#,
        "1.9.52",
    );
    let result = call(&root, &["check", "--root", "."]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("require language 1.9.53"));
    assert!(result.stdout.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn repeated_cancel_reclaims_partial_pages_and_completed_task_metadata() {
    for count in [128, 512] {
        let source = format!(
            r#"import std.numeric as n;import std.numericTransformAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {{}} {{match move r{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
let shape=List<Int>();shape.add(16384);let zero=take(n.zerosFloat(&shape));var done=0;
for i in 0..{count}{{let work=spawn jobs.activation("sigmoid",zero);task.yieldNow();assert(!work.isDone());work.cancel();match await work{{Err(TaskError::Cancelled)=>{{done+=1;}},_=>{{panic("cancel");}}}}}}
Out.println(done);publish;"#
        );
        let root = root(&source, "1.9.53");
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
