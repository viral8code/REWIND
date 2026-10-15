//! PROF-01/02: work attribution and retention report in `rewind profile`.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

const SRC: &str = "fn leaf(n:Int)->Int {
    var total = 0;
    var i = 0;
    while i < n { total += i; i += 1; }
    return total;
}
fn rec(n:Int)->Int {
    if n == 0 { return 0; }
    return n + rec(n - 1);
}
fn mid()->Int {
    return leaf(50) + rec(10);
}
fn main()->Int {
    commit a;
    let xs = [1,2,3];
    commit b;
    Out.println(mid());
    return 0;
}
";

fn setup() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "rewind-prof-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&d).unwrap();
    let d = fs::canonicalize(&d).unwrap();
    fs::write(d.join("p.rw"), SRC).unwrap();
    d
}
fn rewind(d: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(d)
        .args(args)
        .output()
        .unwrap()
}
fn find<'a>(v: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    v["attribution"]["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["function"] == name)
        .unwrap_or_else(|| panic!("no function {name}"))
}

#[test]
fn attribution_is_exclusive_plus_inclusive_and_recursion_counts_once() {
    let d = setup();
    let o = rewind(&d, &["profile", "p.rw"]);
    assert!(o.status.success());
    let v: serde_json::Value = serde_json::from_slice(&o.stderr).unwrap();
    let (mid, leaf, rec) = (find(&v, "mid"), find(&v, "leaf"), find(&v, "rec"));
    assert_eq!(leaf["exclusive"], leaf["inclusive"]);
    assert_eq!(rec["exclusive"], rec["inclusive"]);
    let sum = mid["exclusive"].as_u64().unwrap()
        + leaf["inclusive"].as_u64().unwrap()
        + rec["inclusive"].as_u64().unwrap();
    assert_eq!(mid["inclusive"].as_u64().unwrap(), sum);
    assert!(v["attribution"]["meaning"]["inclusive"].is_string());
}

#[test]
fn profile_does_not_change_program_output() {
    let d = setup();
    let plain = rewind(&d, &["run", "p.rw"]);
    assert_eq!(String::from_utf8_lossy(&plain.stdout).trim(), "1280");
    let prof_json = rewind(&d, &["profile", "p.rw"]);
    assert_eq!(prof_json.stdout, plain.stdout);
    let prof = rewind(&d, &["profile", "p.rw", "--profile-format", "text"]);
    assert!(prof.status.success());
    let t = String::from_utf8_lossy(&prof.stderr);
    assert!(t.contains("leaf"), "{t}");
}

#[test]
fn retention_report_separates_shared_from_exclusive() {
    let d = setup();
    let o = rewind(&d, &["profile", "p.rw"]);
    let v: serde_json::Value = serde_json::from_slice(&o.stderr).unwrap();
    let r = &v["retention"];
    assert!(r["heap_unique_bytes"].as_u64().unwrap() <= r["heap_references_total_bytes"].as_u64().unwrap());
    let roots = r["roots"].as_array().unwrap();
    assert!(roots.iter().any(|x| x["root"] == "current"));
    assert!(roots
        .iter()
        .any(|x| x["root"] == "checkpoint:a" && !x["heap_shared_with"].as_array().unwrap().is_empty()));
    assert!(r["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("RSS")));
}
