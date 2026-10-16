//! LANG-03: mixing units or ID kinds is a type error before anything runs.
use std::{fs, process::Command};

fn check(source: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!(
        "rewind-units-{}-{}",
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

const HEAD: &str = "import std.units as units;\nrecord UserTag {}\nrecord OrderTag {}\n";

#[test]
fn matching_units_and_id_kinds_are_accepted() {
    let (ok, err) = check(&format!(
        "{HEAD}fn wait(s: units.Quantity<units.Second>) -> Int {{ return units.amount(s); }}\nfn main() -> Int {{ return wait(units.of<units.Second>(1)); }}\n"
    ));
    assert!(ok, "{err}");
}

#[test]
fn milliseconds_cannot_be_passed_as_seconds() {
    let (ok, err) = check(&format!(
        "{HEAD}fn wait(s: units.Quantity<units.Second>) -> Int {{ return units.amount(s); }}\nfn main() -> Int {{ return wait(units.of<units.Milli>(1)); }}\n"
    ));
    assert!(!ok);
    assert!(
        err.contains("Quantity<Second>") || err.contains("Quantity<"),
        "{err}"
    );
}

#[test]
fn ids_of_different_kinds_do_not_mix() {
    let (ok, err) = check(&format!(
        "{HEAD}fn find(id: units.Id<UserTag>) -> String {{ return units.text(id); }}\nfn main() -> Int {{\n  match units.idOf<OrderTag>(\"o1\") {{ Ok(id) => {{ Out.println(find(id)); }}, Err(_) => {{}} }}\n  return 0;\n}}\n"
    ));
    assert!(!ok);
    assert!(err.contains("Id<"), "{err}");
}

#[test]
fn the_wrapped_value_is_private() {
    let (ok, err) = check(&format!(
        "{HEAD}fn main() -> Int {{ let s = units.of<units.Second>(1); return s.value; }}\n"
    ));
    assert!(!ok, "{err}");
}
