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
