use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project_with(language: &str, source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-raw-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(
        root.join("rewind.toml"),
        format!("language = \"{language}\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n"),
    )
    .unwrap();
    assert!(call(&root, &["update"]).status.success());
    root
}
fn project(source: &str) -> PathBuf {
    project_with("3.0.0", source)
}
fn call(root: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n")
}
/// Runs `body` inside `fn main` and returns the raw stdout bytes as text.
fn run_raw(source: &str) -> String {
    let root = project(source);
    let out = call(&root, &["run", "--root", "."]);
    assert!(out.status.success(), "{}\n{source}", stderr(&out));
    String::from_utf8_lossy(&out.stdout).into_owned()
}
fn run(source: &str) -> String {
    run_raw(source).replace("\r\n", "\n")
}
fn main_with(body: &str) -> String {
    format!("fn main()->Int effects {{output}} {{\n{body}\n    publish; return 0;\n}}\n")
}
/// The program must be rejected; returns the diagnostic text.
fn rejected(source: &str, code: &str) -> String {
    let root = project(source);
    let out = call(&root, &["check", "--root", "."]);
    assert!(!out.status.success(), "accepted: {source}");
    let err = stderr(&out);
    assert!(err.contains(code), "expected {code}: {err}");
    err
}

#[test]
fn indentation_of_the_closing_line_is_removed() {
    let source = main_with(
        "    let s = \"\"\"\n        hello\n          world\n   \n        \"q\"\n        \"\"\";\n    Out.println(\"[\" + s + \"]\");",
    );
    // A whitespace-only line becomes an empty line; the first and last line breaks are dropped.
    assert_eq!(run(&source), "[hello\n  world\n\n\"q\"]\n");
}

#[test]
fn closing_quotes_may_be_followed_by_code_and_tabs_work() {
    let source = main_with(
        "\tlet s = \"\"\"\n\t\ta\n\t\t\tb\n\t\t\"\"\" + \"!\";\n    Out.println(\"[\" + s + \"]\");",
    );
    assert_eq!(run(&source), "[a\n\tb!]\n");
}

#[test]
fn closing_quotes_at_column_one_remove_nothing() {
    let source = "fn main()->Int effects {output} {\n    let s = \"\"\"\n  two\n\"\"\";\n    Out.println(\"[\" + s + \"]\");\n    publish; return 0;\n}\n";
    assert_eq!(run(source), "[  two]\n");
}

#[test]
fn one_line_form() {
    let source = main_with("    Out.println(\"\"\"a \"b\" c\"\"\");");
    assert_eq!(run(&source), "a \"b\" c\n");
}

#[test]
fn interpolation_and_escapes_are_not_applied() {
    let source = main_with(
        "    let x = 1;\n    Out.println(\"\"\"v=${x} \\n \\t \\\" $${x}\"\"\");\n    let m = \"\"\"\n        ${x}\\n\n        \"\"\";\n    Out.println(m);\n    Out.println(\"v=${x}\");",
    );
    assert_eq!(run(&source), "v=${x} \\n \\t \\\" $${x}\n${x}\\n\nv=1\n");
}

#[test]
fn longer_delimiters_can_hold_triple_quotes() {
    let source = main_with(
        "    Out.println(\"\"\"\"a \"\"\" b\"\"\"\");\n    let s = \"\"\"\"\"\n        x\n        \"\"\"\" and \"\"\"\"\n        \"\"\"\"\";\n    Out.println(s);",
    );
    assert_eq!(run(&source), "a \"\"\" b\nx\n\"\"\"\" and \"\"\"\"\n");
}

#[test]
fn raw_strings_work_as_match_patterns() {
    let source = main_with(
        "    let t = \"a \\\"q\\\" b\";\n    match t {\n        \"\"\"a \"q\" b\"\"\" => { Out.println(\"hit\"); },\n        _ => { Out.println(\"miss\"); },\n    }\n    match \"\\\\n\" {\n        \"\"\"\\n\"\"\" => { Out.println(\"backslash\"); },\n        _ => { Out.println(\"escape\"); },\n    }",
    );
    assert_eq!(run(&source), "hit\nbackslash\n");
}

#[test]
fn raw_strings_work_in_const_and_default_arguments() {
    let source = "const BANNER: String = \"\"\"\n    <b>${x}</b>\n    \"\"\";\nfn greet(s: String = \"\"\"hi ${who}\"\"\")->String effects {} { return s; }\nfn main()->Int effects {output} {\n    Out.println(BANNER);\n    Out.println(greet());\n    Out.println(greet(\"x ${1}\"));\n    publish; return 0;\n}\n";
    assert_eq!(run(source), "<b>${x}</b>\nhi ${who}\nx 1\n");
}

#[test]
fn raw_strings_are_accepted_in_every_supported_language_version() {
    let source = "fn main()->Int effects {output} {\n    Out.println(\"\"\"a ${b}\"\"\");\n    return 0;\n}\n";
    for language in ["3.0.0", "3.6.0", "4.0.0"] {
        let root = project_with(language, source);
        let out = call(&root, &["run", "--root", "."]);
        assert!(out.status.success(), "{language}: {}", stderr(&out));
        let printed = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
        assert_eq!(printed, "a ${b}\n", "{language}");
    }
}

