use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1946-{}-{}",
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
fn cached_scalar_collection_reduces_work_and_old_language_keeps_prior_traversal() {
    let r = root();
    fs::write(r.join("main.rw"), source(4096)).unwrap();
    let mut works = Vec::new();
    for language in ["1.9.46", "1.9.45"] {
        fs::write(
            r.join("rewind.toml"),
            format!(
                r#"entry = "main.rw"
language = "{language}"
source_root = "."
effects = "output"
"#
            ),
        )
        .unwrap();
        ok(&call(&r, &["update"]));
        let out = call(
            &r,
            &[
                "profile",
                "main.rw",
                "--steps",
                "20000000",
                "--native-work",
                "100000000",
                "--history-memory",
                "512MiB",
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "31\n0\n"
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        let profile: serde_json::Value =
            serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap())
                .unwrap();
        assert!(
            profile["gc"]["completed"].as_u64().unwrap() > 0,
            "{profile}"
        );
        works.push(profile["gc"]["work"].as_u64().unwrap());
    }
    assert!(works[0] * 10 < works[1], "new/old GC work {works:?}");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn source_free_collection_and_checkpoint_replay_use_the_selected_traversal() {
    let r = root();
    for mode in ["debug", "compact"] {
        let d = r.join(mode);
        fs::create_dir(&d).unwrap();
        fs::write(
            d.join("main.rw"),
            source(if mode == "debug" { 8 } else { 4096 }),
        )
        .unwrap();
        ok(&call(&d, &["compile", "main.rw"]));
        fs::remove_file(d.join("main.rw")).unwrap();
        fs::remove_dir_all(d.join(".rewind")).unwrap();
        let out = call(
            &d,
            &[
                "run",
                "main.rwc",
                "--steps",
                "20000000",
                "--native-work",
                "100000000",
                "--history-memory",
                "8MiB",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        let replay = call(&d, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "31\n0\n"
        );
    }
    fs::remove_dir_all(r).unwrap();
}
