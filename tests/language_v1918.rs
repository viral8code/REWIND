use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1918-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&r).unwrap();
    r
}
fn call(r: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(r)
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

fn run(r: &Path, source: &str) -> Output {
    fs::write(r.join("main.rw"), source).unwrap();
    call(r, &["run", "main.rw", "--task-steps", "2000000"])
}
#[test]
fn heterogeneous_readiness_preserves_affine_result_and_other_input() {
    let r = root();
    let source = r#"import std.task as task;import std.models as models;
 async fn number()->Int effects {tasks} {task.yieldNow();return 42;}
 async fn model()->models.Model effects {} {match models.create(Map<String,FloatArray>()) {Ok(m)=>{return m;},Err(_)=>{panic("create");}}}
 let left=number();let right=model();
 assert_eq(await task.selectReady(left,right),Ok(1));
 match await right {Ok(actual)=>{assert_eq(models.weights(&actual).len(),0);},Err(_)=>{panic("model task");}}
 assert_eq(await left,Ok(42));
 async fn text()->String effects {} {return "ready";}
 let a=number();let b=text();assert_eq(await a,Ok(42));assert_eq(await b,Ok("ready"));
 assert_eq(await task.selectReady(a,b),Ok(0));
 Out.println("ready");publish;"#;
    ok(&run(&r, source));
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    for mode in ["compact", "debug"] {
        let o = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&o);
        assert_eq!(o.stdout, b"ready\n");
        let replay = call(&r, &["replay", "trace.json", "--root", "."]);
        ok(&replay);
        assert_eq!(replay.stdout, o.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn readiness_reports_failure_and_cancel_without_consuming_or_cancelling_inputs() {
    let r = root();
    ok(&run(
        &r,
        r#"import std.task as task;
 async fn bad()->String effects {} {panic("expected");return "";}
 async fn work()->Int effects {tasks} {task.yieldNow();return 7;}
 let a=bad();let b=work();assert_eq(await task.selectReady(a,b),Ok(0));
 match await a {Err(TaskError::Failed(_))=>{},_=>{panic("lost failure");}}
 assert_eq(await b,Ok(7));
 let c=work();let d=bad();let waiter=task.selectReady(c,d);waiter.cancel();
 assert_eq(await waiter,Err(TaskError::Cancelled));assert_eq(await c,Ok(7));
 d.cancel();assert_eq(await d,Err(TaskError::Cancelled));
 let e=work();e.cancel();let f=work();
 assert_eq(await task.selectReady(e,f),Ok(0));assert_eq(await e,Err(TaskError::Cancelled));assert_eq(await f,Ok(7));
 publish;"#,
    ));
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn readiness_checkpoint_and_language_gate() {
    let r = root();
    ok(&run(
        &r,
        r#"import std.task as task;
 async fn left()->Int effects {tasks} {task.yieldNow();return 3;}
 async fn right()->String effects {} {return "done";}
 let a=left();let b=right();let selection=task.selectReady(a,b);
 commit waiting;assert_eq(await selection,Ok(1));assert_eq(await b,Ok("done"));
 revert waiting;assert_eq(await selection,Ok(1));assert_eq(await b,Ok("done"));assert_eq(await a,Ok(3));drop waiting;publish;"#,
    ));
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.17\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(
        r.join("main.rw"),
        "async fn a()->Int effects {} {return 1;}let x=a();let y=a();let z=x.selectReady(y);",
    )
    .unwrap();
    let invalid = call(&r, &["check", "--root", "."]);
    assert!(!invalid.status.success());
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn repeated_empty_gui_poll_checkpoint_is_source_free_and_fixture_free() {
    let r = root();
    fs::write(r.join("main.rw"),r#"import std.gui as gui;import std.guiWindows as windows;
 fn take<T,E>(r:Result<T,E>)->T effects {} {match move r {Ok(v)=>{return move v;},Err(_)=>{panic("GUI");}}}
 let view=take(gui.window("Wait",160,96));take(windows.present("left",&view));take(windows.present("right",&view));publish;
 for i in 0..30 {assert_eq(windows.poll("left"),Ok(None));}
 commit inside;
 for i in 0..60 {assert_eq(windows.poll("left"),Ok(None));}
 revert inside;
 for i in 0..60 {assert_eq(windows.poll("left"),Ok(None));}
 windows.continueInput();let received=take(windows.nextEventAny());assert_eq(received.window,"right");assert_eq(received.event.kind,"close");
 take(windows.close("left"));take(windows.close("right"));drop inside;Out.println("done");publish;"#).unwrap();
    ok(&call(&r, &["compile", "main.rw", "--allow-effects", "gui"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    for mode in ["compact", "debug"] {
        fs::write(r.join("events.json"),r#"[{"window":"right","event":{"kind":"close","x":0,"y":0,"key":"","width":0,"height":0}}]"#).unwrap();
        let actual = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--allow-effects",
                "gui",
                "--gui-window-events",
                "events.json",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&actual);
        assert_eq!(actual.stdout, b"done\n");
        fs::remove_file(r.join("events.json")).unwrap();
        let replay = call(
            &r,
            &[
                "replay",
                "trace.json",
                "--root",
                ".",
                "--allow-effects",
                "gui",
            ],
        );
        ok(&replay);
        assert_eq!(actual.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
