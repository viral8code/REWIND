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
