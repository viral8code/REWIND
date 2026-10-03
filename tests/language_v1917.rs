use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1917-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&r).unwrap();
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
#[test]
fn imported_functions_do_not_shadow_unrelated_entry_globals_but_real_local_shadowing_warns() {
    let r = root();
    fs::write(
        r.join("helper.rw"),
        "pub fn answer()->Int effects {} {let data=3;return data;}",
    )
    .unwrap();
    fs::write(r.join("main.rw"),"import helper as helper;let data=10;assert_eq(helper.answer(),3);Out.println(data);publish;").unwrap();
    let first = call(&r, &["run", "main.rw"]);
    ok(&first);
    assert_eq!(first.stdout, b"10\n");
    assert!(!String::from_utf8_lossy(&first.stderr).contains("shadows"));
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    fs::remove_file(r.join("helper.rw")).unwrap();
    ok(&call(
        &r,
        &[
            "run",
            "main.rwc",
            "--record-mode",
            "compact",
            "--record",
            "trace.json",
        ],
    ));
    ok(&call(&r, &["replay", "trace.json", "--root", "."]));
    fs::write(
        r.join("main.rw"),
        "let data=10;fn answer()->Int effects {}{let data=3;return data;}assert_eq(answer(),3);",
    )
    .unwrap();
    let actual = call(&r, &["check", "main.rw"]);
    ok(&actual);
    assert!(String::from_utf8_lossy(&actual.stderr).contains("data shadows an outer binding"));
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn imported_functions_cannot_capture_entry_global_but_keep_own_and_transitive_constants() {
    let r = root();
    fs::write(
        r.join("helper.rw"),
        "pub fn read()->Int effects {} {return hidden;}",
    )
    .unwrap();
    fs::write(
        r.join("main.rw"),
        "import helper as helper;var hidden=9;assert_eq(helper.read(),9);",
    )
    .unwrap();
    let bad = call(&r, &["check", "main.rw"]);
    assert!(!bad.status.success());
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("unknown name hidden"),
        "{}",
        String::from_utf8_lossy(&bad.stderr)
    );
    fs::write(r.join("base.rw"), "pub const OFFSET:Int=7;").unwrap();
    fs::write(r.join("helper.rw"),"import base as base;const LOCAL:Int=5;pub fn read()->Int effects {}{return LOCAL+base.OFFSET;}").unwrap();
    fs::write(
        r.join("main.rw"),
        "import helper as helper;let LOCAL=90;assert_eq(helper.read(),12);Out.println(1);publish;",
    )
    .unwrap();
    let good = call(&r, &["run", "main.rw"]);
    ok(&good);
    assert_eq!(good.stdout, b"1\n");
    assert!(!String::from_utf8_lossy(&good.stderr).contains("shadows"));
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn legacy_language_keeps_prior_scope_behavior() {
    let r = root();
    fs::write(
        r.join("rewind.toml"),
        "language=\"1.9.16\"\nsource_root=\".\"\nentry=\"main.rw\"\n",
    )
    .unwrap();
    fs::write(
        r.join("helper.rw"),
        "pub fn read()->Int effects {}{return hidden;}",
    )
    .unwrap();
    fs::write(
        r.join("main.rw"),
        "import helper as helper;var hidden=9;assert_eq(helper.read(),9);",
    )
    .unwrap();
    ok(&call(&r, &["update"]));
    ok(&call(&r, &["run", "main.rw"]));
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn fused_optimizer_accepts_large_finite_weighted_moments_and_reverts_atomic_state() {
    let r = root();
    fs::write(r.join("main.rw"),r#"import std.numeric as numeric;import std.optimize as opt;
 fn take<T>(r:Result<T,StdError>)->T effects {}{match move r{Ok(v)=>{return move v;},Err(e)=>{panic(e.code);}}}
 let dims=List<Int>();dims.add(1);let w=List<Float>();w.add(2.0);let a=take(numeric.fromFloat(&dims,&w));let g=List<Float>();g.add(1e155);let grad=take(numeric.fromFloat(&dims,&g));let weights=Map<String,FloatArray>();weights.set("x",a);let gradients=Map<String,FloatArray>();gradients.set("x",grad);var state=take(opt.adam(&weights,0.9,0.999,0.00000001));commit original;
 let updated=take(opt.adamStep(&mut state,&weights,&gradients,0.1));match updated.get("x"){Some(v)=>{let values=take(numeric.valuesFloat(&v));assert(take(numeric.math("abs",values.get(0)-1.9))<0.000000001);},None=>{panic("weights");}}assert_eq(opt.steps(&state),1);revert original;assert_eq(opt.steps(&state),0);drop original;Out.println(1);publish;
 "#).unwrap();
    let o = call(&r, &["run", "main.rw"]);
    ok(&o);
    assert_eq!(o.stdout, b"1\n");
    fs::remove_dir_all(r).unwrap();
}
