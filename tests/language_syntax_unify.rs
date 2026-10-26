//! Syntax rules that were uneven before 4.0.0 and are now the same everywhere:
//! the comma after a `{ }` match arm, let-else in expression blocks, and `pub effects`.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-unify-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"3.0.0\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    assert!(call(&root, &["update"]).status.success());
    root
}
fn call(root: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn run(source: &str) -> Output {
    call(&project(source), &["run", "--root", "."])
}
fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n")
}

#[test]
fn block_arms_of_a_match_statement_may_omit_the_comma() {
    let out = run("fn show(n: Int) -> Unit {
    match n {
        1 => { Out.println(\"one\"); }
        2 => { Out.println(\"two\"); },
        _ => Out.println(\"many\"),
    }
}
fn main() -> Int { show(1); show(2); show(3); return 0; }
");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "one\ntwo\nmany\n");
}

#[test]
fn a_non_block_arm_still_needs_its_comma() {
    let out = run("fn main() -> Int {
    match 1 {
        1 => Out.println(\"a\")
        _ => Out.println(\"b\"),
    }
    return 0;
}
");
    assert!(!out.status.success());
    assert!(stderr(&out).contains("expected ','"), "{}", stderr(&out));
}

#[test]
fn let_else_works_in_expression_blocks() {
    let out = run("fn f(o: Option<Int>) -> Int {
    let v = {
        let Some(x) = o else { return 7; };
        x + 1
    };
    let w = if v > 0 {
        let Some(y) = o else { return 8; };
        y * 10
    } else {
        0
    };
    return v + w;
}
fn main() -> Int {
    Out.println(f(Some(2)));
    Out.println(f(None));
    return 0;
}
");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "23\n7\n");
}

#[test]
fn let_else_in_an_expression_block_must_leave_the_scope() {
    let out = run("fn main() -> Int {
    let o: Option<Int> = None;
    let v = { let Some(x) = o else { Out.println(\"no\"); }; x };
    return v;
}
");
    assert!(!out.status.success());
    assert!(stderr(&out).contains("let-else block must leave the scope"), "{}", stderr(&out));
}

#[test]
fn effects_declaration_cannot_be_public() {
    let out = run("pub effects { Clock }\nfn main() -> Int { return 0; }\n");
    assert!(!out.status.success());
    assert!(stderr(&out).contains("effects cannot be public"), "{}", stderr(&out));
}
