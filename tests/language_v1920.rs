use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn transient_task_profiles_are_bounded_and_source_free_replay_is_stable() {
    let root = std::env::temp_dir().join(format!("rewind-v1920-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("main.rw"),
        r#"
async fn work()->Int effects {} {return 7;}
var rounds=2000;let arguments=Args.all();if arguments.get(0)=="short" {rounds=300;}
for i in 0..rounds {assert_eq(await work(),Ok(7));}
let kept=work();commit held;
assert_eq(await kept,Ok(7));
revert held;
assert_eq(await kept,Ok(7));drop held;
for i in 0..100 {assert_eq(await work(),Ok(7));}
Out.println("bounded");publish;
"#,
    )
    .unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    for mode in ["debug", "compact"] {
        let output = call(
            &root,
            &[
                "profile",
                "main.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
                "--steps",
                "2000000",
                "--native-work",
                "2000000",
                "--task-steps",
                "100",
                "--",
                if mode == "debug" { "short" } else { "long" },
            ],
        );
        ok(&output);
        assert_eq!(output.stdout, b"bounded\n");
        let profile: serde_json::Value = String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|v| v.get("task_instructions").is_some())
            .unwrap();
        let counters = profile["task_instructions"].as_object().unwrap();
        assert!(
            counters.len() < 80,
            "historical task counters retained: {}",
            counters.len()
        );
        let retained: u64 = counters.values().map(|v| v.as_u64().unwrap()).sum();
        assert!(
            profile["task_instruction_total"].as_u64().unwrap()
                > retained + if mode == "debug" { 300 } else { 2000 }
        );
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(replay.stdout, output.stdout);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn restoring_a_cold_task_cannot_reset_its_consumed_instruction_budget() {
    let root = std::env::temp_dir().join(format!("rewind-v1920-budget-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let function =
        "async fn work()->Int effects {} {var sum=0;for i in 0..30 {sum+=i;}return sum;}";
    fs::write(
        root.join("main.rw"),
        format!("{function}\nlet task=work();assert_eq(await task,Ok(435));publish;"),
    )
    .unwrap();
    let baseline = call(&root, &["profile", "main.rw"]);
    ok(&baseline);
    let profile: serde_json::Value = String::from_utf8_lossy(&baseline.stderr)
        .lines()
        .filter_map(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .find(|v| v.get("task_instructions").is_some())
        .unwrap();
    let consumed = profile["task_instructions"]["1"].as_u64().unwrap();
    assert!(consumed > 30);
    let budget = (consumed + consumed / 2).to_string();
    fs::write(root.join("main.rw"), format!("{function}\nlet task=work();commit cold;assert_eq(await task,Ok(435));revert cold;assert_eq(await task,Err(TaskError::BudgetExceeded(BudgetKind::TaskSteps)));Out.println(\"quota retained\");publish;")).unwrap();
    let output = call(&root, &["run", "main.rw", "--task-steps", &budget]);
    ok(&output);
    assert_eq!(output.stdout, b"quota retained\n");
    fs::remove_dir_all(root).unwrap();
}
