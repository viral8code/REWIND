use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1948-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(r: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(r)
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn libraries(r: &PathBuf) {
    fs::write(
        r.join("numeric.rw"),
        include_str!("../libraries/std/numeric.rw"),
    )
    .unwrap();
    fs::write(
        r.join("numericIndex.rw"),
        include_str!("../libraries/std/numericIndex.rw"),
    )
    .unwrap();
}
#[test]
fn flat_views_and_checkpoints_survive_source_free_debug_and_compact_replay() {
    let r = root();
    libraries(&r);
    let source = include_str!("../examples/numeric-index/main.rw")
        .replace("std.numeric as", "numeric as")
        .replace("std.numericIndex as", "numericIndex as");
    fs::write(r.join("main.rw"), source).unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    for name in ["main.rw", "numeric.rw", "numericIndex.rw"] {
        fs::remove_file(r.join(name)).unwrap();
    }
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
            "99\n2\n"
        );
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn old_language_numeric_import_remains_available_and_flat_primitives_are_gated() {
    let r = root();
    libraries(&r);
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.47\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    fs::write(r.join("main.rw"),r#"import numeric as n;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(2);let a=take(n.zerosInt(&shape));let index=List<Int>();index.add(1);Out.println(take(n.getInt(&a,&index)));publish;"#).unwrap();
    ok(&call(&r, &["update"]));
    let out = call(&r, &["run", "main.rw"]);
    ok(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "0");
    fs::write(
        r.join("main.rw"),
        "fn flat(a:&IntArray)->Result<Int,StdError> effects {} {return stdNumericGetFlatInt(a,0);}",
    )
    .unwrap();
    let out = call(&r, &["check", "main.rw"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("flat numeric indexing requires language 1.9.48"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn large_sparse_array_updates_keep_native_admission_and_old_view_under_small_budget() {
    let r = root();
    libraries(&r);
    fs::write(r.join("main.rw"),r#"import numeric as n;import numericIndex as flat;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(262144);let original=take(n.zerosInt(&shape));var a=original;
for i in 0..4096{a=take(flat.withInt(&a,262143,i));}
assert_eq(take(flat.getInt(&original,262143)),0);assert_eq(take(flat.getInt(&a,262143)),4095);
assert_eq(take(flat.lengthInt(&a)),262144);Out.println(take(flat.getInt(&a,262143)));publish;"#).unwrap();
    let out = call(
        &r,
        &[
            "profile",
            "main.rw",
            "--steps",
            "20000000",
            "--native-work",
            "10000000",
            "--history-memory",
            "8MiB",
        ],
    );
    ok(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "4095");
    let stderr = String::from_utf8(out.stderr).unwrap();
    let p: serde_json::Value =
        serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap()).unwrap();
    assert!(p["gc"]["completed"].as_u64().unwrap() > 0, "{p}");
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn flat_update_pays_native_work_before_publication() {
    let r = root();
    fs::write(r.join("main.rw"), r#"
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(513);var a=take(stdNumericZerosInt(&shape));
a=take(stdNumericWithFlatInt(&a,256,99));Out.println(99);publish;
"#).unwrap();
    let out = call(&r, &["run", "main.rw", "--native-work", "1536"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("NativeWork"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty());
    fs::remove_dir_all(r).unwrap();
}
