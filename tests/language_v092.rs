use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn temp() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v092-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn cmd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(args)
        .output()
        .unwrap()
}
fn success(out: &Output) {
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
fn project(root: &Path, source: &str, effects: &str) {
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(root.join("rewind.toml"),format!("language = \"0.9.2\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"{effects}\"\n")).unwrap();
    success(&cmd(&["update", "--root", root.to_str().unwrap()]));
}
#[test]
fn codec_boundaries_are_typed_and_preserve_unicode_positions() {
    let root = temp();
    project(
        &root,
        r#"
 assert_eq(stdTextLength("a日本"),3);assert_eq(stdTextSlice("a日本",1,3),Ok("日本"));assert_eq(stdTextFind("a日本","本"),Some(2));assert_eq(stdTextFind("日本",""),Some(0));
 assert_eq(stdParseInt("-9223372036854775808",10),Ok(-9223372036854775807-1));assert_eq(stdFormatInt(-9223372036854775807-1,16),Ok("-8000000000000000"));assert_eq(stdShiftLeft(1,63),Ok(-9223372036854775807-1));assert_eq(stdShiftRight(-4,1),Ok(-2));assert_eq(stdShiftUnsigned(-1,63),Ok(1));assert_eq(stdMulMod(-3,4,5),Ok(3));
 match stdTextSplit("a",""){Err(e)=>{assert_eq(e.code,"EmptyDelimiter");},Ok(_)=>{panic("expected failure");}}
 match stdTextSlice("日本",0,3){Err(e)=>{assert_eq(e.code,"Range");},Ok(_)=>{panic("expected failure");}}
 match stdParseInt(" 1",10){Err(e)=>{assert_eq(e.code,"InvalidNumber");},Ok(_)=>{panic("expected failure");}}
 match stdParseInt("1",1){Err(e)=>{assert_eq(e.code,"Radix");},Ok(_)=>{panic("expected failure");}}
 match stdParseFloat("NaN"){Err(e)=>{assert_eq(e.code,"NumberRange");},Ok(_)=>{panic("expected failure");}}
 match stdMulMod(1,2,0){Err(e)=>{assert_eq(e.code,"Modulus");},Ok(_)=>{panic("expected failure");}}
 match File.readBytes("invalid"){Ok(b)=>{match stdDecode(b){Err(e)=>{assert_eq(e.code,"Utf8");assert_eq(e.offset,1);},Ok(_)=>{panic("expected failure");}}},Err(_)=>{panic("missing fixture");}}
 match File.readText("large"){Ok(s)=>{match stdTextTokens(s){Err(e)=>{assert_eq(e.code,"Limit");},Ok(_)=>{panic("expected failure");}}},Err(_)=>{panic("missing fixture");}}
 "#,
        "fileRead",
    );
    fs::write(root.join("invalid"), b"a\xff").unwrap();
    fs::write(root.join("large"), vec![b' '; 1024 * 1024 + 1]).unwrap();
    success(&cmd(&["run", "--root", root.to_str().unwrap()]));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn generic_result_types_survive_branches_closures_cache_and_artifacts() {
    let root = temp();
    project(
        &root,
        r#"
 fn value<T:Share,E:Share>(r:Result<T,E>,fallback:T)->T effects {} {match r {Ok(v)=>{return v;},Err(_)=>{return fallback;}}}
 fn first<T:Send>(xs:Frozen<List<T>>)->T effects {} {return thaw(xs.get(0));}
 let xs=List<Int>();xs.add(9);assert_eq(first(freeze(xs)),9);
 fn nested<T:Share,E:Share>(r:Result<T,E>,fallback:T)->T effects {} {return value(r,fallback);}
 let good:Result<Int,String>=Ok(3);let bad:Result<Int,String>=Err("no");assert_eq(nested(good,0),3);assert_eq(nested(bad,0),0);
 let task=|n:Int|->Int {return value(bad,n);};assert_eq(task(7),7);
 let container:Option<Result<Int,String>>=Some(bad);match container {Some(r)=>{assert_eq(value(r,8),8);},None=>{panic("missing");}}
 "#,
        "",
    );
    let r = root.to_str().unwrap();
    success(&cmd(&["run", "--root", r]));
    success(&cmd(&["run", "--root", r]));
    let artifact = root.join("app.json");
    success(&cmd(&[
        "build",
        "--root",
        r,
        "--output",
        artifact.to_str().unwrap(),
    ]));
    fs::remove_file(root.join("main.rw")).unwrap();
    success(&cmd(&[
        "run-artifact",
        artifact.to_str().unwrap(),
        "--root",
        r,
    ]));
    let mut corrupted: Value = serde_json::from_slice(&fs::read(&artifact).unwrap()).unwrap();
    corrupted["payload"]["program"]["structs"]["StdError"]["fields"][0][1] =
        Value::String("Int".into());
    use sha2::{Digest, Sha256};
    corrupted["sha256"] = Value::String(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&corrupted["payload"]).unwrap())
    ));
    fs::write(&artifact, serde_json::to_vec(&corrupted).unwrap()).unwrap();
    let rejected = cmd(&["run-artifact", artifact.to_str().unwrap(), "--root", r]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("standard primitive layout"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn mutable_payloads_cannot_be_aliased_by_result_annotation() {
    let root = temp();
    project(
        &root,
        "let xs=List<Int>();let r:Result<List<Int>,String>=Ok(move xs);let alias=r;",
        "",
    );
    assert!(!cmd(&["check", "--root", root.to_str().unwrap()])
        .status
        .success());
    fs::write(root.join("main.rw"), "assert_eq(stdBitAnd(3,1),1);").unwrap();
    let manifest = fs::read_to_string(root.join("rewind.toml")).unwrap();
    fs::write(root.join("rewind.toml"), manifest.replace("0.9.2", "0.9.1")).unwrap();
    success(&cmd(&["update", "--root", root.to_str().unwrap()]));
    assert!(!cmd(&["check", "--root", root.to_str().unwrap()])
        .status
        .success());
    fs::write(root.join("rewind.toml"), manifest).unwrap();
    fs::write(root.join("main.rw"), "record stdParseInt {value:Int}").unwrap();
    success(&cmd(&["update", "--root", root.to_str().unwrap()]));
    assert!(!cmd(&["check", "--root", root.to_str().unwrap()])
        .status
        .success());
    fs::remove_dir_all(root).unwrap();
}
fn sdk_project(root: &Path, source: &str, effects: &str) {
    project(root, source, effects);
    let manifest = fs::read_to_string(root.join("rewind.toml")).unwrap();
    fs::write(root.join("rewind.toml"), manifest.replace("0.9.2", "0.9.4")).unwrap();
}
#[test]
fn signed_sdk_is_reproducible_installable_and_detects_tampering() {
    let root = temp();
    let key = root.join("key");
    let public_out = cmd(&["keygen", key.to_str().unwrap()]);
    success(&public_out);
    let public = String::from_utf8(public_out.stdout)
        .unwrap()
        .trim()
        .to_string();
    let sdk = root.join("sdk");
    let sdk2 = root.join("sdk2");
    for out in [&sdk, &sdk2] {
        success(&cmd(&[
            "sdk-build",
            "--output",
            out.to_str().unwrap(),
            "--key",
            key.to_str().unwrap(),
        ]));
    }
    let text_api: Value =
        serde_json::from_slice(&fs::read(sdk.join("share/rewind/doc/std/text.api.json")).unwrap())
            .unwrap();
    assert!(text_api.to_string().contains("fn:trim"));
    assert_eq!(
        fs::read(sdk.join("sdk.json")).unwrap(),
        fs::read(sdk2.join("sdk.json")).unwrap()
    );
    assert_eq!(
        fs::read(sdk.join("sdk.json.signature")).unwrap(),
        fs::read(sdk2.join("sdk.json.signature")).unwrap()
    );
    assert!(!cmd(&[
        "sdk-build",
        "--output",
        sdk.to_str().unwrap(),
        "--key",
        key.to_str().unwrap()
    ])
    .status
    .success());
    let verify = || {
        cmd(&[
            "sdk-verify",
            "--sdk",
            sdk.to_str().unwrap(),
            "--public-key",
            &public,
        ])
    };
    success(&verify());
    let consumer = root.join("consumer");
    fs::create_dir(&consumer).unwrap();
    sdk_project(
        &consumer,
        &format!(
            "import std.graph as graph;{}",
            include_str!("../examples/v092/main.rw")
        ),
        "input,output",
    );
    success(&cmd(&[
        "sdk-install",
        "--root",
        consumer.to_str().unwrap(),
        "--sdk",
        sdk.to_str().unwrap(),
        "--public-key",
        &public,
    ]));
    let lock: Value =
        serde_json::from_slice(&fs::read(consumer.join("rewind.lock")).unwrap()).unwrap();
    assert!(lock.to_string().contains("rewind-sdk"));
    assert!(lock.to_string().contains(env!("CARGO_PKG_VERSION")));
    let manifest = fs::read(consumer.join("rewind.toml")).unwrap();
    assert!(!cmd(&[
        "sdk-install",
        "--root",
        consumer.to_str().unwrap(),
        "--sdk",
        sdk.to_str().unwrap(),
        "--public-key",
        &public
    ])
    .status
    .success());
    assert_eq!(manifest, fs::read(consumer.join("rewind.toml")).unwrap());
    let trace = consumer.join("trace.json");
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run",
            "--root",
            consumer.to_str().unwrap(),
            "--record",
            trace.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"2 3 -4\n").unwrap();
    let output = child.wait_with_output().unwrap();
    success(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "1");
    success(&cmd(&[
        "replay",
        trace.to_str().unwrap(),
        "--root",
        consumer.to_str().unwrap(),
    ]));
    let artifact = consumer.join("app.json");
    success(&cmd(&[
        "build",
        "--root",
        consumer.to_str().unwrap(),
        "--output",
        artifact.to_str().unwrap(),
    ]));
    fs::remove_file(consumer.join("main.rw")).unwrap();
    fs::remove_dir_all(consumer.join("vendor")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run-artifact",
            artifact.to_str().unwrap(),
            "--root",
            consumer.to_str().unwrap(),
            "--allow-effects",
            "input,output",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"5 7\n").unwrap();
    let output = child.wait_with_output().unwrap();
    success(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "12");
    let app = root.join("app");
    fs::create_dir(&app).unwrap();
    fs::create_dir(app.join("assets")).unwrap();
    for name in ["main.rw", "rewind.toml", "assets/config.json"] {
        fs::copy(
            sdk.join("share/rewind/examples/app").join(name),
            app.join(name),
        )
        .unwrap();
    }
    success(&cmd(&[
        "sdk-install",
        "--root",
        app.to_str().unwrap(),
        "--sdk",
        sdk.to_str().unwrap(),
        "--public-key",
        &public,
    ]));
    let production = root.join("production");
    success(&cmd(&[
        "install",
        "--production",
        "--root",
        app.to_str().unwrap(),
        "--output",
        production.to_str().unwrap(),
    ]));
    let release = production.join("rewind.release.json");
    success(&cmd(&[
        "sign-release",
        release.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
    ]));
    success(&cmd(&[
        "verify-release",
        release.to_str().unwrap(),
        "--public-key",
        &public,
    ]));
    let app_artifact = production.join("app.json");
    success(&cmd(&[
        "build",
        "--root",
        production.to_str().unwrap(),
        "--output",
        app_artifact.to_str().unwrap(),
    ]));
    fs::remove_file(production.join("main.rw")).unwrap();
    fs::remove_dir_all(production.join("vendor")).unwrap();
    success(&cmd(&[
        "run-artifact",
        app_artifact.to_str().unwrap(),
        "--root",
        production.to_str().unwrap(),
        "--allow-effects",
        "args,env,fileRead,fileWrite,output",
        "--allow-env",
        "REWIND_COUNT",
    ]));
    assert!(production.join("state.json").exists());
    let binary = sdk.join("bin/rewind");
    let original = fs::read(&binary).unwrap();
    fs::write(&binary, b"tampered").unwrap();
    assert!(!verify().status.success());
    fs::write(&binary, original).unwrap();
    let lib = sdk.join("lib/rewind/std/text.rw");
    fs::write(lib, b"changed").unwrap();
    assert!(!verify().status.success());
    fs::copy(
        sdk2.join("lib/rewind/std/text.rw"),
        sdk.join("lib/rewind/std/text.rw"),
    )
    .unwrap();
    success(&verify());
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let collision = root.join("collision");
        fs::create_dir(&collision).unwrap();
        sdk_project(&collision, "", "");
        symlink(root.join("missing"), collision.join("vendor")).unwrap();
        assert!(!cmd(&[
            "sdk-install",
            "--root",
            collision.to_str().unwrap(),
            "--sdk",
            sdk.to_str().unwrap(),
            "--public-key",
            &public
        ])
        .status
        .success());
        symlink(root.join("key"), sdk.join("unlisted")).unwrap();
        assert!(!verify().status.success());
    }
    fs::remove_dir_all(root).unwrap();
}
