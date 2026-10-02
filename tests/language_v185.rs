use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v185-{}-{}",
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
fn unicode_source_free_replay_and_checkpoint() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/unicode/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(
        String::from_utf8(output.stdout.clone()).unwrap(),
        "3\n🇯🇵\nfalse\ntrue\nA1ffi\nSTRASSE\n3\n"
    );
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(output.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn unicode_freeze_tasks_boundaries_and_version_tables() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.unicode as unicode;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let text="é🇯🇵";let offsets=take(unicode.graphemeOffsets(&text));Out.println(offsets.get(0));Out.println(offsets.get(1));Out.println(offsets.get(2));
let clusters=take(unicode.graphemes(&text));let saved=freeze(move clusters);let owned=thaw(saved);
async fn count(items:List<String>)->Int effects {} {return items.len();}let task=spawn count(move owned);Out.println(take(await task));
let greek="ΟΣ İ";Out.println(take(unicode.lower(&greek)));let composed="é";let nfd=take(unicode.normalize(&composed,unicode.Form::NFD));Out.println(take(unicode.isNormalized(&nfd,unicode.Form::NFD)));
let words="Hello, world!";let parts=take(unicode.words(&words));Out.println(parts.len());let sentences=take(unicode.sentences(&words));Out.println(sentences.len());
match unicode.graphemeSlice(&text,-1,2){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("invalid index");}}
Out.println(take(unicode.versions()));publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    let actual = String::from_utf8(output.stdout).unwrap();
    assert!(
        actual.starts_with("0\n3\n11\n2\nος i̇\ntrue\n2\n1\nUnicodeIndex\n"),
        "{actual}"
    );
    assert!(actual.contains("normalization=17.0.0;segmentation=17.0.0;case="));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn unicode_native_work_is_precharged_before_transformation() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"import std.unicode as unicode;let value="é";unicode.normalize(&value,unicode.Form::NFC);Out.println("unreachable");publish;"#).unwrap();
    let output = call(&root, &["run", "main.rw", "--native-work", "1"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("NativeWorkBudgetExceeded"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
