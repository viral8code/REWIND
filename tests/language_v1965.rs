use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1965-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn task_failure_envelope_preserves_actual_diagnostic_and_typed_budgets_source_free() {
    for mode in ["debug", "compact"] {
        let root = root();
        fs::write(root.join("main.rw"),r#"import std.taskError as failures;
async fn broken()->Int effects {} {panic("deliberate task error");return 0;}
match await spawn broken(){Err(e)=>{let f=failures.describe(move e);assert_eq(f.code,"Panic");match move f.diagnostic{Some(d)=>{assert_eq(d.code,"Panic");assert(d.line>0);assert_eq(d.message,"panic: deliberate task error");assert_eq(d.source,"main.rw");},None=>{panic("lost diagnostic");}}},Ok(_)=>{panic("missing failure");}}
let exhausted=failures.describe(TaskError::BudgetExceeded(BudgetKind::NativeWork));assert_eq(exhausted.code,"NativeWorkBudgetExceeded");assert_eq(exhausted.budget,Some(BudgetKind::NativeWork));assert_eq(exhausted.diagnostic,None);
assert_eq(failures.asStd(TaskError::Cancelled),StdError("TaskCancelled",0));assert_eq(failures.describe(TaskError::TimedOut).code,"TaskTimedOut");
Out.println("causes retained");publish;
"#).unwrap();
        ok(&call(&root, &["compile", "main.rw"]));
        fs::remove_file(root.join("main.rw")).unwrap();
        let _ = fs::remove_dir_all(root.join(".rewind"));
        let out = call(
            &root,
            &[
                "run",
                "main.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "causes retained\n"
        );
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn cooperative_forward_reports_child_native_budget_instead_of_generic_task_error() {
    let root = root();
    fs::write(root.join("main.rw"),r#"import std.numeric as n;import std.autodiff as ad;import std.autodiffForwardAsync as forward;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("setup failed");}}}
let shape=List<Int>();shape.add(16384);let input=take(n.zerosFloat(&shape));
match await spawn forward.parameter(ad.create(),"x",input){Ok(Err(e))=>{Out.println("nested "+e.code);},Ok(Ok(_))=>{Out.println("success");},Err(TaskError::BudgetExceeded(BudgetKind::NativeWork))=>{Out.println("outer budget");},Err(_)=>{panic("unexpected task failure");}}publish;"#).unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(root.join(".rewind"));
    for (budget, expected) in [
        ("1000000", "nested NativeWorkBudgetExceeded\n"),
        ("3000000", "success\n"),
    ] {
        let out = call(
            &root,
            &[
                "run",
                "main.rwc",
                "--native-work",
                budget,
                "--record",
                "trace.json",
                "--record-mode",
                "compact",
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            expected
        );
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn large_snapshot_and_iterator_preserve_pages_and_checkpoint_source_free() {
    let root = root();
    fs::write(root.join("main.rw"),r#"import std.numeric as n;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("setup");}}}
let dims=List<Int>();dims.add(65536);let zero=take(n.zerosFloat(&dims));let input=take(n.affine(&zero,1.0,0.25));let values=thaw(freeze(take(n.valuesFloat(&input))));
let frozen=freeze(values);let cursor=values.iter();values.set(0,9.0);assert_eq(cursor.next(),Some(0.25));let restored=thaw(frozen);assert_eq(restored.len(),65536);assert_eq(restored.get(0),0.25);restored.set(0,7.0);assert_eq(thaw(frozen).get(0),0.25);
commit snapshot;assert_eq(cursor.next(),Some(0.25));values.set(1,8.0);revert snapshot;assert_eq(values.get(1),0.25);assert_eq(cursor.next(),Some(0.25));drop snapshot;
let mapping=Map<Int,Int>();for i in 0..10001{mapping.set(i,i);}let frozenMap=freeze(mapping);let keys=mapping.iter();mapping.set(0,-1);assert_eq(thaw(frozenMap).get(0),Some(0));assert_eq(keys.next(),Some(0));
Out.println("large snapshots retained");publish;"#).unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(root.join(".rewind"));
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--native-work",
            "100000000",
            "--steps",
            "10000000",
            "--history-memory",
            "128MiB",
            "--record",
            "trace.json",
            "--record-mode",
            "compact",
        ],
    );
    ok(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        "large snapshots retained\n"
    );
    let replay = call(&root, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn large_snapshot_budget_failure_is_precise_and_old_artifacts_keep_old_limit() {
    let root = root();
    fs::write(root.join("main.rw"),r#"import std.numeric as n;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("setup");}}}
let dims=List<Int>();dims.add(20000);let zero=take(n.zerosInt(&dims));let values=take(n.valuesInt(&zero));let frozen=freeze(values);Out.println(frozen.len());publish;"#).unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    let low = call(&root, &["run", "main.rwc", "--native-work", "1000000"]);
    assert!(!low.status.success());
    assert!(
        String::from_utf8_lossy(&low.stderr).contains("NativeWorkBudgetExceeded"),
        "{}",
        String::from_utf8_lossy(&low.stderr)
    );
    assert!(low.stdout.is_empty());
    let high = call(&root, &["run", "main.rwc", "--native-work", "10000000"]);
    ok(&high);
    fs::write(
        root.join("rewind.toml"),
        "language = \"1.9.64\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(
        root.join("main.rw"),
        "let items=List<Int>();for i in 0..10001{items.add(i);}let frozen=freeze(items);publish;\n",
    )
    .unwrap();
    ok(&call(&root, &["update"]));
    let old = call(&root, &["run", "--root", ".", "--native-work", "10000000"]);
    assert!(!old.status.success());
    assert!(
        String::from_utf8_lossy(&old.stderr).contains("Map key snapshot budget exceeded"),
        "{}",
        String::from_utf8_lossy(&old.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
