use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1962-{}-{}",
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
fn opaque_identity_restores_after_source_free_cache_independent_replay() {
    let root = root();
    fs::write(root.join("main.rw"),r#"import std.numeric as n;import std.numericIdentityAsync as identity;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let dims=List<Int>();dims.add(65537);let array=take(n.zerosFloat(&dims));let empty=take(stdEncode(""));
let expected=take(stdNumericTensorKey("parameter:x",0,empty,empty,&array));
assert_eq(take(await spawn identity.checkFinite(array)),Ok(()));
let job=spawn identity.key(array,"parameter:x",0,empty,empty);for i in 0..3{task.yieldNow();}assert(!job.isDone());commit running;
assert_eq(take(take(await job)),expected);Out.println("key");publish;
revert running;assert(!job.isDone());assert_eq(take(take(await job)),expected);Out.println("restored");publish;
let rejected=take(take(await spawn identity.prepare(array)));assert_eq(stdNumericIdentityKey(&rejected,"x",-1,empty,empty),Err(StdError("NumericDomain",0)));
Out.println("identity done");publish;
"#).unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(root.join(".rewind"));
    for mode in ["debug", "compact"] {
        let out = call(
            &root,
            &[
                "run",
                "main.rwc",
                "--native-work",
                "100000000",
                "--task-steps",
                "1000000",
                "--steps",
                "10000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "key\nrestored\nidentity done\n"
        );
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn identity_work_is_versioned_and_private() {
    for source in [
        "let forged=TensorIdentityWork();publish;",
        "record TensorIdentityWork{code:Int}publish;",
    ] {
        let root = root();
        fs::write(root.join("main.rw"), source).unwrap();
        assert!(!call(&root, &["compile", "main.rw"]).status.success());
        fs::remove_dir_all(root).unwrap();
    }
    let root = root();
    fs::write(
        root.join("rewind.toml"),
        "language = \"1.9.61\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(root.join("main.rw"),"record TensorIdentityWork{code:Int}let w=TensorIdentityWork(3);assert_eq(w.code,3);publish;").unwrap();
    ok(&call(&root, &["update"]));
    ok(&call(&root, &["run", "main.rw"]));
    fs::write(root.join("main.rw"),"let shape=List<Int>();let a=stdNumericZerosFloat(&shape);stdNumericIdentityInit(&a);publish;").unwrap();
    let out = call(&root, &["compile", "main.rw"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("requires language 1.9.62"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cooperative_forward_matches_sync_nodes_values_and_gradients() {
    let root = root();
    fs::write(
        root.join("main.rw"),
        include_str!("fixtures/v1962-forward-paths.rw"),
    )
    .unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(root.join(".rewind"));
    ok(&call(
        &root,
        &[
            "run",
            "main.rwc",
            "--native-work",
            "100000000",
            "--task-steps",
            "1000000",
            "--steps",
            "10000000",
        ],
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unfinished_forward_restores_and_cancels_with_source_free_replay() {
    let root = root();
    fs::write(
        root.join("main.rw"),
        include_str!("fixtures/v1962-forward-restore.rw"),
    )
    .unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(root.join(".rewind"));
    for mode in ["debug", "compact"] {
        let out = call(
            &root,
            &[
                "run",
                "main.rwc",
                "--native-work",
                "100000000",
                "--task-steps",
                "1000000",
                "--steps",
                "10000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "finished\nrestored\ncancelled\n"
        );
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cooperative_forward_rejects_invalid_graphs_and_keeps_constant_gradients_absent() {
    let root = root();
    fs::write(
        root.join("main.rw"),
        include_str!("fixtures/v1962-forward-errors.rw"),
    )
    .unwrap();
    ok(&call(&root, &["run", "main.rw"]));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn repeated_forward_cancellation_releases_unretained_identity_and_tape_state() {
    for count in [64, 256] {
        let root = root();
        let source = format!(
            r#"import std.numeric as n;import std.autodiff as ad;import std.autodiffForwardAsync as forward;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {{}} {{match move r{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
let dims=List<Int>();dims.add(65537);let array=take(n.zerosFloat(&dims));var finished=0;
for i in 0..{count}{{let pending=spawn forward.parameter(ad.create(),"x",array);task.yieldNow();assert(!pending.isDone());pending.cancel();match await pending{{Err(TaskError::Cancelled)=>{{finished+=1;}},_=>{{panic("partial tape");}}}}}}
Out.println(finished);publish;"#
        );
        fs::write(root.join("main.rw"), source).unwrap();
        let out = call(
            &root,
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
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            count.to_string()
        );
        let stderr = String::from_utf8(out.stderr).unwrap();
        let profile: serde_json::Value = serde_json::from_str(
            stderr
                .lines()
                .rev()
                .find(|line| line.starts_with('{'))
                .unwrap(),
        )
        .unwrap();
        assert!(profile["gc"]["completed"].as_u64().unwrap() > 0);
        assert!(profile["numeric_pages"]["live_bytes"].as_u64().unwrap() < 4 * 1024 * 1024);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn synchronous_autodiff_import_keeps_historical_language_and_pure_effects() {
    let root = root();
    fs::write(
        root.join("rewind.toml"),
        "language = \"1.9.58\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(root.join("main.rw"), r#"import numeric as n;import autodiff as ad;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(1);let array=take(n.zerosFloat(&shape));let tape=ad.create();let x=take(ad.parameter(&mut tape,"x",array));let loss=take(ad.sum(&mut tape,x));let gradients=take(ad.backward(&tape,loss));assert(take(ad.gradient(&gradients,x))!=None);publish;"#).unwrap();
    let modules = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("libraries/std");
    for module in ["numeric.rw", "autodiff.rw"] {
        fs::copy(modules.join(module), root.join(module)).unwrap();
    }
    ok(&call(&root, &["update"]));
    ok(&call(&root, &["run", "main.rw"]));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compact_artifacts_accept_equivalent_pretty_encoding_after_sources_are_removed() {
    let root = root();
    fs::write(
        root.join("main.rw"),
        include_str!("fixtures/v1962-forward-errors.rw"),
    )
    .unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    let compact = fs::read(root.join("main.rwc")).unwrap();
    let artifact: serde_json::Value = serde_json::from_slice(&compact).unwrap();
    let pretty = serde_json::to_vec_pretty(&artifact).unwrap();
    assert!(compact.len() * 2 < pretty.len());
    fs::write(root.join("pretty.rwc"), pretty).unwrap();
    fs::remove_file(root.join("main.rw")).unwrap();
    let _ = fs::remove_dir_all(root.join(".rewind"));
    ok(&call(&root, &["run", "main.rwc"]));
    ok(&call(&root, &["run", "pretty.rwc"]));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rehashed_artifact_cannot_expose_private_native_identity_snapshot_fields() {
    use sha2::{Digest, Sha256};
    let root = root();
    fs::write(root.join("main.rw"), "publish;").unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    let mut artifact: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("main.rwc")).unwrap()).unwrap();
    artifact["payload"]["program"]["structs"]["TensorIdentityWork"]["private_fields"] =
        serde_json::json!([]);
    artifact["sha256"] = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&artifact["payload"]).unwrap())
    )
    .into();
    fs::write(
        root.join("forged.rwc"),
        serde_json::to_vec(&artifact).unwrap(),
    )
    .unwrap();
    let out = call(&root, &["run", "forged.rwc"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("invalid tensor identity work layout"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
