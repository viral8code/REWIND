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
fn success(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn fixture(source: &str, effects: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v08-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(root.join("rewind.toml"),format!("language = \"0.8\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"{effects}\"\n")).unwrap();
    success(&cmd("update", &root, &[]));
    root
}
fn repl(root: &Path, source: &str) -> Vec<Value> {
    let mut child = command("repl")
        .arg("--root")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    success(&out);
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
#[test]
fn repl_rolls_back_failed_heap_file_and_checkpoint_changes() {
    let root = fixture("", "fileRead,fileWrite,output");
    let rows=repl(&root,"var n=1;let xs=List<Int>();xs.add(7);commit saved;\nn=9;xs.add(8);File.writeText(\"failed\",\"bad\");commit failed;assert_eq(1,2);\nassert_eq(n,1);assert_eq(xs.len(),1);File.writeText(\"ok\",\"value\");Out.println(n);publish;\nn=4;\nrevert saved;assert_eq(n,1);assert_eq(xs.len(),1);\n:state\n:quit\n");
    assert_eq!(rows[0]["ok"], true);
    assert_eq!(rows[1]["ok"], false);
    assert_eq!(rows[1]["state"], rows[0]["state"]);
    assert!(rows[1]["diagnostic"]["code"].is_string());
    assert_eq!(rows[2]["ok"], true);
    assert_eq!(rows[2]["state"]["stdout"], "1\n");
    assert_eq!(rows[4]["ok"], true);
    assert_eq!(rows[4]["state"]["heap"]["1"], "1");
    assert!(!root.join("failed").exists());
    assert!(!root.join("ok").exists());
    assert!(!rows[5]["state"]["checkpoints"]
        .as_array()
        .unwrap()
        .contains(&json!("failed")));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn repl_type_errors_do_not_reserve_bindings_and_reset_clears_session() {
    let root = fixture("", "");
    let rows=repl(&root,"fn inc(n:Int)->Int effects {}{return n+1;}\nlet n=inc(2);\nlet broken:Int=\"wrong\";\nlet broken=4;assert_eq(broken,4);\nfn later()->Int effects {}{return 1;}\n:reset\nlet n=9;assert_eq(n,9);\n:quit\n");
    assert_eq!(rows[0]["ok"], true);
    assert_eq!(rows[1]["ok"], true);
    assert_eq!(rows[2]["ok"], false);
    assert_eq!(rows[3]["ok"], true);
    assert!(rows[4]["error"]
        .as_str()
        .unwrap()
        .contains("declarations must precede"));
    assert_eq!(rows[5]["reset"], true);
    assert_eq!(rows[6]["ok"], true);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn repl_reuses_observations_after_failure_and_enforces_budget() {
    let root = fixture("", "fileRead,input,clock");
    fs::write(root.join("data"), "one").unwrap();
    // Interact while changing the host fixture after its first observation.
    let mut child = command("repl")
        .arg("--root")
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
    use std::io::BufRead;
    input
        .write_all(b"let first=File.readText(\"data\");assert_eq(first,Ok(\"one\"));\n")
        .unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["ok"], true);
    fs::write(root.join("data"), "two").unwrap();
    input.write_all(b"assert_eq(first,Ok(\"one\"));assert_eq(File.readText(\"data\"),Ok(\"one\"));\nwhile true{}\nassert_eq(first,Ok(\"one\"));assert_eq(In.readLine(),None);\n:quit\n").unwrap();
    drop(input);
    let mut rows = Vec::new();
    for line in output.lines() {
        rows.push(serde_json::from_str::<Value>(&line.unwrap()).unwrap());
    }
    assert!(child.wait().unwrap().success());
    assert_eq!(rows[0]["ok"], true);
    assert!(rows[1]["error"]
        .as_str()
        .unwrap()
        .contains("ExecutionBudgetExceeded"));
    assert_eq!(rows[2]["ok"], true);
    fs::remove_dir_all(root).unwrap();
}
fn dev_fixture() -> PathBuf {
    let root = fixture(
        "import helper.api;test fn helper_works(){assert_eq(answer(),42);}",
        "",
    );
    let package = root.join("vendor/helper");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("rewind.package.json"),
        r#"{"name":"helper","version":"1.0.0","effects":[]}"#,
    )
    .unwrap();
    fs::write(
        package.join("api.rw"),
        "pub fn answer()->Int effects {}{return 42;}",
    )
    .unwrap();
    let seed = root.join("seed");
    fs::write(&seed, "07".repeat(32)).unwrap();
    let signed = command("sign")
        .arg(&package)
        .arg("--key")
        .arg(&seed)
        .output()
        .unwrap();
    success(&signed);
    let public = String::from_utf8(signed.stdout).unwrap();
    fs::write(root.join("rewind.toml"),format!("language = \"0.8\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"\"\n[dev_dependencies]\nhelper = \"file:vendor/helper\"\n[dependency_versions]\nhelper = \"^1.0\"\n[dependency_signers]\nhelper = \"author\"\n[trust]\nauthor = \"{}\"\n",public.trim())).unwrap();
    success(&cmd("update", &root, &[]));
    root
}
#[test]
fn dev_dependencies_are_test_only_and_share_one_verified_lock() {
    let root = dev_fixture();
    let lock = fs::read(root.join("rewind.lock")).unwrap();
    assert!(!cmd("run", &root, &[]).status.success());
    assert!(!cmd("build", &root, &[]).status.success());
    let recording = root.join("test-trace.json");
    success(&cmd(
        "test",
        &root,
        &["--record", recording.to_str().unwrap()],
    ));
    success(&cmd("replay", &root, &[recording.to_str().unwrap()]));
    fs::write(
        root.join("manual.md"),
        "```rewind\nimport helper.api;assert_eq(answer(),42);\n```\n",
    )
    .unwrap();
    success(&cmd(
        "doctest",
        &root,
        &[root.join("manual.md").to_str().unwrap()],
    ));
    fs::write(
        root.join("main.rw"),
        "import vendor.helper.api;assert_eq(answer(),42);",
    )
    .unwrap();
    let out = cmd("run", &root, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("development-only module"));
    fs::write(root.join("main.rw"), "assert_eq(1,1);").unwrap();
    success(&cmd("run", &root, &[]));
    assert_eq!(fs::read(root.join("rewind.lock")).unwrap(), lock);
    fs::write(
        root.join("vendor/helper/api.rw"),
        "pub fn answer()->Int effects {}{return 99;}",
    )
    .unwrap();
    let out = cmd("run", &root, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("signature verification failed"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn runtime_dependencies_can_reach_dev_named_transitive_packages() {
    let root = dev_fixture();
    let package = root.join("vendor/app");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("rewind.package.json"),
        r#"{"name":"app","version":"1.0.0","effects":[],"dependencies":{"helper":"^1.0"}}"#,
    )
    .unwrap();
    fs::write(
        package.join("api.rw"),
        "import helper.api;pub fn result()->Int effects {}{return answer();}",
    )
    .unwrap();
    success(
        &command("sign")
            .arg(&package)
            .arg("--key")
            .arg(root.join("seed"))
            .output()
            .unwrap(),
    );
    let manifest = fs::read_to_string(root.join("rewind.toml"))
        .unwrap()
        .replace(
            "[dev_dependencies]",
            "[dependencies]\napp = \"file:vendor/app\"\n[dev_dependencies]",
        )
        .replace(
            "[dependency_versions]",
            "[dependency_versions]\napp = \"^1.0\"",
        )
        .replace(
            "[dependency_signers]",
            "[dependency_signers]\napp = \"author\"",
        );
    fs::write(root.join("rewind.toml"), manifest).unwrap();
    success(&cmd("update", &root, &[]));
    fs::write(
        root.join("main.rw"),
        "import app.api;assert_eq(result(),42);",
    )
    .unwrap();
    success(&cmd("run", &root, &[]));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn previous_compiler_trace_is_inspectable_but_not_replayable() {
    let root = fixture("", "");
    let trace = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v08-indexed.trace.json");
    let bytes = fs::read(&trace).unwrap();
    let out = command("timeline").arg(&trace).output().unwrap();
    success(&out);
    let out = command("compatibility").arg(&trace).output().unwrap();
    success(&out);
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["viewable"], true);
    assert_eq!(value["replayable"], false);
    assert!(!cmd("replay", &root, &[trace.to_str().unwrap()])
        .status
        .success());
    let mut child = command("debug-session")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"step\nback\nstate\nquit\n")
        .unwrap();
    success(&child.wait_with_output().unwrap());
    assert_eq!(fs::read(&trace).unwrap(), bytes);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn inspection_export_omits_execution_data_and_never_overwrites_source() {
    let root = fixture("", "");
    let trace = root.join("old.json");
    fs::write(&trace, include_bytes!("fixtures/v08-indexed.trace.json")).unwrap();
    let original = fs::read(&trace).unwrap();
    let export = root.join("inspection.json");
    let out = command("trace-export")
        .arg(&trace)
        .arg("--output")
        .arg(&export)
        .output()
        .unwrap();
    success(&out);
    let value: Value = serde_json::from_slice(&fs::read(&export).unwrap()).unwrap();
    assert_eq!(value["kind"], "rewind-inspection");
    assert_eq!(value["source_compiler"], "0.8.0");
    success(&command("timeline").arg(&export).output().unwrap());
    assert!(value.get("observations").is_none());
    assert!(value.get("fingerprint").is_none());
    assert_eq!(
        value["source_sha256"],
        format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(&original))
    );
    let compat = command("compatibility").arg(&export).output().unwrap();
    success(&compat);
    assert_eq!(
        serde_json::from_slice::<Value>(&compat.stdout).unwrap()["viewable"],
        true
    );
    assert!(!cmd("replay", &root, &[export.to_str().unwrap()])
        .status
        .success());
    assert!(!cmd("run-artifact", &root, &[export.to_str().unwrap()])
        .status
        .success());
    assert!(!command("trace-export")
        .arg(&trace)
        .arg("--output")
        .arg(&trace)
        .output()
        .unwrap()
        .status
        .success());
    assert!(!command("trace-export")
        .arg(&trace)
        .arg("--output")
        .arg(&export)
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(fs::read(&trace).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn v08_preserves_v07_records_const_properties_and_verified_artifacts() {
    let root=fixture("record Point{x:Int}const P:Point=Point(7);assert_eq(P.x,7);assert_eq(property(7,1,|bits:Int|->Int{return 4;},|x:Int|->Int{return x/2;},|x:Int|->Bool{return true;}),Ok(()));", "");
    success(&cmd("run", &root, &[]));
    let artifact = root.join("program.json");
    success(&cmd(
        "build",
        &root,
        &["--output", artifact.to_str().unwrap()],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    success(&cmd("run-artifact", &root, &[artifact.to_str().unwrap()]));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn const_indirect_calls_cannot_escape_boundary_checks() {
    let root=fixture("fn boundary()->Int effects {}{commit saved;return 1;}fn wrapper()->Int effects {}{let callback=boundary;return callback();}const N:Int=wrapper();","");
    let out = cmd("check", &root, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("const cannot use checkpoint"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn signed_inspection_export_preserves_secret_masking_and_rejects_tampering() {
    let root=fixture("match In.readSecretLine(){Some(s)=>{let plain=reveal(s);Out.println(plain);},None=>{panic(\"missing\");}}","input,output");
    let secret = root.join("secret-input");
    fs::write(&secret, "private-v08-fixture-token\n").unwrap();
    let trace = root.join("trace.json");
    success(&cmd(
        "run",
        &root,
        &[
            "--record",
            trace.to_str().unwrap(),
            "--secret-input",
            secret.to_str().unwrap(),
        ],
    ));
    let seed = root.join("seed");
    fs::write(&seed, "09".repeat(32)).unwrap();
    let signed = command("sign-trace")
        .arg(&trace)
        .arg("--key")
        .arg(&seed)
        .output()
        .unwrap();
    success(&signed);
    let public = String::from_utf8(signed.stdout).unwrap();
    let exported = root.join("inspection.json");
    success(
        &command("trace-export")
            .arg(&trace)
            .arg("--output")
            .arg(&exported)
            .arg("--public-key")
            .arg(public.trim())
            .output()
            .unwrap(),
    );
    let text = fs::read_to_string(&exported).unwrap();
    assert!(!text.contains("private-v08-fixture-token"));
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap()["source_signature_verified"],
        true
    );
    let mut value: Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
    value["debug"]["index"]["deltas"][0] = json!([{"path":42}]);
    fs::write(&trace, serde_json::to_vec(&value).unwrap()).unwrap();
    let bad = root.join("bad.json");
    assert!(!command("trace-export")
        .arg(&trace)
        .arg("--output")
        .arg(&bad)
        .arg("--public-key")
        .arg(public.trim())
        .output()
        .unwrap()
        .status
        .success());
    assert!(!command("trace-export")
        .arg(&trace)
        .arg("--output")
        .arg(&bad)
        .output()
        .unwrap()
        .status
        .success());
    assert!(!bad.exists());
    fs::remove_dir_all(root).unwrap();
}
