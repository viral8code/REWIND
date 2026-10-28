//! Language 4.2.0 additions: bit operators, borrowing a temporary, the integer `match` table, and the
//! GUI helpers that need no window (`ellipse`, `setBounds`, `setColor`, `waitEvent` on a fixture).
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-extras-{}-{}",
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
fn run(source: &str) -> (String, String) {
    let out = rewind(&project(source), &["run", "main.rw"]);
    (
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n"),
    )
}

#[test]
fn bit_operators_follow_rust_precedence() {
    let (out, err) = run("fn main() -> Int {
    let a = 12;
    let b = 10;
    let xs = List<List<Int>>();
    xs.add(List<Int>());
    Out.println(a & b);
    Out.println(a | b);
    Out.println(a ^ b);
    Out.println(1 << 40);
    Out.println(-16 >> 2);
    Out.println(a & 1 == 0);
    Out.println((a >> 2 & 1) + 1 << 1);
    Out.println(xs.len());
    return 0;
}
");
    assert_eq!(out, "8\n14\n6\n1099511627776\n-4\ntrue\n4\n1\n", "{err}");
}

#[test]
fn a_shift_amount_outside_the_word_is_an_error_and_floats_are_rejected() {
    let (_, err) = run("fn main() -> Int {\n    Out.println(1 << 64);\n    return 0;\n}\n");
    assert!(err.contains("shift amount"), "{err}");
    let (_, err) = run("fn main() -> Int {\n    Out.println(1.5 & 2.5);\n    return 0;\n}\n");
    assert!(err.contains("bit operators require Int"), "{err}");
}

#[test]
fn a_temporary_can_be_borrowed_in_a_call() {
    let (out, err) = run("fn total(xs: &List<Int>, extra: Int) -> Int {
    var n = extra;
    for x in xs { n += x; }
    return n;
}
fn main() -> Int {
    Out.println(total(&List<Int>(), 3));
    return 0;
}
");
    assert_eq!(out, "3\n", "{err}");
}

#[test]
fn an_integer_match_uses_a_table_and_keeps_first_arm_semantics() {
    let (out, err) = run("fn pow3(n: Int) -> Int {
    return match n { 0 => 1, 1 => 3, 2 => 9, 3 => 27, 4 => 81, _ => -1 };
}
fn pick(n: Int) -> Int {
    let r = match n { 5 => 50, 1 => 10, 3 => 30, _ => { let k = n * 2; k } };
    return r;
}
fn main() -> Int {
    Out.println(pow3(0) + pow3(3));
    Out.println(pow3(4));
    Out.println(pow3(9));
    Out.println(pick(1) + pick(5) + pick(8));
    return 0;
}
");
    assert_eq!(out, "28\n81\n-1\n76\n", "{err}");
}

