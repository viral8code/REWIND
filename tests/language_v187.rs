use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v187-{}-{}",
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
fn success(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
#[test]
fn csv_chunks_checkpoint_and_source_free_replay() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/csv-stream/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
        ],
    );
    success(&out);
    assert_eq!(
        String::from_utf8(out.stdout.clone()).unwrap(),
        "name\nactive\nnote\nValidation\n{\"active\":true,\"name\":\"日本語\",\"note\":\"a\\\"b\\n日\"}\n日本語\n"
    );
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
