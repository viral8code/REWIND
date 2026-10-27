use std::{fs, path::Path, process::Command};

const SOURCE: &str = "effects {external};import std.text as text;\n\
fn main() -> Int {\n\
    var first=\"\";\n\
    var second=\"\";\n\
    external {\n\
        first=text.lineSeparator();\n\
        second=text.lineSeparator();\n\
    }\n\
    Out.println(text.length(first));\n\
    Out.println(text.length(second));\n\
    Out.println(first==\"\\n\" || first==\"\\r\\n\");\n\
    Out.println(first==second);\n\
    return 0;\n\
}\n";

const SINGLE: &str = "effects {external};import std.text as text;
fn main() -> Int {
    var sep=\"\";
    external {
        sep=text.lineSeparator();
    }
    Out.println(text.length(sep));
    return 0;
}
";

fn root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("rewind-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    root
}

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn line_separator_matches_the_host_and_needs_the_external_effect() {
    let root = root("std-text-line-separator");
    fs::write(root.join("main.rw"), SOURCE).unwrap();
    let result = run(&root, &["run", "main.rw", "--allow-effects", "external"]);
    success(&result);
    let text = String::from_utf8(result.stdout).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    let expected = if cfg!(windows) { "2" } else { "1" };
    assert_eq!(lines, [expected, expected, "true", "true"]);
    let denied = run(&root, &["run", "main.rw"]);
    assert!(!denied.status.success());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recorded_line_separator_replays_the_recorded_value() {
    let root = root("std-text-line-separator-replay");
    fs::write(root.join("main.rw"), SOURCE).unwrap();
    success(&run(
        &root,
        &["compile", "main.rw", "--allow-effects", "external"],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = run(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            "external",
            "--record",
            "trace.json",
        ],
    );
    success(&output);
    // The trace holds the observed value, so replay does not look at the host again.
    let trace = fs::read_to_string(root.join("trace.json")).unwrap();
    let host = if cfg!(windows) { r"\\r\\n" } else { r"\\n" };
    let recorded = format!(r#""outcome": "{{\"Ok\":\"{host}\"}}""#);
    assert!(trace.contains(&recorded), "recorded outcome not found");
    let replay = run(
        &root,
        &["replay", "trace.json", "--allow-effects", "external"],
    );
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replay_returns_the_recorded_separator_not_the_host_value() {
    let root = root("std-text-line-separator-swap");
    fs::write(root.join("main.rw"), SINGLE).unwrap();
    success(&run(
        &root,
        &[
            "run",
            "main.rw",
            "--allow-effects",
            "external",
            "--record",
            "trace.json",
        ],
    ));
    let trace = fs::read_to_string(root.join("trace.json")).unwrap();
    let host = if cfg!(windows) { r"\\r\\n" } else { r"\\n" };
    let other = if cfg!(windows) { r"\\n" } else { r"\\r\\n" };
    let recorded = format!(r#""outcome": "{{\"Ok\":\"{host}\"}}""#);
    let swapped = format!(r#""outcome": "{{\"Ok\":\"{other}\"}}""#);
    assert!(trace.contains(&recorded), "recorded outcome not found");
    fs::write(root.join("trace2.json"), trace.replace(&recorded, &swapped)).unwrap();
    // The edited trace no longer matches its recorded final state, so replay ends with a mismatch, but the
    // program has already seen the recorded value: the printed length follows the trace, not the host.
    let result = run(
        &root,
        &["replay", "trace2.json", "--allow-effects", "external"],
    );
    if cfg!(windows) {
        // The edited value is shorter than the recorded one, so replay reaches the program.
        let out = String::from_utf8(result.stdout).unwrap();
        assert_eq!(out.lines().next(), Some("1"));
    } else {
        // On other hosts the edit makes the value longer, which the bounded external ledger
        // of the trace rejects before the program runs: replay never falls back to the host.
        assert!(!result.status.success());
        let err = String::from_utf8_lossy(&result.stderr);
        assert!(err.contains("ReplayMismatch"), "{err}");
    }
    fs::remove_dir_all(root).unwrap();
}