#[test]
fn crlf_sources_give_the_same_value() {
    let source = main_with(
        "    let s = \"\"\"\n        a\n\n          b\n        \"\"\";\n    Out.println(\"[\" + s + \"]\");\n    Out.println(\"\"\"one\"\"\");",
    );
    let lf = run_raw(&source);
    let crlf = run_raw(&source.replace('\n', "\r\n"));
    assert_eq!(lf, crlf);
    let cr = run_raw(&source.replace('\n', "\r"));
    assert_eq!(lf, cr);
    assert_eq!(lf.replace("\r\n", "\n"), "[a\n\n  b]\none\n");
}

#[test]
fn indentation_error_names_the_line() {
    let source = "fn main()->Int effects {} {\n    let s = \"\"\"\n        ok\n      bad\n        \"\"\";\n    return 0;\n}\n";
    let err = rejected(source, "RawStringIndent");
    assert!(err.contains("main.rw:4:1"), "{err}");
    assert!(err.contains("indented like the closing quotes"), "{err}");
}

#[test]
fn indentation_must_match_exactly_with_tabs_and_spaces() {
    let source = "fn main()->Int effects {} {\n\tlet s = \"\"\"\n\t\ta\n        b\n\t\t\"\"\";\n    return 0;\n}\n";
    rejected(source, "RawStringIndent");
}

#[test]
fn missing_closing_quotes() {
    let source =
        "fn main()->Int effects {} {\n    let s = \"\"\"\n        text\n    return 0;\n}\n";
    let err = rejected(source, "RawStringUnterminated");
    assert!(err.contains("main.rw:2:13"), "{err}");
    let source = "fn main()->Int effects {} {\n    let s = \"\"\"abc\";\n    return 0;\n}\n";
    rejected(source, "RawStringOpening");
}

#[test]
fn quote_run_longer_than_the_delimiter() {
    let source = "fn main()->Int effects {} {\n    let s = \"\"\"\n        a \"\"\" b\n        \"\"\";\n    return 0;\n}\n";
    let err = rejected(source, "RawStringQuote");
    assert!(err.contains("main.rw:3:11"), "{err}");
    assert!(err.contains("more than 3 quotes"), "{err}");
    let source = "fn main()->Int effects {} {\n    let s = \"\"\"a\"\"\"\";\n    return 0;\n}\n";
    rejected(source, "RawStringQuote");
    let source =
        "fn main()->Int effects {} {\n    let s = \"\"\"a \"\"\"\" b\"\"\";\n    return 0;\n}\n";
    rejected(source, "RawStringQuote");
}

#[test]
fn empty_one_line_form_is_rejected() {
    let source = "fn main()->Int effects {} {\n    let s = \"\"\"\"\"\";\n    return 0;\n}\n";
    let err = rejected(source, "RawString");
    assert!(err.contains("empty string"), "{err}");
}

#[test]
fn text_after_the_opening_quotes_of_a_multiline_form() {
    let source = "fn main()->Int effects {} {\n    let s = \"\"\"text\n        more\n        \"\"\";\n    return 0;\n}\n";
    rejected(source, "RawStringOpening");
}

#[test]
fn dollar_prefixed_form_stays_invalid() {
    let source = "fn main()->Int effects {} {\n    let s = $\"\"\"a\"\"\";\n    return 0;\n}\n";
    let root = project(source);
    let out = call(&root, &["check", "--root", "."]);
    assert!(!out.status.success());
}

#[test]
fn positions_after_a_raw_string_are_correct() {
    let source = "fn main()->Int effects {} {\n    let s = \"\"\"\n        a\n        b\n        \"\"\"; let y = ;\n    return 0;\n}\n";
    let root = project(source);
    let out = call(&root, &["check", "--root", "."]);
    assert!(!out.status.success());
    // `let y = ;` is on line 5; the `;` follows the closing quotes and `let y = `.
    assert!(stderr(&out).contains("main.rw:5:"), "{}", stderr(&out));
}

#[test]
fn empty_string_and_plain_strings_are_unchanged() {
    let source = main_with(
        "    Out.println(\"[\" + \"\" + \"]\");\n    Out.println(\"x\");\n    Out.println(\"\" + \"\");\n    Out.println(\"a\\\"b\");",
    );
    assert_eq!(run(&source), "[]\nx\n\na\"b\n");
}

#[test]
fn formatter_does_not_touch_a_valid_multiline_raw_string() {
    let source = "fn main()->Int effects {output} {\n    let s = \"\"\"\n        { // x\n          \"y\"\n        \"\"\";\n    if true {\n        Out.println(s);\n    }\n    publish; return 0;\n}\n";
    let root = project(source);
    let fmt = call(&root, &["fmt", "main.rw"]);
    assert!(fmt.status.success(), "{}", stderr(&fmt));
    assert_eq!(source, fs::read_to_string(root.join("main.rw")).unwrap());
    let check = call(&root, &["fmt", "main.rw", "--check"]);
    assert!(check.status.success(), "{}", stderr(&check));
    let out = call(&root, &["run", "--root", "."]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        "{ // x\n  \"y\"\n"
    );
}
