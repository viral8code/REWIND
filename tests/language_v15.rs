use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn run(text: &str, command: &str, extra: &[&str]) -> std::process::Output {
    let dir = std::env::temp_dir().join(format!(
        "rewind-v15-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("main.rw");
    fs::write(&path, text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            command,
            path.to_str().unwrap(),
            "--allow-effects",
            "external,clock",
        ])
        .args(extra)
        .output()
        .unwrap();
    fs::remove_dir_all(&dir).unwrap();
    out
}
#[test]
fn source_clock_restore_fresh_and_compile() {
    let text = r#"import std.external as host;
var first=0;var second=0;commit saved;
external {match host.millis(){Ok(n)=>{first=n;},Err(e)=>{Out.println(e.code);}}}
Out.println(first);publish;revert saved;
external {match host.millis(){Ok(n)=>{second=n;},Err(e)=>{Out.println(e.code);}}}
Out.println(second);publish;
external fresh {match host.millis(){Ok(n)=>{Out.println(n>=second);},Err(e)=>{Out.println(e.code);}}}publish;"#;
    let out = run(text, "run", &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<_> = lines.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], lines[1]);
    assert_eq!(lines[2], "true");
    let out = run(text, "compile", &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn lexical_boundaries_are_checked() {
    for text in [
        "external {commit a;}",
        "external {publish;}",
        "external {external {}}",
        "import std.external as host;host.millis();",
        "fn main()->Int effects {external,clock} {external {return 1;}return 0;}",
    ] {
        let out = run(text, "run", &[]);
        assert!(!out.status.success(), "accepted {text}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("ExternalBoundary"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
#[test]
fn helper_cannot_hide_checkpoint_effect() {
    let out = run(
        "fn hidden()->Unit effects {} {commit a;} external {hidden();}",
        "run",
        &[],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("ExternalBoundary"));
}

#[test]
fn external_record_replay_and_source_free_execution() {
    let dir = std::env::temp_dir().join(format!("rewind-v15-record-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("main.rw");
    let trace = dir.join("trace.json");
    let text = r#"import std.external as host;
var saved=0;commit before;
external {match host.millis(){Ok(n)=>{saved=n;},Err(_)=>{saved=-1;}}}
Out.println(saved>0);publish;
revert before;
external {match host.millis(){Ok(n)=>{saved=n;},Err(_)=>{saved=-1;}}}
Out.println(saved>0);publish;"#;
    fs::write(&source, text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run",
            source.to_str().unwrap(),
            "--allow-effects",
            "external,clock",
            "--record",
            trace.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"true\ntrue\n");
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "replay",
            trace.to_str().unwrap(),
            "--root",
            dir.to_str().unwrap(),
            "--allow-effects",
            "external,clock",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"true\ntrue\n");
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "compile",
            source.to_str().unwrap(),
            "--allow-effects",
            "external,clock",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::remove_file(&source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run",
            dir.join("main.rwc").to_str().unwrap(),
            "--allow-effects",
            "external,clock",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"true\ntrue\n");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn external_keyword_is_contextual_and_main_requires_a_region() {
    let out = run(
        "var external=1;external=2;Out.println(external);publish;",
        "run",
        &[],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"2\n");
    let out=run("import std.external as host;fn main()->Int effects {external,clock} {host.millis();return 0;}","check",&[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("ExternalBoundary"));
}
