use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v1926-{}-{}",
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
fn source_free_fft_hands_off_and_restores_mid_kernel_state_in_both_trace_modes() {
    for mode in ["debug", "compact"] {
        let root = root();
        fs::write(
            root.join("main.rw"),
            include_str!("../examples/fft-async/main.rw"),
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
            "fft done\n"
        );
        let trace: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("trace.json")).unwrap()).unwrap();
        assert!(trace["schedule_choices"].as_array().unwrap().len() > 30);
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(replay.stdout, out.stdout);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn cancellation_after_an_initial_chunk_does_not_return_a_partial_spectrum() {
    let root = root();
    fs::write(root.join("main.rw"),r#"import std.numeric as numeric;import std.fftAsync as fft;import std.task as task;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(65536);let zero=take(numeric.zerosFloat(&shape));let real=take(numeric.mapFloat("exp",&zero));
let work=spawn fft.transform(real,zero,false);task.yieldNow();assert(!work.isDone());work.cancel();match await work{Err(TaskError::Cancelled)=>{Out.println("cancelled");},_=>{panic("partial FFT escaped");}}publish;"#).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rw",
            "--native-work",
            "100000000",
            "--steps",
            "20000000",
        ],
    );
    ok(&out);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        "cancelled\n"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn prior_language_user_records_are_not_reclassified_as_new_native_server_handles() {
    let root = root();
    fs::write(
        root.join("rewind.toml"),
        "language = \"1.9.24\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(root.join("main.rw"),r#"record HttpServer{code:Int}let values=List<HttpServer>();values.add(HttpServer(7));assert_eq(values.get(0).code,7);publish;"#).unwrap();
    ok(&call(&root, &["update"]));
    ok(&call(&root, &["run", "main.rw"]));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn cancelled_fft_loops_release_pages_and_task_quota_metadata() {
    for count in [128, 512] {
        let root = root();
        let source = format!(
            r#"import std.numeric as numeric;import std.fftAsync as fft;import std.task as task;
fn take<T,E>(value:Result<T,E>)->T effects {{}} {{match move value{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
let shape=List<Int>();shape.add(16384);let zero=take(numeric.zerosFloat(&shape));let real=take(numeric.mapFloat("exp",&zero));var done=0;
for i in 0..{count}{{let work=spawn fft.transform(real,zero,false);task.yieldNow();work.cancel();match await work{{Err(TaskError::Cancelled)=>{{done+=1;}},_=>{{panic("cancel");}}}}}}
Out.println(done);publish;"#
        );
        fs::write(root.join("main.rw"), source).unwrap();
        let out = call(
            &root,
            &[
                "profile",
                "main.rw",
                "--native-work",
                "250000000",
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
        assert!(
            profile["numeric_pages"]["live_bytes"].as_u64().unwrap() < 4 * 1024 * 1024,
            "{profile}"
        );
        let quotas = profile["task_instructions"].as_object().unwrap();
        let tasks = profile["scheduler"]["tasks"].as_object().unwrap();
        assert_eq!(quotas.len(), tasks.len());
        assert!(quotas.len() < 64, "{profile}");
        assert!(profile["gc"]["completed"].as_u64().unwrap() > 0);
        fs::remove_dir_all(root).unwrap();
    }
}
