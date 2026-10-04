use std::{fs, process::Command};
#[test]
fn source_free_live_gui_reuses_held_input_cancels_without_consuming_and_releases_observations() {
    let root = std::env::temp_dir().join(format!("rewind-v1922-gui-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/gui-live/main.rw"),
    )
    .unwrap();
    fs::write(root.join("events.json"),r#"[{"window":"one","event":{"kind":"key","x":0,"y":0,"key":"日本語","width":0,"height":0}}]"#).unwrap();
    let effects = "gui,tasks,external,live";
    let call = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap()
    };
    let compiled = call(&["compile", "main.rw", "--allow-effects", effects]);
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    fs::remove_file(root.join("main.rw")).unwrap();
    let result = call(&[
        "profile",
        "main.rwc",
        "--allow-effects",
        effects,
        "--gui-window-events",
        "events.json",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"done\n");
    let profile: serde_json::Value = String::from_utf8_lossy(&result.stderr)
        .lines()
        .filter_map(|v| serde_json::from_str(v).ok())
        .last()
        .unwrap();
    let runtime = &profile["runtime"];
    assert_eq!(runtime["external_live"]["retained_operations"], 0);
    assert_eq!(runtime["gui_windows"]["observed"].as_u64().unwrap_or(0), 0);
    let denied = call(&[
        "run",
        "main.rwc",
        "--allow-effects",
        effects,
        "--gui-window-events",
        "events.json",
        "--record",
        "trace.json",
    ]);
    assert!(!denied.status.success());
    assert!(denied.stdout.is_empty());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("ExternalLiveRecordingUnsupported"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn live_gui_requires_the_live_boundary_and_remains_main_task_only() {
    let root = std::env::temp_dir().join(format!("rewind-v1922-effects-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let effects = "gui,tasks,external,live";
    for source in [
        "import std.guiWindows as windows;let input=windows.nextEventAnyLiveAsync();",
        "import std.guiWindows as windows;external {let input=windows.nextEventAnyLiveAsync();}",
        "import std.guiWindows as windows;async fn forbidden()->Unit effects {gui,tasks,external,live} {external live {let input=windows.nextEventAnyLiveAsync();}}",
    ] {
        fs::write(root.join("main.rw"),source).unwrap();
        let output=Command::new(env!("CARGO_BIN_EXE_rewind")).current_dir(&root).args(["check","main.rw","--allow-effects",effects]).output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("ExternalBoundary")||String::from_utf8_lossy(&output.stderr).contains("GuiMainTaskOnly"),"{}",String::from_utf8_lossy(&output.stderr));
    }
    fs::remove_dir_all(root).unwrap();
}
