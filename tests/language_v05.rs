use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn fixture(source: &str, effects: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v05-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(root.join("rewind.toml"),format!("language = \"0.5\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"{effects}\"\n")).unwrap();
    let output = cmd("update", &root, &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    root
}
fn cmd(mode: &str, root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(mode)
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(source: &str, effects: &str, expected: &[u8]) {
    let root = fixture(source, effects);
    let output = cmd("run", &root, &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
    fs::remove_dir_all(root).unwrap();
}
fn rejected(source: &str, message: &str) {
    let root = fixture(source, "tasks,output,fileRead,fileWrite");
    let output = cmd("check", &root, &[]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_values_thaw_and_typed_task_errors() {
    ok(
        r#"
        async fn count(items:Frozen<List<Int>>)->Int {return items.len();}
        let xs=List<Int>();xs.add(7);let frozen=freeze(xs);let copy=thaw(frozen);copy.add(8);
        assert_eq(xs.len(),1);assert_eq(frozen.len(),1);assert_eq(copy.len(),2);
        let task=spawn count(frozen);assert_eq(await task,Ok(1));
        let c=Channel<Int>(1);c.close();assert_eq(await c.receive(),Err(TaskError::ChannelClosed));
        Out.println("ok");publish;
    "#,
        "tasks,output",
        b"ok\n",
    );
    rejected(
        "let xs=List<Int>();let frozen=freeze(xs);frozen.add(1);",
        "cannot mutate Frozen",
    );
}
#[test]
fn move_borrow_and_checkpoint_ownership_are_checked() {
    rejected(
        "let xs=List<Int>();let ys=move xs;xs.len();",
        "use after move",
    );
    rejected(
        "let xs=List<Int>();let borrowed=&xs;xs.add(1);",
        "while borrowed",
    );
    rejected(
        "let xs=List<Int>();let borrowed=&mut xs;let c=Channel<Int>(1);await c.send(1);",
        "cannot cross await",
    );
    rejected("async fn read(h:FileHandle)->Unit {}", "not Send");
    ok("let xs=List<Int>();{let borrowed=&mut xs;borrowed.add(1);}assert_eq(xs.len(),1);commit saved;let ys=move xs;revert saved;assert_eq(xs.len(),1);","",b"");
}
#[test]
fn effects_are_checked_per_reachable_function() {
    ok("pub fn pure()->Int effects {} {return 7;} pub fn unused()->Unit effects {output} {Out.println(1);}assert_eq(pure(),7);","",b"");
    rejected(
        "pub fn missing()->Unit effects {} {Out.println(1);}",
        "undeclared effect output",
    );
    let root = fixture(
        "fn noisy()->Unit {Out.println(1);}let action=noisy;action();",
        "",
    );
    let output = cmd("check", &root, &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("effect output is not allowed"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn artifact_executes_without_sources_and_rejects_tampering() {
    let root = fixture("Out.println(42);publish;", "output");
    let output = cmd("build", &root, &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let artifact = root.join("rewind.build.json");
    let bytes = fs::read(&artifact).unwrap();
    let build = cmd("build", &root, &[]);
    assert!(build.status.success());
    assert_eq!(fs::read(&artifact).unwrap(), bytes);
    fs::remove_file(root.join("main.rw")).unwrap();
    fs::remove_file(root.join("rewind.toml")).unwrap();
    fs::remove_file(root.join("rewind.lock")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run-artifact")
        .arg(&artifact)
        .arg("--root")
        .arg(&root)
        .arg("--allow-effects")
        .arg("output")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"42\n");
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["payload"]["build"]["bytecode"][0]["operation"] = serde_json::json!("Jump(999999)");
    fs::write(&artifact, serde_json::to_vec(&json).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run-artifact")
        .arg(&artifact)
        .arg("--root")
        .arg(&root)
        .arg("--allow-effects")
        .arg("output")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("hash mismatch"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn groups_join_and_failures_are_not_silently_dropped() {
    ok("async fn worker()->Unit {Out.println(1);} {using group=TaskGroup();group.add(worker());}publish;","tasks,output",b"1\n");
    ok("async fn worker()->Unit {panic(\"ignored\");}let task=spawn worker();task.ignore();await task;","tasks",b"");
    let root = fixture(
        "async fn worker()->Unit {panic(\"unhandled\");}let task=spawn worker();",
        "tasks",
    );
    let output = cmd("run", &root, &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("UnhandledTaskError"));
    fs::remove_dir_all(root).unwrap();
    ok("File.writeText(\"file\",\"a\");using resource=File.open(\"file\");let unrelated=||->Int{return 1;};async fn pure()->Int{return 7;}let task=spawn pure();assert_eq(await task,Ok(7));","fileRead,fileWrite,tasks",b"");
}
#[test]
fn generic_impl_associated_types_and_standard_iteration() {
    ok(
        r#"
        struct Box<T>{value:T}
        trait Get {type Item;fn get(self:Self)->Self::Item;}
        impl<T> Get for Box<T> {type Item=T;fn get(self:Self)->T effects {} {return self.value;}}
        let box=Box<Int>(7);assert_eq(box.get(),7);
        let xs=List<Int>();xs.add(1);xs.add(2);var sum=0;for x in xs {sum+=x;}assert_eq(sum,3);
        let iter=xs.iter();commit before;assert_eq(iter.next(),Some(1));revert before;assert_eq(iter.next(),Some(1));
        var text="";for character in "あb" {text=text+character;}assert_eq(text,"あb");
    "#,
        "",
        b"",
    );
    rejected("struct Box<T>{value:T}trait Get{fn get(self:Self)->Int;}impl<T> Get for Box<T>{fn get(self:Self)->Int{return 1;}}impl Get for Box<Int>{fn get(self:Self)->Int{return 2;}}","overlapping impls");
}
#[test]
fn key_generation_signed_files_and_failure_record_replay() {
    let root = fixture("panic(\"recorded failure\");", "");
    let seed = root.join("private.seed");
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("keygen")
        .arg(&seed)
        .output()
        .unwrap();
    assert!(output.status.success());
    let public = String::from_utf8(output.stdout).unwrap();
    assert_eq!(fs::read_to_string(&seed).unwrap().trim().len(), 64);
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("keygen")
        .arg(&seed)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let record = root.join("record.json");
    let output = cmd("run", &root, &["--record", record.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(record.exists());
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("sign-trace")
        .arg(&record)
        .arg("--key")
        .arg(&seed)
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&record)
        .arg("--root")
        .arg(&root)
        .arg("--verify-key")
        .arg(public.trim())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("recorded failure") && !error.contains("ReplayMismatch"),
        "{error}"
    );
    fs::write(&record, "{}").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("verify-trace")
        .arg(&record)
        .arg("--public-key")
        .arg(public.trim())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("signature verification failed"));
    fs::remove_dir_all(root).unwrap();
}

fn interactive(mode: &str, root: &Path, args: &[&str], input: &[u8]) -> Output {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(mode)
        .arg("--root")
        .arg(root)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn transferable_closures_and_mutable_results_have_independent_owners() {
    ok(
        r#"
        async fn compute(f:fn()->Int)->Int {return f();}
        var n=7;let f=||->Int{return n;};let t=spawn compute(move f);n=9;assert_eq(await t,Ok(7));
        async fn items()->List<Int>{let xs=List<Int>();xs.add(1);return xs;}
        let result_task=spawn items();match await result_task {Ok(xs)=>xs.add(2),Err(_)=>panic("unexpected"),}
        match await result_task {Ok(xs)=>assert_eq(xs.len(),1),Err(_)=>panic("unexpected"),}
    "#,
        "tasks,fileRead,fileWrite,output,input,clock,random,env,args,locale",
        b"",
    );
    rejected(
        "using h=File.open(\"file\");async fn bad()->Unit{h.read(1);}",
        "async capture h",
    );
    rejected("using h=File.open(\"file\");let f=||->Unit{h.read(1);};async fn take(f:fn()->Unit)->Unit{f();}let t=take(move f);","is not Send");
    rejected(
        "using h=File.open(\"file\");freeze(h);",
        "freeze requires snapshot",
    );
    ok("struct Box<T>{value:T}let xs=List<Int>();xs.add(1);let b=Box<List<Int>>(move xs);let frozen=freeze(b);assert_eq(frozen.value.len(),1);for x in frozen.value {assert_eq(x,1);}","",b"");
    rejected(
        "struct Box{value:Int}let b=freeze(Box(1));b.value=2;",
        "cannot mutate Frozen",
    );
    rejected(
        "fn take(xs:List<Int>)->Unit{}let xs=List<Int>();take(&xs);",
        "lexical borrow cannot escape",
    );
}
#[test]
fn secret_values_and_derived_aggregates_are_redacted() {
    let root=fixture("let s=secret(\"classified\");let derived=s+\"-derived\";let xs=List<Secret<String>>();xs.add(derived);Out.println(xs);commit saved;publish;","output");
    let trace = root.join("trace.json");
    let output = cmd("run", &root, &["--record", trace.to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"<redacted: Secret>\n");
    let recorded = fs::read_to_string(&trace).unwrap();
    assert!(!recorded.contains("classified"), "{recorded}");
    fs::remove_dir_all(root).unwrap();
    rejected("let s=secret(true);s&&true;", "reveal Secret<Bool>");
}
#[test]
fn failed_test_and_explored_schedule_can_be_replayed() {
    let root = fixture(
        r#"async fn producer(c:Channel<Int>,n:Int)->Unit{await c.send(n);}test fn choices()->Unit{let c=Channel<Int>(2);let a=spawn producer(c,1);let b=spawn producer(c,2);await a;await b;assert_eq(await c.receive(),Ok(1));}"#,
        "tasks",
    );
    let trace = root.join("failed.json");
    let output = cmd(
        "test",
        &root,
        &["--explore", "20", "--record", trace.to_str().unwrap()],
    );
    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
    assert_eq!(json["result"]["ok"], false);
    assert_eq!(json["test"], "choices");
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!replay.status.success());
    assert!(
        !String::from_utf8_lossy(&replay.stderr).contains("ReplayMismatch"),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn lsp_checks_unsaved_text_and_returns_hover_definition_completion() {
    let root = fixture("fn answer()->Int{return 1;}assert_eq(answer(),1);", "");
    let uri = format!(
        "file:///{}",
        root.join("main.rw").to_string_lossy().replace('\\', "/")
    );
    let requests = [
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":"fn answer()->Int{return 2;}assert_eq(answer(),2);"}}}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":{"line":0,"character":39}}}),
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":{"line":0,"character":39}}}),
        serde_json::json!({"jsonrpc":"2.0","id":4,"method":"textDocument/completion","params":{"textDocument":{"uri":uri}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":"let n:Int=\"bad\";"}]}}),
        serde_json::json!({"jsonrpc":"2.0","id":5,"method":"shutdown"}),
        serde_json::json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let mut input = Vec::new();
    for req in requests {
        let bytes = serde_json::to_vec(&req).unwrap();
        input.extend(format!("Content-Length: {}\r\n\r\n", bytes.len()).as_bytes());
        input.extend(bytes);
    }
    let output = interactive("lsp", &root, &[], &input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("hoverProvider")
            && text.contains("fn answer")
            && text.contains("\"label\":\"answer\"")
            && text.contains("type mismatch"),
        "{text}"
    );
    assert_eq!(
        fs::read_to_string(root.join("main.rw")).unwrap(),
        "fn answer()->Int{return 1;}assert_eq(answer(),1);"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn recorded_debugger_steps_without_publishing_or_modifying_trace() {
    use std::io::Write;
    let root = fixture(
        "var n=1;commit saved;n=2;File.writeText(\"out.txt\",\"new\");publish;",
        "fileWrite",
    );
    let trace = root.join("trace.json");
    let output = cmd("run", &root, &["--record", trace.to_str().unwrap()]);
    assert!(output.status.success());
    let bytes = fs::read(&trace).unwrap();
    fs::remove_file(root.join("out.txt")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("debug-session")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"step\nstate\ncheckpoint saved\nstate\ncontinue\nfiles\nquit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("\"event\":1") && text.contains("\"found\":true") && text.contains("out.txt"),
        "{text}"
    );
    assert!(!root.join("out.txt").exists());
    assert_eq!(fs::read(&trace).unwrap(), bytes);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn package_mirrors_fix_transitive_requirements_and_revoked_keys() {
    let root = fixture("import math.api;assert_eq(answer(),42);", "");
    let seed = root.join("seed");
    fs::write(&seed, "07".repeat(32)).unwrap();
    let mut public = String::new();
    for (name,metadata,source) in [
        ("math",r#"{"name":"math","version":"1.2.3","effects":["output"],"dependencies":{"util":"^2.0"}}"#,"effects {output}pub fn answer()->Int effects {}{return 42;}pub fn unused()->Unit effects {output}{Out.println(1);}"),
        ("util",r#"{"name":"util","version":"2.0.1","effects":[]}"#,"pub fn empty()->Unit effects {} {}")
    ]{let path=root.join(format!("vendor/{name}"));fs::create_dir_all(&path).unwrap();fs::write(path.join("rewind.package.json"),metadata).unwrap();fs::write(path.join("api.rw"),source).unwrap();let sign=Command::new(env!("CARGO_BIN_EXE_rewind")).arg("sign").arg(&path).arg("--key").arg(&seed).output().unwrap();assert!(sign.status.success());public=String::from_utf8(sign.stdout).unwrap();}
    let manifest=format!("language = \"0.5\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"\"\n[dependencies]\nmath = \"file:vendor/math\"\n[dependency_versions]\nmath = \"^1.2.0\"\n[registry]\nutil = \"file:vendor/util\"\n[dependency_signers]\nmath = \"author\"\nutil = \"author\"\n[trust]\nauthor = \"{}\"\n",public.trim());
    fs::write(root.join("rewind.toml"), &manifest).unwrap();
    let update = cmd("update", &root, &[]);
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    assert!(String::from_utf8_lossy(&update.stderr).contains("selected util"));
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("rewind.lock")).unwrap()).unwrap();
    assert_eq!(lock["dependencies"]["util"]["requirement"], "^2.0");
    let run = cmd("run", &root, &[]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    fs::write(
        root.join("rewind.toml"),
        manifest.replacen("[dependencies]", "revoked = \"author\"\n[dependencies]", 1),
    )
    .unwrap();
    let run = cmd("run", &root, &[]);
    assert!(!run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("revoked package signer"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn migration_is_previewed_and_only_written_explicitly() {
    let root = fixture("assert_eq(1,1);", "");
    let manifest = root.join("rewind.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest).unwrap().replace("0.5", "0.4"),
    )
    .unwrap();
    let before = fs::read(&manifest).unwrap();
    let preview = cmd("migrate", &root, &[]);
    assert!(preview.status.success());
    assert_eq!(fs::read(&manifest).unwrap(), before);
    let write = cmd("migrate", &root, &["--write"]);
    assert!(
        write.status.success(),
        "{}",
        String::from_utf8_lossy(&write.stderr)
    );
    assert!(fs::read_to_string(&manifest).unwrap().contains("0.5"));
    assert!(cmd("run", &root, &[]).status.success());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn artifact_bytes_are_root_independent_and_verified_after_rehash() {
    use sha2::{Digest, Sha256};
    let source = "pub fn pure()->Int effects {}{return 7;}assert_eq(pure(),7);";
    let a = fixture(source, "");
    let b = fixture(source, "");
    assert!(cmd("build", &a, &[]).status.success());
    assert!(cmd("build", &b, &[]).status.success());
    let bytes = fs::read(a.join("rewind.build.json")).unwrap();
    assert_eq!(bytes, fs::read(b.join("rewind.build.json")).unwrap());
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["payload"]["build"]["bytecode"][0]["operation"] = "Jump(99999)".into();
    let hash = Sha256::digest(serde_json::to_vec(&json["payload"]).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    json["sha256"] = hash.into();
    let path = a.join("bad.json");
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run-artifact")
        .arg(path)
        .arg("--root")
        .arg(&a)
        .output()
        .unwrap();
    assert!(!run.status.success());
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("bytecode/type IR mismatch"),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    fs::remove_dir_all(a).unwrap();
    fs::remove_dir_all(b).unwrap();
}

#[test]
fn cleanup_causes_generic_send_and_function_value_effects_are_checked() {
    let root = fixture(
        "fn fail()->Unit{defer ||->Unit{panic(\"first cleanup\");};defer ||->Unit{panic(\"second cleanup\");};panic(\"original\");}fail();",
        "",
    );
    let output = cmd("run", &root, &[]);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("original")
            && error.contains("first cleanup")
            && error.contains("second cleanup"),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
    ok(
        "async fn id<T:Send>(n:T)->T{return n;}let t=spawn id(7);assert_eq(await t,Ok(7));",
        "tasks",
        b"",
    );
    let root = fixture(
        "fn pure()->Unit{}fn noisy()->Unit{Out.println(1);}var action=pure;action=noisy;action();",
        "",
    );
    let check = cmd("check", &root, &[]);
    assert!(!check.status.success());
    assert!(String::from_utf8_lossy(&check.stderr).contains("not allowed"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn effect_checks_follow_shadowed_handles_and_function_parameters() {
    let root = fixture(
        "using h=File.open(\"file\");h.write(\"x\");{let h=List<Int>();}",
        "fileRead",
    );
    let output = cmd("check", &root, &[]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("effect fileWrite is not allowed"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
    let root=fixture("fn pure()->Unit{}fn noisy()->Unit{Out.println(1);}let action=pure;pub fn apply(action:fn()->Unit)->Unit effects {}{action();}apply(noisy);", "");
    let output = cmd("check", &root, &[]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("undeclared effect"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
    let root=fixture("fn pure()->Unit{}fn noisy()->Unit{Out.println(1);}let action=pure;{let action=noisy;action();}","");
    let output = cmd("check", &root, &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not allowed"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn group_failure_cancels_and_self_cancellation_is_cooperative() {
    ok("async fn waiting()->Unit{while true {}}fn parent()->Result<Unit,String>{using group=TaskGroup();group.add(waiting());return Err(\"business\");}assert_eq(parent(),Err(\"business\"));","tasks",b"");
    ok("async fn worker(c:Channel<Task<Unit>>)->Unit{match await c.receive(){Ok(t)=>t.cancel(),Err(_)=>panic(\"receive\"),}panic(\"must cancel before this instruction\");}let c=Channel<Task<Unit>>(1);let t=spawn worker(c);await c.send(t);assert_eq(await t,Err(TaskError::Cancelled));","tasks",b"");
}
