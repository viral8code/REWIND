use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-string-append-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn run_script(source: &str) -> String {
    let root = dir();
    fs::write(root.join("main.rw"), source).unwrap();
    let out = call(&root, &["run", "main.rw"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    fs::remove_dir_all(root).unwrap();
    String::from_utf8(out.stdout).unwrap()
}
#[test]
fn string_append_is_restored_by_revert_to_committed_value() {
    let out = run_script(
        r#"
fn main() -> Int {
    var s = "ab";
    commit base;
    s += "cd";
    revert base;
    Out.println(s);
    return 0;
}
"#,
    );
    assert_eq!(out, "ab\n");
}
#[test]
fn long_string_append_is_restored_by_revert_to_committed_value() {
    let out = run_script(
        r#"
fn main() -> Int {
    var s = "";
    for i in 0..300 {
        s += "ab";
    }
    commit base;
    for i in 0..300 {
        s += "cd";
    }
    revert base;
    Out.println(s);
    return 0;
}
"#,
    );
    assert_eq!(out, format!("{}\n", "ab".repeat(300)));
}
#[test]
fn string_append_does_not_change_an_earlier_alias() {
    let out = run_script(
        r#"
fn main() -> Int {
    var s = "x";
    let t = s;
    s += "y";
    Out.println(t);
    Out.println(s);
    return 0;
}
"#,
    );
    assert_eq!(out, "x\nxy\n");
}
#[test]
fn long_string_append_does_not_change_an_earlier_alias() {
    let out = run_script(
        r#"
fn main() -> Int {
    var s = "";
    for i in 0..300 {
        s += "ab";
    }
    let t = s;
    s += "cd";
    Out.println(t == "ab".repeat(300));
    return 0;
}
"#,
    );
    assert_eq!(out, "true\n");
}
#[test]
fn string_append_of_itself_doubles_the_value() {
    let out = run_script(
        r#"
fn main() -> Int {
    var s = "ab";
    s += s;
    Out.println(s);
    return 0;
}
"#,
    );
    assert_eq!(out, "abab\n");
}
#[test]
fn long_string_append_of_itself_doubles_the_value() {
    let out = run_script(
        r#"
fn main() -> Int {
    var s = "";
    for i in 0..300 {
        s += "ab";
    }
    s += s;
    Out.println(s == "ab".repeat(600));
    return 0;
}
"#,
    );
    assert_eq!(out, "true\n");
}
