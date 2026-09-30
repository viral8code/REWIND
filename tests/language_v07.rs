use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn cmd(mode: &str, root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(mode)
        .args(args)
        .arg("--root")
        .arg(root)
        .output()
        .unwrap()
}
fn fixture(source: &str, effects: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v07-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(root.join("rewind.toml"),format!("language = \"0.7\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"{effects}\"\n")).unwrap();
    success(&cmd("update", &root, &[]));
    root
}
fn success(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn run(source: &str) {
    let root = fixture(source, "");
    success(&cmd("run", &root, &[]));
    fs::remove_dir_all(root).unwrap();
}
fn reject(source: &str, message: &str) {
    let root = fixture(source, "output,tasks,fileWrite,input");
    let out = cmd("check", &root, &[]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains(message),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn records_are_deeply_immutable_shareable_and_matchable() {
    run("enum Color{Red,Blue(Int)}record Point{x:Int,color:Color}fn read<T:Share>(x:T)->T effects {}{return x;}let p=Point(4,Color::Blue(7));let other=read(p);match other{Point{x:n,color:_}=>{assert_eq(n,4);}}assert_eq(p.x,4);");
    reject(
        "record Point{x:Int}let p=Point(4);p.x=7;",
        "record fields are immutable",
    );
    reject("record Bad{items:List<Int>}", "requires a Share field");
    run("record Box{items:Frozen<List<Int>>}let xs=List<Int>();xs.add(3);let b=Box(freeze(xs));assert_eq(b.items.len(),1);");
    reject("let x=match true{_=>1,true=>2};", "unreachable match arm");
}
#[test]
fn const_validation_checks_purity_state_and_bounded_execution() {
    run("fn twice(n:Int)->Int effects {}{return n*2;}const ONE:Int=1;pub const FOUR:Int=twice(ONE+1);assert_eq(FOUR,4);record Point{x:Int}const P:Point=Point(7);assert_eq(P.x,7);");
    reject(
        "pub const N:Unit=Out.println(1);",
        "const requires a pure expression",
    );
    reject(
        "const XS:List<Int>=List<Int>();",
        "const requires a Share value",
    );
    reject(
        "fn looping()->Int effects {}{while true{}return 1;}const N:Int=looping();",
        "ConstEvaluationFailed",
    );
    reject(
        "fn bad()->Int effects {}{commit saved;return 1;}const N:Int=bad();",
        "const cannot use checkpoint",
    );
}
#[test]
fn api_snapshot_detects_effect_ownership_and_default_changes() {
    let root=fixture("pub fn answer(n:&Int)->Int effects {}{return 1;}pub trait Read{fn read(self:&Self)->Int effects {}{return 1;}}","");
    let before = root.join("before.json");
    let after = root.join("after.json");
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", before.to_str().unwrap()],
    ));
    fs::write(root.join("main.rw"),"pub fn answer(value:&Int)->Int effects {}{return 1;}pub trait Read{fn read(self:&Self)->Int effects {}{return 1;}}").unwrap();
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", after.to_str().unwrap()],
    ));
    let diff = |deny: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rewind"));
        command.arg("api-diff").arg(&before).arg(&after);
        if deny {
            command.arg("--deny-breaking");
        }
        command.output().unwrap()
    };
    success(&diff(true));
    fs::write(root.join("main.rw"),"pub fn answer(value:Int)->Int effects {}{return value;}pub trait Read{fn read(self:&Self)->Int effects {}{return 2;}}").unwrap();
    success(&cmd(
        "api-snapshot",
        &root,
        &["--output", after.to_str().unwrap()],
    ));
    let output = diff(true);
    assert!(!output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["compatible"], false);
    assert_eq!(result["changes"].as_array().unwrap().len(), 2);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn documentation_examples_are_isolated_and_cannot_publish_host_files() {
    let root = fixture("", "fileWrite,fileRead");
    let doc = root.join("manual.md");
    fs::write(&doc,"# Example\n```rewind\nFile.writeText(\"out.txt\",\"virtual\");publish;assert_eq(File.readText(\"out.txt\"),Ok(\"virtual\"));\n```\n```rewind\nassert_eq(File.readText(\"out.txt\"),Err(FileError(\"x\")));\n```\n").unwrap();
    // Each block starts with a new journal; the second block explicitly checks absence.
    fs::write(&doc,"```rewind\nFile.writeText(\"out.txt\",\"virtual\");publish;\n```\n```rewind\nmatch File.readText(\"out.txt\"){Err(_)=>{},Ok(_)=>{panic(\"not isolated\");}}\n```\n").unwrap();
    let out = cmd("doctest", &root, &[doc.to_str().unwrap()]);
    success(&out);
    assert!(!root.join("out.txt").exists());
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"examples\":2"));
    fs::write(&doc, "```rewind\nassert_eq(1,2);\n```\n").unwrap();
    assert!(!cmd("doctest", &root, &[doc.to_str().unwrap()])
        .status
        .success());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn timeline_uses_index_without_sources_and_preserves_record() {
    let root = fixture(
        "var n=1;commit saved;n=2;File.writeText(\"out\",\"value\");",
        "fileWrite",
    );
    let trace = root.join("trace.json");
    success(&cmd("run", &root, &["--record", trace.to_str().unwrap()]));
    let bytes = fs::read(&trace).unwrap();
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("timeline")
        .arg(&trace)
        .arg("--task")
        .arg("0")
        .output()
        .unwrap();
    success(&output);
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(result["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| !e["files"].as_array().unwrap().is_empty()));
    assert_eq!(fs::read(&trace).unwrap(), bytes);
    assert!(!root.join("out").exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn records_roundtrip_verified_source_free_artifacts() {
    let root = fixture(
        "record Pair{a:Int,b:Int}let pair=Pair(2,3);assert_eq(pair.a+pair.b,5);",
        "",
    );
    let artifact = root.join("artifact.json");
    success(&cmd(
        "build",
        &root,
        &["--output", artifact.to_str().unwrap()],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = cmd("run-artifact", &root, &[artifact.to_str().unwrap()]);
    success(&out);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn composable_property_generators_shrink_typed_inputs_and_record_seeds() {
    run("fn gen(bits:Int)->(Int,Bool) effects {}{return (8,true);}fn smaller(pair:(Int,Bool))->(Int,Bool) effects {}{return (pair._0/2,pair._1);}match property(7,10,gen,smaller,|pair:(Int,Bool)|->Bool{return pair._0<2;}){Err(f)=>{assert_eq(f.seed,7);assert_eq(f.case,0);assert_eq(f.input._0,2);assert_eq(f.shrinks,2);},Ok(_)=>{panic(\"expected failure\");}}");
    reject("var n=0;let gen=|bits:Int|->Int{n+=1;return n;};property(7,10,gen,|x:Int|->Int{return x;},|x:Int|->Bool{return true;});","property callbacks must be pure Share");
    let root=fixture("property(7,1,|bits:Int|->String{return \"sample\";},|text:String|->String{return text;},|text:String|->Bool{return false;});","");
    let trace = root.join("trace.json");
    success(&cmd("run", &root, &["--record", trace.to_str().unwrap()]));
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
    assert_eq!(value["audit"][0]["kind"], "propertyFailure");
    assert_eq!(value["audit"][0]["seed"], 7);
    success(&cmd("replay", &root, &[trace.to_str().unwrap()]));
    fs::remove_dir_all(root).unwrap();
}
