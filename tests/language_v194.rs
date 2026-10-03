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
fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn imported_nominal_type_parameters_shadow_module_symbols_in_source_free_replay() {
    let root = std::env::temp_dir().join(format!("rewind-v194-generics-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("helper.rw"),
        r#"
pub fn T()->Int effects {} {return 9;}
pub struct Box<T> {value:T}
pub enum Choice<T> {Item(T),Empty}
pub type Alias<T> = Option<T>;
pub fn value()->Int effects {} {
    let b:Box<Int> = Box(7);
    let c:Choice<Int> = Choice::Item(b.value);
    let alias:Alias<Int> = Some(3);
    assert_eq(alias,Some(3));
    match c {Choice::Item(v)=>{return v+T();},Choice::Empty=>{return T();}}
}

"#,
    )
    .unwrap();
    fs::write(
        root.join("main.rw"),
        "import helper as h;Out.println(h.value());publish;",
    )
    .unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    fs::remove_file(root.join("helper.rw")).unwrap();
    let out = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    ok(&out);
    assert_eq!(out.stdout, b"16\n");
    let replay = call(&root, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn history_budget_is_recorded_and_replay_rejects_changed_limits() {
    let root = std::env::temp_dir().join(format!("rewind-v194-budget-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), "Out.println(7);publish;").unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--history-memory",
            "1MiB",
            "--history-storage",
            "2MiB",
            "--spill-threshold",
            "0",
            "--record",
            "trace.json",
        ],
    );
    ok(&out);
    let trace: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("trace.json")).unwrap()).unwrap();
    assert_eq!(trace["history_budget"]["history_memory"], 1024 * 1024);
    assert_eq!(trace["history_budget"]["history_storage"], 2 * 1024 * 1024);
    assert_eq!(trace["history_budget"]["spill_threshold"], 0);
    let replay = call(&root, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    let changed = call(&root, &["replay", "trace.json", "--history-memory", "2MiB"]);
    assert!(!changed.status.success());
    assert!(String::from_utf8_lossy(&changed.stderr).contains("ReplayMismatch: history budget"));
    for value in ["0", "-1", "18446744073709551615GiB", "1MB", "word"] {
        let invalid = call(&root, &["run", "main.rwc", "--history-memory", value]);
        assert!(!invalid.status.success(), "accepted invalid budget {value}");
    }
    fs::remove_dir_all(root).unwrap();
}
