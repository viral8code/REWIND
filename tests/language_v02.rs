use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture(source: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "rewind-v02-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let script = root.join("main.rw");
    fs::write(&script, source).unwrap();
    (root, script)
}
fn command(mode: &str, script: &Path, root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(mode)
        .arg(script)
        .arg("--root")
        .arg(root)
        .output()
        .unwrap()
}

#[test]
fn functions_loops_structs_and_collections_run() {
    let (root, script) = fixture(
        r#"
        fn fib(n: Int) -> Int {
            if n < 2 { return n; }
            return fib(n - 1) + fib(n - 2);
        }
        struct Item { name: String, count: Int, }
        let item = Item("total", fib(7));
        let xs = List<Int>();
        xs.add(item.count);
        let table = Map<String,Int>();
        table.set("b", 2);
        table.set("a", xs.get(0));
        let keys = table.keys();
        assert(keys.get(0) == "a");
        assert(keys.get(1) == "b");
        Out.println(table.get("a"));
        publish;
    "#,
    );
    let output = command("run", &script, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Some(13)\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_collections_reject_wrong_elements_during_check() {
    let (root, script) = fixture("let xs = List<Int>();\nxs.add(\"bad\");\n");
    let output = command("check", &script, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("List<Int> cannot contain String"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_reports_positioned_type_errors_without_host_effects() {
    let (root, script) = fixture("let value: Int = \"wrong\";\nFile.writeText(\"x\", \"bad\");\n");
    let output = command("check", &script, &root);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("1:1: type mismatch"), "{error}");
    assert!(!root.join("x").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn result_propagation_defer_and_virtual_output() {
    let (root, script) = fixture(
        r#"
        fn load(path: String) -> Result<String,FileError> {
            defer Out.println("closed");
            let text = File.readText(path)?;
            return Ok(text);
        }
        match load("missing.txt") {
            Ok(text) => Out.println(text),
            Err(error) => Out.println("failed"),
        }
        publish;
    "#,
    );
    let output = command("run", &script, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"closed\nfailed\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn test_functions_have_independent_virtual_files() {
    let (root, _script) = fixture(
        r#"
        test fn first() { File.writeText("shared", "a"); }
        test fn second() {
            match File.readText("shared") {
                Ok(value) => assert(false),
                Err(error) => assert(true),
            }
        }
    "#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("test")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.join("shared").exists());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("ok first") && error.contains("ok second"),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cyclic_import_is_rejected() {
    let (root, script) = fixture("import a;\n");
    fs::write(root.join("a.rw"), "import main;\n").unwrap();
    let output = command("check", &script, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cyclic import"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoint_restores_collection_and_struct_mutations() {
    let (root, script) = fixture(
        r#"
        struct Counter { value: Int, }
        let counter = Counter(1);
        let values = List<Int>();
        values.add(1);
        let map = Map<Int,String>();
        map.set(2, "two");
        commit base;
        branch other {
            counter.value = 9;
            values.add(9);
            map.set(1, "one");
        }
        assert(counter.value == 1);
        assert(values.len() == 1);
        assert(map.get(1) == None);
        counter.value = 3;
        values.add(3);
        revert base;
        assert(counter.value == 1);
        assert(values.len() == 1);
        Out.println(map.get(2));
        publish;
    "#,
    );
    let output = command("run", &script, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Some(two)\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn revert_in_function_keeps_continuation_and_trace_is_separate() {
    let (root, script) = fixture(
        r#"
        fn example() -> Int {
            var n = 4;
            commit inside;
            n = 9;
            revert inside;
            return n;
        }
        Out.println(example());
        publish;
    "#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(&script)
        .arg("--root")
        .arg(&root)
        .arg("--trace")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"4\n");
    let trace = String::from_utf8_lossy(&output.stderr);
    assert!(
        trace.contains("checkpoint=inside") && trace.contains("stdin_cursor=0"),
        "{trace}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resume_restarts_at_saved_program_counter_until_execution_budget() {
    let (root, script) = fixture("runtime { executionSteps = 30; }\ncommit base;\nresume base;\n");
    let output = command("run", &script, &root);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("ExecutionBudgetExceeded"), "{error}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resume_restores_completed_function_frame_and_return_site() {
    let (root, script) = fixture(
        r#"
        runtime { executionSteps = 80; }
        fn source() -> Int {
            commit inside;
            return 7;
        }
        let value = source();
        assert(value == 7);
        resume inside;
    "#,
    );
    let output = command("run", &script, &root);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("ExecutionBudgetExceeded"), "{error}");
    assert!(!error.contains("InvalidContinuation"), "{error}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn revert_rejects_missing_function_continuation() {
    let (root, script) = fixture(
        r#"
        fn source() -> Unit { commit inside; }
        source();
        revert inside;
    "#,
    );
    let output = command("run", &script, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("InvalidContinuation"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn result_payloads_and_float_comparisons_follow_types() {
    let (root, script) = fixture("fn wrong() -> Result<Int,FileError> { return Ok(\"text\"); }");
    let output = command("check", &script, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("return type mismatch"));
    fs::write(
        &script,
        r#"
        let nan = 0.0 / 0.0;
        assert(nan != nan);
        assert(-0.0 == 0.0);
        Out.println(1.5 + 2.0);
        publish;
    "#,
    )
    .unwrap();
    let output = command("run", &script, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"3.5\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn published_output_survives_revert_without_duplication() {
    let (root, script) = fixture(
        r#"
        Out.println("first");
        commit base;
        publish;
        revert base;
        Out.println("second");
        publish;
    "#,
    );
    let output = command("run", &script, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"first\nsecond\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn logical_operators_short_circuit_and_bytes_round_trip() {
    let (root, script) = fixture(
        r#"
        assert((false && 1 / 0 == 0) == false);
        assert(true || 1 / 0 == 0);
        let data = Bytes("abc");
        File.writeBytes("data.bin", data);
        match File.readBytes("data.bin") {
            Ok(value) => assert(value == data),
            Err(error) => assert(false),
        }
        publish;
    "#,
    );
    let output = command("run", &script, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(root.join("data.bin")).unwrap(), b"abc");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn v01_example_runs_through_v02_command_without_translation() {
    let (root, script) = fixture(include_str!("../examples/route.rw"));
    let output = command("run", &script, &root);
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
fn imports_initialize_once_in_dependency_order() {
    let (root, script) = fixture("import b;\nimport a;\nOut.println(\"main\");\npublish;\n");
    fs::write(root.join("a.rw"), "Out.println(\"a\");\n").unwrap();
    fs::write(root.join("b.rw"), "import a;\nOut.println(\"b\");\n").unwrap();
    let output = command("run", &script, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"a\nb\nmain\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn while_for_break_continue_and_else_have_block_scopes() {
    let (root, script) = fixture(
        r#"
        var i = 0;
        var total = 0;
        while i < 10 {
            i += 1;
            if i % 2 == 0 { continue; }
            if i > 7 { break; }
            total += i;
        }
        for n in 0..3 {
            if n == 1 { continue; }
            total += n;
        }
        if total == 18 { Out.println("good"); }
        else { Out.println("bad"); }
        publish;
    "#,
    );
    let output = command("run", &script, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"good\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn file_handle_supports_binary_read_seek_write_and_deferred_close() {
    let (root, script) = fixture(
        r#"
        fn read_tail() -> Bytes {
            let handle = File.open("data.bin");
            defer handle.close();
            handle.seek(1);
            return handle.readBytes(2);
        }
        File.writeBytes("data.bin", Bytes("abc"));
        assert(read_tail() == Bytes("bc"));
        let writer = File.open("data.bin");
        writer.seek(3);
        writer.writeBytes(Bytes("d"));
        writer.close();
        publish;
    "#,
    );
    let output = command("run", &script, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(root.join("data.bin")).unwrap(), b"abcd");
    fs::remove_dir_all(root).unwrap();
}
