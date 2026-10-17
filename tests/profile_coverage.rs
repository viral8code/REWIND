use std::fs;
use std::process::Command;

const SOURCE: &str = "fn pick(x:Int)->Int {
    if x>0 {
        return 1;
    }
    return 2;
}
fn unused()->Int {
    return 9;
}
fn main()->Int {
    return pick(5);
}
";

#[test]
fn profile_reports_the_lines_that_never_ran() {
    let root = std::env::temp_dir().join(format!("rewind-coverage-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("cv.rw"), SOURCE).unwrap();
    let json = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&root)
        .args(["profile", "cv.rw", "--profile-format", "json"])
        .output()
        .unwrap();
    let report: serde_json::Value = serde_json::from_slice(
        String::from_utf8_lossy(&json.stderr)
            .lines()
            .next()
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let coverage = &report["coverage"];
    assert_eq!(coverage["complete"], true);
    let sources = coverage["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 1);
    // Counted by hand: lines 2,3,10,11 run; 5 (fall-through after the taken branch) and 8 do not.
    assert_eq!(sources[0]["executable_lines"], 6);
    assert_eq!(sources[0]["covered_lines"], 4);
    assert_eq!(sources[0]["uncovered"], serde_json::json!([5, 8]));
    let text = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&root)
        .args(["profile", "cv.rw", "--profile-format", "text"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&text.stderr)
        .contains("coverage cv.rw: 4/6 lines (uncovered: [5,8])"));
    fs::remove_dir_all(&root).ok();
}
