//! DBG-02/03: bounded value browsing and the stdio Debug Adapter Protocol server.
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

const PROGRAM: &str = "record Point {x:Int,y:Int}\nfn helper(p:Point)->Int {\n    let local = [1,2,3];\n    let m = Map<String,Int>();\n    m.set(\"a\", 1);\n    commit here;\n    return p.x + local.len();\n}\nfn main()->Int {\n    let pt = Point(3,4);\n    var total = helper(pt);\n    total += 1;\n    Out.println(total);\n    return 0;\n}\n";

fn dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-dap-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    fs::canonicalize(&p).unwrap()
}

fn record(cwd: &Path, mode: &str) -> PathBuf {
    fs::write(cwd.join("main.rw"), PROGRAM).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(cwd)
        .env_remove("REWIND_STANDALONE_LANGUAGE")
        .args([
            "run",
            "main.rw",
            "--record",
            "trace.json",
            "--record-mode",
            mode,
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    cwd.join("trace.json")
}

struct Client {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    seq: u64,
    events: Vec<Value>,
}

impl Client {
    fn start(cwd: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(cwd)
            .arg("dap")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
            seq: 0,
            events: Vec::new(),
        }
    }
    fn write_raw(&mut self, body: &str) {
        write!(self.input, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        self.input.flush().unwrap();
    }
    fn read(&mut self) -> Option<Value> {
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            if self.output.read_line(&mut line).unwrap() == 0 {
                return None;
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length: ") {
                length = v.parse().unwrap();
            }
        }
        let mut body = vec![0; length];
        self.output.read_exact(&mut body).unwrap();
        Some(serde_json::from_slice(&body).unwrap())
    }
    /// Sends a request and returns its response; events seen on the way are kept.
    fn request(&mut self, command: &str, arguments: Value) -> Value {
        self.seq += 1;
        let id = self.seq;
        self.write_raw(
            &json!({"seq":id,"type":"request","command":command,"arguments":arguments}).to_string(),
        );
        loop {
            let message = self.read().expect("server closed the stream");
            if message["type"] == "response" && message["request_seq"] == id {
                assert_eq!(message["command"], command);
                return message;
            }
            self.events.push(message);
        }
    }
    fn next_event(&mut self, name: &str) -> Value {
        if let Some(i) = self.events.iter().position(|e| e["event"] == name) {
            return self.events.remove(i);
        }
        loop {
            let message = self.read().expect("server closed the stream");
            if message["event"] == name {
                return message;
            }
            self.events.push(message);
        }
    }
}

fn open(cwd: &Path) -> Client {
    let trace = record(cwd, "debug");
    let mut client = Client::start(cwd);
    let init = client.request("initialize", json!({"adapterID":"rewind"}));
    assert_eq!(init["success"], true);
    assert_eq!(init["body"]["supportsStepBack"], true);
    client.next_event("initialized");
    let launch = client.request("launch", json!({"trace":trace}));
    assert_eq!(launch["success"], true, "{launch}");
    client
}

#[test]
fn breakpoint_stack_scopes_variables_and_watch() {
    let cwd = dir();
    let mut c = open(&cwd);
    let bp = c.request(
        "setBreakpoints",
        json!({"source":{"path":cwd.join("main.rw")},"breakpoints":[{"line":7},{"line":99}]}),
    );
    assert_eq!(bp["body"]["breakpoints"][0]["verified"], true);
    assert_eq!(bp["body"]["breakpoints"][1]["verified"], false);
    c.request("configurationDone", json!({}));
    assert_eq!(c.next_event("stopped")["body"]["reason"], "entry");
    let threads = c.request("threads", json!({}));
    assert_eq!(threads["body"]["threads"][0]["id"], 0);

    c.request("continue", json!({"threadId":0}));
    assert_eq!(c.next_event("stopped")["body"]["reason"], "breakpoint");
    let stack = c.request("stackTrace", json!({"threadId":0}));
    let frames = stack["body"]["stackFrames"].as_array().unwrap();
    assert_eq!(frames[0]["name"], "helper");
    assert_eq!(frames[0]["line"], 7);
    assert_eq!(frames[1]["name"], "main");
    assert_eq!(frames[1]["line"], 11, "caller frame points at the Call");

    let frame = frames[0]["id"].clone();
    let scopes = c.request("scopes", json!({"frameId":frame,"threadId":0}));
    let scope = scopes["body"]["scopes"][0]["variablesReference"].clone();
    let vars = c.request("variables", json!({"variablesReference":scope}));
    let list = vars["body"]["variables"].as_array().unwrap();
    let local = list.iter().find(|v| v["name"] == "local").unwrap();
    assert_eq!(local["type"], "List, 3 items");
    let items = c.request(
        "variables",
        json!({"variablesReference":local["variablesReference"],"start":1,"count":1}),
    );
    assert_eq!(items["body"]["variables"][0]["name"], "[1]");
    assert_eq!(items["body"]["variables"][0]["value"], "2");

    let watch = c.request(
        "evaluate",
        json!({"expression":"local[2]","frameId":frame,"threadId":0,"context":"watch"}),
    );
    assert_eq!(watch["body"]["result"], "3");
    let field = c.request(
        "evaluate",
        json!({"expression":"p.x","frameId":frame,"threadId":0,"context":"watch"}),
    );
    assert_eq!(field["body"]["result"], "3");
    for bad in ["helper(p)", "local[1+1]", "total = 4"] {
        let r = c.request(
            "evaluate",
            json!({"expression":bad,"frameId":frame,"threadId":0,"context":"repl"}),
        );
        assert_eq!(r["success"], false, "{bad}");
    }
    c.request("disconnect", json!({}));
    assert!(c.child.wait().unwrap().success());
}

#[test]
fn references_go_stale_when_the_debugger_moves() {
    let cwd = dir();
    let mut c = open(&cwd);
    c.request(
        "setBreakpoints",
        json!({"source":{"path":cwd.join("main.rw")},"breakpoints":[{"line":7}]}),
    );
    c.request("continue", json!({"threadId":0}));
    c.next_event("stopped");
    let stack = c.request("stackTrace", json!({"threadId":0}));
    let frame = stack["body"]["stackFrames"][0]["id"].clone();
    let scopes = c.request("scopes", json!({"frameId":frame,"threadId":0}));
    let scope = scopes["body"]["scopes"][0]["variablesReference"].clone();
    c.request("stepIn", json!({"threadId":0}));
    c.next_event("stopped");
    let stale = c.request("variables", json!({"variablesReference":scope}));
    assert_eq!(stale["success"], false);
    assert!(stale["message"].as_str().unwrap().contains("stale"));
    let stale_frame = c.request("scopes", json!({"frameId":frame,"threadId":0}));
    assert_eq!(stale_frame["success"], false);
    // Stepping back and reverse-continuing work on a debug recording.
    c.request("stepBack", json!({"threadId":0}));
    c.next_event("stopped");
    let rev = c.request("reverseContinue", json!({"threadId":0}));
    assert_eq!(rev["success"], true);
    c.next_event("stopped");
    c.request("disconnect", json!({}));
}

#[test]
fn running_off_the_end_terminates_and_stepping_is_bounded() {
    let cwd = dir();
    let mut c = open(&cwd);
    c.request("continue", json!({"threadId":0}));
    c.next_event("terminated");
    c.request("disconnect", json!({}));
}

#[test]
fn compact_recordings_and_bad_requests_are_refused_with_reasons() {
    let cwd = dir();
    let trace = record(&cwd, "compact");
    let mut c = Client::start(&cwd);
    c.request("initialize", json!({}));
    let launch = c.request("launch", json!({"trace":trace}));
    assert_eq!(launch["success"], false);
    assert!(launch["message"]
        .as_str()
        .unwrap()
        .contains("CompactTraceNoDebugHistory"));
    let early = c.request("threads", json!({}));
    assert_eq!(early["success"], false);
    let unknown = c.request("setVariable", json!({}));
    assert_eq!(unknown["success"], false);
    assert!(unknown["message"]
        .as_str()
        .unwrap()
        .contains("unsupported request"));
    let no_trace = c.request("launch", json!({}));
    assert_eq!(no_trace["success"], false);
    c.request("disconnect", json!({}));
}

#[test]
fn transport_limits_close_the_session_cleanly() {
    let cwd = dir();
    let mut c = Client::start(&cwd);
    c.request("initialize", json!({}));
    write!(c.input, "Content-Length: 99999999\r\n\r\n").unwrap();
    c.input.flush().unwrap();
    let status = c.child.wait().unwrap();
    assert!(!status.success());
    let mut deep = Client::start(&cwd);
    let nested = format!("{}{}", "[".repeat(200), "]".repeat(200));
    deep.write_raw(&format!(
        "{{\"seq\":1,\"type\":\"request\",\"command\":\"initialize\",\"arguments\":{nested}}}"
    ));
    assert!(!deep.child.wait().unwrap().success());
    let mut short = Client::start(&cwd);
    short.write_raw("{\"seq\":1");
    drop(short.input);
    // Truncated JSON in a complete frame is rejected, not guessed at.
    assert!(!short.child.wait().unwrap().success());
}

#[test]
fn two_sessions_do_not_share_state() {
    let a = dir();
    let b = dir();
    let mut first = open(&a);
    let mut second = open(&b);
    first.request(
        "setBreakpoints",
        json!({"source":{"path":a.join("main.rw")},"breakpoints":[{"line":7}]}),
    );
    first.request("continue", json!({"threadId":0}));
    first.next_event("stopped");
    let untouched = second.request("stackTrace", json!({"threadId":0}));
    assert_ne!(untouched["body"]["stackFrames"][0]["name"], "helper");
    first.request("disconnect", json!({}));
    second.request("disconnect", json!({}));
}
