use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str, language: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v1957-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("main.rw"),
        source.replace("import std.", "import "),
    )
    .unwrap();
    for name in [
        "numeric",
        "numericRange",
        "numericIndex",
        "numericShapeAsync",
        "task",
    ] {
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
fn contraction_and_logical_copy_restore_unfinished_tasks_and_source_free_replay() {
    let root = std::env::temp_dir().join(format!("rewind-shape-smoke-{}", std::process::id()));
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            repo.join("scripts/smoke-shape-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            root.as_os_str(),
            repo.join("examples/shape-async/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn shape_empty_scalar_bad_dimensions_and_progress_are_typed() {
    let source = r#"import std.numeric as n;import std.numericShapeAsync as jobs;import std.numericIndex as index;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
fn dims(n:Int)->List<Int> effects {} {let d=List<Int>();if n>=0{d.add(n);}return move d;}
let shape=dims(0);let empty=take(n.zerosFloat(&shape));
let zero=take(take(await spawn jobs.sumToShape(empty,dims(-1))));assert_eq(take(index.getFloat(&zero,0)),0.0);
match take(await spawn jobs.reshapeLogical(empty,dims(-1))){Err(e)=>{assert_eq(e.code,"NumericShape");},_=>{panic("empty reshape");}}
let one=dims(1);let x=take(n.zerosFloat(&one));
match take(await spawn jobs.sumToShape(x,dims(2))){Err(e)=>{assert_eq(e.code,"NumericShape");},_=>{panic("dimension");}}
let init=take(stdNumericSumShapeInit(&x,&one));
match stdNumericSumShapeStep(&x,&init._0,&init._1,9,0){Err(e)=>{assert_eq(e.code,"NumericDomain");},_=>{panic("phase");}}
match stdNumericReshapeLogicalStep(&x,&x,-1){Err(e)=>{assert_eq(e.code,"NumericIndex");},_=>{panic("cursor");}}
Out.println(true);publish;"#;
    let root = root(source, "1.9.57");
    let result = call(&root, &["run", "--root", "."]);
    ok(&result);
    assert_eq!(result.stdout, b"true\n");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn previous_selected_language_rejects_new_shape_kernels() {
    let root=root("let shape=List<Int>();shape.add(0);match stdNumericZerosFloat(&shape){Ok(x)=>{stdNumericReshapeLogicalInit(&x,&shape);},_=>{}}publish;","1.9.56");
    let result = call(&root, &["check", "--root", "."]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("require language 1.9.57"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn repeated_cancel_releases_shape_accumulators_and_large_virtual_input_under_small_budget() {
    for count in [256, 1024] {
        let source = format!(
            r#"import std.numeric as n;import std.numericShapeAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {{}} {{match move r{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
fn dims(n:Int)->List<Int> effects {{}} {{let d=List<Int>();d.add(n);return move d;}}
let shape=dims(1048576);let x=take(n.zerosFloat(&shape));var done=0;
for i in 0..{count}{{let work=spawn jobs.sumToShape(x,dims(1048576));task.yieldNow();assert(!work.isDone());work.cancel();match await work{{Err(TaskError::Cancelled)=>{{done+=1;}},_=>{{panic("cancel");}}}}}}
Out.println(done);publish;"#
        );
        let root = root(&source, "1.9.57");
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
        let p: serde_json::Value =
            serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap())
                .unwrap();
        assert!(p["gc"]["completed"].as_u64().unwrap() > 0);
        assert!(p["numeric_pages"]["live_bytes"].as_u64().unwrap() < 4 * 1024 * 1024);
        fs::remove_dir_all(root).unwrap();
    }
}
