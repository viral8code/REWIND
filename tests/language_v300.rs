use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v300-{}-{}",
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
    let root = project(source);
    call(&root, &["run", "--root", "."])
}
fn run_allowing(source: &str, effects: &str) -> Output {
    let root = project(source);
    fs::write(
        root.join("rewind.toml"),
        format!(
            "language = \"3.0.0\"
source_root = \".\"
entry = \"main.rw\"
effects = \"{effects}\"
"
        ),
    )
    .unwrap();
    assert!(call(&root, &["update"]).status.success());
    call(&root, &["run", "--root", "."])
}
fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into()
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into()
}

#[test]
fn main_is_the_entry_point() {
    let out =
        run("fn main()->Int effects {output} { Out.println(\"hello\"); publish; return 0; }\n");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "hello\n");
}

#[test]
fn top_level_statements_are_rejected() {
    let out = run("fn main()->Int effects {} { return 0; }\nlet x=1;\n");
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("top-level statements are not allowed"),
        "{}",
        stderr(&out)
    );
}

fn main_output(body: &str) -> Output {
    run(&format!(
        "{body}\nfn main()->Int effects {{output}} {{ run(); publish; return 0; }}\n"
    ))
}

#[test]
fn match_arms_may_be_blocks_with_early_return_and_a_tail_value() {
    let out = main_output(
        r#"fn parse(n:Int)->Result<Int,String> effects {} {
    if n<0 { return Err("negative"); }
    return Ok(n*2);
}
fn describe(n:Int)->Result<String,String> effects {} {
    let doubled=match parse(n) {
        Ok(v)=>v,
        Err(e)=>{ return Err(e); }
    };
    let label=if doubled>10 { "big" } else { "small" };
    let extra={
        let base=doubled+1;
        base*2
    };
    return Ok(label+":"+extra.format());
}
fn run()->Unit effects {output} {
    match describe(3) { Ok(s)=>{ Out.println(s); }, Err(e)=>{ Out.println("err "+e); } }
    match describe(-1) { Ok(s)=>{ Out.println(s); }, Err(e)=>{ Out.println("err "+e); } }
    match describe(9) { Ok(s)=>{ Out.println(s); }, Err(e)=>{ Out.println("err "+e); } }
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "small:14\nerr negative\nbig:38\n");
}

