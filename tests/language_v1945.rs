use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1945-{}-{}",
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

fn source(size: usize) -> String {
    let mut s =
        format!("let items=List<Int>();for i in 0..{size}{{items.add(i);}}commit original;");
    for i in 0..32 {
        s += &format!("items.set(0,{i});commit snapshot{i};");
    }
    s += "Out.println(items.get(0));publish;revert original;Out.println(items.get(0));publish;";
    for i in 0..32 {
        s += &format!("drop snapshot{i};");
    }
    s += "drop original;";
    s
}
#[test]
fn many_checkpoint_roots_fit_a_budget_for_shared_scalar_storage() {
    let r = root();
    fs::write(r.join("main.rw"), source(4096)).unwrap();
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
        "31\n0\n"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    let p: serde_json::Value =
        serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap()).unwrap();
    assert!(
        p["shared_payloads"]["live_bytes"].as_u64().unwrap() < 8 * 1024 * 1024,
        "{p}"
    );
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn previous_language_preserves_conservative_per_root_native_admission() {
    let r = root();
    fs::write(r.join("main.rw"), source(4096)).unwrap();
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.44\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
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
fn unique_container_roots_restore_with_source_free_debug_and_compact_replay() {
    let r = root();
    fs::write(r.join("main.rw"), source(8)).unwrap();
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
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "31\n0\n"
        );
    }
    fs::remove_dir_all(r).unwrap();
}
