use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-quirk-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"3.0.0\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    assert!(call(&root, &["update"]).status.success());
    root
}
fn call(root: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn run(source: &str) -> Output {
    call(&project(source), &["run", "--root", "."])
}
fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n")
}

const ENUM: &str = "enum E { A, B, P { x: Int } }\nstruct Point { x: Int }\n";

fn expect_output(body: &str, expected: &str) {
    let out = run(&format!(
        "{ENUM}fn main() -> Int {{\n{body}\n    return 0;\n}}\n"
    ));
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), expected);
}

#[test]
fn match_scrutinee_may_be_a_bare_enum_path() {
    expect_output(
        "let v = E::B; match E::A { E::A => { Out.println(1); }, _ => { Out.println(2); }, } match v { E::A => { Out.println(3); }, _ => { Out.println(4); }, }",
        "1\n4\n",
    );
}

#[test]
fn if_and_while_conditions_may_end_in_an_enum_path() {
    expect_output(
        "let v = E::A; var n = 0;\n if v == E::A { n = n + 1; }\n if v != E::A { n = n + 100; } else { n = n + 10; }\n while v == E::B { n = 1000; }\n Out.println(n);",
        "11\n",
    );
}

#[test]
fn empty_block_after_a_path_in_a_condition_is_a_block() {
    expect_output(
        "let v = E::A; if v == E::A {} else { Out.println(1); }\n if v == E::B {} else { Out.println(2); }\n
 Out.println(3);",
        "2\n3\n",
    );
}

#[test]
fn constructions_in_conditions_and_scrutinees_still_work() {
    expect_output(
        "let p = E::P { x: 5 };\n if p == E::P { x: 5 } { Out.println(1); }\n match E::P { x: 7 } { E::P { x: n } => { Out.println(n); }, _ => { Out.println(0); }, }\n if p == (E::P { x: 6 }) { Out.println(9); } else { Out.println(2); }",
        "1\n7\n2\n",
    );
}

#[test]
fn empty_construction_inside_brackets_of_a_condition_stays_a_construction() {
    let out = run(&format!(
        "{ENUM}fn same(a: E, b: E) -> Bool {{ return a == b; }}\nfn main() -> Int {{\n if same(E::A, E::A) {{ Out.println(1); }}\n return 0;\n}}\n"
    ));
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "1\n");
}

#[test]
fn construction_outside_conditions_is_unchanged() {
    expect_output(
        "let p = E::P { x: 3 }; match p { E::P { x: n } => { Out.println(n); }, _ => { Out.println(0); }, }",
        "3\n",
    );
}

#[test]
fn comparison_chain_with_greater_equal_parses() {
    // `a < b >= c` is a chain of comparisons: it parses, and the type
    // checker (not the parser) reports Bool >= Int.
    let out = run("fn main() -> Int {\n let a = 1; let b = 2; let c = 3;\n let t = a < b >= c;\n return 0;\n}\n");
    assert!(!out.status.success());
    let error = stderr(&out);
    assert!(error.contains("cannot compare Bool and Int"), "{error}");
    assert!(!error.contains("expected"), "{error}");
}

#[test]
fn comparison_chain_with_greater_equal_runs_on_bools() {
    expect_output(
        "let a = 1; let b = 2; let t = a < b == true; Out.println(t); let u = a >= b; Out.println(u);",
        "true\nfalse\n",
    );
}

#[test]
fn type_arguments_closing_with_greater_equal_still_split() {
    expect_output("let xs: List<Int>= [1, 2]; Out.println(xs[1]);", "2\n");
}

fn run_body(body: &str) -> Output {
    run(&format!(
        "{ENUM}fn main() -> Int {{\n{body}\n    return 0;\n}}\n"
    ))
}

#[test]
fn runtime_values_accept_integer_literal_forms() {
    expect_output(
        "    runtime { executionSteps = 1_000_000; historyMemory = 0x10 KiB; spillThreshold = 0b1000 KB; }\n    Out.println(\"ok\");",
        "ok\n",
    );
}

#[test]
fn runtime_values_reject_non_integer_tokens() {
    let out = run_body("    runtime { executionSteps = 1.5; }");
    assert!(!out.status.success());
    let out = run_body("    runtime { executionSteps = _1; }");
    assert!(!out.status.success());
}

#[test]
fn string_pattern_is_a_single_literal_token() {
    expect_output(
        "    match \"a\" {\n        \"a\" => { Out.println(\"hit\"); },\n        _ => { Out.println(\"miss\"); },\n    }",
        "hit\n",
    );
    let out = run_body("    let x = \"b\";\n    match \"a\" {\n        \"a\" + x => { Out.println(\"hit\"); },\n        _ => { Out.println(\"miss\"); },\n    }");
    assert!(!out.status.success());
    assert!(
        !stderr(&out).contains("invalid literal pattern"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn non_ascii_symbols_are_rejected_by_the_lexer() {
    for body in [
        "    let x = 1 ☃ 2;",
        "    let ② = 1;",
        "    let x = 1\u{200b};",
    ] {
        let out = run_body(body);
        assert!(!out.status.success(), "{body}");
        assert!(
            stderr(&out).contains("unexpected character"),
            "{body}: {}",
            stderr(&out)
        );
    }
    // 名前の 2 文字目以降は is_alphanumeric なので、x² は 1 つの名前。
    expect_output("    let x² = 3;\n    Out.println(x²);", "3\n");
    // 文字列とコメントの中は何でも書ける。
    expect_output("    // ☃\n    Out.println(\"☃ ②\");", "☃ ②\n");
}

#[test]
fn integer_patterns_accept_only_integer_tokens() {
    expect_output(
        "    match -5 {\n        -5..-1 => { Out.println(\"neg\"); },\n        _ => { Out.println(\"other\"); },\n    }",
        "neg\n",
    );
    for pattern in ["-_", "0.._1", "-__", "-1.5"] {
        let out = run_body(&format!("    match 1 {{\n        {pattern} => {{ Out.println(\"x\"); }},\n        _ => {{ Out.println(\"y\"); }},\n    }}"));
        assert!(!out.status.success(), "{pattern}");
        let err = stderr(&out);
        assert!(err.contains("expected an integer"), "{pattern}: {err}");
        assert!(
            !err.contains("integer literal overflow"),
            "{pattern}: {err}"
        );
    }
    let out = run_body("    match 1 {\n        99999999999999999999 => { Out.println(\"x\"); },\n        _ => { Out.println(\"y\"); },\n    }");
    assert!(
        stderr(&out).contains("integer literal overflow"),
        "{}",
        stderr(&out)
    );
    let out = run_body("    let v = -_9223372036854775808;");
    assert!(!out.status.success());
}
