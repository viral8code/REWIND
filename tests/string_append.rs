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
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
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
    run_script_with(source, &[])
}
fn run_script_with(source: &str, extra: &[&str]) -> String {
    let root = dir();
    fs::write(root.join("main.rw"), source).unwrap();
    let mut args = vec!["run", "main.rw"];
    args.extend_from_slice(extra);
    let out = call(&root, &args);
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
fn run_failure(source: &str) -> String {
    let root = dir();
    fs::write(root.join("main.rw"), source).unwrap();
    let out = call(&root, &["run", "main.rw"]);
    assert!(!out.status.success());
    fs::remove_dir_all(root).unwrap();
    String::from_utf8_lossy(&out.stderr).into_owned()
}
#[test]
fn field_append_is_restored_by_revert_to_committed_value() {
    let out = run_script(
        r#"
import std.text as text;
struct Buf { buf: String }
fn main() -> Int {
    var b = Buf("ab");
    for i in 0..300 {
        b.buf += "ab";
    }
    commit base;
    for i in 0..300 {
        b.buf += "cd";
    }
    Out.println(b.buf.byteLen());
    revert base;
    Out.println(b.buf == "ab".repeat(301));
    b.buf += "z";
    Out.println(b.buf.byteLen());
    return 0;
}
"#,
    );
    assert_eq!(out, "true\n603\n");
}
#[test]
fn field_append_does_not_change_a_shared_string() {
    let out = run_script(
        r#"
import std.text as text;
struct Buf { buf: String }
fn main() -> Int {
    var s = "";
    for i in 0..300 {
        s += "ab";
    }
    var b = Buf(s);
    b.buf += "cd";
    Out.println(s == "ab".repeat(300));
    Out.println(b.buf == "ab".repeat(300) + "cd");
    var short = "x";
    var c = Buf(short);
    c.buf += "y";
    Out.println(short);
    Out.println(c.buf);
    c.buf += c.buf;
    Out.println(c.buf);
    return 0;
}
"#,
    );
    assert_eq!(out, "true\ntrue\nx\nxy\nxyxy\n");
}
#[test]
fn field_append_type_errors_are_unchanged() {
    let err = run_failure(
        r#"
import std.text as text;
struct Buf { buf: String }
fn main() -> Int {
    var b = Buf("a");
    b.buf += 1;
    return 0;
}
"#,
    );
    assert!(err.contains("type mismatch") || err.contains("expects") || err.contains("cannot"), "{err}");
    let err = run_failure(
        r#"
struct Num { n: Int }
fn main() -> Int {
    var b = Num(1);
    b.n += "x";
    return 0;
}
"#,
    );
    assert!(err.contains("type mismatch") || err.contains("expects") || err.contains("cannot"), "{err}");
}
#[test]
fn field_append_of_many_pieces_finishes_in_reasonable_time() {
    let out = run_script_with(
        r#"
import std.text as text;
struct Buf { buf: String }
fn main() -> Int {
    var b = Buf("");
    for i in 0..300000 {
        b.buf += "ab";
    }
    Out.println(b.buf.byteLen());
    return 0;
}
"#,
        &["--steps", "100000000000", "--native-work", "100000000000"],
    );
    assert_eq!(out, "600000\n");
}
#[test]
fn text_slice_boundaries_agree_for_ascii_and_mixed_text() {
    // 末尾に日本語を足した文字列は走査の経路を通る。先頭部分の切り出し結果は ASCII の経路と一致する。
    let out = run_script(
        r#"
import std.text as text;
fn show(r: Result<String, StdError>) -> String {
    return match r {
        Ok(v) => "ok:" + v,
        Err(_) => "err",
    };
}
fn main() -> Int {
    let a = "hello world";
    let m = "hello world日本";
    let pairs = [[0, 0], [0, 5], [6, 11], [11, 11], [3, 3], [0, 11], [5, 4], [0, 12], [11, 12], [12, 12], [-1, 2], [2, -1]];
    var same = true;
    for p in pairs {
        let x = show(text.slice(a, p[0], p[1]));
        let y = show(text.slice(m, p[0], p[1]));
        if p[1] <= 11 && x != y {
            same = false;
            Out.println("diff " + x + " " + y);
        }
    }
    Out.println(same);
    Out.println(show(text.slice(a, 0, 12)));
    Out.println(show(text.slice(a, 12, 12)));
    Out.println(show(text.slice(a, 11, 11)));
    Out.println(show(text.slice(a, 5, 4)));
    Out.println(show(text.slice(a, -1, 2)));
    Out.println(show(text.slice("", 0, 0)));
    Out.println(show(text.slice("", 0, 1)));
    Out.println(show(text.slice(m, 11, 13)));
    Out.println(show(text.slice(m, 13, 14)));
    return 0;
}
"#,
    );
    assert_eq!(
        out,
        "true\nerr\nerr\nok:\nerr\nerr\nok:\nerr\nok:日本\nerr\n"
    );
}