#[test]
fn if_without_else_is_unit_and_else_if_chains_work() {
    let out = main_output(
        r#"fn grade(n:Int)->String effects {} {
    return if n>=90 { "A" } else if n>=80 { "B" } else { "C" };
}
fn run()->Unit effects {output} {
    var count=0;
    let ignored=if count==0 { count+=1; };
    Out.println(grade(95)+grade(85)+grade(10)+count.format());
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ABC1\n");
}

#[test]
fn block_arm_types_must_agree() {
    let out = main_output(
        r#"fn run()->Unit effects {} {
    let x=if true { 1 } else { "no" };
}"#,
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("match arm expects"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn block_expressions_respect_ownership() {
    let out = main_output(
        r#"fn run()->Unit effects {} {
    let items=List<Int>();
    let moved={ let inner=move items; inner };
    items.add(1);
}"#,
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("use after move"), "{}", stderr(&out));
}

#[test]
fn while_true_without_break_needs_no_trailing_return() {
    let out = main_output(
        r#"fn firstOver(limit:Int)->Int effects {} {
    var n=0;
    while true {
        n+=3;
        if n>limit { return n; }
    }
}
fn run()->Unit effects {output} { Out.println(firstOver(10).format()); }"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "12\n");
    let out = main_output(
        r#"fn broken(limit:Int)->Int effects {} {
    var n=0;
    while true {
        n+=3;
        if n>limit { break; }
    }
}
fn run()->Unit effects {} { }"#,
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("return on every path"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn running_without_main_is_an_error() {
    let out = run("fn helper()->Int effects {} { return 1; }\n");
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("start at fn main"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn let_else_binds_for_the_rest_of_the_block_and_must_exit() {
    let out = main_output(
        r#"fn first(items:&Map<String,Int>)->Result<Int,String> effects {} {
    let Some(head)=items.get("a") else { return Err("empty"); };
    let doubled=head*2;
    return Ok(doubled);
}
fn run()->Unit effects {output} {
    let full=Map<String,Int>();full.set("a",21);
    let none=Map<String,Int>();
    match first(&full) { Ok(v)=>{ Out.println(v.format()); }, Err(e)=>{ Out.println(e); } }
    match first(&none) { Ok(v)=>{ Out.println(v.format()); }, Err(e)=>{ Out.println(e); } }
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "42\nempty\n");
    let out = main_output(
        r#"fn run()->Unit effects {} {
    let Some(x)=Some(1) else { };
}"#,
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("must leave the scope"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn if_let_works_as_statement_and_expression() {
    let out = main_output(
        r#"fn label(value:Option<Int>)->String effects {} {
    return if let Some(n)=value { "has "+n.format() } else { "none" };
}
fn run()->Unit effects {output} {
    if let Some(n)=Some(5) { Out.println(n.format()); } else { Out.println("no"); }
    let nothing:Option<Int>=None;
    if let Some(n)=nothing { Out.println(n.format()); }
    Out.println(label(Some(7)));
    Out.println(label(None));
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "5\nhas 7\nnone\n");
}

#[test]
fn collection_literals_build_typed_lists_and_maps() {
    let out = main_output(
        r#"fn total(values:&List<Int>)->Int effects {} {
    var sum=0;
    for i in 0..values.len() { sum+=values.get(i); }
    return sum;
}
fn run()->Unit effects {output} {
    let numbers=[1, 2, 3,];
    let nested=[[1], [2, 3]];
    let ages={"ada": 36, "grace": 45};
    Out.println(total(&numbers).format());
    Out.println(nested.len().format());
    Out.println(nested.get(1).len().format());
    match ages.get("grace") { Some(age)=>{ Out.println(age.format()); }, None=>{ Out.println("none"); } }
    let words=["a", "b"];
    Out.println(words.get(1));
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "6\n2\n2\n45\nb\n");
}

#[test]
fn collection_literals_reject_mixed_and_untyped_forms() {
    let out = main_output("fn run()->Unit effects {} { let mixed=[1, \"two\"]; }");
    assert!(!out.status.success());
    let out = main_output("fn run()->Unit effects {} { let empty=[]; }");
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("empty list literal"),
        "{}",
        stderr(&out)
    );
    let out = main_output("fn run()->Unit effects {} { let m={\"a\": 1, \"b\": \"x\"}; }");
    assert!(!out.status.success());
}

#[test]
fn tuple_elements_are_read_with_dot_index() {
    let out = main_output(
        r#"fn pair()->(Int,(String,Int)) effects {} { return (1,("a",2)); }
fn run()->Unit effects {output} {
    let t=pair();
    Out.println(t.0.format());
    Out.println(t.1.0);
    Out.println(t.1.1.format());
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "1\na\n2\n");
}

#[test]
fn index_syntax_reads_and_writes_lists_and_maps() {
    let out = main_output(
        r#"fn run()->Unit effects {output} {
    let xs=[10, 20, 30];
    xs[1]=25;
    xs[2]+=5;
    Out.println((xs[0]+xs[1]+xs[2]).format());
    let grid=[[1, 2], [3, 4]];
    Out.println(grid[1][0].format());
    let ages={"ada": 36};
    ages["ada"]=37;
    match ages["ada"] { Some(a)=>{ Out.println(a.format()); }, None=>{ Out.println("none"); } }
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "70\n3\n37\n");
}

#[test]
fn string_interpolation_formats_embedded_expressions() {
    let out = main_output(
        r#"fn label(name:String,count:Int)->String effects {} {
    return "${name}: ${count + 1} item(s), done=${count > 0}";
}
fn run()->Unit effects {output} {
    Out.println(label("tasks", 2));
    Out.println("literal $${not interpolated} and ${1 + 1}");
    Out.println("plain text");
    let ratio=2.5;
    Out.println("r=${ratio}");
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "tasks: 3 item(s), done=true\nliteral ${not interpolated} and 2\nplain text\nr=2.5\n"
    );
}

#[test]
fn string_interpolation_reports_malformed_forms() {
    let out = main_output("fn run()->Unit effects {} { let s=\"a ${1 + \"; }");
    assert!(!out.status.success());
    let out = main_output("fn run()->Unit effects {} { let s=\"a ${}\"; }");
    assert!(!out.status.success());
    assert!(stderr(&out).contains("empty ${}"), "{}", stderr(&out));
}

#[test]
fn expression_bodied_functions_return_their_expression() {
    let out = main_output(
        r#"fn double(x:Int)->Int = x*2;
fn greet(name:String)->String = "hi ${name}";
fn run()->Unit effects {output} {
    Out.println(double(21).format());
    Out.println(greet("ada"));
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "42\nhi ada\n");
    let out = main_output("fn bad(x:Int) = x;\nfn run()->Unit effects {} { }");
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("needs a return type"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn output_is_published_when_main_returns_without_publish() {
    let out = run("fn main()->Int effects {output} { Out.println(\"implicit\"); return 0; }\n");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "implicit\n");
}

#[test]
fn panic_in_main_does_not_publish_pending_output() {
    let out = run(
        "fn main()->Int effects {output} { Out.println(\"lost\"); panic(\"boom\"); return 0; }\n",
    );
    assert!(!out.status.success());
    assert_eq!(stdout(&out), "");
}

#[test]
fn explicit_publish_still_publishes_early() {
    let out = run("fn main()->Int effects {output} { Out.println(\"one\"); publish; Out.println(\"two\"); return 0; }\n");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "one\ntwo\n");
}

#[test]
fn const_is_a_declaration_and_usable_from_functions() {
    let out = run(
        "const LIMIT: Int = 6 * 7;\nconst NAME: String = \"n\";\nfn twice()->Int effects {} { return LIMIT * 2; }\nfn main()->Int effects {output} { Out.println(LIMIT); Out.println(twice()); Out.println(NAME); return 0; }\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "42\n84\nn\n");
    // A plain `let` at the top level is still rejected.
    let out = run("let x = 1;\nfn main()->Int effects {} { return 0; }\n");
    assert!(stderr(&out).contains("top-level"), "{}", stderr(&out));
}

#[test]
fn map_keys_can_be_iterated_with_for() {
    let out = run(
        "fn main()->Int effects {output} {\n    let typed = Map<String,Int>();\n    typed.set(\"b\", 2);\n    typed.set(\"a\", 1);\n    for k in typed.keys() { Out.println(k); }\n    let literal = {\"x\": 1, \"y\": 2};\n    for k in literal.keys() { Out.println(k); }\n    let ids = Map<Int,String>();\n    ids.set(7, \"seven\");\n    for k in ids.keys() { Out.println(k + 1); }\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "a\nb\nx\ny\n8\n");
}

#[test]
fn iterators_over_shared_borrows_can_be_consumed() {
    let out = run(
        "fn total(xs: &List<Int>) -> Int effects {} {\n    var s = 0;\n    for x in xs { s = s + x; }\n    for x in xs.iter() { s = s + x; }\n    let it = xs.iter();\n    let first = it.next();\n    let doubled = xs.iter().map(|x: Int| -> Int { return x * 2; }).collect();\n    return s + doubled.len();\n}\nfn names(m: &Map<String,Int>) -> Int effects {} {\n    var n = 0;\n    for k in m.keys() { n = n + 1; }\n    return n;\n}\nfn main()->Int effects {output} {\n    let xs = [1, 2, 3];\n    Out.println(total(&xs));\n    let m = Map<String,Int>();\n    m.set(\"a\", 5);\n    m.set(\"b\", 6);\n    Out.println(names(&m));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "15\n2\n");
}

#[test]
fn shared_borrows_still_reject_mutating_the_owner() {
    for body in [
        "xs.add(4);",
        "for x in xs.iter() { xs.add(x); }",
        "let it = xs.iter(); xs.push(1);",
    ] {
        let out = run(&format!(
            "fn f(xs: &List<Int>) -> Int effects {{}} {{\n    {body}\n    return 0;\n}}\nfn main()->Int effects {{output}} {{ let xs = [1]; Out.println(f(xs)); return 0; }}\n"
        ));
        assert!(!out.status.success(), "{body} was accepted");
        assert!(
            stderr(&out).contains("cannot mutate"),
            "{body}: {}",
            stderr(&out)
        );
    }
}

#[test]
fn question_mark_error_type_mismatch_is_rejected_before_running() {
    let out = run(
        "fn g() -> Result<Int, Int> effects {} {\n    let x = \"1\".parseInt()?;\n    return Ok(x);\n}\nfn main()->Int effects {output} { Out.println(g()); return 0; }\n",
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("does not match"), "{}", stderr(&out));
    assert!(stderr(&out).contains("mapErr"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty());
}

#[test]
fn question_mark_needs_a_result_returning_function() {
    let out = run(
        "fn h() -> Int effects {} {\n    let x = \"1\".parseInt()?;\n    return x;\n}\nfn main()->Int effects {output} { Out.println(h()); return 0; }\n",
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("returns Result"), "{}", stderr(&out));
}

#[test]
fn question_mark_with_matching_or_converted_error_types_still_runs() {
    let out = run(
        "fn f() -> Result<Int, String> effects {} {\n    let x = \"1\".parseInt()?;\n    return Ok(x);\n}\nfn g() -> Result<Int, Int> effects {} {\n    let x = \"2\".parseInt().mapErr(|e: String| -> Int { return e.byteLen(); })?;\n    return Ok(x);\n}\nfn main()->Int effects {output} { Out.println(f()); Out.println(g()); return 0; }\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "Ok(1)\nOk(2)\n");
}

#[test]
fn unknown_generic_trait_bounds_are_rejected() {
    let out = run("fn f<T: Bogus>(a: T) -> T effects {} { return a; }\nfn main()->Int effects {} { return 0; }\n");
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("unknown trait bound Bogus"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn builtin_and_user_trait_bounds_are_accepted() {
    let out = run(
        "trait Named { fn name(self: &Self) -> String effects {}; }\nrecord A { n: String }\nimpl Named for A { fn name(self: &A) -> String effects {} { return self.n; } }\nfn pick<T: Named + Share>(a: &T) -> String effects {} { return a.name(); }\nfn big<T: Ord + Share>(a: T, b: T) -> T effects {} { if a.cmp(b) >= 0 { return a; } return b; }\nfn main()->Int effects {output} { let a = A(\"x\"); Out.println(pick(&a)); Out.println(big(1, 2)); return 0; }\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "x\n2\n");
}

#[test]
fn short_closures_take_parameter_types_from_the_call() {
    let out = run(
        "fn apply(f: fn(Int) -> Int effects {}, x: Int) -> Int effects {} { return f(x); }\nfn parse(text: String) -> Result<Int, String> effects {} { return text.parseInt(); }\nfn main()->Int effects {output} {\n    let xs = [1, 2, 3, 4];\n    let evens = xs.iter().filter(|n| n % 2 == 0).map(|n| n * 10).collect();\n    Out.println(evens.len());\n    Out.println(evens.get(1) + 1);\n    Out.println(xs.iter().fold(0, |a, b| a + b));\n    Out.println(apply(|n| n + 1, 4));\n    Out.println(parse(\"5\").map(|n| n * 2));\n    Out.println(parse(\"x\").mapErr(|e| e.byteLen()));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "2\n41\n10\n5\nOk(10)\nErr(20)\n");
}

#[test]
fn short_closure_without_context_asks_for_annotations() {
    let out = run(
        "fn main()->Int effects {output} {\n    let add = |a, b| a + b;\n    Out.println(add(1, 2));\n    return 0;\n}\n",
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("cannot infer the type of closure parameter a"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn short_closure_in_annotated_let_is_typed_from_the_annotation() {
    let out = run(
        "fn main()->Int effects {output} {\n    let add: fn(Int, Int) -> Int effects {} = |a, b| a + b;\n    let k = 2;\n    let scale: fn(Int) -> Int effects {} = |n| n * k;\n    Out.println(add(1, 2));\n    Out.println(scale(5));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "3\n10\n");
}

#[test]
fn short_closure_in_annotated_let_compiles_like_it_runs() {
    let root = project(
        "fn main()->Int effects {output} {\n    let add: fn(Int, Int) -> Int effects {} = |a, b| a + b;\n    Out.println(add(1, 2));\n    return 0;\n}\n",
    );
    let ran = call(&root, &["run", "--root", "."]);
    assert!(ran.status.success(), "{}", stderr(&ran));
    let compiled = call(&root, &["compile", "main.rw"]);
    assert!(compiled.status.success(), "{}", stderr(&compiled));
    let out = call(&root, &["run", "main.rwc"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), stdout(&ran));
    assert_eq!(stdout(&out), "3\n");
}

fn assert_rejected_before_running(body: &str, needle: &str) {
    let out = run(&format!(
        "fn main()->Int effects {{output}} {{\n    Out.println(\"ran\");\n{body}\n    return 0;\n}}\n"
    ));
    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(!stdout(&out).contains("ran"), "{}", stdout(&out));
    assert!(stderr(&out).contains(needle), "{}", stderr(&out));
}

#[test]
fn short_closure_in_let_rejects_bad_annotations_before_running() {
    assert_rejected_before_running(
        "    let f: fn(Int, Int) -> Int effects {} = |a| a;",
        "closure takes 1 parameters but the annotation fn(Int,Int)->Int!{} takes 2",
    );
    assert_rejected_before_running(
        "    let f: fn(Int, Int) -> Int effects {} = |a, b| a.nosuch();",
        "nosuch",
    );
    assert_rejected_before_running(
        "    let f: fn(Int) -> Int effects {} = |a| { Out.println(a); a };",
        "type mismatch",
    );
    assert_rejected_before_running("    let x: Int = |a| a;", "type mismatch");
    assert_rejected_before_running(
        "    let f = |a, b| a + b;",
        "cannot infer the type of closure parameter a",
    );
}

#[test]
fn short_closure_in_return_and_assign_is_typed_from_the_expected_type() {
    let out = run(
        "fn mk(k: Int) -> fn(Int) -> Int effects {} { return |n| n + k; }\nfn main()->Int effects {output} {\n    var f: fn(Int) -> Int effects {} = mk(1);\n    f = |n| n * 3;\n    Out.println(f(4));\n    Out.println(mk(2)(5));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "12\n7\n");
}

#[test]
fn short_closure_in_return_and_assign_rejects_bad_bodies() {
    let prelude =
        "fn mk() -> fn(Int) -> Int effects {} { return |n| { Out.println(n); n.nosuch() }; }\n";
    let out = run(&format!(
        "{prelude}fn main()->Int effects {{output}} {{ Out.println(\"ran\"); return 0; }}\n"
    ));
    assert!(!out.status.success());
    assert!(!stdout(&out).contains("ran"), "{}", stdout(&out));
    assert!(stderr(&out).contains("nosuch"), "{}", stderr(&out));
    assert_rejected_before_running(
        "    var f: fn(Int) -> Int effects {} = |n| n;\n    f = |n| Out.println(n);",
        "type mismatch",
    );
    let out = run("fn mk() -> fn(Int) -> Int effects {} { return |n| Out.println(n); }\nfn main()->Int effects {output} { return 0; }\n");
    assert!(!out.status.success());
    assert!(stderr(&out).contains("mismatch"), "{}", stderr(&out));
}

#[test]
fn short_closure_argument_to_local_function_value_is_typed_or_rejected() {
    let out = run(
        "fn run2(f: fn(Int) -> Int effects {}, x: Int) -> Int effects {} { return f(x); }\nfn main()->Int effects {output} {\n    let h: fn(fn(Int) -> Int effects {}, Int) -> Int effects {} = run2;\n    Out.println(h(|n| n + 1, 1));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "2\n");
    assert_rejected_before_running(
        "    let h: fn(fn(Int) -> Int effects {}, Int) -> Int effects {} = |g, x| g(x);\n    Out.println(h(|n| n.nosuch(), 1));",
        "nosuch",
    );
}

#[test]
fn short_closure_for_generic_function_is_typed_from_other_arguments() {
    let out = run(
        "fn ap<T>(f: fn(T) -> T effects {}, x: T) -> T effects {} { return f(x); }\nfn main()->Int effects {output} {\n    Out.println(ap(|n| n + 1, 4));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "5\n");
    let out = run(
        "fn ap<T>(f: fn(T) -> T effects {}) -> Int effects {} { return 0; }\nfn main()->Int effects {output} {\n    Out.println(\"ran\");\n    Out.println(ap(|n| n));\n    return 0;\n}\n",
    );
    assert!(!out.status.success());
    assert!(!stdout(&out).contains("ran"), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("closure parameter n"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn short_closure_effect_violation_is_rejected_before_running() {
    assert_rejected_before_running(
        "    let f: fn(Int) -> Int effects {} = |a| Out.println(a);",
        "type mismatch",
    );
}

#[test]
fn print_writes_without_a_newline() {
    let out = run("fn main()->Int effects {output} {\n    Out.print(\"a\");\n    Out.print(1);\n    Out.println(\"b\");\n    return 0;\n}\n");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "a1b\n");
}

#[test]
fn question_mark_works_on_options_in_option_functions() {
    let out = run(
        "fn firstEven(xs: &List<Int>) -> Option<Int> effects {} {\n    for i in 0..xs.len() {\n        if xs.get(i) % 2 == 0 { return Some(xs.get(i)); }\n    }\n    return None;\n}\nfn doubled(xs: &List<Int>) -> Option<Int> effects {} {\n    let n = firstEven(xs)?;\n    return Some(n * 2);\n}\nfn main()->Int effects {output} {\n    let a = [1, 3, 4];\n    let b = [1, 3];\n    Out.println(doubled(&a));\n    Out.println(doubled(&b));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "Some(8)\nNone\n");
    let bad = run("fn f(x: Option<Int>) -> Result<Int, String> effects {} {\n    let v = x?;\n    return Ok(v);\n}\nfn main()->Int effects {} { return 0; }\n");
    assert!(!bad.status.success());
    assert!(stderr(&bad).contains("returns Option"), "{}", stderr(&bad));
}

#[test]
fn println_writes_records_enums_tuples_and_maps_readably() {
    let out = run(
        "record Point { x: Int, y: Int }\nstruct Counter { n: Int, name: String }\nenum Shape { Circle(Int), Empty, Move{x: Int, y: Int} }\nrecord Line { a: Point, b: Point }\nfn main()->Int effects {output} {\n    Out.println(Point(3, 4));\n    Out.println(Counter(1, \"c\"));\n    Out.println(Shape::Circle(2));\n    Out.println(Shape::Empty);\n    Out.println(Shape::Move{x: 1, y: 2});\n    Out.println(Line(Point(1, 2), Point(3, 4)));\n    Out.println(Some(Point(1, 2)));\n    Out.println((1, \"a\"));\n    Out.println([Point(1, 2)]);\n    Out.println({\"a\": 1, \"b\": 2});\n    Out.println(freeze([1, 2]));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "Point{x: 3, y: 4}\nCounter{n: 1, name: c}\nShape::Circle(2)\nShape::Empty\nShape::Move{x: 1, y: 2}\nLine{a: Point{x: 1, y: 2}, b: Point{x: 3, y: 4}}\nSome(Point{x: 1, y: 2})\n(1, a)\n[Point{x: 1, y: 2}]\n{a: 1, b: 2}\n[1, 2]\n"
    );
}

#[test]
fn everyday_list_map_and_string_methods_work() {
    let out = run(
        "fn main()->Int effects {output} {\n    let xs = [3, 1, 2];\n    Out.println(xs.isEmpty());\n    Out.println(xs.contains(2));\n    Out.println(xs.indexOf(1));\n    Out.println(xs.first());\n    Out.println(xs.last());\n    xs.sort();\n    Out.println(xs);\n    xs.reverse();\n    xs.insert(1, 9);\n    Out.println(xs);\n    Out.println(xs.removeAt(0));\n    xs.clear();\n    Out.println(xs.isEmpty());\n    Out.println([\"a\", \"b\"].join(\"-\"));\n    let m = {\"a\": 1, \"b\": 2};\n    Out.println(m.contains(\"a\"));\n    Out.println(m.getOr(\"z\", 7));\n    Out.println(m.values());\n    Out.println(\"  hi  \".trim());\n    Out.println(\"banana\".replace(\"an\", \"AN\"));\n    Out.println(\"ab\".repeat(3));\n    Out.println(\"a\nb\".lines());\n    Out.println(freeze([5, 6]).contains(6));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "false\ntrue\nSome(1)\nSome(3)\nSome(2)\n[1, 2, 3]\n[3, 9, 2, 1]\n3\ntrue\na-b\ntrue\n7\n[1, 2]\nhi\nbANANa\nababab\n[a, b]\ntrue\n"
    );
}

#[test]
fn mutating_list_methods_respect_borrows_and_frozen_values() {
    for body in [
        "xs.insert(0, 1);",
        "xs.clear();",
        "xs.reverse();",
        "xs.sort();",
    ] {
        let out = run(&format!(
            "fn f(xs: &List<Int>) -> Unit effects {{}} {{\n    {body}\n}}\nfn main()->Int effects {{}} {{ return 0; }}\n"
        ));
        assert!(!out.status.success(), "{body} was accepted");
        assert!(
            stderr(&out).contains("cannot mutate"),
            "{body}: {}",
            stderr(&out)
        );
    }
    let frozen = run("fn main()->Int effects {} {\n    let f = freeze([2, 1]);\n    f.sort();\n    return 0;\n}\n");
    assert!(!frozen.status.success());
    assert!(
        stderr(&frozen).contains("cannot mutate Frozen"),
        "{}",
        stderr(&frozen)
    );
    let range = run(
        "fn main()->Int effects {} {\n    let xs = [1];\n    xs.removeAt(3);\n    return 0;\n}\n",
    );
    assert!(!range.status.success());
    assert!(
        stderr(&range).contains("out of bounds"),
        "{}",
        stderr(&range)
    );
}

#[test]
fn while_let_and_tuple_patterns_in_for_loops_work() {
    let out = run(
        "fn main()->Int effects {output} {\n    let xs = [3, 1, 2];\n    while let Some(x) = xs.pop() {\n        Out.println(x);\n    }\n    let names = [\"a\", \"b\"];\n    for (i, name) in names.iter().enumerate() {\n        Out.println(\"${i}: ${name}\");\n    }\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "2\n1\n3\n0: a\n1: b\n");
}

#[test]
fn option_result_number_and_slice_methods_work() {
    let out = run(
        "fn main()->Int effects {output} {\n    let o: Option<Int> = Some(2);\n    let none: Option<Int> = None;\n    let r: Result<Int, String> = Ok(1);\n    let bad: Result<Int, String> = Err(\"x\");\n    Out.println(o.isSome());\n    Out.println(none.isNone());\n    Out.println(none.valueOr(7));\n    Out.println(r.isOk());\n    Out.println(bad.isErr());\n    Out.println(bad.valueOr(9));\n    let n = -5;\n    Out.println(n.abs());\n    Out.println(n.max(3));\n    Out.println(n.min(3));\n    Out.println(2.pow(10));\n    let f = 2.5;\n    Out.println(f.floor());\n    Out.println(f.ceil());\n    Out.println(16.0.sqrt());\n    let xs = [1, 2, 3, 4];\n    Out.println(xs.slice(1, 3));\n    let ys = [9];\n    ys.extend(freeze(xs));\n    Out.println(ys);\n    Out.println(\"Straße\".toUpper());\n    Out.println(\"ABC\".toLower());\n    Out.println(\"hello\".slice(1, 3));\n    return 0;\n}\n",
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "true\ntrue\n7\ntrue\ntrue\n9\n5\n3\n-5\n1024\n2\n3\n4\n[2, 3]\n[9, 1, 2, 3, 4]\nSTRASSE\nabc\nOk(el)\n"
    );
}

#[test]
fn integer_helpers_report_overflow() {
    let out = run("fn main()->Int effects {} {\n    let n = 2.pow(63);\n    return 0;\n}\n");
    assert!(!out.status.success());
    assert!(stderr(&out).contains("IntegerOverflow"), "{}", stderr(&out));
}

#[test]
fn random_below_stays_in_range_and_rejects_bad_limits() {
    let source = "fn main()->Int effects {random, output} {
    var ok = true;
    for i in 0..50 {
        let n = Random.below(6);
        if n < 0 || n > 5 { ok = false; }
    }
    Out.println(ok);
    return 0;
}
";
    let out = run_allowing(source, "random,output");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "true\n");
    let bad = run_allowing(
        "fn main()->Int effects {random} {
    let n = Random.below(0);
    return 0;
}
",
        "random",
    );
    assert!(!bad.status.success());
}

#[test]
fn unknown_trait_bounds_on_types_are_rejected() {
    for source in [
        "struct S<T: Bogus> { v: List<T> }\nfn main()->Int effects {} { return 0; }\n",
        "enum E<T: Bogus> { A(T) }\nfn main()->Int effects {} { return 0; }\n",
        "type L<T: Bogus> = List<T>;\nfn main()->Int effects {} { return 0; }\n",
    ] {
        let out = run(source);
        assert!(!out.status.success(), "{source} was accepted");
        assert!(
            stderr(&out).contains("unknown trait bound Bogus"),
            "{}",
            stderr(&out)
        );
    }
    let fine = run("struct S<T: Ord> { v: List<T> }\nfn main()->Int effects {} { return 0; }\n");
    assert!(fine.status.success(), "{}", stderr(&fine));
}

#[test]
fn send_bound_accepts_borrowed_call_argument() {
    let out = main_output(
        r#"fn id<T: Send>(x: T) -> T effects {} { return move x; }
fn total(xs: &List<Int>) -> Int effects {} {
    var sum=0;
    for i in 0..xs.len() { sum+=xs.get(i); }
    return sum;
}
fn run()->Unit effects {output} {
    let xs=[1, 2, 3];
    Out.println(id(total(&xs)));
}"#,
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "6\n");
}
