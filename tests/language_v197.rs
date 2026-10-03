use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v197-{}-{}",
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
#[test]
fn grapheme_view_source_free_checkpoint_and_replay() {
    let root = root();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/gui-grapheme/main.rw"),
    )
    .unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    for mode in ["debug", "compact"] {
        let out = call(
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
        ok(&out);
        assert_eq!(out.stdout, "éb\n".as_bytes());
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(root).unwrap();
}
