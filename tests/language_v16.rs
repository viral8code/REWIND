use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v16-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn success(out: &std::process::Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn fixture(body: &'static [u8], delay: u64) -> (String, thread::JoinHandle<()>, Arc<AtomicUsize>) {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/binary", server.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let worker = thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut data = Vec::new();
        let mut byte = [0];
        while !data.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            data.push(byte[0]);
            assert!(data.len() < 32768);
        }
        observed.fetch_add(1, Ordering::SeqCst);
        thread::sleep(std::time::Duration::from_millis(delay));
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nX-Test: binary\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(body);
    });
    (url, worker, count)
}
fn fetch(url: &str, limit: usize) -> String {
    format!(
        r#"
import std.http as http;
var pending:Option<Task<Result<HttpResponse,HttpError>>>=None;
external {{pending=Some(http.get("{url}",1000,{limit}));}}
match pending {{Some(task)=>{{match await task {{Ok(response)=>{{match response {{Ok(r)=>{{Out.println(r.status);Out.println(stdBytesLength(r.body));}},Err(e)=>{{Out.println(e.code);Out.println(e.phase);}}}}}},Err(_)=>{{panic("task failed");}}}}}},None=>{{panic("missing task");}}}}
publish;
"#
    )
}
#[test]
fn binary_http_revert_and_offline_replay_never_repeat_request() {
    let root = dir();
    let (url, worker, count) = fixture(b"a\xff\0", 15);
    let body = fetch(&url, 128).replace("import std.http as http;", "");
    let source = format!("import std.http as http;commit saved;{{{body}}}revert saved;{{{body}}}");
    fs::write(root.join("main.rw"), source).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rw",
            "--allow-effects",
            "external,network,tasks",
            "--record",
            "trace.json",
        ],
    );
    success(&out);
    assert_eq!(out.stdout, b"200\n3\n200\n3\n");
    worker.join().unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let replay = call(
        &root,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            "external,network,tasks",
        ],
    );
    success(&replay);
    assert_eq!(replay.stdout, out.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn body_limit_is_a_typed_response_received_error() {
    let root = dir();
    let (url, worker, _) = fixture(b"too big", 0);
    fs::write(root.join("main.rw"), fetch(&url, 3)).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rw",
            "--allow-effects",
            "external,network,tasks",
        ],
    );
    success(&out);
    assert_eq!(out.stdout, b"HttpBodyLimit\nResponseReceived\n");
    worker.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn network_permission_and_external_boundary_are_required() {
    let root = dir();
    fs::write(root.join("main.rw"), fetch("http://127.0.0.1:1", 64)).unwrap();
    let out = call(
        &root,
        &["compile", "main.rw", "--allow-effects", "external,tasks"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("network"));
    fs::write(
        root.join("main.rw"),
        "import std.http as http;var task=http.get(\"http://127.0.0.1:1\",10,64);",
    )
    .unwrap();
    let out = call(
        &root,
        &[
            "compile",
            "main.rw",
            "--allow-effects",
            "external,network,tasks",
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("external"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn gui_poll_and_other_tasks_continue_while_http_is_pending() {
    let root = dir();
    let (url, worker, _) = fixture(b"ok", 200);
    let source = format!(
        r#"
import std.http as http;import std.gui as gui;
async fn other()->Unit effects {{output}} {{Out.println("other task");}}
let otherTask=spawn other();
match gui.window("HTTP",320,240) {{Err(e)=>{{panic(e.code);}},Ok(view)=>{{
    gui.present(&view);publish;
    var pending:Option<Task<Result<HttpResponse,HttpError>>>=None;
    external {{pending=Some(http.get("{url}",1000,128));}}
    match pending {{None=>{{panic("missing task");}},Some(task)=>{{
        match gui.pollEvent() {{Ok(Some(event))=>{{assert_eq(event.kind,"close");assert_eq(task.isDone(),false);Out.println("responsive GUI");}},_=>{{panic("missing event");}}}}
        gui.close();
        match await task {{Ok(Ok(response))=>{{assert_eq(response.status,200);}},_=>{{panic("request failed");}}}}
    }}}}
}}}}
await otherTask;publish;
"#
    );
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(
        root.join("events.json"),
        r#"[{"kind":"close","x":0,"y":0,"key":"","width":0,"height":0}]"#,
    )
    .unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rw",
            "--allow-effects",
            "external,network,tasks,gui",
            "--gui-events",
            "events.json",
            "--record",
            "trace.json",
        ],
    );
    success(&out);
    assert_eq!(out.stdout, b"responsive GUI\nother task\n");
    worker.join().unwrap();
    let replay = call(
        &root,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            "external,network,tasks,gui",
        ],
    );
    success(&replay);
    assert_eq!(replay.stdout, out.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn an_async_function_can_submit_in_a_region_then_await_outside_it() {
    let root = dir();
    let (url, worker, _) = fixture(b"ok", 0);
    fs::write(root.join("main.rw"),format!(r#"
import std.http as http;
async fn load()->Result<HttpResponse,HttpError> effects {{external,network,tasks}} {{
    var pending:Option<Task<Result<HttpResponse,HttpError>>>=None;
    external {{pending=Some(http.get("{url}",1000,128));}}
    match pending {{Some(task)=>{{match await task {{Ok(r)=>{{return r;}},Err(_)=>{{return Err(HttpError("TaskCancelled","Unknown",0));}}}}}},None=>{{panic("missing task");}}}}
}}
let task=load();match await task {{Ok(Ok(response))=>{{Out.println(response.status);}},_=>{{panic("failed");}}}}publish;
"#)).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rw",
            "--allow-effects",
            "external,network,tasks",
        ],
    );
    success(&out);
    assert_eq!(out.stdout, b"200\n");
    worker.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn compiled_http_program_runs_without_source_and_replays_without_server() {
    let root = dir();
    let (url, worker, _) = fixture(b"ok", 0);
    fs::write(root.join("main.rw"), fetch(&url, 128)).unwrap();
    let compiled = call(
        &root,
        &[
            "compile",
            "main.rw",
            "--allow-effects",
            "external,network,tasks",
        ],
    );
    success(&compiled);
    fs::remove_file(root.join("main.rw")).unwrap();
    if root.join(".rewind").exists() {
        fs::remove_dir_all(root.join(".rewind")).unwrap();
    }
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            "external,network,tasks",
            "--record",
            "trace.json",
        ],
    );
    success(&out);
    assert_eq!(out.stdout, b"200\n2\n");
    worker.join().unwrap();
    let replay = call(
        &root,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            "external,network,tasks",
        ],
    );
    success(&replay);
    assert_eq!(replay.stdout, out.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn uri_components_are_pure_and_binary_request_headers_are_typed() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        r#"
import std.http as http;
assert_eq(http.component("a b/日本+?"),Ok("a%20b%2F%E6%97%A5%E6%9C%AC%2B%3F"));
match stdEncode("") {Err(_)=>{panic("encoding failed");},Ok(body)=>{
    let headers=List<HttpHeader>();headers.add(HttpHeader("X-Test",body));
    let request=http.Request("GET","http://127.0.0.1:1",body,10,128,freeze(headers),body,"");
    assert_eq(request.method,"GET");
}}
"#,
    )
    .unwrap();
    let out = call(&root, &["run", "main.rw"]);
    success(&out);
    fs::remove_dir_all(root).unwrap();
}
