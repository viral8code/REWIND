use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1915-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&r).unwrap();
    fs::write(r.join("main.rw"), source).unwrap();
    r
}
fn call(r: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(r)
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
const COMMON: &str = r#"import std.numeric as numeric; import std.sparse as sparse; import std.fft as fft;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("numeric result");}}}
fn f(values:&List<Float>)->FloatArray effects {} {let shape=List<Int>();shape.add(values.len());return take(numeric.fromFloat(&shape,values));}
fn ints(values:&List<Int>)->IntArray effects {} {let shape=List<Int>();shape.add(values.len());return take(numeric.fromInt(&shape,values));}
fn convert(n:Int)->Result<Float,String> effects {} {return n.toFloatChecked();}
fn close(a:Float,b:Float)->Unit effects {} {assert(take(stdNumericMath("abs",a-b))<0.00000001);}
fn error<T>(result:Result<T,StdError>,code:String)->Unit effects {} {match move result{Err(e)=>{assert_eq(e.code,code);},Ok(_)=>{panic("expected failure");}}}
"#;
#[test]
fn public_sparse_fft_example_runs_source_free_and_replays_debug_and_compact() {
    let r = root(include_str!("../examples/sparse-fft/main.rw"));
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    fs::remove_dir_all(r.join(".rewind")).unwrap();
    for mode in ["debug", "compact"] {
        let o = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&o);
        assert_eq!(o.stdout, b"true\n2\ntrue\ntrue\n");
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(replay.stdout, o.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn csr_duplicates_views_checkpoint_recovery_and_fft_match_independent_source_references() {
    let r = root(&format!(
        r#"{COMMON}
 let rows=List<Int>();rows.add(1);rows.add(0);rows.add(1);rows.add(0);rows.add(1);rows.add(2);
 let cols=List<Int>();cols.add(2);cols.add(1);cols.add(2);cols.add(1);cols.add(0);cols.add(2);
 let vals=List<Float>();vals.add(1.0);vals.add(5.0);vals.add(-1.0);vals.add(-3.0);vals.add(4.0);vals.add(7.0);
 let rr=ints(&rows);let cc=ints(&cols);let vv=f(&vals);var model=take(sparse.create(3,3,&rr,&cc,&vv));
 let values=List<Float>();values.add(10.0);values.add(20.0);values.add(30.0);let rhs=f(&values);let product=take(sparse.matvec(&model,&rhs));let cells=take(numeric.valuesFloat(&product));assert_eq(cells.get(0),40.0);assert_eq(cells.get(1),40.0);assert_eq(cells.get(2),210.0);
 commit original;let twice=take(numeric.scale(&model.values,2.0));model=sparse.Matrix(model.rows,model.cols,model.offsets,model.indices,twice);let changed=take(sparse.matvec(&model,&rhs));let changedValues=take(numeric.valuesFloat(&changed));assert_eq(changedValues.get(2),420.0);revert original;assert_eq(take(sparse.matvec(&model,&rhs)),product);drop original;
 for size in 0..6 {{var count=1;for k in 0..size{{count*=2;}}let re=List<Float>();let im=List<Float>();for k in 0..count{{re.add(take(convert((k*k)%17))-4.0);im.add(take(convert((k*7)%11))-3.0);}}let real=f(&re);let imaginary=f(&im);let transformed=take(fft.transform(&real,&imaginary,false));let rv=take(numeric.valuesFloat(&transformed.real));let iv=take(numeric.valuesFloat(&transformed.imag));for k in 0..count{{var a=0.0;var b=0.0;for t in 0..count{{let angle=-6.283185307179586*take(convert(k))*take(convert(t))/take(convert(count));let c=take(stdNumericMath("cos",angle));let s=take(stdNumericMath("sin",angle));a+=re.get(t)*c-im.get(t)*s;b+=re.get(t)*s+im.get(t)*c;}}close(rv.get(k),a);close(iv.get(k),b);}}let restored=take(fft.transform(&transformed.real,&transformed.imag,true));let rrv=take(numeric.valuesFloat(&restored.real));for k in 0..count{{close(rrv.get(k),re.get(k));}}}}
 Out.println(1);publish;"#
    ));
    let o = call(
        &r,
        &[
            "run",
            "main.rw",
            "--steps",
            "10000000",
            "--native-work",
            "10000000",
        ],
    );
    ok(&o);
    assert_eq!(o.stdout, b"1\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn invalid_sparse_shapes_indices_transforms_and_scalars_fail_without_mutating_inputs() {
    let r = root(&format!(
        r#"{COMMON}
 let shape=List<Int>();shape.add(3);let three=take(numeric.zerosFloat(&shape));error(fft.transform(&three,&three,false),"NumericShape");
 let rows=List<Int>();rows.add(-1);let cols=List<Int>();cols.add(0);let vals=List<Float>();vals.add(1.0);let rr=ints(&rows);let cc=ints(&cols);let vv=f(&vals);error(sparse.create(1,1,&rr,&cc,&vv),"NumericIndex");let preserved=take(numeric.valuesInt(&rr));assert_eq(preserved.len(),rows.len());assert_eq(preserved.get(0),rows.get(0));
 let os=List<Int>();os.add(0);os.add(2);let ids=List<Int>();ids.add(0);let offsets=ints(&os);let indices=ints(&ids);let invalid=sparse.Matrix(1,1,offsets,indices,vv);error(sparse.matvec(&invalid,&vv),"NumericShape");
 let duplicateOffsets=List<Int>();duplicateOffsets.add(0);duplicateOffsets.add(2);let duplicateIndices=List<Int>();duplicateIndices.add(0);duplicateIndices.add(0);let duplicateValues=List<Float>();duplicateValues.add(1.0);duplicateValues.add(1.0);let dupOs=ints(&duplicateOffsets);let dupIs=ints(&duplicateIndices);let dupVs=f(&duplicateValues);let duplicate=sparse.Matrix(1,1,dupOs,dupIs,dupVs);error(sparse.matvec(&duplicate,&vv),"NumericIndex");
 let tinyValues=List<Float>();tinyValues.add(1e-200);let tiny=f(&tinyValues);let tinyNorm=take(numeric.norm2(&tiny));assert(tinyNorm>0.0);
 let nanValues=List<Float>();nanValues.add(0.0/0.0);let nan=f(&nanValues);error(fft.transform(&nan,&nan,false),"NumericNonFinite");error(numeric.scale(&vv,1.0/0.0),"NumericNonFinite");
 let empty=List<Float>();let zero=f(&empty);let emptyResult=take(fft.convolve(&zero,&vv));assert_eq(take(numeric.valuesFloat(&emptyResult)).len(),0);
 Out.println(1);publish;"#
    ));
    let o = call(&r, &["run", "main.rw"]);
    ok(&o);
    assert_eq!(o.stdout, b"1\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn conjugate_gradient_exhaustion_validation_fairness_and_cancellation_are_typed() {
    let r = root(&format!(
        r#"{COMMON}import std.sparseAsync as solver;import std.task as task;
 async fn heartbeat(queue:Channel<Int>)->Unit effects {{tasks}} {{for i in 0..4{{assert_eq(await queue.send(i),Ok(()));task.yieldNow();}}}}
 let row=List<Int>();let col=List<Int>();let value=List<Float>();let wanted=List<Float>();
 for i in 0..40{{row.add(i);col.add(i);value.add(4.0);wanted.add(take(convert(i%7+1)));if i>0{{row.add(i);col.add(i-1);value.add(-1.0);}}if i<39{{row.add(i);col.add(i+1);value.add(-1.0);}}}}
 let rows=ints(&row);let cols=ints(&col);let vals=f(&value);let matrix=take(sparse.create(40,40,&rows,&cols,&vals));let exact=f(&wanted);let right=take(sparse.matvec(&matrix,&exact));
 let short=spawn solver.conjugateGradient(matrix,right,0.0000000001,1);let limited=take(take(await short));assert(!limited.converged);assert_eq(limited.iterations,1);
 let queue=Channel<Int>(8);let heart=spawn heartbeat(queue);let work=spawn solver.conjugateGradient(matrix,right,0.0000000001,100);let result=take(take(await work));assert(result.converged);assert(result.residual<0.00000001);assert_eq(await heart,Ok(()));for i in 0..4{{assert_eq(await queue.receive(),Ok(i));}}queue.close();
 let found=take(numeric.valuesFloat(&result.solution));let expected=take(numeric.valuesFloat(&exact));for i in 0..40{{close(found.get(i),expected.get(i));}}
 let stopped=spawn solver.conjugateGradient(matrix,right,0.0000000000000001,100);task.yieldNow();assert(!stopped.isDone());stopped.cancel();assert_eq(await stopped,Err(TaskError::Cancelled));
 let bad=spawn solver.conjugateGradient(matrix,right,0.0,10);error(take(await bad),"NumericDomain");Out.println(1);publish;
 "#
    ));
    let o = call(
        &r,
        &[
            "run",
            "main.rw",
            "--native-work",
            "10000000",
            "--steps",
            "10000000",
            "--task-steps",
            "2000000",
        ],
    );
    ok(&o);
    assert_eq!(o.stdout, b"1\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn new_primitives_have_a_language_gate_and_native_work_is_enforced() {
    let r=root("let shape=List<Int>();shape.add(8);let a=stdNumericZerosFloat(&shape);match move a{Ok(v)=>{stdNumericFft(&v,&v,false);},Err(_)=>{panic(\"zeros\");}}");
    fs::write(
        r.join("rewind.toml"),
        "language=\"1.9.14\"\nsource_root=\".\"\nentry=\"main.rw\"\n",
    )
    .unwrap();
    ok(&call(&r, &["update"]));
    let o = call(&r, &["check"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("require language 1.9.15"));
    fs::remove_file(r.join("rewind.toml")).unwrap();
    fs::remove_file(r.join("rewind.lock")).unwrap();
    let o = call(&r, &["run", "main.rw", "--native-work", "1000"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("NativeWorkBudgetExceeded"));
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn profiling_measures_gc_separately_and_does_not_affect_replay() {
    let r = root(&format!(
        r#"{COMMON}let dims=List<Int>();dims.add(2048);let zero=take(numeric.zerosFloat(&dims));let ones=take(numeric.mapFloat("exp",&zero));commit retained;for i in 0..150{{let value=take(fft.transform(&ones,&zero,false));assert(take(numeric.norm2(&value.imag))<0.00000001);}}drop retained;Out.println(1);publish;"#
    ));
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    let out = call(
        &r,
        &[
            "profile",
            "main.rwc",
            "--native-work",
            "1000000000",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"1\n");
    let profile: serde_json::Value = String::from_utf8_lossy(&out.stderr)
        .lines()
        .filter(|s| s.starts_with('{'))
        .map(|s| serde_json::from_str(s).unwrap())
        .last()
        .unwrap();
    assert!(profile["gc"]["completed"].as_u64().unwrap() > 0);
    assert_eq!(profile["gc"]["failed"], 0);
    assert!(profile["gc"]["duration_nanos"].as_u64().unwrap() > 0);
    assert!(
        profile["numeric_pages"]["allocated_bytes"]
            .as_u64()
            .unwrap()
            > profile["numeric_pages"]["live_bytes"].as_u64().unwrap()
    );
    let replay = call(&r, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(replay.stdout, out.stdout);
    fs::remove_dir_all(r).unwrap();
}
