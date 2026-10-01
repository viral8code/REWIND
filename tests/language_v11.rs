use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "rewind-v11-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn run(path: &std::path::Path, source: &str) -> serde_json::Value {
    fs::write(path.join("main.rw"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run",
            path.join("main.rw").to_str().unwrap(),
            "--diagnostic-format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    serde_json::from_slice(&output.stderr).unwrap()
}
#[test]
fn runtime_failure_keeps_leaf_and_callers_after_cleanup_and_without_sources() {
    let path = root();
    fs::write(
        path.join("math.rw"),
        "pub fn divide(n:Int)->Int effects {} {return 1/n;}\n",
    )
    .unwrap();
    let source="import math as math;\nfn outer(n:Int)->Int effects {} {return math.divide(n);}\nouter(0);\n";
    let d = run(&path, source);
    let d = &d["diagnostic"];
    assert_eq!(d["code"], "DivisionByZero");
    assert_eq!(d["source"], "math.rw");
    assert_eq!(d["line"], 1);
    assert_eq!(d["frames"].as_array().unwrap().len(), 3);
    assert_eq!(d["frames"][1]["source"], "main.rw");
    assert_eq!(d["frames"][1]["line"], 2);
    assert_eq!(d["frames"][2]["line"], 3);
    assert!(!d["hints"].as_array().unwrap().is_empty());
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["compile", path.join("main.rw").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_file(path.join("main.rw")).unwrap();
    fs::remove_file(path.join("math.rw")).unwrap();
    fs::remove_dir_all(path.join(".rewind")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(path.join("main.rwc"))
        .args(["--diagnostic-format", "json"])
        .output()
        .unwrap();
    let actual: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(&actual["diagnostic"], d);
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn stable_codes_distinguish_arithmetic_bounds_assertions_and_typo_hints() {
    let path = root();
    for (source, code) in [
        ("1/0;", "DivisionByZero"),
        ("(-9223372036854775807-1)/(-1);", "IntegerOverflow"),
        ("let xs=List<Int>();xs.get(0);", "IndexOutOfBounds"),
        ("assert(false);", "AssertionFailed"),
    ] {
        let d = run(&path, source);
        assert_eq!(d["diagnostic"]["code"], code, "{d}");
    }
    let d = run(&path, "let score=10;Out.println(scroe);");
    assert_eq!(d["diagnostic"]["code"], "UnknownName");
    assert!(d["diagnostic"]["hints"][0]
        .as_str()
        .unwrap()
        .contains("score"));
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn primary_failure_survives_failing_defer() {
    let path = root();
    let d=run(&path,"fn cleanup()->Unit effects {} {panic(\"cleanup failed\");}fn leaf()->Int effects {} {defer ||->Unit{cleanup();};return 1/0;}leaf();");
    assert_eq!(d["diagnostic"]["code"], "DivisionByZero");
    assert!(!d["diagnostic"]["frames"].as_array().unwrap().is_empty());
    assert!(!d["diagnostic"]["causes"].as_array().unwrap().is_empty());
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn formatter_preserves_multiline_strings() {
    let path = root();
    let source = "let text=\"first   \n  second   \nlast\";\nOut.println(text);\npublish;\n";
    let file = path.join("main.rw");
    fs::write(&file, source).unwrap();
    let before = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();
    let formatted = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("fmt")
        .arg(&file)
        .output()
        .unwrap();
    assert!(formatted.status.success());
    let after = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();
    assert!(after.status.success());
    assert_eq!(before.stdout, after.stdout);
    assert_eq!(fs::read_to_string(file).unwrap(), source);
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn standalone_lsp_supports_std_incremental_edits_hints_and_formatting() {
    use serde_json::json;
    use std::io::Write;
    use std::process::Stdio;
    let path = root();
    let file = path.join("main.rw");
    fs::write(&file, "").unwrap();
    let uri = format!(
        "file:///{}",
        file.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let source="import std.integer as integer;\nfn work()->Unit effects {} {\nlet score=integer.add(1,2);\nassert(true);\n}\n";
    let requests = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/formatting","params":{"textDocument":{"uri":uri},"options":{"tabSize":4,"insertSpaces":true}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"range":{"start":{"line":3,"character":7},"end":{"line":3,"character":11}},"text":"scroe == score"}]}}),
        json!({"jsonrpc":"2.0","id":3,"method":"shutdown","params":{}}),
        json!({"jsonrpc":"2.0","method":"exit","params":{}}),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["lsp", "--root", path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for request in requests {
        let bytes = serde_json::to_vec(&request).unwrap();
        write!(stdin, "Content-Length: {}\r\n\r\n", bytes.len()).unwrap();
        stdin.write_all(&bytes).unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut rest = output.stdout.as_slice();
    let mut messages = Vec::new();
    while !rest.is_empty() {
        let end = rest.windows(4).position(|b| b == b"\r\n\r\n").unwrap();
        let header = std::str::from_utf8(&rest[..end]).unwrap();
        let n: usize = header
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        messages.push(
            serde_json::from_slice::<serde_json::Value>(&rest[end + 4..end + 4 + n]).unwrap(),
        );
        rest = &rest[end + 4 + n..];
    }
    assert_eq!(messages[0]["result"]["capabilities"]["textDocumentSync"], 2);
    assert!(
        messages.iter().any(|m| m["params"]["version"] == 1
            && m["params"]["diagnostics"]
                .as_array()
                .is_some_and(|ds| ds.is_empty())),
        "{messages:?}"
    );
    assert!(messages.iter().any(|m| m["id"] == 2
        && m["result"][0]["newText"]
            .as_str()
            .is_some_and(|s| s.contains("    let score"))));
    assert!(
        messages.iter().any(|m| m["params"]["version"] == 2
            && m["params"]["diagnostics"][0]["data"]["hints"][0]
                .as_str()
                .is_some_and(|s| s.contains("score"))),
        "{messages:?}"
    );
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn text_slice_scalar_boundaries_and_task_failure_metadata() {
    let path = root();
    let source="import std.text as text;assert_eq(text.slice(\"a😀界\",1,3),Ok(\"😀界\"));assert_eq(text.slice(\"a😀界\",3,3),Ok(\"\"));assert_eq(text.slice(\"\",0,0),Ok(\"\"));match text.slice(\"a😀界\",3,4){Err(_)=>{},_=>{panic(\"range\");}}async fn fail()->Int effects {} {return 1/0;}let task=spawn fail();match await task {Err(TaskError::Failed(d))=>{assert_eq(d.code,\"DivisionByZero\");assert_eq(d.taskId,Some(1));},_=>{panic(\"diagnostic\");}}";
    fs::write(path.join("main.rw"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(path.join("main.rw"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(path).unwrap();
}
