use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "rewind-v12-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn invoke(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(args)
        .output()
        .unwrap()
}
fn success(out: Output, expected: &[u8]) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected);
}
fn source(path: &Path, text: &str) -> PathBuf {
    let file = path.join("main.rw");
    fs::write(&file, text).unwrap();
    file
}
fn failure(path: &Path, text: &str) -> serde_json::Value {
    let file = source(path, text);
    let out = invoke(&["run", file.to_str().unwrap(), "--diagnostic-format", "json"]);
    assert!(!out.status.success());
    serde_json::from_slice(&out.stderr).unwrap()
}
#[test]
fn numeric_literals_cover_bases_separators_signed_limits_and_patterns() {
    let path = root();
    let file = source(
        &path,
        r#"
assert_eq(1_000_000,1000000);assert_eq(0xDEAD_beef,3735928559);
assert_eq(0B1010_0101,165);assert_eq(0o7_55,493);
assert_eq(1_2.5_0e+0_1,125.0);assert_eq(1.5e-0_1,0.15);
assert_eq(0x7fff_ffff_ffff_ffff,9223372036854775807);
assert_eq(-0x8000_0000_0000_0000,-9223372036854775808);
assert_eq(-9_223_372_036_854_775_808,-9223372036854775808);
match -0x8000_0000_0000_0000 {-0x8000_0000_0000_0000=>{},_=>{panic("min pattern");}}
match -2 {-0x3..-0x1=>{},_=>{panic("range pattern");}}
var total=0;for n in 0b0..0b11 {total+=n;}assert_eq(total,3);
Out.println("numbers");publish;
"#,
    );
    success(invoke(&["run", file.to_str().unwrap()]), b"numbers\n");
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn malformed_numbers_and_unicode_escapes_report_codes_and_locations() {
    let path = root();
    for literal in [
        "1_", "1__0", "0x", "0x_ff", "0xff_", "0b102", "0o8", "1e", "1e+", "1e_2", "1.2_e3",
    ] {
        let d = failure(&path, &format!("\nlet n={literal};"));
        assert_eq!(
            d["diagnostic"]["code"], "InvalidNumericLiteral",
            "{literal}: {d}"
        );
        assert_eq!(d["diagnostic"]["line"], 2);
        assert_eq!(d["diagnostic"]["column"], 7);
    }
    for literal in [
        "0x8000_0000_0000_0000",
        "0x1_0000_0000_0000_0000",
        "-0x8000_0000_0000_0001",
    ] {
        let d = failure(&path, &format!("let n={literal};"));
        assert_eq!(d["diagnostic"]["code"], "IntegerOverflow", "{d}");
    }
    for escape in [
        r"\u{}",
        r"\u{D800}",
        r"\u{110000}",
        r"\u{0000000}",
        r"\u{xyz}",
        r"\u1234",
        r"\u{123",
    ] {
        let d = failure(&path, &format!("let s=\"{escape}\";"));
        assert_eq!(d["diagnostic"]["code"], "InvalidUnicodeEscape", "{d}");
    }
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn nested_comments_unicode_and_formatter_preserve_behavior() {
    let path = root();
    let file = source(
        &path,
        r#"/* outer { " //
  /* nested } " */ trailing   
*/
if true {
/* not syntax: } { " */
let text="\u{754c}\u{1F600}\0";
assert_eq(text.charLen(),3);
assert_eq(text.byteLen(),8);
// /* is a line comment
Out.println("/* text */");
}
publish;
"#,
    );
    let expected = b"/* text */\n";
    success(invoke(&["run", file.to_str().unwrap()]), expected);
    success(invoke(&["fmt", file.to_str().unwrap()]), b"");
    let formatted = fs::read_to_string(&file).unwrap();
    assert!(formatted.contains("  /* nested } \" */ trailing   \n"));
    assert!(formatted.contains("    let text="));
    success(invoke(&["fmt", file.to_str().unwrap(), "--check"]), b"");
    success(invoke(&["run", file.to_str().unwrap()]), expected);
    let d = failure(&path, "/* first\n /* nested */\n*/\nOut.println(missing);");
    assert_eq!(d["diagnostic"]["line"], 4);
    assert_eq!(d["diagnostic"]["column"], 13);
    let d = failure(&path, "\n/* open\n/* nested */");
    assert_eq!(d["diagnostic"]["code"], "UnterminatedComment");
    assert_eq!(d["diagnostic"]["line"], 2);
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn new_syntax_and_conditional_revert_survive_record_and_source_free_artifacts() {
    let path = root();
    let file = source(&path, include_str!("../examples/v12/main.rw"));
    let record = path.join("record.json");
    success(
        invoke(&[
            "run",
            file.to_str().unwrap(),
            "--record",
            record.to_str().unwrap(),
        ]),
        b"Odd\n",
    );
    success(
        invoke(&[
            "replay",
            record.to_str().unwrap(),
            "--root",
            path.to_str().unwrap(),
        ]),
        b"Odd\n",
    );
    success(invoke(&["compile", file.to_str().unwrap()]), b"");
    fs::remove_file(file).unwrap();
    fs::remove_dir_all(path.join(".rewind")).unwrap();
    success(
        invoke(&["run", path.join("main.rwc").to_str().unwrap()]),
        b"Odd\n",
    );
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn revert_rejects_sibling_blocks_and_later_loop_iterations() {
    let path = root();
    for text in [
        "if true {commit old;}if true {revert old;}",
        "fn work()->Unit effects {} {{commit old;}{revert old;}}work();",
        "var n=0;while n<2 {if n==0 {commit old;}else {revert old;}n+=1;}",
    ] {
        let d = failure(&path, text);
        assert_eq!(d["diagnostic"]["code"], "InvalidContinuation", "{d}");
        assert!(!d["diagnostic"]["hints"].as_array().unwrap().is_empty());
    }
    let file=source(&path,"var n=1;commit base;if true {{n=2;revert base;assert_eq(n,1);}}drop base;Out.println(n);publish;");
    success(invoke(&["run", file.to_str().unwrap()]), b"1\n");
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn command_help_does_not_execute_and_application_help_argument_is_preserved() {
    for command in ["run", "compile", "check", "test", "fmt", "lsp", "replay"] {
        let out = invoke(&[command, "--help"]);
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains(&format!("rewind {command}")));
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
    }
    let out = Command::new(env!("CARGO_BIN_EXE_rewindc"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("rewindc FILE.rw"));
    let path = root();
    let file = source(&path, "Out.println(Args.all().get(0));publish;");
    success(
        invoke(&["run", file.to_str().unwrap(), "--", "--help"]),
        b"--help\n",
    );
    fs::remove_dir_all(path).unwrap();
}
