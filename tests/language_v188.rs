use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v188-{}-{}",
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
fn json_source_free_events_bigint_checkpoint_and_replay() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/json-stream/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&out);
    assert_eq!(
        String::from_utf8(out.stdout.clone()).unwrap(),
        "1267650600228229401496703205376\n2\n3\n日本\n3\n0\n"
    );
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn json_failure_preserves_pending_number_and_cancel_closes() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.jsonStream as json;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let reader=take(json.reader());take(json.feed(&mut reader,take(stdEncode("[1e")),false));
match json.feed(&mut reader,take(stdEncode("]")),true){Err(error)=>{Out.println(error.code);Out.println(error.offset);},Ok(_)=>{panic("bad number accepted");}}
take(json.feed(&mut reader,take(stdEncode("+400]")),true));var reading=true;while reading{match take(json.next(&mut reader)){None=>{reading=false;},Some(event)=>{match event{JsonStreamEvent::Number(raw,position)=>{Out.println(raw);Out.println(position);},_=>{}}}}}
take(json.cancel(&mut reader));match json.feed(&mut reader,take(stdEncode("")),false){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("closed");}}publish;
"#).unwrap();
    let out = call(&root, &["run", "main.rw"]);
    success(&out);
    assert_eq!(out.stdout, b"JsonNumberSyntax\n1\n1e+400\n1\nJsonClosed\n");
    let out = call(&root, &["run", "main.rw", "--native-work", "1"]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("NativeWorkBudgetExceeded"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn json_pending_checkpoint_and_task_transfer() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.jsonStream as json;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let reader=take(json.reader());take(json.feed(&mut reader,take(stdEncode("\"part")),false));commit pending;
take(json.feed(&mut reader,take(stdEncode(" one\"")),true));match take(json.next(&mut reader)){None=>{panic("missing");},Some(event)=>{match event{JsonStreamEvent::Text(text,_)=>{Out.println(text);},_=>{panic("wrong event");}}}}publish;
revert pending;let saved=freeze(move reader);let copy=thaw(saved);
async fn complete(reader:json.Reader)->String effects {} {take(json.feed(&mut reader,take(stdEncode(" two\"")),true));match take(json.next(&mut reader)){None=>{panic("missing");},Some(event)=>{match event{JsonStreamEvent::Text(text,_)=>{return text;},_=>{panic("wrong event");}}}}}
let job=spawn complete(move copy);Out.println(take(await job));drop pending;publish;
"#).unwrap();
    let out = call(&root, &["run", "main.rw", "--record", "trace.json"]);
    success(&out);
    assert_eq!(out.stdout, b"part one\npart two\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
