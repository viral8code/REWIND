use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn cmd(mode: &str, root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(mode)
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn success(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn fixture(source: &str, effects: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v091-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(root.join("rewind.toml"),format!("language = \"0.9.1\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"{effects}\"\n")).unwrap();
    success(&cmd("update", &root, &[]));
    root
}
fn error_case(input: &str, code: &str) -> String {
    format!("match jsonParse({}) {{Err(e)=>{{assert_eq(e.code,{});}},Ok(_)=>{{panic(\"expected parse error\");}}}}",serde_json::to_string(input).unwrap(),serde_json::to_string(code).unwrap())
}
#[test]
fn json_ast_is_immutable_canonical_and_recordable() {
    let root = fixture(
        r#"record Payload{data:Json}pub const DATA:Json=Json::Int(2);let text="{\"z\":[true,null,-3,1.5],\"a\":\"日本語\"}";match jsonParse(text){Ok(v)=>{let p=Payload(v);assert_eq(jsonKind(p.data),"Object");assert_eq(jsonGet(p.data,"missing"),None);assert_eq(jsonKeys(p.data).get(0),"a");assert_eq(jsonText(Json::Text("ok")),Some("ok"));assert_eq(jsonInt(Json::Int(4)),Some(4));match jsonStringify(p.data){Ok(s)=>{assert_eq(s,"{\"a\":\"日本語\",\"z\":[true,null,-3,1.5]}");},Err(_)=>{panic("unexpected stringify error");}}},Err(_)=>{panic("unexpected parse error");}}"#,
        "",
    );
    let trace = root.join("trace.json");
    success(&cmd("run", &root, &["--record", trace.to_str().unwrap()]));
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    success(&replay);
    let artifact = root.join("program.json");
    success(&cmd(
        "build",
        &root,
        &["--output", artifact.to_str().unwrap()],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run-artifact")
        .arg(&artifact)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    success(&run);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn json_reports_duplicates_range_syntax_and_depth_without_input_values() {
    let mut source = String::new();
    for (input, code) in [
        ("{\"a\":1,\"a\":2}", "DuplicateKey"),
        ("9223372036854775808", "NumberRange"),
        ("1e400", "NumberRange"),
        ("01", "Syntax"),
        ("1.", "Syntax"),
        ("[1,]", "Syntax"),
        ("\"\\ud800\"", "Syntax"),
    ] {
        source += &error_case(input, code);
    }
    source += &error_case(&("[".repeat(65) + "0" + &"]".repeat(65)), "DepthLimit");
    source += r#"match jsonParse("{\n \"password\": \"secret-value\",\n ?}"){Err(e)=>{assert_eq(e.line,3);assert(e.column>0);assert(e.offset>0);},Ok(_)=>{panic("expected error");}}assert_eq(jsonStringify(Json::Float(1.0)),Ok("1.0"));"#;
    let root = fixture(&source, "");
    success(&cmd("run", &root, &[]));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn json_utf8_byte_and_item_budgets_are_separate() {
    let root = fixture(
        r#"fn expect(path:String,code:String) effects {fileRead}{match File.readBytes(path){Ok(b)=>{match jsonParseBytes(b){Err(e)=>{assert_eq(e.code,code);},Ok(_)=>{panic("expected failure");}}},Err(_)=>{panic("fixture missing");}}}expect("utf8","Utf8");expect("large","ByteLimit");expect("items","ItemLimit");"#,
        "fileRead",
    );
    fs::write(root.join("utf8"), b"\n\xff").unwrap();
    fs::write(root.join("large"), vec![b' '; 1024 * 1024 + 1]).unwrap();
    fs::write(
        root.join("items"),
        format!("[{}]", vec!["0"; 65_536].join(",")),
    )
    .unwrap();
    success(&cmd("run", &root, &[]));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn application_main_status_diagnostics_and_secret_redaction() {
    let root = fixture(
        "fn main()->Int effects {}{return 7;}test fn independent(){assert_eq(2,2);}",
        "",
    );
    let o = cmd("run", &root, &["--diagnostic-format", "json"]);
    assert_eq!(o.status.code(), Some(7));
    let d: Value = serde_json::from_slice(&o.stderr).unwrap();
    assert_eq!(d["exit_status"], 7);
    assert_eq!(d["diagnostic"]["code"], "ApplicationExit7");
    success(&cmd("test", &root, &[]));
    fs::write(
        root.join("main.rw"),
        "let token=secret(\"private-token\");appStatus(2,reveal(token));",
    )
    .unwrap();
    let o = cmd("run", &root, &["--diagnostic-format", "json"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&o.stderr).contains("private-token"));
    fs::write(
        root.join("main.rw"),
        "fn main()->Int effects {}{panic(\"failed\");}",
    )
    .unwrap();
    assert_eq!(cmd("run", &root, &[]).status.code(), Some(70));
    fs::write(root.join("main.rw"), "appStatus(64,\"bad\");").unwrap();
    assert_eq!(cmd("run", &root, &[]).status.code(), Some(65));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn libraries_are_compiled_and_their_contract_tests_execute() {
    let root = fixture("", "");
    for entry in
        fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stdlib-v091"))
            .unwrap()
    {
        let p = entry.unwrap().path();
        if p.extension().is_some_and(|e| e == "rw") {
            fs::copy(&p, root.join(p.file_name().unwrap())).unwrap();
        }
    }
    fs::write(root.join("main.rw"), "import tests; ").unwrap();
    success(&cmd("test", &root, &[]));
    for name in ["json.rw", "config.rw", "args.rw"] {
        assert_eq!(
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/stdlib-v091")
                    .join(name)
            )
            .unwrap(),
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("examples/v091/lib")
                    .join(name)
            )
            .unwrap()
        );
    }
    fs::remove_dir_all(root).unwrap();
}
fn example() -> PathBuf {
    let root = fixture("", "args,env,fileRead,fileWrite,output");
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/v091");
    fs::copy(base.join("main.rw"), root.join("main.rw")).unwrap();
    fs::copy(base.join("rewind.toml"), root.join("rewind.toml")).unwrap();
    for dir in ["lib", "assets"] {
        fs::create_dir(root.join(dir)).unwrap();
        for entry in fs::read_dir(base.join(dir)).unwrap() {
            let p = entry.unwrap().path();
            fs::copy(&p, root.join(dir).join(p.file_name().unwrap())).unwrap();
        }
    }
    success(&cmd("update", &root, &[]));
    root
}
#[test]
fn offline_app_persists_and_replays_observations_without_overwriting_host_state() {
    let root = example();
    let trace = root.join("trace.json");
    let o = cmd(
        "run",
        &root,
        &[
            "--allow-env",
            "REWIND_COUNT",
            "--record",
            trace.to_str().unwrap(),
            "--",
            "--count",
            "4",
        ],
    );
    success(&o);
    assert_eq!(o.stdout, b"5\n");
    assert_eq!(
        fs::read_to_string(root.join("state.json")).unwrap(),
        "{\"count\":5}"
    );
    let o = cmd("run", &root, &["--allow-env", "REWIND_COUNT"]);
    success(&o);
    assert_eq!(o.stdout, b"6\n");
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .arg("--allow-env")
        .arg("REWIND_COUNT")
        .output()
        .unwrap();
    success(&replay);
    assert_eq!(
        fs::read_to_string(root.join("state.json")).unwrap(),
        "{\"count\":6}"
    );
    let o = cmd(
        "run",
        &root,
        &[
            "--allow-env",
            "REWIND_COUNT",
            "--",
            "--unknown",
            "private-token",
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&o.stderr).contains("private-token"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn production_assets_release_signature_and_source_free_artifact_verify_integrity() {
    let root = example();
    let output = root.join("release");
    success(&cmd(
        "install",
        &root,
        &["--production", "--output", output.to_str().unwrap()],
    ));
    assert!(output.join("assets/config.json").exists());
    assert!(output.join("lib/config.rw").exists());
    assert!(!output.join("state.json").exists());
    let artifact = root.join("artifact.json");
    success(&cmd(
        "build",
        &output,
        &["--output", artifact.to_str().unwrap()],
    ));
    let seed = root.join("signer.seed");
    let key = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("keygen")
        .arg(&seed)
        .output()
        .unwrap();
    success(&key);
    let public_text = String::from_utf8(key.stdout).unwrap();
    let public = public_text.trim();
    let release = output.join("rewind.release.json");
    let sign = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("sign-release")
        .arg(&release)
        .arg("--key")
        .arg(&seed)
        .output()
        .unwrap();
    success(&sign);
    let verify = || {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .arg("verify-release")
            .arg(&release)
            .arg("--public-key")
            .arg(public)
            .output()
            .unwrap()
    };
    success(&verify());
    let original_asset = fs::read(output.join("assets/config.json")).unwrap();
    fs::write(output.join("assets/config.json"), "{\"count\":99}").unwrap();
    assert!(!verify().status.success());
    assert!(!cmd("run", &output, &["--allow-env", "REWIND_COUNT"])
        .status
        .success());
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .arg("run-artifact")
            .arg(&artifact)
            .arg("--root")
            .arg(&output)
            .arg("--allow-effects")
            .arg("args,env,fileRead,fileWrite,output")
            .arg("--allow-env")
            .arg("REWIND_COUNT")
            .output()
            .unwrap()
    };
    assert!(!run().status.success());
    fs::write(output.join("assets/config.json"), original_asset).unwrap();
    success(&verify());
    fs::remove_file(output.join("main.rw")).unwrap();
    success(&run());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn asset_allowlist_rejects_missing_traversal_and_symlinks() {
    let root = fixture("", "");
    let manifest = fs::read_to_string(root.join("rewind.toml")).unwrap();
    for path in [
        "../escape.json",
        "missing.json",
        "main.rw",
        ".rewind/cache/x",
    ] {
        fs::write(
            root.join("rewind.toml"),
            format!(
                "{manifest}[assets]\na = {}\n",
                serde_json::to_string(path).unwrap()
            ),
        )
        .unwrap();
        assert!(!cmd("update", &root, &[]).status.success());
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("main.rw"), root.join("link.json")).unwrap();
        fs::write(
            root.join("rewind.toml"),
            format!("{manifest}[assets]\na = \"link.json\"\n"),
        )
        .unwrap();
        assert!(!cmd("update", &root, &[]).status.success());
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn application_publish_conflict_and_partial_failure_have_distinct_codes() {
    // Host file conflict is detected before replacement; partial publication is
    // sticky and must never be treated as automatically retryable.
    let root = fixture("", "");
    let file = root.join("state.json");
    fs::write(&file, "old").unwrap();
    let mut rt = rewind::Runtime::new(&root).unwrap();
    rt.read_file("state.json").unwrap();
    rt.write_file("state.json", b"new").unwrap();
    fs::write(&file, "other").unwrap();
    assert!(matches!(
        rt.publish(false, &mut Vec::new(), &mut Vec::new()),
        Err(rewind::Error::ExternalStateConflict(_))
    ));
    assert_eq!(fs::read(&file).unwrap(), b"other");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn partially_applied_publish_is_not_retryable_and_reports_exit_73() {
    use std::process::Stdio;
    let root = fixture(
        "fn main()->Int effects {fileWrite,output}{defer ||->Unit{panic(\"cleanup failed\");};File.writeText(\"state.json\",\"applied\");Out.println(\"output\");publish;return 0;}",
        "fileWrite,output",
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg("--root")
        .arg(&root)
        .arg("--diagnostic-format")
        .arg("json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let o = child.wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(73),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let d: Value = serde_json::from_slice(&o.stderr).unwrap();
    assert_eq!(d["diagnostic"]["code"], "PublishPartiallyApplied");
    assert_eq!(d["retryable"], false);
    assert_eq!(fs::read(root.join("state.json")).unwrap(), b"applied");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn json_primitives_reject_secrets_and_old_language_and_validate_artifact_layout() {
    let root = fixture("jsonStringify(secret(\"private-token\"));", "");
    assert!(!cmd("check", &root, &[]).status.success());
    fs::write(root.join("main.rw"), "let j=Json::Int(2);").unwrap();
    let artifact = root.join("program.json");
    success(&cmd(
        "build",
        &root,
        &["--output", artifact.to_str().unwrap()],
    ));
    let mut data: Value = serde_json::from_slice(&fs::read(&artifact).unwrap()).unwrap();
    data["payload"]["program"]["enums"]["Json"]["variants"]["Int"] =
        serde_json::json!([["0", "String"]]);
    use sha2::Digest;
    data["sha256"] = serde_json::json!(format!(
        "{:x}",
        sha2::Sha256::digest(serde_json::to_vec(&data["payload"]).unwrap())
    ));
    fs::write(&artifact, serde_json::to_vec(&data).unwrap()).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run-artifact")
        .arg(&artifact)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!run.status.success());
    fs::write(
        root.join("rewind.toml"),
        "language = \"0.9\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"\"\n",
    )
    .unwrap();
    fs::write(root.join("main.rw"), "jsonParse(\"null\");").unwrap();
    success(&cmd("update", &root, &[]));
    assert!(!cmd("check", &root, &[]).status.success());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn json_nested_ast_replays_with_documented_depth() {
    let input = "[".repeat(64) + "0" + &"]".repeat(64);
    let source = format!(
        "let deep=jsonParse({});",
        serde_json::to_string(&input).unwrap()
    );
    let root = fixture(&source, "");
    let trace = root.join("deep.json");
    success(&cmd("run", &root, &["--record", trace.to_str().unwrap()]));
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    success(&replay);
    fs::remove_dir_all(root).unwrap();
}
