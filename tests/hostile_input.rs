//! Hostile and malformed sources must be rejected cleanly: no panic, no stack overflow, no hang.
//! Every case runs the real `rewind` binary on a source of at most 1 MiB (the compiler's own limit)
//! and checks that it ends by itself, quickly, with an ordinary failure or success exit code.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const LIMIT: Duration = Duration::from_secs(30);

fn dir() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-hostile-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}

struct Outcome {
    code: i32,
    stderr: String,
    seconds: f64,
}

fn run(mode: &str, source: &[u8]) -> Outcome {
    let root = dir();
    fs::write(root.join("main.rw"), source).unwrap();
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&root)
        .args([mode, "main.rw"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{mode} did not finish within {LIMIT:?} on a {} byte source", source.len());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        stderr = String::from_utf8_lossy(&bytes).into_owned();
    }
    let _ = fs::remove_dir_all(&root);
    Outcome {
        // A signal or a crash has no ordinary code; map it to a large number.
        code: status.code().unwrap_or(1000),
        stderr,
        seconds: started.elapsed().as_secs_f64(),
    }
}

/// The process ended on its own with an ordinary exit code (0 to 100) and no Rust panic.
fn assert_clean(name: &str, mode: &str, source: &[u8]) {
    let out = run(mode, source);
    assert!(
        (0..=100).contains(&out.code),
        "{name} ({mode}): exit code {} (a crash or panic?)\n{}",
        out.code,
        out.stderr.chars().take(400).collect::<String>()
    );
    assert!(
        !out.stderr.contains("panicked at") && !out.stderr.contains("overflowed its stack"),
        "{name} ({mode}): {}",
        out.stderr.chars().take(400).collect::<String>()
    );
    assert!(out.seconds < 25.0, "{name} ({mode}): took {:.1}s", out.seconds);
}

fn main_with(body: &str) -> Vec<u8> {
    format!("fn main() -> Int {{\n{body}\n    return 0;\n}}\n").into_bytes()
}

fn both(name: &str, source: &[u8]) {
    assert_clean(name, "check", source);
    assert_clean(name, "fmt", source);
}

#[test]
fn deeply_nested_brackets_are_rejected_without_overflowing_the_stack() {
    for (name, open, close) in [
        ("parens", "(", ")"),
        ("lists", "[", "]"),
        ("blocks", "{", "}"),
    ] {
        for depth in [1_000usize, 60_000, 400_000] {
            let body = format!("let x = {}1{};", open.repeat(depth), close.repeat(depth));
            both(&format!("{name} x{depth}"), &main_with(&body));
        }
    }
}

#[test]
fn deeply_nested_statements_and_types_are_rejected_cleanly() {
    let depth = 50_000usize;
    // nested ifs, matches, closures, generic types, patterns, tuples
    both("ifs", &main_with(&format!("{}1{}", "if true { ".repeat(depth), " }".repeat(depth))));
    both(
        "matches",
        &main_with(&format!("{}1{}", "match 1 { _ => { ".repeat(depth), " } }".repeat(depth))),
    );
    both("closures", &main_with(&format!("let f = {}1;", "|a: Int| ".repeat(depth))));
    both(
        "types",
        &main_with(&format!("let x: {}Int{};", "List<".repeat(depth), ">".repeat(depth))),
    );
    both(
        "patterns",
        &main_with(&format!(
            "match 1 {{ {}x{} => {{ }} }}",
            "(".repeat(depth),
            ", y)".repeat(depth)
        )),
    );
    both("tuples", &main_with(&format!("let t = {}1{};", "(1, ".repeat(depth), ")".repeat(depth))));
    both("unary", &main_with(&format!("let x = {}1;", "!".repeat(500_000))));
    both("refs", &main_with(&format!("let x = {}y;", "&".repeat(500_000))));
}

