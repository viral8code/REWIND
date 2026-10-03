use std::{fs, process::Command};
#[test]
fn completed_task_releases_arguments_but_preserves_result_and_cold_checkpoint() {
    let root = std::env::temp_dir().join(format!("rewind-v192-task-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let source=format!("async fn work(value:String)->Int effects {{}} {{return 7;}}let pending=work(\"{}\");commit cold;assert_eq(await pending,Ok(7));Out.println(7);publish;revert cold;assert_eq(await pending,Ok(7));Out.println(7);assert_eq(await pending,Ok(7));Out.println(7);publish;","x".repeat(61440));
    fs::write(root.join("main.rw"), source).unwrap();
    let call = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap()
    };
    let compiled = call(&["compile", "main.rw"]);
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(&["run", "main.rwc", "--record", "trace.json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"7\n7\n7\n");
    let trace: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("trace.json")).unwrap()).unwrap();
    let bytes = trace["debug"]["final"]["scheduler"]["logical_storage_bytes"]
        .as_u64()
        .unwrap();
    assert!(bytes < 8192, "completed scheduler retains {} bytes", bytes);
    let replay = call(&["replay", "trace.json"]);
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
