use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn documented_route_publishes_only_selected_branch() {
    let root = std::env::temp_dir().join(format!(
        "rewind-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/route.rw");
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(script)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"start\n5\n");
    assert_eq!(
        fs::read(root.join("result.txt")).unwrap(),
        b"initial\n\nrouteB=5"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn list_handle_and_branch_state_restore() {
    let root = std::env::temp_dir().join(format!(
        "rewind-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let script = root.join("program.rw");
    fs::write(
        &script,
        r#"
        File.write("data.txt", "abc");
        var f = File.open("data.txt");
        var list = List();
        list.add(10);
        f.seek(100);
        commit base;
        branch routeA { list.add(20); f.seek(500); Out.println("discarded"); }
        list.add(30);
        f.seek(200);
        revert base;
        Out.println(list);
        Out.println(f.position);
        publish;
    "#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(&script)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[10]\n100\n");
    assert_eq!(fs::read(root.join("data.txt")).unwrap(), b"abc");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn runtime_budget_and_file_handle_reads_work_in_scripts() {
    let root = std::env::temp_dir().join(format!(
        "rewind-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let script = root.join("program.rw");
    fs::write(
        &script,
        r#"
        runtime { historyMemory = 1MB; historyStorage = 1MB; spillThreshold = 2; }
        File.create("data.txt");
        File.append("data.txt", "abcdef");
        var f = File.openSnapshot("data.txt");
        f.seek(2);
        Out.println(f.read(3));
        File.truncate("data.txt", 3);
        publish;
    "#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(&script)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"cde\n");
    assert_eq!(fs::read(root.join("data.txt")).unwrap(), b"abc");
    fs::remove_dir_all(root).unwrap();
}
