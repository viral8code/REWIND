use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project(language: &str, source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v400-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
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
fn call(root: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn run(language: &str, source: &str) -> Output {
    call(&project(language, source), &["run", "--root", "."])
}
/// Runs `main.rw` without a manifest: the language is the compiler's own (4.0.0)
/// and the bundled std modules are importable.
fn run_standalone(source: &str) -> Output {
    let root = std::env::temp_dir().join(format!(
        "rewind-v400-standalone-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    call(&root, &["run", "main.rw"])
}
fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n")
}

const KEYWORDS: &[&str] = &[
    "fn", "let", "var", "const", "struct", "record", "enum", "trait", "impl", "type", "import",
    "as", "pub", "private", "async", "mut", "if", "else", "match", "while", "for", "in", "break",
    "continue", "return", "defer", "using", "commit", "revert", "resume", "drop", "publish",
    "branch", "runtime", "external", "move", "spawn", "await", "true", "false",
];

#[test]
fn keywords_cannot_be_names_in_language_4_0_0() {
    for word in KEYWORDS {
        let out = run(
            "4.0.0",
            &format!("fn main() -> Int {{\n    let {word} = 1;\n    return 0;\n}}\n"),
        );
        assert!(!out.status.success(), "let {word}");
        let err = stderr(&out);
        assert!(err.contains("ReservedWord"), "let {word}: {err}");
        assert!(err.contains(&format!("'{word}'")), "let {word}: {err}");
    }
}

#[test]
fn keyword_names_are_rejected_in_every_binding_position() {
    for body in [
        "fn commit() -> Int { return 1; }\nfn main() -> Int { return 0; }\n",
        "fn f(drop: Int) -> Int { return drop; }\nfn main() -> Int { return 0; }\n",
        "struct S { revert: Int }\nfn main() -> Int { return 0; }\n",
        "fn main() -> Int {\n    for branch in 0..2 { Out.println(branch); }\n    return 0;\n}\n",
        "fn main() -> Int {\n    let f = |publish: Int| publish + 1;\n    return f(1);\n}\n",
        "fn main() -> Int {\n    let xs = [1];\n    match xs.len() { resume => { return 0; }, }\n}\n",
    ] {
        let out = run("4.0.0", body);
        assert!(!out.status.success(), "{body}");
        assert!(stderr(&out).contains("ReservedWord"), "{body}: {}", stderr(&out));
    }
}

#[test]
fn the_same_programs_still_work_in_language_3_6_0() {
    for word in ["commit", "drop", "branch", "publish", "resume", "revert", "if", "true"] {
        let out = run(
            "3.6.0",
            &format!("fn main() -> Int {{\n    let {word} = 1;\n    Out.println({word});\n    return 0;\n}}\n"),
        );
        // `let if = 1; Out.println(if)` cannot be used as a value, so only the
        // words that were usable as names must keep working.
        if matches!(word, "commit" | "drop" | "branch" | "publish" | "resume" | "revert") {
            assert!(out.status.success(), "{word}: {}", stderr(&out));
            assert_eq!(stdout(&out), "1\n", "{word}");
        }
    }
}

#[test]
fn the_null_literal_is_removed_in_language_4_0_0_only() {
    let source = "fn main() -> Int {\n    let x = null;\n    return 0;\n}\n";
    let out = run("4.0.0", source);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("ReservedWord"), "{err}");
    assert!(err.contains("null literal was removed"), "{err}");
    assert!(err.contains("None"), "{err}");
    let out = run("3.6.0", source);
    assert!(out.status.success(), "{}", stderr(&out));
}

#[test]
fn contextual_words_and_none_stay_usable_in_language_4_0_0() {
    let out = run(
        "4.0.0",
        "enum Level { None, Low }\nstruct Box { value: Int }\nfn main() -> Int {\n    let value = 3;\n    let capture = 4;\n    let test = 5;\n    let level = Level::None;\n    match level { Level::None => { Out.println(value + capture + test); }, _ => { Out.println(0); }, }\n    let b = Box(value);\n    Out.println(b.value);\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "12\n3\n");
}

#[test]
fn import_paths_and_effect_names_may_use_keyword_words() {
    let out = run_standalone(
        "import std.external as host;\nimport std.text as text;\nfn main() -> Int {\n    Out.println(text.length(\"abc\"));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "3\n");
}

#[test]
fn bundled_std_modules_compile_in_language_4_0_0() {
    let out = run_standalone(
        "import std.binding as binding;\nimport std.historyGraph as graph;\nimport std.guiCombo as combo;\nfn main() -> Int {\n    Out.println(\"ok\");\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ok\n");
}

#[test]
fn line_breaks_inside_string_literals_are_lf_whatever_the_source_uses() {
    let lf = "fn main() -> Int {\n    let s = \"a\nb\";\n    Out.println(s.charLen());\n    return 0;\n}\n";
    let crlf = lf.replace('\n', "\r\n");
    let cr = lf.replace('\n', "\r");
    let a = run_standalone(lf);
    assert!(a.status.success(), "{}", stderr(&a));
    // "a", line break, "b": three characters, in every kind of source.
    assert_eq!(stdout(&a), "3\n");
    let b = run_standalone(&crlf);
    assert!(b.status.success(), "{}", stderr(&b));
    assert_eq!(stdout(&b), "3\n");
    let c = run_standalone(&cr);
    assert!(c.status.success(), "{}", stderr(&c));
    assert_eq!(stdout(&c), "3\n");
}

#[test]
fn standalone_files_never_run_as_an_older_language_in_a_released_compiler() {
    let root = std::env::temp_dir().join(format!(
        "rewind-v400-old-standalone-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    // A file without `fn main` used to run as language 2.0.0 (`auto`).
    fs::write(root.join("main.rw"), "Out.println(1);\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&root)
        .env_remove("REWIND_LEGACY_LANGUAGES")
        .env("REWIND_STANDALONE_LANGUAGE", "auto")
        .args(["run", "main.rw"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("top-level statements"), "{}", stderr(&out));
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&root)
        .env_remove("REWIND_LEGACY_LANGUAGES")
        .env("REWIND_STANDALONE_LANGUAGE", "2.0.0")
        .args(["run", "main.rw"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}
