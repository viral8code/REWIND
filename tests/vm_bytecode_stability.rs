//! VM の変数まわりの最適化で、`Op` の Debug 出力（artifact の `bytecode`）と、
//! debug 記録のトレースの operation の列が変わらないことを固定する。
//! 期待値は、最適化の前（HEAD の 3bd88a3）の処理系で採った SHA-256 とステップ数。

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::process::Command;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/vm_ops_bytecode.rw"
);
const BYTECODE_OPS: usize = 241;
const BYTECODE_SHA256: &str = "51faf5c49a39fd5c63eeb9ad86ebeb72caf4d4ef49f277a4c46aeb0fb6c30d91";
const TRACE_OPS: usize = 485;
const TRACE_SHA256: &str = "0bcda57066e86301f86b5da0764732d9cb70a6a592626147af6ff29eab938d59";

fn scratch(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("rewind-bytecode-stability-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn sha256_lines(lines: &[String]) -> String {
    let digest = Sha256::digest(lines.join("\n").as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn collect_operations(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(op)) = map.get("operation") {
                out.push(op.clone());
            }
            for v in map.values() {
                collect_operations(v, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_operations(v, out)),
        _ => {}
    }
}

#[test]
fn compiled_bytecode_debug_strings_are_unchanged() {
    let output = scratch("ops.rwc");
    let status = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["compile", FIXTURE, "--output"])
        .arg(&output)
        .status()
        .unwrap();
    assert!(status.success());
    let artifact: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    let ops: Vec<String> = artifact["payload"]["build"]["bytecode"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["operation"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ops.len(), BYTECODE_OPS);
    assert_eq!(sha256_lines(&ops), BYTECODE_SHA256);
    // 主な Op が含まれていること。
    for name in [
        "Load(",
        "Declare(",
        "Store(",
        "StoreField(",
        "Binary(",
        "Call(",
        "MakeClosure",
        "DeferValue",
        "FailMatch",
        "Enter",
        "Exit",
        "JumpFalse",
    ] {
        assert!(ops.iter().any(|o| o.starts_with(name)), "missing {name}");
    }
}

#[test]
fn debug_trace_operation_sequence_is_unchanged_and_replays() {
    // replay は記録した entry（ファイル名）をトレースと同じ場所から探すので、ソースを並べて置く。
    let source = scratch("vm_ops_bytecode.rw");
    fs::copy(FIXTURE, &source).unwrap();
    let trace = scratch("ops.trace.json");
    let run = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(&source)
        .args(["--record-mode", "debug", "--record"])
        .arg(&trace)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let value: Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
    let mut ops = Vec::new();
    collect_operations(&value, &mut ops);
    assert_eq!(ops.len(), TRACE_OPS);
    assert_eq!(sha256_lines(&ops), TRACE_SHA256);
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&trace)
        .output()
        .unwrap();
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
}
