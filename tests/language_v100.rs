use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v100-language-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    root
}
fn run(root: &std::path::Path, source: &str) -> std::process::Output {
    let file = root.join("main.rw");
    fs::write(&file, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run",
            file.to_str().unwrap(),
            "--steps",
            "2000000",
            "--native-work",
            "2000000",
        ])
        .output()
        .unwrap()
}
#[test]
fn compiler_rejects_deep_syntax_and_oversized_source_with_diagnostics() {
    for source in [
        format!("let x={}true;", "!".repeat(200)),
        " ".repeat(1024 * 1024 + 1),
    ] {
        let root = project();
        let result = run(&root, &source);
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("CompilerBudgetExceeded"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
