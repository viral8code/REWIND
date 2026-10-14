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
    let out = run("fn main()->Int effects {output} { Out.println(\"hello\"); publish; return 0; }\n");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "hello\n");
}

#[test]
fn top_level_statements_are_rejected() {
    let out = run("fn main()->Int effects {} { return 0; }\nlet x=1;\n");
    assert!(!out.status.success());
    assert!(stderr(&out).contains("top-level statements are not allowed"), "{}", stderr(&out));
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
    assert!(stderr(&out).contains("match arm expects"), "{}", stderr(&out));
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
    assert!(stderr(&out).contains("require language 3.0.0"), "{}", stderr(&out));
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
    assert!(stderr(&out).contains("return on every path"), "{}", stderr(&out));
}

#[test]
fn running_without_main_is_an_error() {
    let out = run("fn helper()->Int effects {} { return 1; }\n");
    assert!(!out.status.success());
    assert!(stderr(&out).contains("start at fn main"), "{}", stderr(&out));
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
    assert!(stderr(&out).contains("must leave the scope"), "{}", stderr(&out));
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
