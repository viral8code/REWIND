use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v187-{}-{}",
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
fn csv_chunks_checkpoint_and_source_free_replay() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/csv-stream/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&out);
    assert_eq!(
        String::from_utf8(out.stdout.clone()).unwrap(),
        "name\nnote\n日本語\na\"b\n日\nname\n"
    );
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn csv_file_chunks_drain_without_retaining_whole_file() {
    let root = dir();
    fs::write(root.join("data.csv"), b"1,2\r\n3,4\n").unwrap();
    fs::write(root.join("main.rw"),r#"
import std.csvStream as csv;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let reader=take(csv.reader());let file=File.openSnapshot("data.csv");var total=0;var done=false;
while !done {let chunk=file.readBytes(3);if stdBytesLength(chunk)==0{take(csv.finish(&mut reader));done=true;}else{take(csv.feed(&mut reader,chunk,false));}var draining=true;while draining{match take(csv.next(&mut reader)){None=>{draining=false;},Some(row)=>{total+=take(stdParseInt(row.get(0),10));}}}}
file.close();Out.println(total);Out.println(take(csv.position(&reader)));publish;
"#).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rw",
            "--allow-effects",
            "fileRead,output",
            "--record",
            "trace.json",
        ],
    );
    success(&out);
    assert_eq!(out.stdout, b"4\n9\n");
    fs::remove_file(root.join("data.csv")).unwrap();
    let replay = call(
        &root,
        &["replay", "trace.json", "--allow-effects", "fileRead,output"],
    );
    success(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn csv_errors_preserve_reader_and_native_work_precedes_parsing() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.csvStream as csv;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let reader=take(csv.reader());take(csv.feed(&mut reader,take(stdEncode("ok,")),false));
match csv.feed(&mut reader,take(stdEncode("bad\"")),true){Err(error)=>{Out.println(error.code);Out.println(error.offset);},Ok(_)=>{panic("accepted bad quote");}}
take(csv.feed(&mut reader,take(stdEncode("good")),true));match take(csv.next(&mut reader)){None=>{panic("lost row");},Some(row)=>{Out.println(row.get(1));}}take(csv.cancel(&mut reader));match csv.feed(&mut reader,take(stdEncode("")),false){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("closed");}}publish;
"#).unwrap();
    let out = call(&root, &["run", "main.rw"]);
    success(&out);
    assert_eq!(out.stdout, b"CsvQuote\n6\ngood\nCsvClosed\n");
    let out = call(&root, &["run", "main.rw", "--native-work", "1"]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("NativeWorkBudgetExceeded"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn csv_pending_checkpoint_and_frozen_task_transfer() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.csvStream as csv;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let reader=take(csv.reader());take(csv.feed(&mut reader,take(stdEncode("\"old")),false));commit pending;
take(csv.feed(&mut reader,take(stdEncode(" first\"")),true));match take(csv.next(&mut reader)){None=>{panic("missing");},Some(row)=>{Out.println(row.get(0));}}publish;
revert pending;let saved=freeze(move reader);let copy=thaw(saved);
async fn complete(reader:csv.Reader)->String effects {} {take(csv.feed(&mut reader,take(stdEncode(" second\"")),true));match take(csv.next(&mut reader)){None=>{panic("missing");},Some(row)=>{return row.get(0);}}}
let job=spawn complete(move copy);Out.println(take(await job));drop pending;publish;
"#).unwrap();
    let out = call(&root, &["run", "main.rw", "--record", "trace.json"]);
    success(&out);
    assert_eq!(out.stdout, b"old first\nold second\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
