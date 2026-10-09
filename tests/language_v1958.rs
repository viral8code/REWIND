use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1958-{}-{}",
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
fn python() -> &'static str {
    if cfg!(windows) {
        "python"
    } else {
        "python3"
    }
}
const COMMON: &str = r#"import std.autodiff as ad;import std.autodiffAsync as jobs;import std.numeric as n;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
fn model(x:FloatArray)->(ad.Tape,ad.Node) effects {} {var tape=ad.create();let node=take(ad.parameter(&mut tape,"x",x));let square=take(ad.unary(&mut tape,"square",node));let loss=take(ad.sum(&mut tape,square));return (move tape,loss);}
"#;
#[test]
fn independent_finite_differences_all_operations_and_branch_keys_match() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = Command::new(python())
        .args([
            repo.join("scripts/check-autodiff-async.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
}
#[test]
fn unfinished_reverse_tasks_restore_and_replay_without_source_or_cache() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let r = std::env::temp_dir().join(format!("rewind-backward-smoke-{}", std::process::id()));
    let result = Command::new(python())
        .args([
            repo.join("scripts/smoke-autodiff-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            r.as_os_str(),
            repo.join("examples/autodiff-async/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn work_bridge_rejects_early_state_and_wrong_gradient_shapes() {
    let source = format!(
        r#"{COMMON}
let shape=List<Int>();shape.add(1);let x=take(n.zerosFloat(&shape));let graph=model(x);var work=take(ad.backwardWork(move graph._0,graph._1));
match ad.nextBackward(&mut work){{Err(e)=>{{assert_eq(e.code,"AutodiffState");}},_=>{{panic("early traversal");}}}}
match ad.backwardAccumulated(&work,0){{Err(e)=>{{assert_eq(e.code,"AutodiffState");}},_=>{{panic("early lookup");}}}}
assert(take(ad.prepareBackward(&mut work)));let step=take(ad.nextBackward(&mut work));let badshape=List<Int>();badshape.add(2);let wrong=take(n.zerosFloat(&badshape));
match ad.storeBackward(&mut work,step.a,wrong){{Err(e)=>{{assert_eq(e.code,"NumericShape");}},_=>{{panic("bad shape");}}}}
match ad.storeBackward(&mut work,-1,x){{Err(e)=>{{assert_eq(e.code,"AutodiffNode");}},_=>{{panic("bad node");}}}}
match ad.finishBackward(move work){{Err(e)=>{{assert_eq(e.code,"AutodiffState");}},_=>{{panic("early finish");}}}}Out.println(true);publish;"#
    );
    let r = root(&source);
    let o = call(&r, &["run", "main.rw"]);
    ok(&o);
    assert_eq!(o.stdout, b"true\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn repeated_kernel_cancellation_releases_unretained_tape_and_gradient_work() {
    for count in [64, 256] {
        let source = format!(
            r#"{COMMON}
let one=List<Int>();one.add(1);let cells=List<Float>();cells.add(0.5);let scalar=take(n.fromFloat(&one,&cells));let shape=List<Int>();shape.add(32769);let x=take(n.broadcastFloat(&scalar,&shape));var finished=0;
for i in 0..{count}{{let graph=model(x);let work=spawn jobs.backward(move graph._0,graph._1);task.yieldNow();assert(!work.isDone());work.cancel();match await work{{Err(TaskError::Cancelled)=>{{finished+=1;}},_=>{{panic("partial gradients");}}}}}}
Out.println(finished);publish;"#
        );
        let r = root(&source);
        let o = call(
            &r,
            &[
                "profile",
                "main.rw",
                "--history-memory",
                "16MiB",
                "--steps",
                "30000000",
                "--native-work",
                "3000000000",
            ],
        );
        ok(&o);
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), count.to_string());
        let stderr = String::from_utf8(o.stderr).unwrap();
        let profile: serde_json::Value =
            serde_json::from_str(stderr.lines().rev().find(|l| l.starts_with('{')).unwrap())
                .unwrap();
        assert!(profile["gc"]["completed"].as_u64().unwrap() > 0);
        assert!(profile["numeric_pages"]["live_bytes"].as_u64().unwrap() < 4 * 1024 * 1024);
        fs::remove_dir_all(r).unwrap();
    }
}
#[test]
fn large_tape_metadata_initialization_yields_before_finishing() {
    let source = format!(
        r#"{COMMON}
async fn marker()->Int effects {{tasks}}{{return 7;}}
let shape=List<Int>();shape.add(1);let x=take(n.zerosFloat(&shape));var tape=ad.create();let parameter=take(ad.parameter(&mut tape,"x",x));for i in 0..4096{{take(ad.constant(&mut tape,x));}}let loss=take(ad.sum(&mut tape,parameter));
let work=spawn jobs.backward(move tape,loss);let fast=spawn marker();assert_eq(take(await fast),7);assert(!work.isDone());commit preparing;let gradients=take(take(await work));match take(ad.gradient(&gradients,parameter)){{Some(array)=>{{assert_eq(take(n.sum(&array)),1.0);}},None=>{{panic("missing gradient");}}}}drop preparing;Out.println(true);publish;"#
    );
    let r = root(&source);
    let o = call(
        &r,
        &[
            "run",
            "main.rw",
            "--task-steps",
            "2000000",
            "--steps",
            "30000000",
            "--native-work",
            "1000000000",
            "--history-memory",
            "64MiB",
        ],
    );
    ok(&o);
    assert_eq!(o.stdout, b"true\n");
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn native_pointer_cancels_materialized_backward_and_replays_disconnected() {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let r = std::env::temp_dir().join(format!("rewind-backward-native-gui-{}", std::process::id()));
    let result = Command::new(python())
        .args([
            repo.join("scripts/smoke-backward-native-gui-sdk.py")
                .as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            r.as_os_str(),
            repo.join("examples/backward-gui/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(r).unwrap();
}
