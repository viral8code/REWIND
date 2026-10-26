use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v183-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
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
fn decimal_computation_map_and_json_survive_source_free_replay() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/decimal/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(output.stdout, b"0.3\n0.1\n2\ntrue\n1\n\"0.1\"\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn sqlite_numeric_affinity_cannot_round_blob_backed_decimals() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/decimal-sqlite/main.rw")
            .replace("\":memory:\"", "\"data.sqlite\""),
    )
    .unwrap();
    success(&call(
        &root,
        &["compile", "main.rw", "--allow-effects", "external,db,tasks"],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            "external,db,tasks",
            "--record",
            "trace.json",
        ],
    );
    success(&output);
    assert_eq!(output.stdout, b"12345678901234567890.12340000\n");
    fs::remove_file(root.join("data.sqlite")).unwrap();
    let replay = call(
        &root,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            "external,db,tasks",
        ],
    );
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    assert!(!root.join("data.sqlite").exists());
    fs::remove_dir_all(root).unwrap();
}
