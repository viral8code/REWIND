use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "rewind-v1936-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn call(root: &PathBuf, args: &[&str]) -> Output {
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
fn retained_unicode_text_survives_closure_container_checkpoint_source_free_and_replay() {
    for mode in ["debug", "compact"] {
        let root = root();
        let source = include_str!("../examples/text-storage/main.rw");
        fs::write(root.join("main.rw"), source).unwrap();
        ok(&call(&root, &["compile", "main.rw"]));
        fs::remove_file(root.join("main.rw")).unwrap();
        if root.join(".rewind").exists() {
            fs::remove_dir_all(root.join(".rewind")).unwrap();
        }
        let run = call(
            &root,
            &[
                "run",
                "main.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&run);
        assert_eq!(run.stdout, b"text restored\n");
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(run.stdout, replay.stdout);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn large_text_byte_length_admits_constant_metadata_work() {
    let root = root();
    let text = "x".repeat(65536);
    fs::write(
        root.join("main.rw"),
        format!(
            "let text={};assert_eq(text.byteLen(),65536);Out.println(1);publish;",
            serde_json::to_string(&text).unwrap()
        ),
    )
    .unwrap();
    let out = call(&root, &["run", "main.rw", "--native-work", "100"]);
    ok(&out);
    assert_eq!(out.stdout, b"1\n");
    fs::remove_dir_all(root).unwrap();
}
