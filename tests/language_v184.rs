use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v184-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    root
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn datetime_source_free_replay_and_dst_map_key() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/datetime/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(output.stdout,b"3600\n2024-11-03T06:30:00.123456789Z\n2024-11-03T05:30:00.123456789Z\n32400\nsame instant\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn datetime_cannot_be_forged_or_read_clock_from_a_pure_function() {
    for source in [
        "let forged=Instant();",
        "let forged=Duration();",
        "record Instant {}",
        "fn stdDateTimeParse(value:String)->Int effects {} {return 0;}",
        "import std.clock as clock;fn hidden()->Result<Instant,StdError> effects {} {return clock.now();}",
    ] {
        let root=dir();fs::write(root.join("main.rw"),source).unwrap();
        assert!(!call(&root,&["compile","main.rw"]).status.success());
        fs::remove_dir_all(root).unwrap();
    }
}
