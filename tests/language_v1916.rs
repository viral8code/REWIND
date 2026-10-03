use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v1916-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    root
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
const COMMON: &str = r#"import std.autodiff as ad;import std.numeric as numeric;
fn take<T>(result:Result<T,StdError>)->T effects {} {match move result{Ok(v)=>{return move v;},Err(e)=>{panic(e.code);}}}
fn fail<T>(result:Result<T,StdError>,code:String)->Unit effects {} {match move result{Err(e)=>{assert_eq(e.code,code);},Ok(_)=>{panic("expected error");}}}
fn gradient(result:Result<Option<FloatArray>,StdError>)->FloatArray effects {} {match take(result){Some(v)=>{return v;},None=>{panic("missing derivative");}}}
fn two(a:Int,b:Int)->List<Int> effects {} {let values=List<Int>();values.add(a);values.add(b);return move values;}
fn cell(array:&FloatArray,row:Int,col:Int)->Float effects {} {let indices=two(row,col);return take(numeric.getFloat(array,&indices));}
fn close(a:Float,b:Float)->Unit effects {} {assert(take(numeric.math("abs",a-b))<0.000001);}
"#;
#[test]
fn nonlinear_matrix_and_broadcast_reverse_gradients_match_independent_finite_differences() {
    let r = root(&format!(
        r#"{COMMON}
 fn objective(x:&FloatArray,w:&FloatArray,b:&FloatArray)->Float effects {{}} {{var total=0.0;for row in 0..2{{for col in 0..2{{var z=0.0;for k in 0..3{{z+=cell(x,row,k)*cell(w,k,col);}}let index=List<Int>();index.add(col);z+=take(numeric.getFloat(b,&index));let t=take(numeric.math("tanh",z));let s=1.0/(1.0+take(numeric.math("exp",-t)));total+=s*s+s;}}}}return total/4.0;}}
 let xs=List<Float>();xs.add(0.3);xs.add(-0.7);xs.add(1.2);xs.add(-0.4);xs.add(0.8);xs.add(0.2);let xshape=two(2,3);let x=take(numeric.fromFloat(&xshape,&xs));
 let ws=List<Float>();ws.add(0.2);ws.add(-0.1);ws.add(0.4);ws.add(0.3);ws.add(-0.5);ws.add(0.7);let wshape=two(3,2);let w=take(numeric.fromFloat(&wshape,&ws));
 let bs=List<Float>();bs.add(0.1);bs.add(-0.2);let bshape=List<Int>();bshape.add(2);let b=take(numeric.fromFloat(&bshape,&bs));
 var tape=ad.create();let xn=take(ad.constant(&mut tape,x));let wn=take(ad.parameter(&mut tape,"w",w));let bn=take(ad.parameter(&mut tape,"b",b));let product=take(ad.matmul(&mut tape,xn,wn));let outputShape=two(2,2);let biases=take(ad.broadcast(&mut tape,bn,&outputShape));let z=take(ad.binary(&mut tape,"add",product,biases));let t=take(ad.unary(&mut tape,"tanh",z));let s=take(ad.unary(&mut tape,"sigmoid",t));let square=take(ad.unary(&mut tape,"square",s));let joined=take(ad.binary(&mut tape,"add",square,s));let loss=take(ad.mean(&mut tape,joined));let derivatives=take(ad.backward(&tape,loss));let dw=gradient(ad.gradient(&derivatives,wn));let db=gradient(ad.gradient(&derivatives,bn));assert_eq(take(ad.gradient(&derivatives,xn)),None);
 let epsilon=0.00001;for row in 0..3{{for col in 0..2{{let index=two(row,col);let value=cell(&w,row,col);let plus=take(numeric.withFloat(&w,&index,value+epsilon));let minus=take(numeric.withFloat(&w,&index,value-epsilon));let expected=(objective(&x,&plus,&b)-objective(&x,&minus,&b))/(2.0*epsilon);close(cell(&dw,row,col),expected);}}}}
 for col in 0..2{{let index=List<Int>();index.add(col);let value=take(numeric.getFloat(&b,&index));let plus=take(numeric.withFloat(&b,&index,value+epsilon));let minus=take(numeric.withFloat(&b,&index,value-epsilon));close(take(numeric.getFloat(&db,&index)),(objective(&x,&w,&plus)-objective(&x,&w,&minus))/(2.0*epsilon));}}
 let lossv=take(ad.value(&tape,loss));close(take(numeric.sum(&lossv)),objective(&x,&w,&b));Out.println(1);publish;
 "#
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
fn transpose_reshape_repeated_parents_constants_foreign_handles_and_checkpoint_are_checked() {
    let r = root(&format!(
        r#"{COMMON}
 let dimensions=two(2,3);let values=List<Float>();for i in 0..6{{values.add(0.5);}}let a=take(numeric.fromFloat(&dimensions,&values));var tape=ad.create();let x=take(ad.parameter(&mut tape,"x",a));commit before;
 let transposed=take(ad.transpose(&mut tape,x));let flat=List<Int>();flat.add(6);let reshaped=take(ad.reshape(&mut tape,transposed,&flat));let doubled=take(ad.binary(&mut tape,"add",reshaped,reshaped));let loss=take(ad.mean(&mut tape,doubled));let derivatives=take(ad.backward(&tape,loss));let result=gradient(ad.gradient(&derivatives,x));let cells=take(numeric.valuesFloat(&result));for i in 0..6{{close(cells.get(i),1.0/3.0);}}
 revert before;let exp=take(ad.unary(&mut tape,"exp",x));let log=take(ad.unary(&mut tape,"log",exp));let sum=take(ad.sum(&mut tape,log));let secondDerivatives=take(ad.backward(&tape,sum));let secondResult=gradient(ad.gradient(&secondDerivatives,x));let secondCells=take(numeric.valuesFloat(&secondResult));for i in 0..6{{close(secondCells.get(i),1.0);}}drop before;
 var other=ad.create();let changed=take(numeric.scale(&a,2.0));let foreign=take(ad.parameter(&mut other,"x",changed));fail(ad.value(&tape,foreign),"AutodiffNode");fail(ad.gradient(&secondDerivatives,foreign),"AutodiffNode");fail(ad.backward(&tape,x),"AutodiffLoss");
 let scalarShape=List<Int>();let scalarValues=List<Float>();scalarValues.add(3.0);let scalar=take(numeric.fromFloat(&scalarShape,&scalarValues));let constant=take(ad.constant(&mut tape,scalar));let empty=take(ad.backward(&tape,constant));assert_eq(take(ad.gradient(&empty,constant)),None);Out.println(1);publish;
 "#
    ));
    let o = call(&r, &["run", "main.rw"]);
    ok(&o);
    assert_eq!(o.stdout, b"1\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn optimizers_reject_mismatched_late_parameters_without_advancing_state_and_revert_moments() {
    let r=root(&format!(r#"{COMMON}import std.optimize as opt;
 let dims=List<Int>();dims.add(1);let source=List<Float>();source.add(2.0);let value=take(numeric.fromFloat(&dims,&source));let weights=Map<String,FloatArray>();weights.set("a",value);weights.set("z",value);var state=take(opt.adam(&weights,0.9,0.999,0.00000001));var fresh=take(opt.adam(&weights,0.9,0.999,0.00000001));
 let wrong=Map<String,FloatArray>();wrong.set("a",value);let two=List<Int>();two.add(2);let invalid=take(numeric.zerosFloat(&two));wrong.set("z",invalid);fail(opt.adamStep(&mut state,&weights,&wrong,0.1),"NumericShape");assert_eq(opt.steps(&state),0);
 let gradients=Map<String,FloatArray>();gradients.set("a",value);gradients.set("z",value);let sgd=take(opt.sgd(&weights,&gradients,0.1));let v=take(gradient(Ok(sgd.get("a"))).reshapeFloat(&dims));
 commit original;let actual=take(opt.adamStep(&mut state,&weights,&gradients,0.1));let expected=take(opt.adamStep(&mut fresh,&weights,&gradients,0.1));assert_eq(actual.get("a"),expected.get("a"));assert_eq(actual.get("z"),expected.get("z"));assert_eq(opt.steps(&state),1);revert original;assert_eq(opt.steps(&state),0);drop original;
 fail(opt.sgd(&weights,&gradients,0.0),"OptimizerRate");fail(opt.adam(&weights,1.0,0.999,0.00000001),"OptimizerConfiguration");Out.println(1);publish;
 "#).replace("let v=take(gradient(Ok(sgd.get(\"a\"))).reshapeFloat(&dims));","match sgd.get(\"a\"){Some(v)=>{let cells=take(numeric.valuesFloat(&v));close(cells.get(0),1.8);},None=>{panic(\"sgd\");}}"));
    let o = call(&r, &["run", "main.rw"]);
    ok(&o);
    assert_eq!(o.stdout, b"1\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn training_and_saved_model_run_source_free_and_replay_without_the_model_file() {
    let r = root(include_str!("../examples/model-training/main.rw"));
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
    let args = [
        "run",
        "main.rwc",
        "--allow-effects",
        "fileRead,fileWrite",
        "--steps",
        "20000000",
        "--native-work",
        "20000000",
        "--history-memory",
        "256MiB",
        "--record-mode",
        "compact",
        "--record",
        "trace.json",
    ];
    let o = call(&r, &args);
    ok(&o);
    assert_eq!(o.stdout, b"true\n");
    let model = fs::read(r.join("model.rwm")).unwrap();
    assert!(model.starts_with(b"RWMDL\0\x01\0"));
    fs::write(r.join("reader.rw"),r#"effects {fileRead};import std.models as models;match models.load("model.rwm"){Ok(model)=>{assert(models.get(&model,"linear")!=None);Out.println(true);},Err(e)=>{panic(e.code);}}publish;"#).unwrap();
    ok(&call(
        &r,
        &["compile", "reader.rw", "--allow-effects", "fileRead"],
    ));
    fs::remove_file(r.join("reader.rw")).unwrap();
    let observed = call(
        &r,
        &[
            "run",
            "reader.rwc",
            "--allow-effects",
            "fileRead",
            "--record-mode",
            "compact",
            "--record",
            "reader-trace.json",
        ],
    );
    ok(&observed);
    assert_eq!(observed.stdout, b"true\n");
    fs::remove_file(r.join("model.rwm")).unwrap();
    let replay = call(
        &r,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            "fileRead,fileWrite",
            "--root",
            ".",
        ],
    );
    ok(&replay);
    assert_eq!(replay.stdout, o.stdout);
    let observed_replay = call(
        &r,
        &[
            "replay",
            "reader-trace.json",
            "--allow-effects",
            "fileRead",
            "--root",
            ".",
        ],
    );
    ok(&observed_replay);
    assert_eq!(observed_replay.stdout, observed.stdout);
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn tensor_primitives_are_gated_and_private_node_handles_cannot_be_forged() {
    let r=root("let dimensions=List<Int>();let a=stdNumericZerosFloat(&dimensions);match move a{Ok(v)=>{stdNumericCheckFinite(&v);},Err(_)=>{}}");
    fs::write(
        r.join("rewind.toml"),
        "language=\"1.9.15\"\nsource_root=\".\"\nentry=\"main.rw\"\n",
    )
    .unwrap();
    ok(&call(&r, &["update"]));
    let o = call(&r, &["check"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("require language 1.9.16"));
    fs::remove_file(r.join("rewind.toml")).unwrap();
    fs::write(
        r.join("main.rw"),
        "import std.autodiff as ad;let key=stdEncode(\"\");let n=ad.Node(0,key);",
    )
    .unwrap();
    let o = call(&r, &["check", "main.rw"]);
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("private"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    fs::remove_dir_all(r).unwrap();
}
