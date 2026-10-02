use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v186-{}-{}",
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
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn regex_source_free_replay_checkpoint_and_map() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/regex/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(
        output.stdout,
        b"Alice-42\nAlice\nBob-7\nfalse\ntrue\n3\ntrue\n"
    );
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(output.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn regex_unicode_bytes_optional_captures_and_empty_progress() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.regex as regex;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let pattern=take(regex.compile("(?P<word>\\p{L}+)(-([0-9]+))?"));let input="!日本語";
match take(regex.find(&pattern,&input)){None=>{panic("missing");},Some(found)=>{let span=regex.span(&found);Out.println(span.start);Out.println(span.end);Out.println(take(regex.text(&input,span)));match take(regex.group(&found,2)){None=>{Out.println("unmatched");},Some(_)=>{panic("optional group");}}}}
let names=take(regex.groupNames(&pattern));match names.get(1){None=>{panic("name");},Some(name)=>{Out.println(name);}}
let raw=List<Int>();raw.push(0);raw.push(255);raw.push(255);let bytes=take(stdBytesFromList(&raw));let bytePattern=take(regex.compileBytes("\\xFF+"));
match take(regex.findBytes(&bytePattern,bytes)){None=>{panic("byte match");},Some(found)=>{let span=regex.span(&found);let chunk=take(regex.bytes(bytes,span));Out.println(take(stdBytesGet(chunk,0)));}}
match regex.find(&bytePattern,&input){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("wrong kind");}}
let empty=take(regex.compile(""));let scalar="é";var cursor=0;var count=0;var finished=false;while !finished{match take(regex.findFrom(&empty,&scalar,cursor)){None=>{finished=true;},Some(found)=>{count+=1;match regex.next(&found){None=>{finished=true;},Some(next)=>{cursor=next;}}}}}Out.println(count);publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "1\n10\n日本語\nunmatched\nword\n255\nRegexType\n2\n"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn regex_freeze_task_transfer_and_scalar_identity() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.regex as regex;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let pattern=take(regex.compileOptions("abc",regex.Options(true,false,false)));let saved=freeze(pattern);let owned=thaw(saved);
async fn check(pattern:Regex,text:String)->Bool effects {} {return take(regex.isMatch(&pattern,&text));}let job=spawn check(move owned,"ABC");Out.println(take(await job));
let second=take(regex.compileOptions("abc",regex.Options(true,false,false)));Out.println(pattern==second);Out.println(pattern.hash()==second.hash());let encoded=take(regex.toJson(&pattern));let decoded=take(regex.fromJson(move encoded));Out.println(pattern==decoded);let input="abc";match regex.text(&input,regex.Span(1,9)){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("range");}}publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(output.stdout, b"true\ntrue\ntrue\ntrue\nRegexIndex\n");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn regex_constructor_is_reserved_and_budget_is_checked_first() {
    for source in [
        "let forged=Regex();",
        "record Regex {}",
        "fn stdRegexCompile(value:String)->Int effects {} {return 0;}",
    ] {
        let root = dir();
        fs::write(root.join("main.rw"), source).unwrap();
        assert!(!call(&root, &["compile", "main.rw"]).status.success());
        fs::remove_dir_all(root).unwrap();
    }
    let root = dir();
    fs::write(root.join("main.rw"),r#"import std.regex as regex;regex.compile("a{1000000}");Out.println("unreachable");publish;"#).unwrap();
    let output = call(&root, &["run", "main.rw", "--native-work", "1"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("NativeWorkBudgetExceeded"));
    fs::remove_dir_all(root).unwrap();
}