#[test]
fn very_long_chains_and_lists_do_not_blow_up() {
    let n = 100_000usize;
    both("sum chain", &main_with(&format!("let x = 1{};", " + 1".repeat(n))));
    both("compare chain", &main_with(&format!("let x = 1{};", " == 1".repeat(n))));
    both("method chain", &main_with(&format!("let x = a{};", ".b()".repeat(n))));
    both("long list", &main_with(&format!("let x = [{}1];", "1, ".repeat(n))));
    both("many args", &main_with(&format!("f({}1);", "1, ".repeat(n))));
    both("many statements", &main_with(&"let a = 1;\n".repeat(n)));
    both("many commas", &main_with(&format!("f({});", ",".repeat(n))));
    both("many semicolons", &main_with(&";".repeat(500_000)));
}

#[test]
fn raw_string_delimiters_and_bodies_are_handled_in_linear_time() {
    let q = |n: usize| "\"".repeat(n);
    // a huge opening run, a huge body, and many near-miss closing runs
    both("quotes only", &main_with(&format!("let s = {};", q(900_000))));
    both("long open, no close", &main_with(&format!("let s = {}abc;", q(5_000))));
    both("long raw string", &main_with(&format!("let s = {}\n{}\n{};", q(3), "x".repeat(900_000), q(3))));
    both(
        "near-miss closers",
        &main_with(&format!("let s = {}\n{}\n{};", q(6), format!("{} a ", q(5)).repeat(100_000), q(6))),
    );
    both(
        "many raw strings",
        &main_with(&format!("{}", "let s = \"\"\"a\"\"\";\n".repeat(60_000))),
    );
    both("unterminated", &main_with("let s = \"\"\"\n  abc"));
    both("unterminated plain", &main_with(&format!("let s = \"{}", "a".repeat(900_000))));
    both("unterminated comment", &main_with(&format!("/* {}", "/* ".repeat(200_000))));
    both("nested comments", &main_with(&format!("{}{}", "/* ".repeat(200_000), "*/ ".repeat(200_000))));
}

#[test]
fn malformed_bytes_and_odd_characters_are_rejected_cleanly() {
    both("empty", b"");
    both("nul bytes", b"fn main() -> Int {\0 return 0; }\0\0");
    both("invalid utf-8", b"fn main() -> Int { let s = \"\xff\xfe\"; return 0; }");
    both("lone surrogate bytes", b"fn main() -> Int { let \xed\xa0\x80 = 1; return 0; }");
    both("bom", "\u{feff}fn main() -> Int { return 0; }".as_bytes());
    both(
        "zero width and bidi",
        "fn main() -> Int { let a\u{200b}b = 1; let \u{202e}c = 2; return 0; }".as_bytes(),
    );
    both("only cr", &"\r".repeat(500_000).into_bytes());
    both("crlf soup", &"\r\n\r".repeat(200_000).into_bytes());
    both("long identifier", &main_with(&format!("let {} = 1;", "a".repeat(900_000))));
    both("many unicode letters", &main_with(&format!("let {} = 1;", "界".repeat(300_000))));
    both("emoji soup", &main_with(&"🙂".repeat(200_000)));
    both("all ascii punctuation", &main_with(&"!@#$%^&*()_+-=[]{}|;:'\",.<>/?`~\\".repeat(10_000)));
}

#[test]
fn reserved_words_and_numbers_in_bulk_are_handled() {
    both(
        "reserved names",
        &main_with(&"let commit = 1;\nlet drop = 2;\nlet fn = 3;\n".repeat(30_000)),
    );
    both("huge number", &main_with(&format!("let n = {};", "9".repeat(900_000))));
    both("huge hex", &main_with(&format!("let n = 0x{};", "f".repeat(900_000))));
    both("huge float", &main_with(&format!("let n = 1.{}e{};", "0".repeat(400_000), "9".repeat(400_000))));
    both("number soup", &main_with(&"1.2.3.4.5.6.7 0x0x0x 1e1e1e ".repeat(30_000)));
}

