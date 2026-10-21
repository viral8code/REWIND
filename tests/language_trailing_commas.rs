use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-trailing-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
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
fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n")
}
/// Removes every trailing comma that the sources below write as `,)` `,>` `,|` `,}` `,]`.
fn without_trailing(source: &str) -> String {
    source
        .replace(",)", ")")
        .replace(",>", ">")
        .replace(",|", "|")
        .replace(",}", "}")
        .replace(",]", "]")
}
fn check_and_run(source: &str) -> (Output, Output) {
    let root = project(source);
    (
        call(&root, &["check", "--root", "."]),
        call(&root, &["run", "--root", "."]),
    )
}
fn same_result(source: &str, expected: &str) {
    let plain_source = without_trailing(source);
    assert_ne!(plain_source, source, "the sample has no trailing comma");
    let (check, run) = check_and_run(source);
    assert!(check.status.success(), "check: {}", stderr(&check));
    assert!(run.status.success(), "run: {}", stderr(&run));
    let (plain_check, plain_run) = check_and_run(&plain_source);
    assert!(plain_check.status.success(), "{}", stderr(&plain_check));
    assert_eq!(stdout(&run), stdout(&plain_run));
    assert_eq!(stdout(&run), expected);
}
fn rejected(source: &str) {
    let root = project(source);
    let check = call(&root, &["check", "--root", "."]);
    assert!(!check.status.success(), "accepted: {source}");
    assert!(
        stderr(&check).contains("expected"),
        "unclear message: {}",
        stderr(&check)
    );
}
fn wrap(body: &str) -> String {
    format!("{body}\nfn main()->Int effects {{output}} {{ run(); publish; return 0; }}\n")
}

#[test]
fn function_parameters_and_defaults() {
    same_result(
        &wrap(
            "fn add(a:Int, b:Int,)->Int effects {} { return a+b; }\nfn dflt(a:Int, b:Int=2,)->Int effects {} { return a*b; }\nfn run()->Unit effects {output} { Out.println(add(1,2,).format()); Out.println(dflt(3,).format()); }",
        ),
        "3\n6\n",
    );
}

#[test]
fn call_arguments_positional_and_named() {
    same_result(
        &wrap(
            "fn sub(a:Int, b:Int)->Int effects {} { return a-b; }\nfn run()->Unit effects {output} { Out.println(sub(9,4,).format()); Out.println(sub(a: 9, b: 4,).format()); Out.println(sub(9, b: 4,).format()); }",
        ),
        "5\n5\n5\n",
    );
}

#[test]
fn type_parameters_type_arguments_and_where() {
    same_result(
        &wrap(
            "fn big<T: Ord + Share,>(a:T, b:T,)->T effects {} { if a.cmp(b) >= 0 { return a; } return b; }\nfn big2<T,>(a:T, b:T,)->T effects {} where T: Ord + Share, { if a.cmp(b) >= 0 { return a; } return b; }\nfn run()->Unit effects {output} {\n    let m = Map<String,Int,>();\n    m.set(\"a\", 1,);\n    Out.println(m.len().format());\n    Out.println(big<Int,>(1, 2,).format());\n    Out.println(big2(5, 3,).format());\n    let l: List<Int,> = [1, 2, 3,];\n    Out.println(l.len().format());\n}",
        ),
        "1\n2\n5\n3\n",
    );
}

#[test]
fn function_types_closures_and_short_closures() {
    same_result(
        &wrap(
            "fn apply(f: fn(Int,Int,)->Int effects {}, a:Int)->Int effects {} { return f(a, a,); }\nfn run()->Unit effects {output} {\n    let c = |a:Int, b:Int,| -> Int { return a+b; };\n    Out.println(apply(c, 4,).format());\n    let s: fn(Int,Int,)->Int effects {} = |a, b,| a*b;\n    Out.println(apply(s, 4,).format());\n}",
        ),
        "8\n16\n",
    );
}

#[test]
fn enum_variants_and_trait_methods() {
    same_result(
        &wrap(
            "enum E { A(Int,Int,), B { x: Int, }, }\ntrait Mix { fn mix(self: &Self, n: Int,) -> Int effects {}; }\nrecord R { v: Int, }\nimpl Mix for R { fn mix(self: &R, n: Int,) -> Int effects {} { return self.v+n; } }\nfn run()->Unit effects {output} {\n    let e = E::A(1, 2,);\n    match e { E::A(x, y,) => { Out.println((x+y).format()); }, E::B { x: x, } => { Out.println(x.format()); }, }\n    let r = R(10,);\n    Out.println(r.mix(5,).format());\n}",
        ),
        "3\n15\n",
    );
}

#[test]
fn tuples_and_captures_stay_accepted() {
    same_result(
        &wrap(
            "fn run()->Unit effects {output} { let t = (1, 2,); let (a, b,) = t; Out.println((a+b).format()); let f: fn(Int,)->Int effects {} captures {Send,} = |x:Int| -> Int { return x; }; Out.println(f(1,).format()); }",
        ),
        "3\n1\n",
    );
}

#[test]
fn empty_and_double_commas_are_rejected() {
    rejected(&wrap("fn f(a:Int)->Int effects {} { return a; }\nfn run()->Unit effects {} { f(,); }"));
    rejected(&wrap("fn f(a:Int,b:Int)->Int effects {} { return a; }\nfn run()->Unit effects {} { f(1,,2); }"));
    rejected(&wrap("fn f(,)->Int effects {} { return 1; }\nfn run()->Unit effects {} { }"));
    rejected(&wrap("fn f(a:Int,,b:Int)->Int effects {} { return a; }\nfn run()->Unit effects {} { }"));
    rejected(&wrap("fn f<,>()->Int effects {} { return 1; }\nfn run()->Unit effects {} { }"));
    rejected(&wrap("fn f(g: fn(,)->Int)->Int effects {} { return 1; }\nfn run()->Unit effects {} { }"));
    rejected(&wrap("fn run()->Unit effects {} { let m = Map<,>(); }"));
}

#[test]
fn formatter_keeps_trailing_commas_and_is_idempotent() {
    let source = wrap(
        "fn add(a:Int,\nb:Int,\n)->Int effects {} { return a+b; }\nfn run()->Unit effects {output} {\nOut.println(add(\n1,\n2,\n).format());\nlet m = Map<String,Int,>();\n}",
    );
    let root = project(&source);
    let fmt = call(&root, &["fmt", "main.rw"]);
    assert!(fmt.status.success(), "{}", stderr(&fmt));
    let once = fs::read_to_string(root.join("main.rw")).unwrap();
    assert!(once.contains("b:Int,\n"), "{once}");
    assert!(once.contains("Map<String,Int,>"), "{once}");
    let check = call(&root, &["fmt", "main.rw", "--check"]);
    assert!(check.status.success(), "{}", stderr(&check));
    let fmt2 = call(&root, &["fmt", "main.rw"]);
    assert!(fmt2.status.success());
    assert_eq!(once, fs::read_to_string(root.join("main.rw")).unwrap());
    let run = call(&root, &["run", "--root", "."]);
    assert!(run.status.success(), "{}", stderr(&run));
    assert_eq!(stdout(&run), "3\n");
}
