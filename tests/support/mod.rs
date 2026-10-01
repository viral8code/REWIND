use std::{
    fs,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub fn run(source: &str) -> Output {
    let root = std::env::temp_dir().join(format!(
        "rewind-increment-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let file = root.join("main.rw");
    fs::write(&file, source).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["run", file.to_str().unwrap(), "--task-steps", "10000000"])
        .output()
        .unwrap();
    fs::remove_dir_all(root).unwrap();
    result
}
pub fn ok(source: &str) {
    let output = run(source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
