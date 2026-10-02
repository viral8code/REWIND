use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v17-{}-{}",
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
fn compiled_sqlite_example_and_nested_frozen_rows_replay_without_source_or_database() {
    let root = dir();
    let source =
        include_str!("../examples/database/main.rw").replace("\":memory:\"", "\"data.sqlite\"");
    fs::write(root.join("main.rw"), source).unwrap();
    success(&call(
        &root,
        &["compile", "main.rw", "--allow-effects", "external,db,tasks"],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            "external,db,tasks",
            "--record",
            "trace.json",
        ],
    );
    success(&output);
    assert_eq!(output.stdout, b"1\nAlice\n42\n");
    fs::remove_file(root.join("data.sqlite")).unwrap();
    let replay = call(
        &root,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            "external,db,tasks",
        ],
    );
    success(&replay);
    assert_eq!(output.stdout, replay.stdout);
    assert!(!root.join("data.sqlite").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn db_capability_and_affine_ownership_are_checked_before_execution() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        "import std.db as db; external {let task=db.sqlite(\":memory:\",false,1000);} publish;",
    )
    .unwrap();
    let output = call(
        &root,
        &["compile", "main.rw", "--allow-effects", "external,tasks"],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("db"));
    fs::write(root.join("main.rw"),r#"
import std.db as db;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
var opening:Option<Task<Result<DbConnection,DbError>>>=None;
external {opening=Some(db.sqlite(":memory:",false,1000));}
match opening {None=>{panic("missing");},Some(task)=>{let connection=take(take(await task));let snapshot=freeze(connection);}}
publish;
"#).unwrap();
    let output = call(
        &root,
        &["compile", "main.rw", "--allow-effects", "external,db,tasks"],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("native")
            || String::from_utf8_lossy(&output.stderr).contains("freez")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_alias_and_duplicate_columns_return_typed_results() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.db as db;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let columns=List<String>();columns.push("name");columns.push("name");
match db.column(freeze(move columns),"name"){Err(error)=>{assert_eq(error.code,"DbDuplicateColumn");},Ok(_)=>{panic("duplicate silently accepted");}}
var opening:Option<Task<Result<DbConnection,DbError>>>=None;
external {opening=Some(db.sqlite(":memory:",false,5000));}
match opening {None=>{panic("missing");},Some(task)=>{
    let connection=take(take(await task));
    var query:Option<Task<Result<DbCursor,DbError>>>=None;
    external {
        let private=take(db.privateParameter("opaque",secret(DbValue::Text("private-value"))));
        let parameters=List<DbValue>();parameters.push(private);
        query=Some(db.query(&mut connection,"SELECT ?1",freeze(move parameters),5000));
    }
    match query {None=>{panic("missing query");},Some(task)=>{
        let cursor=take(take(await task));
        var read:Option<Task<Result<DbBatch,DbError>>>=None;
        external {read=Some(db.next(&mut cursor,64,1048576,5000));}
        match read {None=>{panic("missing read");},Some(task)=>{match take(await task){Err(error)=>{assert_eq(error.code,"DbSecretResult");Out.println(error.code);},Ok(_)=>{panic("private row exposed");}}}}
    }}
}}
publish;
"#).unwrap();
    let output = call(
        &root,
        &["run", "main.rw", "--allow-effects", "external,db,tasks"],
    );
    success(&output);
    assert_eq!(output.stdout, b"DbSecretResult\n");
    fs::remove_dir_all(root).unwrap();
}
