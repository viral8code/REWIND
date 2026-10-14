use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v300-{}-{}",
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
    let root = project(source);
    call(&root, &["run", "--root", "."])
}
fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into()
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into()
}

#[test]
fn main_is_the_entry_point() {
    let out = run("fn main()->Int effects {output} { Out.println(\"hello\"); publish; return 0; }\n");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "hello\n");
}

#[test]
fn top_level_statements_are_rejected() {
    let out = run("fn main()->Int effects {} { return 0; }\nlet x=1;\n");
    assert!(!out.status.success());
    assert!(stderr(&out).contains("top-level statements are not allowed"), "{}", stderr(&out));
}
