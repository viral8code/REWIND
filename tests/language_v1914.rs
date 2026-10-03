use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1914-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&r).unwrap();
    r
}
fn call(r: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(r)
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn reject(o: &Output, reason: &str) {
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains(reason),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.stdout.is_empty());
}
const MANIFEST: &str =
    "language=\"1.9.13\"\nsource_root=\"src\"\nentry=\"main.rw\"\neffects=\"output\"\n";
fn fixture(assets: bool) -> PathBuf {
    let r = root();
    fs::create_dir(r.join("src")).unwrap();
    fs::write(
        r.join("src/helper.rw"),
        "const ANSWER: Int = 6 * 7; pub fn answer()->Int effects {} { return ANSWER; }",
    )
    .unwrap();
    fs::write(
        r.join("src/main.rw"),
        "import helper as local; Out.println(local.answer()); publish;",
    )
    .unwrap();
    fs::write(
        r.join("rewind.toml"),
        format!(
            "{MANIFEST}{}",
            if assets {
                "[assets]\nmessage=\"message.txt\"\n"
            } else {
                ""
            }
        ),
    )
    .unwrap();
    if assets {
        fs::write(r.join("message.txt"), "日本語 asset").unwrap();
    }
    ok(&call(&r, &["update"]));
    ok(&call(
        &r,
        &["compile", "src/main.rw", "--output", "app.rwc"],
    ));
    fs::remove_dir_all(r.join("src")).unwrap();
    fs::remove_dir_all(r.join(".rewind")).unwrap();
    r
}
#[test]
fn source_free_compiled_project_keeps_manifest_lock_and_assets_and_replays_both_modes() {
    let r = fixture(true);
    for mode in ["debug", "compact"] {
        let o = call(
            &r,
            &[
                "run",
                "app.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&o);
        assert_eq!(o.stdout, b"42\n");
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(replay.stdout, o.stdout);
    }
    // Source compilation keeps requiring a real source graph.
    reject(&call(&r, &["check"]), "");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn runtime_policy_does_not_ignore_permissions_or_stale_missing_malformed_locks() {
    let r = fixture(false);
    let original = fs::read(r.join("rewind.lock")).unwrap();
    fs::write(
        r.join("rewind.toml"),
        MANIFEST.replace("effects=\"output\"", "effects=\"\""),
    )
    .unwrap();
    reject(
        &call(&r, &["run", "app.rwc", "--allow-effects", "output"]),
        "runtime lock mismatch",
    );
    let mut lock: serde_json::Value = serde_json::from_slice(&original).unwrap();
    lock["effects"] = serde_json::json!([]);
    fs::write(r.join("rewind.lock"), serde_json::to_vec(&lock).unwrap()).unwrap();
    reject(
        &call(&r, &["run", "app.rwc", "--allow-effects", "output"]),
        "effect",
    );
    fs::write(r.join("rewind.toml"), MANIFEST).unwrap();
    fs::write(r.join("rewind.lock"), b"not JSON").unwrap();
    reject(&call(&r, &["run", "app.rwc"]), "invalid runtime lock");
    fs::remove_file(r.join("rewind.lock")).unwrap();
    reject(&call(&r, &["run", "app.rwc"]), "requires rewind.lock");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn changed_missing_or_additional_assets_and_language_mismatch_are_rejected_before_output() {
    let r = fixture(true);
    fs::write(r.join("message.txt"), "modified").unwrap();
    reject(&call(&r, &["run", "app.rwc"]), "runtime lock mismatch");
    fs::remove_file(r.join("message.txt")).unwrap();
    reject(&call(&r, &["run", "app.rwc"]), "");
    fs::write(r.join("message.txt"), "日本語 asset").unwrap();
    fs::write(
        r.join("rewind.toml"),
        MANIFEST.replace("1.9.13", "1.9.12") + "[assets]\nmessage=\"message.txt\"\n",
    )
    .unwrap();
    let mut lock: serde_json::Value =
        serde_json::from_slice(&fs::read(r.join("rewind.lock")).unwrap()).unwrap();
    lock["language"] = serde_json::json!("1.9.12");
    fs::write(r.join("rewind.lock"), serde_json::to_vec(&lock).unwrap()).unwrap();
    reject(
        &call(&r, &["run", "app.rwc"]),
        "manifest language/assets mismatch",
    );
    fs::remove_dir_all(r).unwrap();
    let r = fixture(false);
    fs::write(r.join("extra.txt"), "extra").unwrap();
    fs::write(
        r.join("rewind.toml"),
        format!("{MANIFEST}[assets]\nextra=\"extra.txt\"\n"),
    )
    .unwrap();
    let mut lock: serde_json::Value =
        serde_json::from_slice(&fs::read(r.join("rewind.lock")).unwrap()).unwrap();
    // A self-consistent policy cannot substitute an asset inventory different from the embedded program.
    use sha2::{Digest, Sha256};
    lock["assets"] = serde_json::json!({"extra.txt":{"bytes":5,"sha256":format!("{:x}",Sha256::digest(b"extra"))}});
    fs::write(r.join("rewind.lock"), serde_json::to_vec(&lock).unwrap()).unwrap();
    reject(
        &call(&r, &["run", "app.rwc"]),
        "manifest language/assets mismatch",
    );
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn malformed_duplicate_and_traversing_manifests_are_not_treated_as_standalone() {
    let r = fixture(false);
    for (manifest, reason) in [
        (format!("{MANIFEST}effects=\"output\"\n"), "duplicate key"),
        (
            MANIFEST.replace("source_root=\"src\"", "source_root=\"../src\""),
            "InvalidPath",
        ),
        (format!("{MANIFEST}[unknown]\n"), "unsupported section"),
    ] {
        fs::write(r.join("rewind.toml"), manifest).unwrap();
        let o = call(&r, &["run", "app.rwc"]);
        assert!(!o.status.success());
        assert!(o.stdout.is_empty());
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(
            err.contains(reason) || reason == "InvalidPath" && err.contains("invalid path"),
            "{err}"
        );
    }
    fs::remove_dir_all(r).unwrap();
}
