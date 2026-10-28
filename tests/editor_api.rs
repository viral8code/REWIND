//! The editor-facing surface of the compiler: `rewind editor-info`, `rewind tokens` and the
//! outline and colouring requests of `rewind lsp`.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "rewind-editor-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&path);
    fs::create_dir(&path).unwrap();
    path
}
fn rewind(args: &[&str], stdin: &str) -> (i32, Value, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let json = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    (
        output.status.code().unwrap(),
        json,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn editor_info_lists_the_words_of_the_language() {
    let (code, info, stderr) = rewind(&["editor-info"], "");
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(info["format"], 1);
    assert_eq!(info["languages"]["latest"], "4.2.1");
    let words = |key: &str| -> Vec<&str> {
        info[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w.as_str().unwrap())
            .collect()
    };
    assert!(words("keywords").contains(&"commit"));
    assert!(words("keywords").contains(&"fn"));
    assert!(words("contextualKeywords").contains(&"value"));
    assert!(words("removedWords").contains(&"null"));
    assert!(!words("keywords").contains(&"value"));
    // The list is the list the compiler enforces: each keyword is refused as a name in 4.0.0.
    let dir = root();
    for word in words("keywords") {
        let source = format!("fn main() -> Int {{ let {word} = 1; return 0; }}\n");
        fs::write(dir.join("main.rw"), source).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&dir)
            .args(["check", "main.rw"])
            .output()
            .unwrap();
        assert!(!out.status.success(), "{word} was accepted as a name");
    }
}

#[test]
fn tokens_work_on_source_that_does_not_compile() {
    let source =
        "fn main() -> Int {\n    let s = \"open;\n    // keep this comment\n    return 0;\n}\n";
    let (code, doc, stderr) = rewind(&["tokens", "-"], source);
    assert_eq!(code, 0, "{stderr}");
    let tokens = doc["tokens"].as_array().unwrap();
    assert_eq!(tokens[0]["kind"], "keyword");
    assert!(tokens
        .iter()
        .any(|t| t["kind"] == "string" && t["error"] == "unterminated"));
    assert_eq!(doc["symbols"][0]["name"], "main");
    assert_eq!(doc["symbols"][0]["kind"], "function");
}

#[test]
fn tokens_refuse_oversized_and_non_utf8_sources() {
    let dir = root();
    let big = dir.join("big.rw");
    fs::write(&big, vec![b' '; (1 << 20) + 1]).unwrap();
    let (code, _, stderr) = rewind(&["tokens", big.to_str().unwrap()], "");
    assert_ne!(code, 0);
    assert!(stderr.contains("CompilerBudgetExceeded"), "{stderr}");
    let (code, _, _) = rewind(&["tokens", "-"], "\u{0}");
    assert_eq!(code, 0);
    let bad = dir.join("bad.rw");
    fs::write(&bad, [0xff, 0xfe, 0x41]).unwrap();
    let (code, _, stderr) = rewind(&["tokens", bad.to_str().unwrap()], "");
    assert_ne!(code, 0);
    assert!(stderr.contains("UTF-8"), "{stderr}");
}

#[test]
fn language_server_serves_outline_and_semantic_tokens() {
    let dir = root();
    let file = dir.join("main.rw");
    fs::write(&file, "").unwrap();
    let uri = format!(
        "file:///{}",
        file.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let source = "struct P { x: Int }\nimpl P { fn len(self) -> Int { return 1; } }\nfn main() -> Int { return 0; } // end\n";
    let requests = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":uri}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"textDocument/semanticTokens/full","params":{"textDocument":{"uri":uri}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"shutdown","params":{}}),
        json!({"jsonrpc":"2.0","method":"exit","params":{}}),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["lsp", "--root", dir.to_str().unwrap()])
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
        let n: usize = std::str::from_utf8(&rest[..end])
            .unwrap()
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        messages.push(serde_json::from_slice::<Value>(&rest[end + 4..end + 4 + n]).unwrap());
        rest = &rest[end + 4 + n..];
    }
    let capabilities = &messages[0]["result"]["capabilities"];
    assert_eq!(capabilities["documentSymbolProvider"], true);
    assert_eq!(
        capabilities["semanticTokensProvider"]["legend"]["tokenTypes"][0],
        "keyword"
    );
    let symbols = messages.iter().find(|m| m["id"] == 2).unwrap()["result"]
        .as_array()
        .unwrap()
        .clone();
    let names: Vec<&str> = symbols
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["P", "P", "len", "main"]);
    assert_eq!(symbols[2]["kind"], 6);
    assert_eq!(
        symbols[3]["range"]["start"],
        json!({"line": 2, "character": 3})
    );
    let data = messages.iter().find(|m| m["id"] == 3).unwrap()["result"]["data"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(data.len() % 5, 0);
    // The first token is `struct`: line 0, column 0, length 6, type keyword.
    assert_eq!(
        data[..5],
        [json!(0), json!(0), json!(6), json!(0), json!(0)]
    );
}

#[test]
fn language_server_resolves_std_in_a_project() {
    let dir = root();
    fs::write(
        dir.join("rewind.toml"),
        "language = \"4.2.1\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    let source = "import std.math as math;\nfn main() -> Int { return 0; }\n";
    fs::write(dir.join("main.rw"), source).unwrap();
    let uri = format!(
        "file:///{}",
        dir.join("main.rw")
            .to_string_lossy()
            .replace(char::from(92), "/")
            .trim_start_matches('/')
    );
    let requests = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":source}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}}),
        json!({"jsonrpc":"2.0","method":"exit","params":{}}),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["lsp", "--root", dir.to_str().unwrap()])
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
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("publishDiagnostics"), "{text}");
    assert!(!text.contains("I/O error"), "{text}");
}
