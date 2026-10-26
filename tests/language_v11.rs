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
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&path);
    fs::create_dir(&path).unwrap();
    path
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
fn language_reference_examples_execute_as_documented() {
    let reference = include_str!("../docs/language-reference.md").replace(
        "
", "
",
    );
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in reference.lines() {
        match current.take() {
            None => {
                if let Some(tag) = line.strip_prefix("```") {
                    current = Some((tag.trim().to_string(), Vec::new()));
                }
            }
            Some((tag, body)) => {
                if line.trim_end() == "```" {
                    blocks.push((
                        tag,
                        body.join(
                            "
",
                        ) + "
",
                    ));
                } else {
                    let mut body = body;
                    body.push(line);
                    current = Some((tag, body));
                }
            }
        }
    }
    let mut ran = 0;
    for (index, (tag, source)) in blocks.iter().enumerate() {
        if !tag.starts_with("rewind") {
            continue;
        }
        ran += 1;
        let path = root();
        fs::write(path.join("main.rw"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
            .arg("run")
            .arg(path.join("main.rw"))
            .args(["--allow-effects", "fileRead,fileWrite,clock,external"])
            .current_dir(&path)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout).replace(
            "
", "
",
        );
        if tag == "rewind" {
            assert!(
                output.status.success(),
                "Reference example {ran} failed:
{source}
{stderr}"
            );
        } else if let Some(token) = tag.strip_prefix("rewind-error") {
            assert!(
                !output.status.success(),
                "Reference example {ran} should fail:
{source}"
            );
            assert!(
                stderr.contains(token.trim()),
                "Reference example {ran} lacks {token}:
{stderr}"
            );
            fs::remove_dir_all(path).unwrap();
            continue;
        } else if let Some(code) = tag.strip_prefix("rewind-exit") {
            let code: i32 = code.trim().parse().unwrap();
            assert_eq!(
                output.status.code(),
                Some(code),
                "Reference example {ran}:
{source}
{stderr}"
            );
        }
        if let Some((next, expected)) = blocks.get(index + 1) {
            if next == "text" {
                assert_eq!(
                    stdout.trim_end(),
                    expected.trim_end(),
                    "Reference example {ran} output:
{source}"
                );
            }
        }
        fs::remove_dir_all(path).unwrap();
    }
    assert!(ran >= 60, "only {ran} runnable examples in the reference");
}
