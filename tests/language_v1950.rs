use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1950-{}-{}",
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
const COMMON: &str = r#"import std.training as training;import std.numeric as numeric;import std.autodiff as ad;
fn take<T>(r:Result<T,StdError>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(e)=>{panic(e.code);}}}
fn fail<T>(r:Result<T,StdError>,code:String)->Unit effects {} {match move r{Err(e)=>{assert_eq(e.code,code);},Ok(_)=>{panic("expected error");}}}
fn vector(value:Float)->FloatArray effects {} {let shape=List<Int>();shape.add(1);let values=List<Float>();values.add(value);return take(numeric.fromFloat(&shape,&values));}
fn scalar(values:&Map<String,FloatArray>,name:String)->Float effects {} {match values.get(name){Some(v)=>{let cells=take(numeric.valuesFloat(&v));return cells.get(0);},None=>{panic("name");}}}
fn close(a:Float,b:Float)->Unit effects {} {assert(take(numeric.math("abs",a-b))<0.000001);}
"#;
#[test]
fn weighted_means_and_scaled_global_clipping_match_independent_values_and_preserve_inputs() {
    let r = root(&format!(
        r#"{COMMON}
 let left=Map<String,FloatArray>();left.set("a",vector(10.0));left.set("b",vector(-2.0));let right=Map<String,FloatArray>();right.set("a",vector(4.0));right.set("b",vector(7.0));let mean=take(training.mergeMean(&left,2,&right,1));close(scalar(&mean,"a"),8.0);close(scalar(&mean,"b"),1.0);close(scalar(&left,"a"),10.0);close(scalar(&right,"b"),7.0);
 let unbalancedLeft=Map<String,FloatArray>();unbalancedLeft.set("a",vector(0.0));let unbalancedRight=Map<String,FloatArray>();unbalancedRight.set("a",vector(6000000000000000.0));let unbalanced=take(training.mergeMean(&unbalancedLeft,6000000000000000,&unbalancedRight,1));close(scalar(&unbalanced,"a"),1.0);
 let gradients=Map<String,FloatArray>();gradients.set("a",vector(3.0));gradients.set("b",vector(4.0));let clipped=take(training.clipGlobalNorm(&gradients,2.0));close(scalar(&clipped,"a"),1.2);close(scalar(&clipped,"b"),1.6);close(scalar(&gradients,"a"),3.0);let unchanged=take(training.clipGlobalNorm(&gradients,5.0));assert_eq(unchanged.get("a"),gradients.get("a"));assert_eq(unchanged.get("b"),gradients.get("b"));
 let tiny=Map<String,FloatArray>();tiny.set("a",vector(1e-308));let unscaled=take(training.clipGlobalNorm(&tiny,1e308));assert_eq(unscaled.get("a"),tiny.get("a"));
 let huge=Map<String,FloatArray>();huge.set("a",vector(1e308));huge.set("b",vector(1e308));let bounded=take(training.clipGlobalNorm(&huge,1.0));close(scalar(&bounded,"a"),1.0/take(numeric.math("sqrt",2.0)));close(scalar(&bounded,"b"),scalar(&bounded,"a"));
 fail(training.mergeMean(&left,0,&right,1),"TrainingSamples");fail(training.mergeMean(&left,9007199254740991,&right,1),"TrainingSamples");fail(training.mergeMean(&left,1,&right,9223372036854775807),"TrainingSamples");fail(training.clipGlobalNorm(&left,0.0),"TrainingNorm");let wrong=Map<String,FloatArray>();wrong.set("a",vector(1.0));wrong.set("z",vector(1.0));fail(training.mergeMean(&left,1,&wrong,1),"TrainingParameters");close(scalar(&left,"b"),-2.0);Out.println(true);publish;
 "#
    ));
    // The parser's exponent syntax is part of the existing numeric language.
    let o = call(&r, &["run", "main.rw", "--native-work", "10000000"]);
    ok(&o);
    assert_eq!(o.stdout, b"true\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn named_sigmoid_gradient_matches_independent_finite_difference_and_rejects_unused_nodes() {
    let loss = |w: f64| {
        let mut total = 0.0;
        for x in [-1.0_f64, -0.25, 0.5, 1.5] {
            let y = 1.0 / (1.0 + (-(2.0 * x - 0.5)).exp());
            let predicted = 1.0 / (1.0 + (-w * x).exp());
            total += (predicted - y).powi(2);
        }
        total / 4.0
    };
    let expected = (loss(0.7 + 1e-6) - loss(0.7 - 1e-6)) / 2e-6;
    let r = root(&format!(
        r#"{COMMON}
 var tape=ad.create();let inputValues=List<Float>();inputValues.add(-1.0);inputValues.add(-0.25);inputValues.add(0.5);inputValues.add(1.5);let labels=List<Float>();for i in 0..4{{let x=inputValues.get(i);labels.add(1.0/(1.0+take(numeric.math("exp",-(2.0*x-0.5)))));}}
 let shape=List<Int>();shape.add(4);shape.add(1);let inputs=take(numeric.fromFloat(&shape,&inputValues));let targetArray=take(numeric.fromFloat(&shape,&labels));let wshape=List<Int>();wshape.add(1);wshape.add(1);let wvalues=List<Float>();wvalues.add(0.7);let w=take(numeric.fromFloat(&wshape,&wvalues));let parameter=take(ad.parameter(&mut tape,"w",w));let input=take(ad.constant(&mut tape,inputs));let target=take(ad.constant(&mut tape,targetArray));let logits=take(ad.matmul(&mut tape,input,parameter));let predicted=take(ad.unary(&mut tape,"sigmoid",logits));let loss=take(training.meanSquaredError(&mut tape,predicted,target));let nodes=Map<String,ad.Node>();nodes.set("w",parameter);let gradients=take(training.gradients(&tape,loss,&nodes));close(scalar(&gradients,"w"),{expected:.17});
 let unused=take(ad.parameter(&mut tape,"unused",w));nodes.set("w",unused);fail(training.gradients(&tape,loss,&nodes),"TrainingUnused");nodes.set("w",target);fail(training.gradients(&tape,loss,&nodes),"TrainingUnused");var other=ad.create();let foreign=take(ad.parameter(&mut other,"foreign",w));nodes.set("w",foreign);fail(training.gradients(&tape,loss,&nodes),"AutodiffNode");nodes.remove("w");fail(training.gradients(&tape,loss,&nodes),"TrainingParameters");nodes.set("",parameter);fail(training.gradients(&tape,loss,&nodes),"TrainingParameters");let after=take(ad.value(&tape,parameter));assert_eq(after,w);Out.println(true);publish;
 "#
    ));
    let o = call(&r, &["run", "main.rw", "--native-work", "10000000"]);
    ok(&o);
    assert_eq!(o.stdout, b"true\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn nonlinear_unequal_batch_training_model_roundtrip_and_both_trace_modes_are_source_free() {
    for mode in ["debug", "compact"] {
        let source = if mode == "debug" {
            include_str!("../examples/nonlinear-training/trace-small.rw")
        } else {
            include_str!("../examples/nonlinear-training/main.rw")
        };
        let r = root(source);
        ok(&call(
            &r,
            &[
                "compile",
                "main.rw",
                "--allow-effects",
                "fileRead,fileWrite",
            ],
        ));
        fs::remove_file(r.join("main.rw")).unwrap();
        fs::remove_dir_all(r.join(".rewind")).unwrap();
        let o = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--allow-effects",
                "fileRead,fileWrite",
                "--steps",
                "100000000",
                "--native-work",
                "1000000000",
                "--history-memory",
                "128MiB",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&o);
        assert_eq!(o.stdout, b"true\n");
        if mode == "compact" {
            fs::remove_file(r.join("nonlinear.rwm")).unwrap();
        }
        let replay = call(
            &r,
            &[
                "replay",
                "trace.json",
                "--allow-effects",
                "fileRead,fileWrite",
            ],
        );
        ok(&replay);
        assert_eq!(replay.stdout, o.stdout);
        fs::remove_dir_all(r).unwrap();
    }
}
