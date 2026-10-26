//! CLI-01: one context resolver for source commands.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-cli-ctx-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    fs::canonicalize(&p).unwrap()
}
fn rewind(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(cwd)
        .env_remove("REWIND_STANDALONE_LANGUAGE")
        .args(args)
        .output()
        .unwrap()
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}
const HELLO: &str = "import std.number as number;\nfn main()->Int {\n    match number.decimal(\"7\") {Ok(_)=>{Out.println(\"true\");},Err(_)=>{Out.println(\"false\");}}\n    return 0;\n}\n";

#[test]
fn explicit_root_without_manifest_is_a_standalone_boundary() {
    let root = dir();
    fs::write(root.join("main.rw"), HELLO).unwrap();
    let other = dir();
    let out = rewind(
        &other,
        &[
            "run",
            root.join("main.rw").to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "true\n");
}

#[test]
fn source_outside_the_explicit_root_is_rejected() {
    let root = dir();
    let elsewhere = dir();
    fs::write(elsewhere.join("main.rw"), HELLO).unwrap();
    let out = rewind(
        &root,
        &[
            "run",
            elsewhere.join("main.rw").to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ],
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("outside --root"), "{}", stderr(&out));
}

#[test]
fn nearest_parent_manifest_wins_regardless_of_cwd() {
    let project = dir();
    fs::create_dir(project.join("src")).unwrap();
    fs::write(
        project.join("rewind.toml"),
        "language = \"3.0.0\"\nsource_root = \"src\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    fs::write(project.join("src/main.rw"), "fn main() -> Int {\n    Out.println(1);\n    publish;\n    return 0;\n}\n").unwrap();
    let ok = rewind(&project, &["update"]);
    assert!(ok.status.success(), "{}", stderr(&ok));
    let cwd = dir();
    let out = rewind(
        &cwd,
        &["run", project.join("src/main.rw").to_str().unwrap()],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "1\n");
}

#[test]
fn broken_manifest_is_an_error_not_standalone() {
    let project = dir();
    fs::write(project.join("rewind.toml"), "language = [not valid\n").unwrap();
    fs::write(project.join("main.rw"), HELLO).unwrap();
    let out = rewind(&dir(), &["run", project.join("main.rw").to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stdout).contains("true"));
}

#[test]
fn doc_uses_the_same_parent_manifest_rule_as_run() {
    let project = dir();
    fs::write(
        project.join("rewind.toml"),
        "language = \"3.0.0\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    fs::write(
        project.join("main.rw"),
        "pub fn answer()->Int effects {} {return 42;}\nfn main()->Int {return 0;}\n",
    )
    .unwrap();
    assert!(rewind(&project, &["update"]).status.success());
    let out = rewind(&dir(), &["doc", project.join("main.rw").to_str().unwrap()]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains("answer"));
}