#[test]
fn ellipse_set_bounds_set_color_and_wait_event_work_on_a_fixture() {
    let root = project(
        "import std.gui as gui;
fn go() -> Result<Unit, gui.GuiError> effects {gui, output} {
    var view = gui.window(\"t\", 200, 200)?;
    gui.ellipse(&mut view, \"c\", 10, 10, 50, 50, 0x00aa00, 0x000000)?;
    gui.setBounds(&mut view, \"c\", 20, 20, 60, 60)?;
    gui.setColor(&mut view, \"c\", 0xff0000)?;
    match gui.setBounds(&mut view, \"c\", 190, 190, 60, 60) { Err(_) => { Out.println(\"bounds\"); }, Ok(_) => { Out.println(\"?\"); } }
    gui.present(&view)?;
    publish;
    match gui.waitEvent(50)? { None => { Out.println(\"idle\"); }, Some(e) => { Out.println(e.kind); } }
    return Ok(());
}
fn main() -> Int effects {gui, output} {
    match go() { Ok(_) => { return 0; }, Err(_) => { Out.println(\"err\"); return 1; } }
}
",
    );
    fs::write(
        root.join("events.json"),
        r#"[{"kind":"idle","x":0,"y":0,"key":"","width":0,"height":0}]"#,
    )
    .unwrap();
    let out = rewind(
        &root,
        &[
            "run",
            "main.rw",
            "--allow-effects",
            "gui,output",
            "--gui-events",
            "events.json",
        ],
    );
    let text = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    assert_eq!(
        text,
        "bounds\nidle\n",
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_project_can_import_std_without_a_lock_or_a_std_dependency() {
    let root = project("import std.text as text;\nfn main() -> Int {\n    Out.println(\"ok\");\n    return 0;\n}\n");
    fs::write(
        root.join("rewind.toml"),
        "language = \"4.2.0\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    for args in [&["run", "--root", "."][..], &["run", "main.rw"][..]] {
        let out = rewind(&root, args);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "ok\n",
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn a_missing_source_root_or_entry_names_the_path() {
    let root = project("fn main() -> Int { return 0; }\n");
    fs::write(
        root.join("rewind.toml"),
        "language = \"4.2.0\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    let err = String::from_utf8_lossy(&rewind(&root, &["run", "--root", "."]).stderr).into_owned();
    assert!(err.contains("source_root `src` was not found"), "{err}");
    fs::write(
        root.join("rewind.toml"),
        "language = \"4.2.0\"\nsource_root = \".\"\nentry = \"nope.rw\"\n",
    )
    .unwrap();
    let err = String::from_utf8_lossy(&rewind(&root, &["run", "--root", "."]).stderr).into_owned();
    assert!(err.contains("entry file `nope.rw` was not found"), "{err}");
}

#[test]
fn bit_helpers_wide_literals_logical_shift_and_complement() {
    let (out, err) = run("fn main() -> Int {
    let a = 0xFEFEFEFEFEFEFEFE;
    Out.println(a);
    Out.println(~0);
    Out.println(a >>> 60);
    Out.println(-1 >>> 1);
    Out.println((1 << 63).lowestBit());
    Out.println((1 << 63).wrappingSub(1));
    Out.println(0xFF.countOnes() + 8.trailingZeros() + 1.leadingZeros());
    Out.println(9223372036854775807.wrappingAdd(1));
    let xs = List<List<List<Int>>>();
    xs.add(List<List<Int>>());
    Out.println(xs.len());
    return 0;
}
");
    assert_eq!(
        out,
        "-72340172838076674\n-1\n15\n9223372036854775807\n-9223372036854775808\n9223372036854775807\n74\n-9223372036854775808\n1\n",
        "{err}"
    );
}

#[test]
fn next_event_skips_the_idle_entries_of_a_timed_wait() {
    let root = project(
        "import std.gui as gui;
fn go() -> Result<Unit, gui.GuiError> effects {gui, output} {
    var view = gui.window(\"t\", 200, 200)?;
    gui.present(&view)?;
    publish;
    let e = gui.nextEvent()?;
    Out.println(e.kind);
    match gui.waitEvent(-1)? { None => { Out.println(\"none\"); }, Some(e) => { Out.println(e.kind); } }
    return Ok(());
}
fn main() -> Int effects {gui, output} {
    match go() { Ok(_) => { return 0; }, Err(_) => { Out.println(\"err\"); return 1; } }
}
",
    );
    fs::write(
        root.join("events.json"),
        r#"[{"kind":"idle","x":0,"y":0,"key":"","width":0,"height":0},{"kind":"pointer","x":1,"y":1,"key":"","width":0,"height":0},{"kind":"idle","x":0,"y":0,"key":"","width":0,"height":0},{"kind":"close","x":0,"y":0,"key":"","width":0,"height":0}]"#,
    )
    .unwrap();
    let out = rewind(
        &root,
        &[
            "run",
            "main.rw",
            "--allow-effects",
            "gui,output",
            "--gui-events",
            "events.json",
        ],
    );
    let text = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    assert_eq!(
        text,
        "pointer\nclose\n",
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_stale_lock_is_ignored_without_dependencies_and_a_mismatch_names_the_field() {
    let root = project("fn main() -> Int {\n    Out.println(5.countOnes());\n    return 0;\n}\n");
    fs::write(
        root.join("rewind.toml"),
        "language = \"4.3.0\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    assert!(rewind(&root, &["update", "--root", "."]).status.success());
    let lock = fs::read_to_string(root.join("rewind.lock")).unwrap();
    fs::write(root.join("rewind.lock"), lock.replace("4.3.0", "4.2.1")).unwrap();
    let out = rewind(&root, &["run", "--root", "."]);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        "2\n",
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn int_methods_cost_no_native_work() {
    let (out, err) = run("runtime { nativeWork = 100000; }
fn main() -> Int {
    var i = 0;
    var n = 0;
    while i < 20000 {
        n += i.countOnes() + i.trailingZeros() + i.lowestBit();
        i += 1;
    }
    Out.println(n > 0);
    return 0;
}
");
    assert_eq!(out, "true\n", "{err}");
}

#[test]
fn small_pure_functions_give_the_same_results_when_called_with_any_arguments() {
    let (out, err) = run("fn shift(x: Int, d: Int) -> Int {
    return match d { 0 => x << 8, 1 => x >> 8, 2 => x << 1, _ => x };
}
fn twice(n: Int) -> Int { return n + n; }
fn bump() -> Int { return 7; }
fn main() -> Int {
    let a = 3;
    let d = 2;
    Out.println(shift(a, d));
    Out.println(shift(a + 1, d - 1));
    Out.println(shift(twice(a), twice(1)));
    Out.println(twice(twice(twice(1))));
    Out.println(twice(-5));
    Out.println(bump() + twice(a));
    let n = 10;
    var i = 0;
    var sum = 0;
    while i < n { sum += shift(i, i % 3); i += 1; }
    Out.println(sum);
    return 0;
}
");
    assert_eq!(out, "6\n0\n12\n8\n-10\n13\n4638\n", "{err}");
}

#[test]
fn a_local_closure_named_like_a_function_is_not_replaced_by_that_function() {
    let (out, err) = run("fn f(x: Int) -> Int { return x + 1; }
fn main() -> Int {
    let g = |x: Int| -> Int { return x * 100; };
    let f = g;
    Out.println(f(2));
    return 0;
}
");
    assert_eq!(out, "200\n", "{err}");
}

#[test]
fn profile_reports_a_wall_clock_reference() {
    let root = project("fn main() -> Int { return 0; }\n");
    let out = rewind(&root, &["profile", "main.rw", "--profile-format", "text"]);
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("wall time (reference only)"), "{text}");
}
