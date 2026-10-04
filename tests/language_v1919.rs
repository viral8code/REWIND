use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1919-{}-{}",
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

#[test]
fn gui_async_typed_event_cancellation_checkpoint_and_offline_source_free_replay() {
    let r = root();
    fs::write(r.join("main.rw"),r#"import std.gui as gui;import std.guiWindows as windows;import std.task as task;
 fn take<T,E>(r:Result<T,E>)->T effects {} {match move r {Ok(v)=>{return move v;},Err(_)=>{panic("GUI wait");}}}
 async fn work()->Int effects {tasks} {for i in 0..100 {task.yieldNow();}return 42;}
 let view=take(gui.window("Async",160,96));take(windows.present("one",&view));publish;
 let cancelled=windows.nextEventAnyAsync();cancelled.cancel();assert_eq(await cancelled,Err(TaskError::Cancelled));
 let input=windows.nextEventAnyAsync();let calculation=work();commit waiting;
 let ready=take(await task.selectReady(input,calculation));assert(ready==0 || ready==1);
 let event=take(take(await input));assert_eq(event.window,"one");assert_eq(event.event.kind,"key");assert_eq(event.event.key,"日本語");assert_eq(await calculation,Ok(42));
 revert waiting;let restored=take(take(await input));assert_eq(restored.event.key,"日本語");assert_eq(await calculation,Ok(42));drop waiting;
 assert_eq(await windows.nextEventAnyAsync(),Ok(Err(StdError("GuiWindowEventTapeEnd",0))));
 take(windows.close("one"));publish;Out.println("done");publish;"#).unwrap();
    ok(&call(&r, &["compile", "main.rw", "--allow-effects", "gui"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    for mode in ["compact", "debug"] {
        fs::write(r.join("events.json"),r#"[{"window":"one","event":{"kind":"key","x":0,"y":0,"key":"日本語","width":0,"height":0}}]"#).unwrap();
        let out = call(
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
        ok(&out);
        assert_eq!(out.stdout, b"done\n");
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
        assert_eq!(replay.stdout, out.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn gui_async_absent_windows_are_typed_and_staging_remains_main_task_only() {
    let r = root();
    fs::write(r.join("main.rw"),r#"import std.guiWindows as windows;assert_eq(await windows.nextEventAnyAsync(),Ok(Err(StdError("GuiWindowNotPublished",0))));"#).unwrap();
    ok(&call(&r, &["run", "main.rw", "--allow-effects", "gui"]));
    fs::write(r.join("main.rw"),r#"import std.guiWindows as windows;async fn forbidden()->Unit effects {gui,tasks} {let input=windows.nextEventAnyAsync();input.cancel();}"#).unwrap();
    let denied = call(&r, &["check", "main.rw", "--allow-effects", "gui"]);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("GuiMainTaskOnly"));
    fs::write(
        r.join("main.rw"),
        r#"import std.guiWindows as windows;external {let input=windows.nextEventAnyAsync();}"#,
    )
    .unwrap();
    assert!(
        !call(&r, &["check", "main.rw", "--allow-effects", "gui,external"])
            .status
            .success()
    );
    fs::remove_dir_all(r).unwrap();
}
