use std::{fs, process::Command};

#[test]
fn gui_controls2_example_runs_from_window_events_without_a_window() {
    let dir = std::env::temp_dir().join(format!("rewind-gui-controls2-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("main.rw"),
        include_str!("../examples/gui-controls2.rw"),
    )
    .unwrap();
    fs::write(
        dir.join("events.json"),
        include_str!("../examples/gui-controls2.events.json"),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .args([
            "run",
            "main.rw",
            "--allow-effects",
            "gui",
            "--gui-window-events",
            "events.json",
            "--native-work",
            "100000000",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let expected = "A||Red|2026-10-21\n".replace("A||", "A|");
    let _ = expected;
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(
        lines,
        [
            "A|Red|2026-10-21",
            "Ad|Red|2026-10-21",
            "Ada|Red|2026-10-21",
            "Ada|Red|2026-10-21",
            "Ada|Red|2026-10-21",
            "Ada|Green|2026-10-21",
            "Ada|Green|2026-10-21",
            "Ada|Green|2026-10-21",
            "Ada|Green|2026-10-22",
            "Ada|Green|2026-10-22",
        ]
    );
    fs::remove_dir_all(dir).unwrap();
}
