use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn command(mode: &str) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rewind"));
    c.arg(mode);
    c
}
fn cmd(mode: &str, root: &Path, args: &[&str]) -> Output {
    command(mode)
        .args(args)
        .arg("--root")
        .arg(root)
        .output()
        .unwrap()
}
fn success(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn fixture(source: &str, effects: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v09-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(root.join("rewind.toml"),format!("language = \"0.9\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"{effects}\"\n")).unwrap();
    success(&cmd("update", &root, &[]));
    root
}
fn repl(root: &Path, input: &str, record: Option<&Path>) -> Output {
    let mut c = command("repl");
    c.arg("--root").arg(root);
    if let Some(p) = record {
        c.arg("--record").arg(p);
    }
    let mut child = c
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn multiline_session_records_failures_generations_and_replays_without_host_io() {
    let root = fixture("", "fileRead,fileWrite,output");
    fs::write(root.join("data"), "old").unwrap();
    let record = root.join("session.json");
    let out=repl(&root,":begin\nfn twice(n:Int)->Int effects {}{\nreturn n*2;\n}\n:end\nvar n=twice(2);commit base;let data=File.readText(\"data\");\nn=9;File.writeText(\"bad\",\"bad\");assert_eq(1,2);\nassert_eq(n,4);assert_eq(data,Ok(\"old\"));\n:begin\nn=99;\n:cancel\n:state\n:quit\n",Some(&record));
    success(&out);
    let data: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    assert_eq!(data["kind"], "rewind-session");
    assert!(data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["response"]["ok"] == false && e["response"]["diagnostic"]["code"].is_string()));
    assert!(data["final_state"]["checkpoint_generations"]["base"].is_number());
    assert_eq!(data["final_state"]["globals"]["n"], "4");
    assert!(!root.join("bad").exists());
    fs::write(root.join("data"), "changed").unwrap();
    success(&cmd("session-replay", &root, &[record.to_str().unwrap()]));
    assert!(!root.join("bad").exists());
    let mut corrupt = data;
    corrupt["entries"][7]["response"]["ok"] = json!(false);
    fs::write(&record, serde_json::to_vec(&corrupt).unwrap()).unwrap();
    assert!(!cmd("session-replay", &root, &[record.to_str().unwrap()])
        .status
        .success());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn transcript_rejects_changed_sources_missing_observations_secrets_and_incomplete_input() {
    let root = fixture("", "fileRead");
    fs::write(root.join("data"), "old").unwrap();
    let record = root.join("session.json");
    success(&repl(
        &root,
        "let data=File.readText(\"data\");assert_eq(data,Ok(\"old\"));\n:quit\n",
        Some(&record),
    ));
    let original = fs::read(&record).unwrap();
    let mut data: Value = serde_json::from_slice(&original).unwrap();
    data["entries"][0]["observations"]["files"] = json!([]);
    fs::write(&record, serde_json::to_vec(&data).unwrap()).unwrap();
    assert!(!cmd("session-replay", &root, &[record.to_str().unwrap()])
        .status
        .success());
    fs::write(&record, &original).unwrap();
    fs::write(root.join("main.rw"), "let changed=1;").unwrap();
    assert!(!cmd("session-replay", &root, &[record.to_str().unwrap()])
        .status
        .success());
    assert!(!repl(&root, ":begin\nlet n=1;\n", None).status.success());
    let secret = root.join("secret.json");
    assert!(
        !repl(&root, "let s=secret(\"private-token\");\n", Some(&secret))
            .status
            .success()
    );
    assert!(!secret.exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn generic_records_enforce_share_for_fields_constructors_and_unused_signatures() {
    let root=fixture("record Box<T:Share>{value:T}fn same<T:Share>(b:Box<T>)->Box<T> effects {}{return b;}let b=Box((3,true));let c=same(b);assert_eq(c.value._0,3);assert_eq(b.value._1,true);","");
    success(&cmd("run", &root, &[]));
    let artifact = root.join("program.json");
    success(&cmd(
        "build",
        &root,
        &["--output", artifact.to_str().unwrap()],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    success(&cmd("run-artifact", &root, &[artifact.to_str().unwrap()]));
    for (source, msg) in [
        ("record Box<T>{value:T}", "Share bounds"),
        (
            "record Box<T:Share>{value:T}let b=Box(List<Int>());",
            "must be Share",
        ),
        (
            "record Box<T:Share>{value:T}fn bad(b:Box<List<Int>>)->Unit effects {}{}",
            "must be Share",
        ),
        (
            "record Box<T:Share>{value:List<T>}",
            "requires a Share field",
        ),
    ] {
        fs::write(root.join("main.rw"), source).unwrap();
        let out = cmd("check", &root, &[]);
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(msg),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn candidate_shrinker_uses_order_dedup_budget_and_reusable_generator_contract() {
    let source="type Generator<T>=fn(Int)->T effects {} captures {Send,Share};record Source<T:Share>{generate:Generator<T>}let source=Source(|bits:Int|->Int{return 8;});fn smaller(n:Int)->Frozen<List<Int>> effects {}{let xs=List<Int>();xs.add(n);xs.add(0);xs.add(n/2);xs.add(n/2);return freeze(xs);}match propertyCandidates(7,3,source.generate,smaller,|n:Int|->Bool{return n<2;}){Err(f)=>{assert_eq(f.input,2);assert_eq(f.shrinks,2);},Ok(_)=>{panic(\"expected failure\");}}";
    let root = fixture(source, "");
    let trace = root.join("trace.json");
    success(&cmd("run", &root, &["--record", trace.to_str().unwrap()]));
    let data: Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
    assert_eq!(data["audit"][0]["strategy"], "ordered-candidates-v1");
    assert!(data["audit"][0]["evaluations"].as_u64().unwrap() <= 64);
    success(&cmd("replay", &root, &[trace.to_str().unwrap()]));
    fs::write(root.join("main.rw"),"propertyCandidates(7,1,|n:Int|->Int{return 2;},|n:Int|->Int{return n/2;},|n:Int|->Bool{return false;});").unwrap();
    assert!(!cmd("check", &root, &[]).status.success());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn api_v2_separates_const_value_from_syntax_and_checks_impl_associated_contracts() {
    let root=fixture("pub const N:Int=1+1;pub record Box{x:Int}pub trait Read{type Item;fn read(self:&Self)->Self::Item effects {};}impl Read for Box{type Item=Int;fn read(self:&Box)->Int effects {}{return self.x;}}","");
    let old = root.join("old.json");
    let new = root.join("new.json");
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", old.to_str().unwrap()],
    ));
    let snapshot: Value = serde_json::from_slice(&fs::read(&old).unwrap()).unwrap();
    assert_eq!(snapshot["format"], 2);
    assert_eq!(
        snapshot["symbols"]["impl:Read:Box"]["associated"]["Item"],
        "Int"
    );
    let source = fs::read_to_string(root.join("main.rw")).unwrap();
    fs::write(root.join("main.rw"), source.replace("1+1", "2")).unwrap();
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", new.to_str().unwrap()],
    ));
    success(
        &command("api-diff")
            .arg(&old)
            .arg(&new)
            .arg("--deny-breaking")
            .output()
            .unwrap(),
    );
    fs::write(root.join("main.rw"), source.replace("1+1", "3")).unwrap();
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", new.to_str().unwrap()],
    ));
    assert!(!command("api-diff")
        .arg(&old)
        .arg(&new)
        .arg("--deny-breaking")
        .output()
        .unwrap()
        .status
        .success());
    fs::remove_dir_all(root).unwrap();
}
fn signed_package(root: &Path, name: &str, source: &str, deps: Value) -> String {
    let path = root.join(format!("vendor/{name}"));
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join("rewind.package.json"),
        serde_json::to_vec(
            &json!({"name":name,"version":"1.0.0","effects":[],"dependencies":deps}),
        )
        .unwrap(),
    )
    .unwrap();
    fs::write(path.join("api.rw"), source).unwrap();
    fs::write(root.join("seed"), "07".repeat(32)).unwrap();
    let out = command("sign")
        .arg(&path)
        .arg("--key")
        .arg(root.join("seed"))
        .output()
        .unwrap();
    success(&out);
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}
fn packages() -> PathBuf {
    let root = fixture("import lib.api;assert_eq(answer(),42);", "");
    let public = signed_package(
        &root,
        "lib",
        "pub fn answer()->Int effects {}{return 42;}",
        json!({}),
    );
    signed_package(
        &root,
        "helper",
        "pub fn extra()->Int effects {}{return 7;}",
        json!({}),
    );
    fs::write(root.join("rewind.toml"),format!("language = \"0.9\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"\"\n[dependencies]\nlib = \"file:vendor/lib\"\n[dev_dependencies]\nhelper = \"file:vendor/helper\"\n[dependency_versions]\nlib = \"^1.0\"\nhelper = \"^1.0\"\n[dependency_signers]\nlib = \"author\"\nhelper = \"author\"\n[trust]\nauthor = \"{public}\"\n")).unwrap();
    success(&cmd("update", &root, &[]));
    root
}
#[test]
fn production_install_omits_dev_packages_verifies_runtime_and_refuses_unverified_tests() {
    let root = packages();
    let installed = root.join("production");
    success(&cmd(
        "install",
        &root,
        &["--production", "--output", installed.to_str().unwrap()],
    ));
    assert!(!installed.join("vendor/helper").exists());
    assert!(!installed.join("seed").exists());
    success(&cmd("run", &installed, &[]));
    assert!(!cmd("test", &installed, &[]).status.success());
    assert!(!cmd("doctest", &installed, &["anything.md"])
        .status
        .success());
    let artifact = installed.join("program.json");
    success(&cmd(
        "build",
        &installed,
        &["--output", artifact.to_str().unwrap()],
    ));
    success(&cmd(
        "run-artifact",
        &installed,
        &[artifact.to_str().unwrap()],
    ));
    fs::write(
        installed.join("vendor/lib/api.rw"),
        "pub fn answer()->Int effects {}{return 99;}",
    )
    .unwrap();
    let out = cmd("run", &installed, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("signature verification failed"));
    assert!(!cmd(
        "install",
        &root,
        &["--production", "--output", installed.to_str().unwrap()]
    )
    .status
    .success());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn api_dependency_contracts_and_checked_conversion_reject_mismatches() {
    let root = packages();
    let old = root.join("old.json");
    let new = root.join("new.json");
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", old.to_str().unwrap()],
    ));
    let data: Value = serde_json::from_slice(&fs::read(&old).unwrap()).unwrap();
    assert!(data["symbols"]
        .as_object()
        .unwrap()
        .keys()
        .any(|s| s.starts_with("dependency:")));
    signed_package(
        &root,
        "lib",
        "pub fn answer()->Int effects {}{return 42;}pub fn another()->Int effects {}{return 0;}",
        json!({}),
    );
    success(&cmd("update", &root, &[]));
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", new.to_str().unwrap()],
    ));
    assert!(!command("api-diff")
        .arg(&old)
        .arg(&new)
        .arg("--deny-breaking")
        .output()
        .unwrap()
        .status
        .success());
    let simple = fixture("pub fn value()->Int effects {}{return 1;}", "");
    let current = simple.join("current.json");
    success(&cmd(
        "api-snapshot",
        &simple,
        &["--output", current.to_str().unwrap()],
    ));
    let mut legacy: Value = serde_json::from_slice(&fs::read(&current).unwrap()).unwrap();
    legacy["format"] = json!(1);
    let legacy_file = simple.join("legacy.json");
    fs::write(&legacy_file, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let converted = simple.join("converted.json");
    success(&cmd(
        "api-convert",
        &simple,
        &[
            legacy_file.to_str().unwrap(),
            "--output",
            converted.to_str().unwrap(),
        ],
    ));
    assert!(!command("api-diff")
        .arg(&legacy_file)
        .arg(&current)
        .output()
        .unwrap()
        .status
        .success());
    fs::write(
        simple.join("main.rw"),
        "pub fn changed()->Int effects {}{return 1;}",
    )
    .unwrap();
    assert!(!cmd(
        "api-convert",
        &simple,
        &[
            legacy_file.to_str().unwrap(),
            "--output",
            converted.to_str().unwrap()
        ]
    )
    .status
    .success());
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(simple).unwrap();
}
#[test]
fn inspection_signatures_have_distinct_domains_and_timeline_checks_them() {
    let root = fixture("", "");
    let old_trace =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v07-indexed.trace.json");
    assert!(!command("timeline")
        .arg(old_trace)
        .output()
        .unwrap()
        .status
        .success());
    let bundle = root.join("inspection.json");
    fs::write(&bundle, include_bytes!("fixtures/v07-inspection.json")).unwrap();
    fs::write(root.join("seed"), "07".repeat(32)).unwrap();
    let signed = command("sign-inspection")
        .arg(&bundle)
        .arg("--key")
        .arg(root.join("seed"))
        .output()
        .unwrap();
    success(&signed);
    let key = String::from_utf8(signed.stdout).unwrap();
    success(
        &command("timeline")
            .arg(&bundle)
            .arg("--public-key")
            .arg(key.trim())
            .output()
            .unwrap(),
    );
    assert!(!command("verify-trace")
        .arg(&bundle)
        .arg("--public-key")
        .arg(key.trim())
        .output()
        .unwrap()
        .status
        .success());
    let mut data: Value = serde_json::from_slice(&fs::read(&bundle).unwrap()).unwrap();
    data["audit"] = json!([{"tampered":true}]);
    fs::write(&bundle, serde_json::to_vec(&data).unwrap()).unwrap();
    assert!(!command("timeline")
        .arg(&bundle)
        .arg("--public-key")
        .arg(key.trim())
        .output()
        .unwrap()
        .status
        .success());
    fs::remove_dir_all(root).unwrap();
}
fn lsp(root: &Path, requests: Vec<Value>) -> Vec<Value> {
    let mut payload = Vec::new();
    for request in requests {
        let bytes = serde_json::to_vec(&request).unwrap();
        payload.extend_from_slice(format!("Content-Length: {}\r\n\r\n", bytes.len()).as_bytes());
        payload.extend_from_slice(&bytes);
    }
    let mut child = command("lsp")
        .arg("--root")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&payload).unwrap();
    let out = child.wait_with_output().unwrap();
    success(&out);
    let mut bytes = out.stdout.as_slice();
    let mut results = Vec::new();
    while !bytes.is_empty() {
        let end = bytes.windows(4).position(|s| s == b"\r\n\r\n").unwrap();
        let header = std::str::from_utf8(&bytes[..end]).unwrap();
        let n = header
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        results.push(serde_json::from_slice(&bytes[end + 4..end + 4 + n]).unwrap());
        bytes = &bytes[end + 4 + n..];
    }
    results
}
#[test]
fn incomplete_editor_source_has_provisional_help_and_source_free_timeline() {
    let source = "fn add(n:Int)->Int effects {}{return n+1;}\nlet value=add(1);\n";
    let root = fixture(source, "");
    let uri = format!(
        "file:///{}",
        root.join("main.rw")
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let trace = root.join("trace.json");
    success(&cmd("run", &root, &["--record", trace.to_str().unwrap()]));
    let partial = "fn add(n:Int)->Int effects {}{return n+1;}\nlet value=add(\n";
    let rows = lsp(
        &root,
        vec![
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":source}}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":partial}]}}),
            json!({"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":uri},"position":{"line":1,"character":14}}}),
            json!({"jsonrpc":"2.0","id":3,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":{"line":1,"character":11}}}),
            json!({"jsonrpc":"2.0","id":4,"method":"textDocument/signatureHelp","params":{"textDocument":{"uri":uri},"position":{"line":1,"character":14}}}),
            json!({"jsonrpc":"2.0","id":5,"method":"rewind/timeline","params":{"uri":format!("file:///{}",trace.to_string_lossy().replace('\\',"/").trim_start_matches('/')),"count":2}}),
            json!({"jsonrpc":"2.0","id":6,"method":"textDocument/rename","params":{"textDocument":{"uri":uri},"position":{"line":1,"character":11},"newName":"changed"}}),
            json!({"jsonrpc":"2.0","id":7,"method":"shutdown","params":{}}),
            json!({"jsonrpc":"2.0","method":"exit","params":{}}),
        ],
    );
    let response = |id: i64| rows.iter().find(|r| r["id"] == id).unwrap();
    assert!(response(2)["result"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["label"] == "add" && v["data"]["provisional"] == true));
    assert!(response(3)["result"]["contents"]["value"]
        .as_str()
        .unwrap()
        .contains("last checked declaration"));
    assert_eq!(
        response(4)["result"]["signatures"][0]["parameters"][0]["label"],
        "n: Int"
    );
    assert_eq!(response(5)["result"]["schema"], "rewind-timeline/v1");
    assert!(response(6)["error"].is_object());
    assert_eq!(fs::read_to_string(root.join("main.rw")).unwrap(), source);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn session_budget_counts_failed_attempts_and_cannot_be_reset() {
    let root = fixture("", "");
    let input = format!(
        "{}:reset\nwhile true{{}}\n:quit\n",
        "while true{}\n".repeat(10)
    );
    let out = repl(&root, &input, None);
    success(&out);
    let rows = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows[9]["response"]["session_steps"], 1_000_000);
    assert!(rows[11]["response"]["error"]
        .as_str()
        .unwrap()
        .contains("SessionExecutionBudgetExceeded"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn candidate_shrinker_deduplicates_frozen_values_independently_of_heap_ids() {
    let source="fn gen(bits:Int)->Frozen<List<Int>> effects {}{let xs=List<Int>();xs.add(1);return freeze(xs);}fn smaller(value:Frozen<List<Int>>)->Frozen<List<Frozen<List<Int>>>> effects {}{let xs=List<Frozen<List<Int>>>();xs.add(gen(0));xs.add(gen(0));return freeze(xs);}match propertyCandidates(7,1,gen,smaller,|value:Frozen<List<Int>>|->Bool{return false;}){Err(f)=>{assert_eq(f.shrinks,0);assert_eq(f.input.len(),1);},Ok(_)=>{panic(\"expected failure\");}}";
    let root = fixture(source, "");
    success(&cmd("run", &root, &[]));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_map_const_snapshot_tracks_contents_without_heap_identity() {
    let root = fixture("fn values()->Frozen<Map<Int,String>> effects {}{let m=Map<Int,String>();m.set(1,\"one\");return freeze(m);}pub const DATA:Frozen<Map<Int,String>>=values();", "");
    let old = root.join("old.json");
    let new = root.join("new.json");
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", old.to_str().unwrap()],
    ));
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", new.to_str().unwrap()],
    ));
    assert_eq!(fs::read(&old).unwrap(), fs::read(&new).unwrap());
    let source = fs::read_to_string(root.join("main.rw"))
        .unwrap()
        .replace("one", "changed");
    fs::write(root.join("main.rw"), source).unwrap();
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", new.to_str().unwrap()],
    ));
    assert_ne!(fs::read(&old).unwrap(), fs::read(&new).unwrap());
    fs::remove_dir_all(root).unwrap();
}
