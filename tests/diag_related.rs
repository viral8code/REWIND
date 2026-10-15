//! DIAG-01: related locations for ownership and effect diagnostics.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-diag-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    fs::canonicalize(&p).unwrap()
}
fn check(source: &str, json: bool) -> Output {
    let cwd = dir();
    fs::write(cwd.join("main.rw"), source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_rewind"));
    command
        .current_dir(&cwd)
        .env_remove("REWIND_STANDALONE_LANGUAGE");
    if json {
        command.args(["--diagnostic-format", "json"]);
    }
    command.args(["check", "main.rw"]).output().unwrap()
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

const MOVED: &str = "fn main()->Int {\n    let xs = List<Int>();\n    let ys = move xs;\n    xs.add(1);\n    return ys.len();\n}\n";
const BORROWED: &str = "fn main()->Int {\n    let xs = List<Int>();\n    xs.add(1);\n    let first = &xs;\n    xs.add(2);\n    Out.println(first.len());\n    return 0;\n}\n";

#[test]
fn use_after_move_points_at_the_move() {
    let o = check(MOVED, false);
    assert!(!o.status.success());
    let text = stderr(&o);
    assert!(text.contains("use after move"), "{text}");
    assert!(
        text.contains("note: value moved here (main.rw:3:"),
        "{text}"
    );
}

#[test]
fn borrow_conflict_points_at_the_borrow_start() {
    let o = check(BORROWED, false);
    let text = stderr(&o);
    assert!(
        text.contains("cannot mutate an owner while borrowed"),
        "{text}"
    );
    assert!(
        text.contains("note: borrow starts here (main.rw:4:"),
        "{text}"
    );
}

#[test]
fn json_diagnostic_carries_the_same_related_locations() {
    let o = check(BORROWED, true);
    let value: serde_json::Value =
        serde_json::from_str(stderr(&o).lines().last().unwrap()).unwrap();
    let related = &value["diagnostic"]["related"];
    assert_eq!(related[0]["message"], "borrow starts here");
    assert_eq!(related[0]["line"], 4);
    assert_eq!(value["exit_status"], 65);
}

#[test]
fn missing_effect_names_the_requesting_operation() {
    let o = check(
        "fn helper()->Int effects {} {\n    Out.println(1);\n    return 0;\n}\nfn main()->Int {\n    return helper();\n}\n",
        false,
    );
    let text = stderr(&o);
    assert!(text.contains("MissingEffect"), "{text}");
    assert!(
        text.contains("caused by[MissingEffect]: operation requires effect output"),
        "{text}"
    );
    assert!(text.contains("main.rw:2:5"), "{text}");
}

#[test]
fn notes_never_contain_local_values() {
    let o = check(
        "fn main()->Int {\n    let secret = \"hunter2-token\";\n    let xs = List<String>();\n    xs.add(secret);\n    let first = &xs;\n    xs.add(\"x\");\n    Out.println(first.len());\n    return 0;\n}\n",
        false,
    );
    assert!(!stderr(&o).contains("hunter2-token"), "{}", stderr(&o));
}

fn run(source: &str) -> Output {
    let cwd = dir();
    fs::write(cwd.join("main.rw"), source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&cwd)
        .env_remove("REWIND_STANDALONE_LANGUAGE")
        .args(["run", "main.rw"])
        .output()
        .unwrap()
}

#[test]
fn revert_after_the_creating_call_returned_explains_why_and_where() {
    let o = run("fn make()->Int {\n    commit inside;\n    return 1;\n}\nfn main()->Int {\n    let a = make();\n    revert inside;\n    return a;\n}\n");
    let text = stderr(&o);
    assert!(text.contains("InvalidContinuation"), "{text}");
    assert!(
        text.contains("checkpoint `inside` was created here, in make (main.rw:2:"),
        "{text}"
    );
    assert!(
        text.contains("the creating function has returned"),
        "{text}"
    );
}

#[test]
fn external_boundary_and_effect_hints_separate_declaration_from_permission() {
    let o = run("fn main()->Int {\n    external { publish; }\n    return 0;\n}\n");
    let text = stderr(&o);
    assert!(text.contains("ExternalBoundary"), "{text}");
    assert!(
        text.contains("do not await, publish, commit, revert"),
        "{text}"
    );
    let missing = check(
        "fn helper()->Int effects {} {\n    Out.println(1);\n    return 0;\n}\nfn main()->Int {\n    return helper();\n}\n",
        false,
    );
    assert!(
        stderr(&missing).contains("A declaration alone grants nothing"),
        "{}",
        stderr(&missing)
    );
}

#[test]
fn deadlock_prints_a_bounded_wait_graph() {
    let o = run("fn main()->Int {\n    let c = Channel<Int>(0);\n    await c.receive();\n    return 0;\n}\n");
    let text = stderr(&o);
    assert!(text.contains("TaskDeadlock"), "{text}");
    assert!(text.contains("wait graph: task 0 waits for task"), "{text}");
}
