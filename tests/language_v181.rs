use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v181-{}-{}",
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
fn advanced_numeric_example_compiles_runs_and_replays_without_source() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/numeric-analysis/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(
        output.stdout,
        b"2\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\n4\ntrue\ntrue\n"
    );
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn decompositions_preserve_record_fields_freeze_and_report_convergence() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.push(2);shape.push(2);let values=List<Float>();values.push(2.0);values.push(1.0);values.push(1.0);values.push(2.0);let a=take(numeric.fromFloat(&shape,&values));
let qr=take(numeric.qr(&a,0.000000000001));let snapshot=freeze(qr);let owned=thaw(snapshot);Out.println(owned.rank);Out.println(owned.permutation.len());
match numeric.eigenSymmetric(&a,0.000000000001,0){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("nonconvergence must fail");}}
let dataShape=List<Int>();dataShape.push(7);let dataValues=List<Float>();dataValues.push(-1.0);dataValues.push(0.0);dataValues.push(0.5);dataValues.push(1.0);dataValues.push(1.5);dataValues.push(2.0);dataValues.push(3.0);
let data=take(numeric.fromFloat(&dataShape,&dataValues));let edgesShape=List<Int>();edgesShape.push(3);let edgesValues=List<Float>();edgesValues.push(0.0);edgesValues.push(1.0);edgesValues.push(2.0);let edges=take(numeric.fromFloat(&edgesShape,&edgesValues));
let result=take(numeric.histogram(&data,&edges));let index=List<Int>();index.push(0);Out.println(take(numeric.getInt(&result.counts,&index)));index.set(0,1);Out.println(take(numeric.getInt(&result.counts,&index)));Out.println(result.underflow);Out.println(result.overflow);publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw", "--allow-effects", "output"]);
    success(&output);
    assert_eq!(output.stdout, b"2\n2\nNumericConvergence\n2\n3\n1\n1\n");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn distribution_state_is_checkpointed_and_inputs_have_explicit_failures() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.distributions as distributions;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
commit seed;Out.println(take(distributions.normal(0.0,1.0)));publish;revert seed;Out.println(take(distributions.normal(0.0,1.0)));publish;
match distributions.exponential(0.0){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("invalid rate");}}
match distributions.normal(0.0,-1.0){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("invalid deviation");}}
match distributions.bernoulli(1.1){Err(error)=>{Out.println(error.code);},Ok(_)=>{panic("invalid probability");}}
publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw", "--record", "trace.json"]);
    success(&output);
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], lines[1]);
    assert!(lines[0].parse::<f64>().unwrap().is_finite());
    assert_eq!(&lines[2..], &["NumericDomain"; 3]);
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    let source = fs::read_to_string(root.join("main.rw")).unwrap();
    fs::write(
        root.join("main.rw"),
        format!("{source}\nfn bad()->Float effects {{}} {{return distributions.uniform();}}\n"),
    )
    .unwrap();
    assert!(
        !call(&root, &["compile", "main.rw", "--allow-effects", "output"])
            .status
            .success()
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn empty_qr_permutation_and_eigen_iterations_pay_native_work_before_allocation() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.push(0);shape.push(1000000);let matrix=take(numeric.zerosFloat(&shape));let qr=take(numeric.qr(&matrix,0.000000000001));publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw", "--native-work", "100000"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("NativeWork"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpointed_normal_sampling_has_reference_mean_and_variance() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.numeric as numeric;import std.distributions as distributions;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let values=List<Float>();var count=0;while count<1000{values.push(take(distributions.normal(0.0,1.0)));count+=1;}
let shape=List<Int>();shape.push(1000);let samples=take(numeric.fromFloat(&shape,&values));
let mean=take(numeric.mean(&samples));let variance=take(numeric.variance(&samples,1));
Out.println(mean>-0.15 && mean<0.15);Out.println(variance>0.8 && variance<1.2);publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(output.stdout, b"true\ntrue\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn incremental_moments_merge_without_samples_and_revert_to_the_original_owner() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let left=numeric.onlineMoments();let right=numeric.onlineMoments();
take(numeric.pushMoment(&mut left,1.0));take(numeric.pushMoment(&mut left,2.0));take(numeric.pushMoment(&mut right,3.0));take(numeric.pushMoment(&mut right,4.0));
commit separate;take(numeric.mergeMoments(&mut left,&right));Out.println(numeric.onlineCount(&left));Out.println(take(numeric.onlineMean(&left)));publish;
revert separate;Out.println(numeric.onlineCount(&left));Out.println(take(numeric.onlineMean(&left)));publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(output.stdout, b"4\n2.5\n2\n1.5\n");
    fs::remove_dir_all(root).unwrap();
}
