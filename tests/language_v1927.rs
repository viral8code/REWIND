use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v1927-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
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
fn source_free_sparse_hands_off_and_restores_mid_kernel_state_in_both_trace_modes() {
    for mode in ["debug", "compact"] {
        let root = root();
        fs::write(
            root.join("main.rw"),
            include_str!("../examples/sparse-async/main.rw"),
        )
        .unwrap();
        ok(&call(&root, &["compile", "main.rw"]));
        fs::remove_file(root.join("main.rw")).unwrap();
        let _ = fs::remove_dir_all(root.join(".rewind"));
        let out = call(
            &root,
            &[
                "run",
                "main.rwc",
                "--steps",
                "20000000",
                "--task-steps",
                "2000000",
                "--native-work",
                "100000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "sparse done\n"
        );
        let trace: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("trace.json")).unwrap()).unwrap();
        assert!(trace["schedule_choices"].as_array().unwrap().len() > 10);
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(replay.stdout, out.stdout);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn old_language_work_name_remains_user_defined_and_new_work_is_opaque() {
    let r = root();
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.26\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(
        r.join("main.rw"),
        "record SparseWork{code:Int}let x=SparseWork(7);assert_eq(x.code,7);publish;",
    )
    .unwrap();
    ok(&call(&r, &["update"]));
    ok(&call(&r, &["run", "main.rw"]));
    fs::remove_dir_all(&r).unwrap();
    let r = root();
    fs::write(r.join("main.rw"), "let forged=SparseWork();publish;").unwrap();
    assert!(!call(&r, &["compile", "main.rw"]).status.success());
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn cancelled_sparse_loops_release_pages_and_task_quota_metadata() {
    let mut input_bytes = None;
    for count in [0, 128, 512, 2048] {
        let root = root();
        let source = format!(
            r#"import std.numeric as numeric;import std.sparse as sparse;import std.sparseAsync as sparseAsync;import std.task as task;
fn take<T,E>(value:Result<T,E>)->T effects {{}} {{match move value{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
let shape=List<Int>();shape.add(16384);let zero=take(numeric.zerosFloat(&shape));let real=take(numeric.mapFloat("exp",&zero));
let offsetShape=List<Int>();offsetShape.add(16385);let offsets=take(numeric.zerosInt(&offsetShape));let emptyShape=List<Int>();emptyShape.add(0);let indices=take(numeric.zerosInt(&emptyShape));let values=take(numeric.zerosFloat(&emptyShape));let matrix=sparse.Matrix(16384,16384,offsets,indices,values);var done=0;
for i in 0..{count}{{let work=spawn sparseAsync.matvec(matrix,real);for turn in 0..6{{task.yieldNow();}}assert(!work.isDone());work.cancel();match await work{{Err(TaskError::Cancelled)=>{{done+=1;}},_=>{{panic("cancel");}}}}}}
Out.println(done);publish;"#
        );
        fs::write(root.join("main.rw"), source).unwrap();
        let out = call(
            &root,
            &[
                "profile",
                "main.rw",
                "--native-work",
                "20000000000",
                "--steps",
                "20000000",
                "--history-memory",
                "8MiB",
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
        let live_bytes = profile["numeric_pages"]["live_bytes"].as_u64().unwrap();
        // The GC threshold bounds allocations since the last collection, not
        // total storage: the fixture's reachable input arrays remain alive.
        if count == 0 {
            input_bytes = Some(live_bytes);
        }
        assert!(
            live_bytes < input_bytes.unwrap() + 4 * 1024 * 1024,
            "{profile}"
        );
        let quotas = profile["task_instructions"].as_object().unwrap();
        let tasks = profile["scheduler"]["tasks"].as_object().unwrap();
        assert_eq!(quotas.len(), tasks.len());
        assert!(quotas.len() < 64, "{profile}");
        if count > 0 {
            assert!(profile["gc"]["completed"].as_u64().unwrap() > 0);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
