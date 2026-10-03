use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v19-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn success(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
#[test]
fn complete_nested_result_option_generic_and_tuple_products() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
fn classify<T>(value:Result<Option<T>,String>)->Int effects {}{match move value{Err(_)=>{return -1;},Ok(None)=>{return 0;},Ok(Some(_))=>{return 1;}}}
let value:Result<Option<Int>,String>=Ok(Some(42));Out.println(classify(move value));
let absent:Result<Option<Int>,String>=Ok(None);Out.println(classify(move absent));
enum Choice {Pair(Bool,Bool),Empty}
fn test(value:Choice)->Int effects {} {match value{Choice::Empty=>{return 0;},Choice::Pair(true,_)=>{return 1;},Choice::Pair(false,true)=>{return 2;},Choice::Pair(false,false)=>{return 3;}}}
Out.println(test(Choice::Pair(false,false)));let tuple=(false,Some(true));match tuple{(true,_)=>{Out.println(0);},(false,None)=>{Out.println(1);},(false,Some(true))=>{Out.println(2);},(false,Some(false))=>{Out.println(3);}}publish;
"#).unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&out);
    assert_eq!(out.stdout, b"1\n0\n3\n2\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(out.stdout, replay.stdout);
    success(&call(&root, &["run", "main.rwc"]));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn uncovered_nested_values_guards_and_unreachable_rows_are_rejected() {
    for (body, expected) in [
        ("match value {None=>{},Some(true)=>{}}", "non-exhaustive"),
        (
            "match value {None=>{},Some(_) if true=>{}}",
            "non-exhaustive",
        ),
        (
            "match value {Some(true)=>{},Some(false)=>{},Some(_)=>{},None=>{}}",
            "unreachable",
        ),
        (
            "match value {Some(_)=>{},Some(true)=>{},None=>{}}",
            "unreachable",
        ),
    ] {
        let root = dir();
        fs::write(
            root.join("main.rw"),
            format!("let value:Option<Bool>=None;{body}"),
        )
        .unwrap();
        let out = call(&root, &["compile", "main.rw"]);
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn module_bindings_are_lexical_across_arms_blocks_and_initializers() {
    let root = dir();
    fs::write(root.join("helper.rw"),r#"
pub fn source()->Int effects {} {return 7;}
pub fn choose(value:Option<Int>)->Int effects {} {let before=source();match value{None=>{return source();},Some(source)=>{return source+before;}}}
pub fn expression(value:Option<Int>)->Int effects {} {return source()+match value{None=>source(),Some(source)=>source};}
pub fn local()->Int effects {} {let before=source();{let source=3;assert_eq(source,3);}return before+source();}
pub fn initializer()->Int effects {} {let source=source();return source;}
pub fn loopScope()->Int effects {} {for source in 0..2{assert_eq(source<2,true);}return source();}
pub fn closure()->Int effects {} {let before=source();let callback=|source:Int|->Int{return source+before;};return callback(2)+source();}
"#).unwrap();
    fs::write(root.join("main.rw"),r#"import helper as h;Out.println(h.choose(Some(3)));Out.println(h.choose(None));Out.println(h.expression(Some(3)));Out.println(h.expression(None));Out.println(h.local());Out.println(h.initializer());Out.println(h.loopScope());Out.println(h.closure());publish;"#).unwrap();
    let out = call(&root, &["run", "main.rw"]);
    success(&out);
    assert_eq!(out.stdout, b"10\n7\n10\n14\n14\n7\n7\n16\n");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn opaque_native_patterns_are_rejected_instead_of_runtime_failure() {
    for source in [
        "import std.regex as r;match r.compile(\"a\"){Err(_)=>{},Ok(p)=>{match p {Regex {}=>{}}}}",
        "let p:CsvStreamState=CsvStreamState();",
        "let p:JsonStreamState=JsonStreamState();",
    ] {
        let root = dir();
        fs::write(root.join("main.rw"), source).unwrap();
        let out = call(&root, &["compile", "main.rw"]);
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("opaque"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn record_fields_integer_intervals_and_list_patterns_keep_coverage() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
record Pair {a:Bool,b:Bool}
let p=Pair(false,true);match p{Pair{a:true}=>{Out.println(0);},Pair{a:false,b:true}=>{Out.println(1);},Pair{a:false,b:false}=>{Out.println(2);}}
let n=4;match n{0..3=>{Out.println(0);},3..6=>{Out.println(1);},_=>{Out.println(2);}}
let xs=List<Bool>();xs.push(false);match xs{[true]=>{Out.println(0);},[false]=>{Out.println(1);},_=>{Out.println(2);}}publish;
"#).unwrap();
    let out = call(&root, &["run", "main.rw"]);
    success(&out);
    assert_eq!(out.stdout, b"1\n1\n1\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_ranges_are_unreachable_even_in_nested_products() {
    for source in [
        "let value=1;match value{2..2=>{},_=>{}}",
        "let value:Option<Int>=None;match value{Some(3..2)=>{},_=>{}}",
        "let value=List<Int>();match value{[2..2]=>{},_=>{}}",
    ] {
        let root = dir();
        fs::write(root.join("main.rw"), source).unwrap();
        let out = call(&root, &["compile", "main.rw"]);
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("unreachable"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
