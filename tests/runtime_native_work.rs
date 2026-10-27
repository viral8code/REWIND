//! Native-work accounting: the limit can be set from the source, a host limit still wins, and a
//! large live heap no longer makes every allocation expensive.
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-nativework-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    root
}
fn rewind(root: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// More native work than the default allows: 400,000 reads from a list.
const READS: &str = "fn main() -> Int {
    let xs = List<Int>();
    var k = 0;
    while k < 100 { xs.add(k); k = k + 1; }
    var i = 0;
    var s = 0;
    while i < 400000 { s = s + xs.get(i % 100); i = i + 1; }
    Out.println(s);
    return 0;
}
";

#[test]
fn the_default_native_work_stops_a_long_loop() {
    let root = project(&format!(
        "runtime {{ executionSteps = 100000000; }}\n{READS}"
    ));
    let out = rewind(&root, &["run", "main.rw"]);
    assert_eq!(out.status.code(), Some(71), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("NativeWorkBudgetExceeded"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn native_work_can_be_raised_from_the_source() {
    let root = project(&format!(
        "runtime {{ executionSteps = 100000000; nativeWork = 50000000; }}\n{READS}"
    ));
    let out = rewind(&root, &["run", "main.rw"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "19800000");
}

#[test]
fn native_work_can_be_lowered_from_the_source() {
    let root = project(
        "runtime { nativeWork = 10; }\nfn main() -> Int {
    let xs = List<Int>();
    var i = 0;
    while i < 1000 { xs.add(i); i = i + 1; }
    return 0;
}
",
    );
    let out = rewind(&root, &["run", "main.rw"]);
    assert_eq!(out.status.code(), Some(71), "{}", stderr(&out));
}

#[test]
fn a_host_limit_cannot_be_raised_by_the_source() {
    let root = project(&format!(
        "runtime {{ executionSteps = 100000000; nativeWork = 500000000; }}\n{READS}"
    ));
    let out = rewind(&root, &["run", "main.rw", "--native-work", "100000"]);
    assert_eq!(out.status.code(), Some(71), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("NativeWorkBudgetExceeded"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_gui_permission_raises_the_default() {
    let root = project(&format!(
        "runtime {{ executionSteps = 100000000; }}\n{READS}"
    ));
    let out = rewind(&root, &["run", "main.rw", "--allow-effects", "output,gui"]);
    assert!(out.status.success(), "{}", stderr(&out));
}

/// A program that holds a few thousand values and then allocates a lot. Before 4.1.0 every 256th
/// allocation walked all of them, which used up the default budget long before the loop ended.
#[test]
fn a_large_live_heap_does_not_make_allocation_expensive() {
    let root = project(
        "runtime { executionSteps = 100000000; }
record P { a: Int, b: Int }
fn make(i: Int) -> P { return P(i, 1); }
fn main() -> Int {
    let keep = List<P>();
    var k = 0;
    while k < 2000 { keep.add(make(k)); k = k + 1; }
    var i = 0;
    var s = 0;
    while i < 60000 { let p = make(i); s = s + p.a; i = i + 1; }
    Out.println(s);
    return 0;
}
",
    );
    let out = rewind(&root, &["run", "main.rw"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1799970000");
}

#[test]
fn the_profile_shows_collector_work_on_its_own_row() {
    let root = project(
        "runtime { executionSteps = 100000000; }
record P { a: Int, b: Int }
fn make(i: Int) -> P { return P(i, 1); }
fn main() -> Int {
    let keep = List<P>();
    var k = 0;
    while k < 2000 { keep.add(make(k)); k = k + 1; }
    var i = 0;
    while i < 20000 { let p = make(i); i = i + 1; }
    return 0;
}
",
    );
    let out = rewind(&root, &["profile", "main.rw"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stderr(&out);
    let line = text
        .lines()
        .rev()
        .find(|l| l.starts_with('{'))
        .expect("profile json");
    let profile: Value = serde_json::from_str(line).unwrap();
    let row = profile["attribution"]["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["function"] == "<gc>")
        .unwrap_or_else(|| panic!("no <gc> row in {profile}"));
    assert!(row["native_work"].as_u64().unwrap() > 0);
    // The constructor is no longer where the collector's work is hidden.
    let make = profile["attribution"]["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["function"] == "make")
        .unwrap();
    assert!(make["native_work"].as_u64().unwrap() < 5 * 22_000, "{make}");
}
