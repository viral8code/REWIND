use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1939-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn native_identity_checkpoint_source_free_debug_compact_replay() {
    let r = root();
    fs::write(
        r.join("main.rw"),
        include_str!("../examples/numeric-digests/main.rw"),
    )
    .unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(r.join(".rewind"));
    for mode in ["debug", "compact"] {
        let out = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--native-work",
                "10000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "127\n0\n"
        );
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
const IDENTITY: &str = r#"import std.numeric as n;
fn take<T>(r:Result<T,StdError>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(e)=>{panic(e.code);}}}
let shape=List<Int>();shape.add(1024);let whole=take(n.zerosFloat(&shape));let view=take(n.sliceFloat(&whole,0,0,1,1));
let empty=take(stdEncode(""));let key=take(stdNumericTensorKey("view",0,empty,empty,&view));assert_eq(stdBytesLength(key),32);Out.println(1);publish;"#;
#[test]
fn backing_storage_identity_is_admitted_independently_of_recording_cache() {
    let r = root();
    fs::write(r.join("main.rw"), IDENTITY).unwrap();
    for mode in ["debug", "compact"] {
        let out = call(
            &r,
            &[
                "run",
                "main.rw",
                "--native-work",
                "12000",
                "--record",
                "denied.json",
                "--record-mode",
                mode,
            ],
        );
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("NativeWork"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.stdout.is_empty());
        let out = call(
            &r,
            &[
                "run",
                "main.rw",
                "--native-work",
                "100000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(out.stdout, b"1\n");
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn older_language_retains_fixed_identity_work_fee() {
    let r = root();
    // Explicit projects resolve library dependencies from their manifest.
    // This fee compatibility fixture uses the native primitives directly.
    let legacy = IDENTITY
        .replace("import std.numeric as n;", "")
        .replace("n.zerosFloat", "stdNumericZerosFloat")
        .replace("n.sliceFloat", "stdNumericSliceFloat");
    fs::write(r.join("main.rw"), legacy).unwrap();
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.38\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    ok(&call(&r, &["update"]));
    let out = call(
        &r,
        &[
            "run",
            "main.rw",
            "--native-work",
            "12000",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"1\n");
    let replay = call(&r, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(r).unwrap();
}
