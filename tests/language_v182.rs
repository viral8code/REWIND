use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v182-{}-{}",
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
fn exact_integer_example_is_source_free_and_replayable() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/bigint/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(output.stdout,b"1267650600228229401496703205378\n1267650600228229401496703205376\n\"1267650600228229401496703205376\"\n1\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn exact_values_freeze_transfer_and_order_map_keys() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.bigint as big;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let value=take(big.parse("123456789012345678901234567890",10));let frozen=freeze(value);let restored=thaw(frozen);
async fn exact(value:BigInt)->String effects {} {return take(big.format(&value,16));}
let task=spawn exact(move restored);Out.println(take(await task));
let m=Map<BigInt,String>();m.set(take(big.fromInt(10)),"ten");m.set(take(big.fromInt(-2)),"negative");m.set(take(big.fromInt(10)),"updated");Out.println(m.len());let keys=m.keys();Out.println(keys.get(0));Out.println(keys.get(1));
let left=take(big.fromInt(7));let right=take(big.parse("007",10));Out.println(left.eq(right));let another=take(big.fromInt(7));Out.println(left.hash()==another.hash());publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(
        output.stdout,
        b"18ee90ff6c373e0ee4e3f0ad2\n2\n-2\n10\ntrue\ntrue\n"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn exact_errors_are_results_and_json_does_not_coerce_float() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.bigint as big;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let two=take(big.fromInt(2));let zero=take(big.fromInt(0));
match big.divide(&two,&zero){Err(e)=>{Out.println(e.code);},Ok(_)=>{panic("division");}}
match big.parse("1_000",10){Err(e)=>{Out.println(e.code);},Ok(_)=>{panic("syntax");}}
match big.parse("10",1){Err(e)=>{Out.println(e.code);},Ok(_)=>{panic("radix");}}
match big.fromJson(Json::Float(9007199254740992.0)){Err(e)=>{Out.println(e.code);},Ok(_)=>{panic("coercion");}}
let n=take(big.pow(&two,100));match big.toInt(&n){Err(e)=>{Out.println(e.code);},Ok(_)=>{panic("overflow");}}
let negative=take(big.fromInt(-3));Out.println(take(big.shift(&negative,false,1)));let five=take(big.fromInt(5));Out.println(take(big.modulo(&negative,&five)));publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(
        output.stdout,
        b"BigIntDivisionByZero\nBigIntSyntax\nBigIntRadix\nBigIntJsonType\nBigIntOverflow\n-2\n2\n"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn large_pow_is_rejected_by_native_budget_before_allocation() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"import std.bigint as big;match big.fromInt(3){Err(_)=>{panic("failed");},Ok(value)=>{let output=big.pow(&value,200000);Out.println("unreachable");publish;}}"#).unwrap();
    let output = call(&root, &["run", "main.rw", "--native-work", "10000"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("NativeWorkBudgetExceeded"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn bigint_constructor_and_names_cannot_be_forged() {
    for source in [
        "let forged=BigInt();",
        "record BigInt {}",
        "fn stdBigIntFromInt(value:Int)->Int effects {} {return value;}",
    ] {
        let root = dir();
        fs::write(root.join("main.rw"), source).unwrap();
        assert!(!call(&root, &["compile", "main.rw"]).status.success());
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn historical_user_bigint_record_does_not_gain_native_key_traits() {
    let root = dir();
    fs::write(
        root.join("rewind.toml"),
        "language=\"1.8.1\"\nsource_root=\".\"\nentry=\"main.rw\"\neffects=\"output\"\n",
    )
    .unwrap();
    fs::write(
        root.join("main.rw"),
        "record BigInt {value:Int} let value=BigInt(3); Out.println(value.value);publish;",
    )
    .unwrap();
    success(&call(&root, &["update", "--root", "."]));
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(output.stdout, b"3\n");
    fs::write(
        root.join("main.rw"),
        "record BigInt {value:Int} let values=Map<BigInt,Int>();",
    )
    .unwrap();
    success(&call(&root, &["update", "--root", "."]));
    let invalid = call(&root, &["compile", "main.rw"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("Ord"));
    fs::remove_dir_all(root).unwrap();
}
