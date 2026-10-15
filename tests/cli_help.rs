//! CLI-02: uniform help and machine-readable option errors.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-cli-help-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    fs::canonicalize(&p).unwrap()
}
fn rewind(cwd: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(cwd)
        .env_remove("REWIND_STANDALONE_LANGUAGE")
        .args(args)
        .output()
        .unwrap()
}
fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn every_subcommand_has_help_that_reads_no_source_and_creates_nothing() {
    let cwd = dir();
    for command in [
        "run",
        "compile",
        "check",
        "test",
        "fmt",
        "lsp",
        "replay",
        "debug",
        "profile",
        "doc",
        "update",
        "lock",
        "keygen",
        "sign",
        "sdk-build",
        "sdk-install",
        "sdk-verify",
        "api-snapshot",
        "api-diff",
        "migrate",
        "debug-session",
    ] {
        let o = rewind(&cwd, &[command, "--help"]);
        assert!(o.status.success(), "{command}: {}", err(&o));
        assert!(
            out(&o).contains(&format!("rewind {command}")),
            "{command}: {}",
            out(&o)
        );
    }
    assert_eq!(
        fs::read_dir(&cwd).unwrap().count(),
        0,
        "help must not create files"
    );
}

#[test]
fn unknown_option_has_a_stable_code_and_points_at_help() {
    let cwd = dir();
    fs::write(cwd.join("main.rw"), "fn main()->Int {return 0;}\n").unwrap();
    let o = rewind(&cwd, &["run", "main.rw", "--bogus"]);
    assert!(!o.status.success());
    assert!(err(&o).contains("CliUnknownOption"), "{}", err(&o));
    assert!(err(&o).contains("rewind run --help"), "{}", err(&o));
    let j = rewind(
        &cwd,
        &["--diagnostic-format", "json", "run", "main.rw", "--bogus"],
    );
    let value: serde_json::Value = serde_json::from_str(err(&j).lines().last().unwrap()).unwrap();
    assert_eq!(value["code"], "CliUnknownOption");
}

#[test]
fn missing_and_invalid_budget_values_are_coded() {
    let cwd = dir();
    fs::write(cwd.join("main.rw"), "fn main()->Int {return 0;}\n").unwrap();
    let missing = rewind(&cwd, &["run", "main.rw", "--steps"]);
    assert!(
        err(&missing).contains("CliMissingValue"),
        "{}",
        err(&missing)
    );
    let invalid = rewind(&cwd, &["run", "main.rw", "--steps", "abc"]);
    assert!(
        err(&invalid).contains("CliInvalidValue"),
        "{}",
        err(&invalid)
    );
}

#[test]
fn arguments_after_double_dash_reach_the_application() {
    let cwd = dir();
    fs::write(
        cwd.join("main.rw"),
        "fn main()->Int {\n    let arguments = Args.all();\n    Out.println(arguments.len());\n    return 0;\n}\n",
    )
    .unwrap();
    let o = rewind(&cwd, &["run", "main.rw", "--", "--help", "x"]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(out(&o), "2\n");
}
