//! LANG-01: named and default call arguments are rewritten to positional calls before checking.
use std::{fs, process::Command};

fn run(source: &str, command: &str) -> (bool, String, String) {
    let dir = std::env::temp_dir().join(format!(
        "rewind-callargs-{}-{}",
        std::process::id(),
        source.len()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("main.rw"), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .args([command, "main.rw"])
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const DECLS: &str = "fn connect(host: String, port: Int = 80, secure: Bool = false) -> String effects {} { return host; }\nfn area(width: Int, height: Int) -> Int effects {} { return width * 10 + height; }\n";

#[test]
fn named_and_default_arguments_land_in_parameter_order() {
    let (_, stdout, err) = run(
        &format!("{DECLS}fn main() -> Int {{ Out.println(area(height: 2, width: 7)); Out.println(connect(secure: true, host: \"h\")); Out.println(area(3, height: 4)); return 0; }}\n"),
        "run",
    );
    assert_eq!(stdout, "72\nh\n34\n", "{err}");
}

#[test]
fn a_default_fills_the_missing_tail_of_a_positional_call() {
    let (ok, stdout, err) = run(
        &format!("fn scale(value: Int, factor: Int = 3) -> Int effects {{}} {{ return value * factor; }}\nfn main() -> Int {{ Out.println(scale(2)); Out.println(scale(2, 5)); return 0; }}\n{}", if DECLS.is_empty() { "" } else { "" }),
        "run",
    );
    assert!(ok, "{err}");
    assert_eq!(stdout, "6\n10\n");
}

fn rejected(body: &str, needle: &str) {
    let (ok, _, err) = run(
        &format!("{DECLS}fn main() -> Int {{ {body} return 0; }}\n"),
        "check",
    );
    assert!(!ok, "accepted: {body}");
    assert!(err.contains(needle), "{body}: {err}");
}

#[test]
fn bad_argument_lists_are_rejected_with_the_parameter_name() {
    rejected(
        "let a = area(width: 1, depth: 2);",
        "unknown parameter 'depth'",
    );
    rejected("let a = area(1, width: 2);", "given twice");
    rejected("let a = area(width: 1);", "missing argument 'height'");
    rejected(
        "let a = area(width: 1, 2);",
        "positional argument cannot follow",
    );
    rejected(
        "let a = mystery(x: 1);",
        "named arguments need a call to a declared function",
    );
}

#[test]
fn module_functions_can_be_called_with_names() {
    let (ok, _, err) = run(
        "import std.retry as retry;\nfn main() -> Int { let p = retry.policy(maxAttempts: 3, baseMillis: 10, maxDelayMillis: 100, totalMillis: 1000, idempotent: true, retryResponses: false); return 0; }\n",
        "check",
    );
    assert!(ok, "{err}");
}

#[test]
fn defaults_must_be_literals_and_trail() {
    let (ok, _, err) = run(
        "fn f(a: Int = 1 + 2) -> Int effects {} { return a; }\nfn main() -> Int { return 0; }\n",
        "check",
    );
    assert!(
        !ok && err.contains("default argument must be a literal"),
        "{err}"
    );
    let (ok, _, err) = run(
        "fn f(a: Int = 1, b: Int) -> Int effects {} { return a; }\nfn main() -> Int { return 0; }\n",
        "check",
    );
    assert!(
        !ok && err.contains("cannot follow one with a default"),
        "{err}"
    );
}
