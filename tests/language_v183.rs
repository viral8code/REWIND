use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v183-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn decimal_computation_map_and_json_survive_source_free_replay() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/decimal/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(output.stdout, b"0.3\n0.1\n2\ntrue\n1\n\"0.1\"\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn sqlite_numeric_affinity_cannot_round_blob_backed_decimals() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/decimal-sqlite/main.rw")
            .replace("\":memory:\"", "\"data.sqlite\""),
    )
    .unwrap();
    success(&call(
        &root,
        &["compile", "main.rw", "--allow-effects", "external,db,tasks"],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            "external,db,tasks",
            "--record",
            "trace.json",
        ],
    );
    success(&output);
    assert_eq!(output.stdout, b"12345678901234567890.12340000\n");
    fs::remove_file(root.join("data.sqlite")).unwrap();
    let replay = call(
        &root,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            "external,db,tasks",
        ],
    );
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    assert!(!root.join("data.sqlite").exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn decimal_storage_preserves_scale_and_json_negative_exponent() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.decimal as dec;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
var amount=take(dec.parse("1.0"));commit saved;amount=take(dec.parse("1.00"));Out.println(take(dec.scale(&amount)));publish;revert saved;Out.println(take(dec.scale(&amount)));publish;
let extreme=take(dec.parse("1e10000"));let json=take(dec.toJson(&extreme));let restored=take(dec.fromJson(json));Out.println(take(dec.scale(&restored)));Out.println(extreme==restored);
let tie=take(dec.parse("2.5"));match dec.quantize(&tie,0,28,DecimalRounding::Exact){Err(e)=>{Out.println(e.code);},Ok(_)=>{panic("inexact");}}
let zero=take(dec.parse("0"));match dec.divide(&tie,&zero,2,28,DecimalRounding::HalfEven){Err(e)=>{Out.println(e.code);},Ok(_)=>{panic("zero");}}
publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(
        output.stdout,
        b"2\n1\n-10000\ntrue\nDecimalInexact\nDecimalDivisionByZero\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn decimals_freeze_transfer_preserve_metadata_and_scalar_hash() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.decimal as dec;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let value=take(dec.parse("1.2300"));let saved=freeze(value);let owned=thaw(saved);
async fn read(value:Decimal)->String effects {} {return take(dec.format(&value));}
let task=spawn read(move owned);Out.println(take(await task));
let a=take(dec.parse("-2.500"));let b=take(dec.parse("-2.5"));Out.println(a==b);Out.println(a.hash()==b.hash());let rounded=take(dec.quantize(&a,0,28,DecimalRounding::HalfEven));Out.println(take(dec.format(&rounded)));publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(output.stdout, b"1.2300\ntrue\ntrue\n-2\n");
    fs::remove_dir_all(root).unwrap();
}
