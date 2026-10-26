// Authenticated on-disk caches require Unix owner/mode validation, so this test only runs on Unix.
#![cfg(unix)]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn fixture(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-cache-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"3.0.0\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"\"\n",
    )
    .unwrap();
    let output = cmd("update", &root, &[]);
    success(&output);
    root
}
fn cmd(mode: &str, root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(mode)
        .arg("--root")
        .arg(root)
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
fn persistent_cache_rebuilds_corruption_and_preserves_artifact_bytes() {
    let root = fixture(
        "fn answer(n:Int)->Int{return n+1;}\nfn main()->Int {\n    assert_eq(answer(2),3);\n    return 0;\n}\n",
    );
    let artifact = root.join("compiled.json");
    success(&cmd(
        "build",
        &root,
        &["--output", artifact.to_str().unwrap()],
    ));
    let expected = fs::read(&artifact).unwrap();
    let entries = fs::read_dir(root.join(".rewind/cache"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert!(!entries.is_empty());
    for entry in entries {
        fs::write(entry, r#"{"payload":true,"auth":[]}"#).unwrap();
    }
    success(&cmd(
        "build",
        &root,
        &["--output", artifact.to_str().unwrap()],
    ));
    assert_eq!(fs::read(&artifact).unwrap(), expected);
    let clean = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .env("REWIND_NO_CACHE", "1")
        .arg("build")
        .arg("--root")
        .arg(&root)
        .arg("--output")
        .arg(&artifact)
        .output()
        .unwrap();
    success(&clean);
    assert_eq!(fs::read(&artifact).unwrap(), expected);
    fs::write(
        root.join("main.rw"),
        "fn answer(n:Int)->String{return n;}\nfn main()->Int {\n    assert_eq(answer(2),3);\n    return 0;\n}\n",
    )
    .unwrap();
    assert!(!cmd("check", &root, &[]).status.success());
    let compatible = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("compatibility")
        .arg(&artifact)
        .output()
        .unwrap();
    success(&compatible);
    let value: serde_json::Value = serde_json::from_slice(&compatible.stdout).unwrap();
    assert_eq!(value["readable"], true);
    fs::remove_dir_all(root).unwrap();
}
