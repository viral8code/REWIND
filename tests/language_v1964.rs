use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1964-{}-{}",
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
fn chunked_input_matches_sync_and_restores_unfinished_owned_work_source_free() {
    for (length, mode) in [(4097, "debug"), (16385, "compact")] {
        let root = root();
        let source = r#"import std.numeric as n;import std.numericInputAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let dims=List<Int>();dims.add(16385);let zeros=take(n.zerosFloat(&dims));let prepared=take(n.affine(&zeros,1.0,0.25));let values=take(n.valuesFloat(&prepared));
let expected=take(n.fromFloat(&dims,&values));let job=spawn jobs.fromFloat(move dims,move values);task.yieldNow();assert(!job.isDone());commit copying;
let first=take(take(await job));assert_eq(first,expected);Out.println("copied");publish;
revert copying;assert(!job.isDone());let second=take(take(await job));assert_eq(second,expected);drop copying;Out.println("restored");publish;
let ints=List<Int>();ints.add(-9223372036854775807-1);ints.add(-1);ints.add(9223372036854775807);let shape=List<Int>();shape.add(3);
let expectedInt=take(n.fromInt(&shape,&ints));assert_eq(take(take(await spawn jobs.fromInt(move shape,move ints))),expectedInt);
let wrong=List<Int>();wrong.add(2);let bad=List<Float>();bad.add(1.0);assert_eq(take(await spawn jobs.fromFloat(move wrong,move bad)),Err(StdError("NumericShape",0)));
let empty=List<Int>();empty.add(0);let expectedEmpty=take(n.zerosInt(&empty));let noValues=List<Int>();assert_eq(take(take(await spawn jobs.fromInt(move empty,move noValues))),expectedEmpty);
Out.println("input done");publish;
"#.replace("16385", &length.to_string());
        fs::write(root.join("main.rw"), source).unwrap();
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
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "copied\nrestored\ninput done\n"
        );
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn repeated_cancelled_input_work_releases_unretained_numeric_pages() {
    let mut baseline = None;
    for count in [0, 64, 256] {
        let root = root();
        let source = format!(
            r#"import std.numeric as n;import std.numericInputAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {{}} {{match move r{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
let dimensions=List<Int>();dimensions.add(8193);let zero=take(n.zerosFloat(&dimensions));let material=take(n.affine(&zero,1.0,0.25));let values=take(n.valuesFloat(&material));let frozen=freeze(move values);var cancelled=0;
for i in 0..{count}{{let shape=List<Int>();shape.add(8193);let copy=thaw(frozen);let job=spawn jobs.fromFloat(move shape,move copy);for turn in 0..1{{task.yieldNow();}}assert(!job.isDone());job.cancel();assert_eq(await job,Err(TaskError::Cancelled));cancelled+=1;}}
Out.println(cancelled);publish;"#
        );
        fs::write(root.join("main.rw"), source).unwrap();
        let out = call(
            &root,
            &[
                "profile",
                "main.rw",
                "--history-memory",
                "32MiB",
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
        let profile: serde_json::Value =
            serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap())
                .unwrap();
        let live = profile["numeric_pages"]["live_bytes"].as_u64().unwrap();
        if count == 0 {
            baseline = Some(live);
        }
        assert!(live < baseline.unwrap() + 4 * 1024 * 1024, "{profile}");
        let quotas = profile["task_instructions"].as_object().unwrap();
        let tasks = profile["scheduler"]["tasks"].as_object().unwrap();
        assert_eq!(quotas.len(), tasks.len());
        assert!(tasks.len() < 64, "{profile}");
        if count > 0 {
            assert!(profile["gc"]["completed"].as_u64().unwrap() > 0);
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn input_chunks_are_rejected_by_the_previous_language_version() {
    let root = root();
    fs::write(
        root.join("rewind.toml"),
        "language = \"1.9.63\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(root.join("main.rw"),"let shape=List<Int>();let values=List<Float>();stdNumericFromFloatInit(&shape,&values);publish;").unwrap();
    ok(&call(&root, &["update"]));
    let out = call(&root, &["check"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("requires language 1.9.64"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
