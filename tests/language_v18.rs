use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v18-{}-{}",
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
fn compiled_numeric_arrays_revert_and_replay_without_source() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/numeric/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(output.stdout, b"1\n2\n99\n0\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn numeric_views_errors_freeze_and_task_transfer_are_owned() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.push(1);shape.push(2);
let values=List<Int>();values.push(3);values.push(4);
let array=take(numeric.fromInt(&shape,&values));
let saved=freeze(array);let restored=thaw(saved);
let target=List<Int>();target.push(3);target.push(2);
let broadcast=take(numeric.broadcastInt(&restored,&target));
let index=List<Int>();index.push(0);index.push(0);
match numeric.withInt(&broadcast,&index,5){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("broadcast must be read-only");}}
let owned=take(numeric.materializeInt(&broadcast));
let changed=take(numeric.withInt(&owned,&index,5));
Out.println(take(numeric.getInt(&changed,&index)));
Out.println(take(numeric.getInt(&restored,&index)));
let row=take(numeric.sliceInt(&owned,1,1,2,-1));
let flat=take(numeric.valuesInt(&row));Out.println(flat.get(0));Out.println(flat.get(1));
async fn readInt(array:IntArray)->Int effects {} {let at=List<Int>();at.push(0);at.push(1);return take(numeric.getInt(&array,&at));}
let task=spawn readInt(move changed);
Out.println(take(await task));
match numeric.math("sqrt",-1.0){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("domain must fail");}}
let emptyShape=List<Int>();emptyShape.push(0);let empty=take(numeric.zerosFloat(&emptyShape));
match numeric.mapFloat("unknown",&empty){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("empty array must validate operation");}}
let wrongShape=List<Int>();wrongShape.push(128);let wrong=take(numeric.zerosFloat(&wrongShape));
match numeric.solve(&wrong,&wrong,0.0){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("rank must fail");}}
publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(
        output.stdout,
        b"NumericReadOnly\n5\n3\n4\n3\n4\nNumericDomain\nNumericDomain\nNumericShape\n"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn numeric_type_cannot_be_forged_or_used_as_another_dtype() {
    let root = dir();
    fs::write(root.join("main.rw"), "let array=FloatArray();publish;").unwrap();
    let output = call(&root, &["compile", "main.rw"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("opaque type constructor"));
    fs::write(root.join("main.rw"),r#"import std.numeric as n;fn wrong(a:&IntArray)->Result<Float,StdError> effects {} {return n.sum(a);}publish;"#).unwrap();
    assert!(!call(&root, &["compile", "main.rw"]).status.success());
    fs::write(
        root.join("main.rw"),
        "fn stdNumericUser()->Int effects {} {return 123;}Out.println(stdNumericUser());publish;",
    )
    .unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(output.stdout, b"123\n");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn numeric_work_budget_refuses_large_matrix_product_before_execution() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.push(128);shape.push(128);
let a=take(numeric.zerosFloat(&shape));let b=take(numeric.matmul(&a,&a));publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw", "--native-work", "100000"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("NativeWork"));
    fs::write(root.join("main.rw"),r#"
import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let left=List<Int>();left.push(4096);left.push(0);let right=List<Int>();right.push(0);right.push(4096);
let a=take(numeric.zerosFloat(&left));let b=take(numeric.zerosFloat(&right));let c=take(numeric.matmul(&a,&b));publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw", "--native-work", "100000"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("NativeWork"));
    fs::write(root.join("main.rw"),r#"
import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let left=List<Int>();left.push(8192);left.push(0);let right=List<Int>();right.push(0);right.push(8192);
let a=take(numeric.zerosFloat(&left));let b=take(numeric.zerosFloat(&right));
match numeric.matmul(&a,&b){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("oversized output");}}publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw", "--native-work", "100000"]);
    success(&output);
    assert_eq!(output.stdout, b"NumericSize\n");
    fs::remove_dir_all(root).unwrap();
}
