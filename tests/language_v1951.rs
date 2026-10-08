use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str, language: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1951-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&r).unwrap();
    fs::write(r.join("main.rw"), source).unwrap();
    fs::write(
        r.join("rewind.toml"),
        format!("language = \"{language}\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n"),
    )
    .unwrap();
    ok(&call(&r, &["update", "--root", "."]));
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
#[test]
fn unique_unrooted_patterns_obey_budget_before_output_while_previous_language_is_preserved() {
    let marker = "sensitive-pattern-".repeat(256);
    let source = format!(
        r#"let base="{marker}";for i in 0..192{{match stdFormatInt(i,10){{Ok(index)=>{{let hidden=secret(base+index);}},Err(_)=>{{panic("format");}}}}}}Out.println(true);publish;"#
    );
    for language in ["1.9.50", "1.9.51"] {
        let r = root(&source, language);
        let o = call(
            &r,
            &[
                "run",
                "--root",
                ".",
                "--history-memory",
                "256KiB",
                "--steps",
                "2000000",
                "--native-work",
                "1000000000",
            ],
        );
        if language == "1.9.50" {
            ok(&o);
            assert_eq!(o.stdout, b"true\n");
        } else {
            assert!(!o.status.success());
            assert!(o.stdout.is_empty());
            let err = String::from_utf8_lossy(&o.stderr);
            assert!(err.contains("error[HistoryMemory]"), "{err}");
            assert!(!err.contains(&marker));
        }
        fs::remove_dir_all(r).unwrap();
    }
}
#[test]
fn duplicate_patterns_survive_revert_begin_without_multiplying_registered_capacity() {
    let marker = "duplicate-sensitive-pattern".repeat(128);
    let source = format!(
        r#"let text="{marker}";commit before;for i in 0..32{{let hidden=secret(text);}}revert before;drop before;Out.println(secret(text));publish;revert begin;let text="{marker}";for i in 0..32{{let hidden=secret(text);}}Out.println(secret(text));publish;"#
    );
    let r = root(&source, "1.9.51");
    let o = call(
        &r,
        &[
            "profile",
            "--root",
            ".",
            "--history-memory",
            "256KiB",
            "--steps",
            "2000000",
        ],
    );
    ok(&o);
    assert!(!String::from_utf8_lossy(&o.stdout).contains(&marker));
    let profile = String::from_utf8_lossy(&o.stderr)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .last()
        .unwrap();
    assert_eq!(profile["sensitive_registry"]["text_patterns"], 1);
    let bytes = profile["sensitive_registry"]["text_bytes"]
        .as_u64()
        .unwrap();
    assert!(bytes >= marker.len() as u64 && bytes < 2 * marker.len() as u64);
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn source_free_secret_derivation_remains_masked_after_revert_and_both_replay_modes() {
    for mode in ["debug", "compact"] {
        let r = root(
            r#"let s=secret("unpublished-private-pattern");commit safe;let copied=reveal(s)+"-derived";let again=secret(copied);revert safe;drop safe;Out.println(s);publish;"#,
            "1.9.51",
        );
        ok(&call(&r, &["compile", "main.rw"]));
        fs::remove_file(r.join("main.rw")).unwrap();
        let o = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&o);
        let trace = fs::read_to_string(r.join("trace.json")).unwrap();
        assert!(!trace.contains("unpublished-private-pattern"));
        assert!(!String::from_utf8_lossy(&o.stdout).contains("unpublished-private-pattern"));
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(replay.stdout, o.stdout);
        fs::remove_dir_all(r).unwrap();
    }
}
