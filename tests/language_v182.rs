use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v182-{}-{}",
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
fn exact_integer_example_is_source_free_and_replayable() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/bigint/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(output.stdout,b"1267650600228229401496703205378\n1267650600228229401496703205376\n\"1267650600228229401496703205376\"\n1\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn bigint_constructor_and_names_cannot_be_forged() {
    for source in [
        "let forged=BigInt();",
        "record BigInt {}",
        "fn stdBigIntFromInt(value:Int)->Int effects {} {return value;}",
    ] {
        let root = dir();
        fs::write(root.join("main.rw"), source).unwrap();
        assert!(!call(&root, &["compile", "main.rw"]).status.success());
        fs::remove_dir_all(root).unwrap();
    }
}

