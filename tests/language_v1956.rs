use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str, language: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v1956-{}-{}",
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
        "graphLargeAsync",
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
fn graph_handoff_checkpoint_and_source_free_recording() {
    let root = std::env::temp_dir().join(format!("rewind-graph-smoke-{}", std::process::id()));
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            repo.join("scripts/smoke-graph-async-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            root.as_os_str(),
            repo.join("examples/graph-async/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn graph_empty_isolated_shape_and_late_endpoint_errors_are_typed() {
    let source = r#"import std.numeric as n;import std.numericRange as range;import std.numericIndex as index;import std.graphLargeAsync as jobs;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let empty=take(range.integers(0,0,0));
match take(await spawn jobs.bfs(0,empty,empty,0)){Err(e)=>{assert_eq(e.code,"NumericIndex");},_=>{panic("empty source");}}
let isolated=take(take(await spawn jobs.bfs(3,empty,empty,1)));
assert_eq(take(index.getInt(&isolated,0)),-1);assert_eq(take(index.getInt(&isolated,1)),0);assert_eq(take(index.getInt(&isolated,2)),-1);
let froms=take(range.integers(0,0,8193));let tos=take(index.withInt(&froms,8192,2));
match take(await spawn jobs.bfs(2,froms,tos,0)){Err(e)=>{assert_eq(e.code,"NumericIndex");},_=>{panic("late endpoint");}}
match take(await spawn jobs.bfs(2,froms,empty,0)){Err(e)=>{assert_eq(e.code,"NumericShape");},_=>{panic("different lengths");}}
match take(await spawn jobs.bfs(1048577,empty,empty,0)){Err(e)=>{assert_eq(e.code,"NumericSize");},_=>{panic("size");}}
let unfinished=take(stdNumericGraphBfsInit(3,&empty,&empty,1));
match stdNumericGraphBfsResult(&unfinished){Err(e)=>{assert_eq(e.code,"NumericDomain");},_=>{panic("partial result");}}
assert(!take(stdNumericGraphBfsDone(&unfinished)));Out.println(true);publish;"#;
    let root = root(source, "1.9.56");
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
fn previous_language_rejects_graph_work_and_new_raw_primitives() {
    let user_record = root(
        "record GraphBfsWork {value:Int} let item=GraphBfsWork(7);Out.println(item.value);publish;",
        "1.9.55",
    );
    let old = call(&user_record, &["run", "--root", "."]);
    ok(&old);
    assert_eq!(old.stdout, b"7\n");
    fs::remove_dir_all(user_record).unwrap();
    let shadow = root("record GraphBfsWork {value:Int} publish;", "1.9.56");
    let rejected = call(&shadow, &["check", "--root", "."]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("reserved graph BFS work type"));
    fs::remove_dir_all(shadow).unwrap();
    let root = root(
        r#"fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}let shape=List<Int>();shape.add(0);let x=take(stdNumericZerosInt(&shape));stdNumericGraphBfsInit(1,&x,&x,0);publish;"#,
        "1.9.55",
    );
    let result = call(&root, &["check", "--root", "."]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("requires language 1.9.56"));
    assert!(result.stdout.is_empty());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn repeated_cancel_collects_graph_work_arrays_and_task_state() {
    for count in [256, 1024] {
        let source = format!(
            r#"import std.numericRange as range;import std.graphLargeAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {{}} {{match move r{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
let froms=take(range.integers(0,0,16384));let tos=froms;var done=0;
for i in 0..{count}{{let work=spawn jobs.bfs(16384,froms,tos,0);task.yieldNow();assert(!work.isDone());work.cancel();match await work{{Err(TaskError::Cancelled)=>{{done+=1;}},_=>{{panic("cancel");}}}}}}
Out.println(done);publish;"#
        );
        let root = root(&source, "1.9.56");
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
        let profile: serde_json::Value =
            serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap())
                .unwrap();
        assert!(profile["gc"]["completed"].as_u64().unwrap() > 0);
        assert!(profile["numeric_pages"]["live_bytes"].as_u64().unwrap() < 4 * 1024 * 1024);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn fixture_gui_input_progresses_while_graph_task_is_pending_and_replays_without_input() {
    let root = std::env::temp_dir().join(format!("rewind-graph-gui-smoke-{}", std::process::id()));
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            repo.join("scripts/smoke-graph-gui-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            root.as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn native_pointer_cancels_large_pending_graph_and_replays_without_display() {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return;
    }
    if !cfg!(any(target_os = "linux", windows)) {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "rewind-graph-native-gui-smoke-{}",
        std::process::id()
    ));
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            repo.join("scripts/smoke-graph-native-gui-sdk.py")
                .as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            root.as_os_str(),
            repo.join("examples/graph-gui/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn huge_virtual_edgeless_graph_can_cancel_after_first_bounded_conversion_under_small_budget() {
    let source = r#"import std.numericRange as range;import std.graphLargeAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let empty=take(range.integers(0,0,0));let work=spawn jobs.bfs(1048576,empty,empty,1048575);
task.yieldNow();assert(!work.isDone());work.cancel();match await work{Err(TaskError::Cancelled)=>{Out.println(true);},_=>{panic("cancel");}}publish;"#;
    let root = root(source, "1.9.56");
    let result = call(
        &root,
        &[
            "run",
            "--root",
            ".",
            "--history-memory",
            "8MiB",
            "--native-work",
            "10000000",
        ],
    );
    ok(&result);
    assert_eq!(result.stdout, b"true\n");
    fs::remove_dir_all(root).unwrap();
}
