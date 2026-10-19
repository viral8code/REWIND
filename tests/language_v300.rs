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
fn older_languages_reject_block_expressions() {
    let root = std::env::temp_dir().join(format!("rewind-v300-old-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("main.rw"),
        "fn main()->Int effects {} { let x={ 1 }; return x; }\n",
    )
    .unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"2.0.0\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"\"\n",
    )
    .unwrap();
    assert!(call(&root, &["update"]).status.success());
    let out = call(&root, &["run", "--root", "."]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("require language 3.0.0"),
        "{}",
        stderr(&out)
    );
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
