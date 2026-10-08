use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1943-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &PathBuf, args: &[&str]) -> Output {
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

fn source(power: usize) -> String {
    format!(
        r#"
var text="x";for i in 0..{power}{{text+=text;}}
let items=List<String>();commit saved;
for i in 0..32{{items.add(text);}}
Out.println(items.len());publish;
revert saved;assert_eq(items.len(),0);Out.println(items.len());drop saved;publish;
"#
    )
}
#[test]
fn repeated_shared_text_fits_a_budget_for_one_payload_and_restores_roots() {
    let r = root();
    fs::write(r.join("main.rw"), source(20)).unwrap();
    let out = call(
        &r,
        &[
            "profile",
            "main.rw",
            "--history-memory",
            "8MiB",
            "--steps",
            "20000000",
            "--native-work",
            "100000000",
        ],
    );
    ok(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        "32\n0\n"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    let p: serde_json::Value =
        serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap()).unwrap();
    let bytes = p["shared_payloads"]["live_bytes"].as_u64().unwrap();
    assert!(bytes >= 1048576 && bytes < 4 * 1048576, "{p}");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn previous_language_keeps_its_conservative_payload_budget() {
    let r = root();
    fs::write(r.join("main.rw"), source(20)).unwrap();
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.42\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    ok(&call(&r, &["update"]));
    let out = call(
        &r,
        &[
            "run",
            "main.rw",
            "--history-memory",
            "8MiB",
            "--steps",
            "20000000",
            "--native-work",
            "100000000",
        ],
    );
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("HistoryMemory"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty());
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn shared_text_checkpoint_survives_source_free_debug_and_compact_replay() {
    let r = root();
    fs::write(r.join("main.rw"), source(12)).unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(r.join(".rewind"));
    for mode in ["debug", "compact"] {
        let out = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--history-memory",
                "8MiB",
                "--steps",
                "20000000",
                "--native-work",
                "100000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "32\n0\n"
        );
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn copy_on_write_preserves_checkpoint_payload_and_cannot_evade_admission() {
    let r = root();
    fs::write(
        r.join("main.rw"),
        r#"
var text="x";for i in 0..20{text+=text;}
Out.println("built");publish;commit old;
text+=text;Out.println("changed");publish;
"#,
    )
    .unwrap();
    let good = call(
        &r,
        &[
            "profile",
            "main.rw",
            "--history-memory",
            "8MiB",
            "--steps",
            "20000000",
            "--native-work",
            "100000000",
        ],
    );
    ok(&good);
    assert_eq!(
        String::from_utf8_lossy(&good.stdout).replace("\r\n", "\n"),
        "built\nchanged\n"
    );
    let stderr = String::from_utf8(good.stderr).unwrap();
    let p: serde_json::Value =
        serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap()).unwrap();
    assert!(
        p["shared_payloads"]["live_bytes"].as_u64().unwrap() >= 3 * 1048576,
        "{p}"
    );
    let denied = call(
        &r,
        &[
            "run",
            "main.rw",
            "--history-memory",
            "3MiB",
            "--steps",
            "20000000",
            "--native-work",
            "100000000",
        ],
    );
    assert!(!denied.status.success());
    assert!(
        String::from_utf8_lossy(&denied.stderr).contains("HistoryMemory"),
        "{}",
        String::from_utf8_lossy(&denied.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&denied.stdout).replace("\r\n", "\n"),
        "built\n"
    );
    fs::remove_dir_all(r).unwrap();
}
