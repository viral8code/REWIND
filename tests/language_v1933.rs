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
Out.println("elimination restored");
return 0;
}
"#
    );
    let out = run(&r, &source, &[]);
    ok(&out);
    assert_eq!(out.stdout, b"elimination restored\n");
    fs::remove_dir_all(r).unwrap();
}
