//! LANG-05: the consuming Result/Option helpers hand a non-Share owner on exactly once.
use std::{fs, process::Command};

fn check(source: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!(
        "rewind-owned-{}-{}",
        std::process::id(),
        source.len()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("main.rw"), source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .args(["check", "main.rw"])
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const HEAD: &str = "import std.result as result;\n";

#[test]
fn a_moved_owner_is_accepted() {
    let (ok, err) = check(&format!(
        "{HEAD}fn main() -> Int {{ let start: Result<List<Int>, String> = Ok(List<Int>()); let sized = result.mapOwned(move start, |o: List<Int>| -> Int {{ return o.len(); }}); return 0; }}\n"
    ));
    assert!(ok, "{err}");
}

#[test]
fn the_result_cannot_be_used_after_it_was_consumed() {
    let (ok, _) = check(&format!(
        "{HEAD}fn main() -> Int {{ let start: Result<List<Int>, String> = Ok(List<Int>()); let sized = result.mapOwned(move start, |o: List<Int>| -> Int {{ return o.len(); }}); let again = result.mapOwned(move start, |o: List<Int>| -> Int {{ return o.len(); }}); return 0; }}\n"
    ));
    assert!(!ok);
}

#[test]
fn the_owner_cannot_be_passed_without_move() {
    let (ok, _) = check(&format!(
        "{HEAD}fn main() -> Int {{ let start: Result<List<Int>, String> = Ok(List<Int>()); let sized = result.mapOwned(start, |o: List<Int>| -> Int {{ return o.len(); }}); return 0; }}\n"
    ));
    assert!(!ok);
}
