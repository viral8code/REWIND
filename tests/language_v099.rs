use std::{fs, process::Command};
#[test]
fn vm_collection_and_replay_preserve_live_checkpoint_values() {
    let root = std::env::temp_dir().join(format!("rewind-v099-replay-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let file = root.join("main.rw");
    let trace = root.join("trace.json");
    fs::write(&file,"var i=0;let kept=List<Int>();kept.add(7);commit base;while i<700{let scratch=List<Int>();scratch.add(i);i+=1;}Out.println(kept.get(0));publish;revert base;Out.println(kept.get(0));publish;").unwrap();
    let first = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run",
            file.to_str().unwrap(),
            "--steps",
            "2000000",
            "--native-work",
            "2000000",
            "--record",
            trace.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stdout, b"7\n7\n");
    let recorded: serde_json::Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
    assert!(
        recorded["debug"]["final"]["runtime"]["heap_objects"]
            .as_u64()
            .unwrap()
            < 100
    );
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "replay",
            trace.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert_eq!(replay.stdout, first.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn native_work_is_charged_before_large_codec_operations() {
    let root = std::env::temp_dir().join(format!("rewind-v099-work-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let file = root.join("main.rw");
    fs::write(&file, "stdEncode(\"abcdefghijklmnop\");").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["run", file.to_str().unwrap(), "--native-work", "4"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("NativeWorkBudgetExceeded"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn cold_tasks_keep_moved_arguments_alive_across_collection() {
    let root = std::env::temp_dir().join(format!("rewind-v099-task-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let file = root.join("main.rw");
    fs::write(&file,"async fn count(items:List<Int>)->Int effects {} {return items.get(0);}fn make()->Task<Int> effects {tasks} {let items=List<Int>();items.add(9);return count(move items);}let pending=make();var i=0;while i<600{let scratch=List<Int>();scratch.add(i);i+=1;}assert_eq(await pending,Ok(9));").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["run", file.to_str().unwrap(), "--steps", "2000000"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn dependency_aliases_are_module_local_and_canonical_across_import_paths() {
    let root = std::env::temp_dir().join(format!("rewind-v099-alias-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("leaf.rw"),
        "pub fn value()->Int effects {} {return 7;}",
    )
    .unwrap();
    fs::write(
        root.join("a.rw"),
        "import leaf as util;pub fn value()->Int effects {} {return util.value()+1;}",
    )
    .unwrap();
    fs::write(
        root.join("b.rw"),
        "import leaf as util;pub fn value()->Int effects {} {return util.value()+2;}",
    )
    .unwrap();
    let file = root.join("main.rw");
    fs::write(&file,"import a as a;import b as b;import std.graph as graph;import std.integer as integer;assert_eq(a.value(),8);assert_eq(b.value(),9);assert_eq(integer.gcd(42,30),Ok(6));").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["compile", file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let artifact = root.join("main.rwc");
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["run", artifact.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    fs::write(&file, "import a as a;assert_eq(util.value(),7);").unwrap();
    let denied = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["run", file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!denied.status.success());
    fs::remove_dir_all(root).unwrap();
}
