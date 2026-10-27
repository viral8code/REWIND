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
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&path);
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
