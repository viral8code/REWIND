use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v200-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn v2_defaults_combine_traits_diagnostics_large_pages_and_published_revert_source_free() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "2.0.0");
    let root = root();
    fs::write(root.join("main.rw"),r#"import std.numeric as n;import std.taskError as failures;import std.unicode as unicode;
struct Item{value:Int}trait Read{fn read(self:&Self)->Int effects {};}impl Read for Item{fn read(self:&Item)->Int effects {} {return self.value;}}
fn read<T:Read>(item:&T)->Int effects {} {return item.read();}
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("setup");}}}
async fn broken()->Int effects {} {panic("v2 diagnostic");return 0;}
let item=Item(10);assert_eq(read(&item),10);
let dims=List<Int>();dims.add(65536);let zero=take(n.zerosFloat(&dims));let nonzero=take(n.affine(&zero,1.0,0.25));let values=take(n.valuesFloat(&nonzero));let saved=freeze(values);let cursor=values.iter();values.set(0,1.0);assert_eq(cursor.next(),Some(0.25));assert_eq(thaw(saved).get(0),0.25);
let actualShape=take(n.shapeFloat(&nonzero));actualShape.set(0,1);assert_eq(take(n.shapeFloat(&nonzero)).get(0),65536);let text="界a";let pieces=take(unicode.graphemes(&text));pieces.add("b");assert_eq(pieces.len(),3);
match await spawn broken(){Err(e)=>{let failure=failures.describe(move e);assert_eq(failure.code,"Panic");match move failure.diagnostic{Some(d)=>{assert_eq(d.source,"main.rw");assert_eq(d.message,"panic: v2 diagnostic");assert(d.line>0);},None=>{panic("diagnostic absent");}}},Ok(_)=>{panic("task succeeded");}}
var score=10;commit first;score=99;Out.println(score);publish;revert first;Out.println(score);publish;drop first;
"#).unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(root.join(".rewind"));
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--native-work",
            "100000000",
            "--history-memory",
            "128MiB",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
        ],
    );
    ok(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        "99\n10\n"
    );
    let replay = call(&root, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn v2_runs_a_source_free_artifact_with_historical_snapshot_limits() {
    let root = root();
    fs::write(
        root.join("rewind.toml"),
        "language = \"1.9.64\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(
        root.join("main.rw"),
        "let items=List<Int>();for i in 0..10001{items.add(i);}let saved=freeze(items);publish;\n",
    )
    .unwrap();
    ok(&call(&root, &["update"]));
    ok(&call(&root, &["compile", "main.rw", "--root", "."]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(root.join(".rewind"));
    let out = call(&root, &["run", "main.rwc", "--native-work", "10000000"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Map key snapshot budget exceeded"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
