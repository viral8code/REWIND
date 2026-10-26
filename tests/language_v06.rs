use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
#[test]
#[cfg(unix)] // Authenticated on-disk caches require Unix owner/mode validation.
fn persistent_cache_rebuilds_corruption_and_preserves_artifact_bytes() {
    let root = fixture(
        "fn answer(n:Int)->Int{return n+1;}assert_eq(answer(2),3);",
        "",
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
    assert!(entries.iter().any(|p| p
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .contains("compiled")));
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
        "fn answer(n:Int)->String{return n;}assert_eq(answer(2),3);",
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
