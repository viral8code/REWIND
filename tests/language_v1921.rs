use std::{fs, process::Command};
#[test]
fn live_requires_permission_and_recording_is_rejected_before_published_output() {
    let root = std::env::temp_dir().join(format!("rewind-v1921-sync-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("main.rw"),
        r#"import std.clock as clock;
Out.println("before");publish;
external live {let instant=clock.now();}
Out.println("after");publish;"#,
    )
    .unwrap();
    let call = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap()
    };
    let denied = call(&["run", "main.rw", "--allow-effects", "external,clock"]);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("live"));
    assert!(denied.stdout.is_empty());
    let recorded = call(&[
        "run",
        "main.rw",
        "--allow-effects",
        "external,clock,live",
        "--record",
        "trace.json",
    ]);
    assert!(!recorded.status.success());
    assert!(String::from_utf8_lossy(&recorded.stderr).contains("ExternalLiveRecordingUnsupported"));
    assert!(recorded.stdout.is_empty());
    assert!(!root.join("trace.json").exists());
    let live = call(&["run", "main.rw", "--allow-effects", "external,clock,live"]);
    assert!(
        live.status.success(),
        "{}",
        String::from_utf8_lossy(&live.stderr)
    );
    assert_eq!(live.stdout, b"before\nafter\n");
    let compile = call(&[
        "compile",
        "main.rw",
        "--allow-effects",
        "external,clock,live",
    ]);
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    fs::remove_file(root.join("main.rw")).unwrap();
    let compiled = call(&["run", "main.rwc", "--allow-effects", "external,clock,live"]);
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert_eq!(compiled.stdout, live.stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_sqlite_tasks_own_handles_and_source_free_reexecution_is_explicit() {
    let root = std::env::temp_dir().join(format!("rewind-v1921-sqlite-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"),r#"
import std.db as db;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value {Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
fn execute(connection:&mut DbConnection,sql:String)->Task<Result<Int,DbError>> effects {external,db,tasks,live} {
 var pending:Option<Task<Result<Int,DbError>>>=None;
 external live {pending=Some(db.execute(connection,sql,freeze(List<DbValue>()),5000));}
 match pending {Some(task)=>{return move task;},None=>{panic("missing task");}}
}
var opening:Option<Task<Result<DbConnection,DbError>>>=None;
external live {opening=Some(db.sqlite(":memory:",false,5000));}
match opening {None=>{panic("missing task");},Some(task)=>{
 let connection=take(take(await task));
 assert_eq(take(take(await execute(&mut connection,"CREATE TABLE values_table(value INTEGER)"))),0);
 var inserting:Option<Task<Result<Int,DbError>>>=None;
 external live {inserting=Some(db.execute(&mut connection,"INSERT INTO values_table VALUES (1)",freeze(List<DbValue>()),5000));}
 commit pending;
 match inserting {Some(task)=>{assert_eq(take(take(await task)),1);},None=>{panic("missing task");}}
 revert pending;
 match inserting {Some(task)=>{assert_eq(take(take(await task)),1);},None=>{panic("missing task");}}
 drop pending;
 for i in 0..24 {assert_eq(take(take(await execute(&mut connection,"INSERT INTO values_table VALUES (1)"))),1);}
 var querying:Option<Task<Result<DbCursor,DbError>>>=None;
 external live {querying=Some(db.query(&mut connection,"SELECT count(*) AS total FROM values_table",freeze(List<DbValue>()),5000));}
 match querying {None=>{panic("missing task");},Some(task)=>{
  let cursor=take(take(await task));
  var reading:Option<Task<Result<DbBatch,DbError>>>=None;
  external live {reading=Some(db.next(&mut cursor,1,1048576,5000));}
  match reading {None=>{panic("missing task");},Some(task)=>{
   let batch=take(take(await task));assert_eq(take(db.integer(batch.rows.get(0),0)),25);
  }}
 }}
}}
var cleanup:Option<Task<Result<Unit,DbError>>>=None;
external live {cleanup=Some(db.waitForCleanup(5000));}
match cleanup {None=>{panic("missing cleanup");},Some(task)=>{take(take(await task));}}
Out.println("live sqlite");publish;
"#).unwrap();
    let call = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap()
    };
    let effects = "external,db,tasks,live";
    let compiled = call(&["compile", "main.rw", "--allow-effects", effects]);
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    fs::remove_file(root.join("main.rw")).unwrap();
    let run = call(&[
        "run",
        "main.rwc",
        "--allow-effects",
        effects,
        "--native-work",
        "5000000",
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, b"live sqlite\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_http_checkpoint_reuses_the_pending_task_and_a_new_factory_submits_again() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        for _ in 0..3 {
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut byte = [0];
            while !bytes.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
                assert!(bytes.len() < 32768);
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
        }
        thread::sleep(Duration::from_millis(20));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    });
    let root = std::env::temp_dir().join(format!("rewind-v1921-http-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let source=r#"
import std.http as http;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value {Ok(v)=>{return move v;},Err(_)=>{panic("request failed");}}}
var pending:Option<Task<Result<HttpResponse,HttpError>>>=None;
external live {pending=Some(http.get("URL",5000,1024));}
commit waiting;
match pending {None=>{panic("missing");},Some(task)=>{assert_eq(take(take(await task)).status,200);}}
revert waiting;
match pending {None=>{panic("missing");},Some(task)=>{assert_eq(take(take(await task)).status,200);}}
drop waiting;
for i in 0..2 {
 var again:Option<Task<Result<HttpResponse,HttpError>>>=None;
 external live {again=Some(http.get("URL",5000,1024));}
 match again {None=>{panic("missing");},Some(task)=>{assert_eq(take(take(await task)).status,200);}}
}
Out.println("live http");publish;
"#.replace("URL",&url);
    fs::write(root.join("main.rw"), source).unwrap();
    let call = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap()
    };
    let effects = "external,network,tasks,live";
    let compiled = call(&["compile", "main.rw", "--allow-effects", effects]);
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    fs::remove_file(root.join("main.rw")).unwrap();
    let run = call(&[
        "profile",
        "main.rwc",
        "--allow-effects",
        effects,
        "--native-work",
        "5000000",
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, b"live http\n");
    let profile: serde_json::Value = String::from_utf8_lossy(&run.stderr)
        .lines()
        .filter_map(|v| serde_json::from_str(v).ok())
        .last()
        .unwrap();
    let live = &profile["runtime"]["external_live"];
    assert_eq!(live["retained_operations"], 0);
    assert_eq!(live["recorded_operations"], 0);
    assert_eq!(live["recorded_polls"], 0);
    worker.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recording_preflight_tracks_live_through_functions_closures_and_unexecuted_branches() {
    let root = std::env::temp_dir().join(format!("rewind-v1921-effects-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for code in [
        "fn work()->Unit effects {external,live} {external live {}} work();",
        "let action=||->Unit {external live {}}; action();",
        "if false {external live {}}",
    ] {
        fs::write(
            root.join("main.rw"),
            format!("Out.println(\"must not publish\");publish;{code}"),
        )
        .unwrap();
        for mode in ["debug", "compact"] {
            let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
                .current_dir(&root)
                .args([
                    "run",
                    "main.rw",
                    "--allow-effects",
                    "external,live",
                    "--record",
                    "trace.json",
                    "--record-mode",
                    mode,
                ])
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("ExternalLiveRecordingUnsupported"),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stdout.is_empty());
            assert!(!root.join("trace.json").exists());
        }
    }
    fs::remove_dir_all(root).unwrap();
}