#[test]
fn generic_and_declaration_floods_are_handled() {
    let n = 40_000usize;
    both("type params", format!("fn f<{}>() -> Int {{ return 0; }}\nfn main() -> Int {{ return 0; }}\n", "T,".repeat(n)).as_bytes());
    both("where bounds", format!("fn f<T>() -> Int where T: {} {{ return 0; }}\nfn main() -> Int {{ return 0; }}\n", "Eq + ".repeat(n)).as_bytes());
    both("effects", format!("fn f() -> Int effects {{{}}} {{ return 0; }}\nfn main() -> Int {{ return 0; }}\n", "output,".repeat(n)).as_bytes());
    both("enum variants", format!("enum E {{ {} }}\nfn main() -> Int {{ return 0; }}\n", "A(Int),".repeat(n)).as_bytes());
    both("many functions", format!("{}fn main() -> Int {{ return 0; }}\n", "fn g() -> Int { return 0; }\n".repeat(n)).as_bytes());
    both("many imports", format!("{}fn main() -> Int {{ return 0; }}\n", "import std.text as t;\n".repeat(n)).as_bytes());
    both("struct fields", format!("struct S {{ {} }}\nfn main() -> Int {{ return 0; }}\n", "a: Int,".repeat(n)).as_bytes());
}

/// A function with thousands of variables, branches and closures used to take quadratic (or worse)
/// time to check: 12,000 `let`s took 46 s and 3,000 closures more than two minutes. These shapes
/// must stay roughly linear.
#[test]
fn many_variables_with_branches_blocks_and_closures_are_checked_in_linear_time() {
    let n = 8_000usize;
    let lets = |i: usize| format!("let a{i} = {i};\n");
    let shapes: Vec<(&str, String)> = vec![
        ("lets", (0..n).map(lets).collect()),
        (
            "lets and ifs",
            (0..n).map(|i| format!("let a{i} = {i};\nif a{i} > 0 {{ Out.println(a{i}); }}\n")).collect(),
        ),
        (
            "lets and whiles",
            (0..n).map(|i| format!("let a{i} = {i};\nwhile false {{ Out.println(a{i}); }}\n")).collect(),
        ),
        (
            "lets and fors",
            (0..n).map(|i| format!("let a{i} = {i};\nfor j in 0..1 {{ Out.println(a{i}); }}\n")).collect(),
        ),
        ("lets and blocks", (0..n).map(|i| format!("let a{i} = {{ {i} }};\n")).collect()),
        (
            "lets and matches",
            (0..n).map(|i| format!("let a{i} = match {i} {{ _ => 1, }};\n")).collect(),
        ),
        (
            "closures",
            (0..n).map(|i| format!("let c{i} = |a: Int| a + 1;\n")).collect(),
        ),
        (
            "closures calling closures",
            std::iter::once("let c0 = |a: Int| a + 1;\n".to_string())
                .chain((1..n).map(|i| format!("let c{i} = |a: Int| c{}(a) + 1;\n", i - 1)))
                .collect(),
        ),
        (
            "lets and println",
            (0..n).map(|i| format!("let a{i} = {i};\nOut.println(1);\n")).collect(),
        ),
        (
            "lets and used",
            std::iter::once("let a0 = 0;\n".to_string())
                .chain((1..n).map(|i| format!("let a{i} = a{} + 1;\n", i - 1)))
                .collect(),
        ),
    ];
    for (name, body) in &shapes {
        let source = main_with(body);
        let out = run("check", &source);
        assert!(
            (0..=100).contains(&out.code),
            "{name}: exit code {}\n{}",
            out.code,
            out.stderr.chars().take(300).collect::<String>()
        );
        // A few seconds at the very most; the old behavior was minutes.
        assert!(out.seconds < 12.0, "{name}: took {:.1}s for {} bytes", out.seconds, source.len());
    }
}

/// Unknown names are the one place that searches the scope for a spelling hint; the search must
/// stay bounded however many names are in scope.
#[test]
fn unknown_names_in_a_huge_scope_stay_cheap() {
    let n = 6_000usize;
    let mut body: String = (0..n).map(|i| format!("let variable{i} = {i};\n")).collect();
    body.push_str(&"missing_name_here;\n".repeat(300));
    let out = run("check", &main_with(&body));
    assert!((0..=100).contains(&out.code), "exit code {}", out.code);
    assert!(out.seconds < 12.0, "took {:.1}s", out.seconds);
}
