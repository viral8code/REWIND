use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1910-{}-{}",
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
fn run(root: &Path, source: &str) -> Output {
    fs::write(root.join("main.rw"), source).unwrap();
    call(
        root,
        &[
            "run",
            "main.rw",
            "--steps",
            "20000000",
            "--task-steps",
            "2000000",
            "--native-work",
            "100000000",
        ],
    )
}
#[test]
fn explicit_handoff_rotates_three_ready_tasks() {
    let r = root();
    let out = run(
        &r,
        r#"import std.task as task;
 async fn work(id:Int)->Unit effects {tasks,output} {for i in 0..3 {Out.println(id);task.yieldNow();}}
 let a=spawn work(1);let b=spawn work(2);let c=spawn work(3);
 assert_eq(await a,Ok(()));assert_eq(await b,Ok(()));assert_eq(await c,Ok(()));publish;"#,
    );
    ok(&out);
    assert_eq!(out.stdout, b"1\n2\n3\n1\n2\n3\n1\n2\n3\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn cooperative_numeric_checkpoint_and_source_free_replay() {
    let r = root();
    fs::write(
        r.join("main.rw"),
        include_str!("../examples/numeric-async/main.rw"),
    )
    .unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    for mode in ["debug", "compact"] {
        let out = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
                "--native-work",
                "100000000",
                "--steps",
                "20000000",
                "--task-steps",
                "2000000",
            ],
        );
        ok(&out);
        assert_eq!(out.stdout, b"9000\n32\n32\n");
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
        let mut trace: serde_json::Value =
            serde_json::from_slice(&fs::read(r.join("trace.json")).unwrap()).unwrap();
        assert!(trace["schedule_choices"].as_array().unwrap().len() > 3);
        trace["schedule_choices"][0] = serde_json::json!(255);
        fs::write(r.join("tampered.json"), serde_json::to_vec(&trace).unwrap()).unwrap();
        let bad = call(&r, &["replay", "tampered.json"]);
        assert!(!bad.status.success());
        assert!(String::from_utf8_lossy(&bad.stderr).contains("ScheduleMismatch"));
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn cancel_after_first_native_chunk_returns_cancelled_task() {
    let r = root();
    let out = run(
        &r,
        r#"import std.numeric as numeric;import std.numericAsync as jobs;import std.task as task;
 fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
 let shape=List<Int>();shape.add(128);shape.add(128);let zeros=take(numeric.zerosFloat(&shape));let a=take(numeric.mapFloat("exp",&zeros));
 let work=spawn jobs.matmul(a,a);task.yieldNow();assert(!work.isDone());work.cancel();
 match await work {Err(TaskError::Cancelled)=>{Out.println("cancelled");},_=>{panic("cancellation failed");}}publish;"#,
    );
    ok(&out);
    assert_eq!(out.stdout, b"cancelled\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn handoff_is_forbidden_in_external_cleanup_and_guards() {
    for source in [
  "import std.task as task;external {task.yieldNow();}",
  "import std.task as task;fn hop()->Unit effects {tasks}{task.yieldNow();}external {hop();}",
  "import std.task as task;fn start()->Unit effects {tasks}{defer ||->Unit {task.yieldNow();};}start();",
  "import std.task as task;fn check()->Bool effects {tasks}{task.yieldNow();return true;}match 0{0 if check()=>{},_=>{}}"
 ] {
  let r=root();fs::write(r.join("main.rw"),source).unwrap();let out=call(&r,&["compile","main.rw"]);assert!(!out.status.success(),"{source}");
  let error=String::from_utf8_lossy(&out.stderr);assert!(error.contains("ExternalBoundary")||error.contains("cleanup")||error.contains("guard"),"{error}");fs::remove_dir_all(r).unwrap();
 }
}

#[test]
fn compact_replay_restores_execution_budget_and_rejects_invalid_budgets() {
    let r = root();
    fs::write(
        r.join("main.rw"),
        "var total=0;for i in 0..150000 {total+=i;}Out.println(total);publish;",
    )
    .unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    let out = call(
        &r,
        &[
            "run",
            "main.rwc",
            "--steps",
            "4000000",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"11249925000\n");
    let trace: serde_json::Value =
        serde_json::from_slice(&fs::read(r.join("trace.json")).unwrap()).unwrap();
    assert!(trace["execution"]["steps"].as_u64().unwrap() > 1000000);
    let replay = call(&r, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    for (name, value) in [
        ("execution_steps", serde_json::json!(0)),
        ("native_work", serde_json::json!("invalid")),
    ] {
        let mut bad = trace.clone();
        bad[name] = value;
        fs::write(r.join("bad.json"), serde_json::to_vec(&bad).unwrap()).unwrap();
        let out = call(&r, &["replay", "bad.json"]);
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("ReplayMismatch: invalid"));
    }
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn named_gui_polling_remains_available_during_cooperative_matrix_task() {
    let r = root();
    let source = r#"import std.gui as gui;import std.guiWindows as windows;import std.numeric as numeric;import std.numericAsync as jobs;import std.task as task;
 fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
 let view=take(gui.window("Calculation",320,160));take(gui.label(&mut view,"state","Calculating",10,10,280,36));take(windows.present("work",&view));publish;
 let shape=List<Int>();shape.add(128);shape.add(128);let zeros=take(numeric.zerosFloat(&shape));let ones=take(numeric.mapFloat("exp",&zeros));
 let work=spawn jobs.matmul(ones,ones);task.yieldNow();assert(!work.isDone());
 match take(windows.pollAny()){Some(e)=>{assert_eq(e.window,"work");assert_eq(e.event.kind,"close");},None=>{panic("input did not progress");}}
 work.cancel();match await work{Err(TaskError::Cancelled)=>{Out.println("cancelled");},_=>{panic("cancellation failed");}}
 take(windows.close("work"));publish;"#;
    fs::write(r.join("main.rw"), source).unwrap();
    ok(&call(&r, &["compile", "main.rw", "--allow-effects", "gui"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    for mode in ["debug", "compact"] {
        fs::write(r.join("events.json"),r#"[{"window":"work","event":{"kind":"close","x":0,"y":0,"key":"","width":0,"height":0}}]"#).unwrap();
        let out = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--allow-effects",
                "gui",
                "--gui-window-events",
                "events.json",
                "--native-work",
                "100000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(out.stdout, b"cancelled\n");
        fs::remove_file(r.join("events.json")).unwrap();
        let replay = call(&r, &["replay", "trace.json", "--allow-effects", "gui"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
