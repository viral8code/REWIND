//! DIST-03: formats are versioned per axis and old or foreign files are rejected explicitly, never reinterpreted.
use std::{fs, path::PathBuf, process::Command};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rewind-compat-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir(&d).unwrap();
    d
}
fn rewind(cwd: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

const SOURCE: &str = "fn main() -> Int { Out.println(\"hi\"); return 0; }\n";

#[test]
fn compat_lists_every_axis_with_its_mismatch_behaviour() {
    let d = dir("report");
    let out = rewind(&d, &["compat", "--json"]);
    assert!(out.status.success());
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["kind"], "rewind-compatibility");
    assert_eq!(doc["compiler"], env!("CARGO_PKG_VERSION"));
    let axes: Vec<_> = doc["axes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["axis"].as_str().unwrap().to_owned())
        .collect();
    for wanted in [
        "compiler",
        "language",
        "artifact",
        "trace",
        "session",
        "lock",
        "api document",
        "sdk manifest",
    ] {
        assert!(
            axes.iter().any(|a| a == wanted),
            "{wanted} missing from {axes:?}"
        );
    }
    for axis in doc["axes"].as_array().unwrap() {
        assert!(!axis["on_mismatch"].as_str().unwrap().is_empty());
    }
    let text = rewind(&d, &["compat"]);
    assert!(String::from_utf8_lossy(&text.stdout).contains("on mismatch"));
    let bad = rewind(&d, &["compat", "--nope"]);
    assert!(!bad.status.success());
}

#[test]
fn an_artifact_from_another_compiler_or_format_is_rejected_not_converted() {
    let d = dir("artifact");
    fs::write(d.join("main.rw"), SOURCE).unwrap();
    assert!(rewind(&d, &["compile", "main.rw"]).status.success());
    let ok = rewind(&d, &["run", "main.rwc"]);
    assert_eq!(String::from_utf8_lossy(&ok.stdout).trim(), "hi");
    let mut artifact: serde_json::Value =
        serde_json::from_slice(&fs::read(d.join("main.rwc")).unwrap()).unwrap();
    artifact["compiler"] = "0.0.1".into();
    fs::write(d.join("old.rwc"), serde_json::to_vec(&artifact).unwrap()).unwrap();
    let rejected = rewind(&d, &["run", "old.rwc"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("unsupported format/compiler"));
    artifact["compiler"] = env!("CARGO_PKG_VERSION").into();
    artifact["format"] = 1.into();
    fs::write(d.join("v1.rwc"), serde_json::to_vec(&artifact).unwrap()).unwrap();
    let rejected = rewind(&d, &["run", "v1.rwc"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("unsupported format/compiler"));
}

#[test]
fn a_trace_from_another_compiler_is_not_replayed() {
    let d = dir("trace");
    fs::write(d.join("main.rw"), SOURCE).unwrap();
    assert!(rewind(&d, &["run", "main.rw", "--record", "trace.json"])
        .status
        .success());
    let ok = rewind(&d, &["replay", "trace.json"]);
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let mut trace: serde_json::Value =
        serde_json::from_slice(&fs::read(d.join("trace.json")).unwrap()).unwrap();
    trace["compiler"] = "0.0.1".into();
    fs::write(d.join("old.json"), serde_json::to_vec(&trace).unwrap()).unwrap();
    let rejected = rewind(&d, &["replay", "old.json"]);
    assert!(!rejected.status.success());
    trace["compiler"] = env!("CARGO_PKG_VERSION").into();
    trace["format"] = 7.into();
    fs::write(d.join("newer.json"), serde_json::to_vec(&trace).unwrap()).unwrap();
    assert!(!rewind(&d, &["replay", "newer.json"]).status.success());
}
