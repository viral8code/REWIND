use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn project() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v100-language-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn run(root: &std::path::Path, source: &str) -> std::process::Output {
    let file = root.join("main.rw");
    fs::write(&file, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run",
            file.to_str().unwrap(),
            "--steps",
            "2000000",
            "--native-work",
            "2000000",
        ])
        .output()
        .unwrap()
}
fn success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn opaque_fields_reject_reads_writes_construction_and_patterns_but_allow_factories() {
    let root = project();
    fs::write(root.join("bag.rw"), "pub struct Bag{private value:Int}pub fn make(value:Int)->Bag effects {} {return Bag(value);}pub fn get(bag:&Bag)->Int effects {} {return bag.value;}").unwrap();
    let result=run(&root,"import bag as bag;let value=bag.make(9);assert_eq(bag.get(&value),9);commit old;publish;revert old;assert_eq(bag.get(&value),9);");
    success(&result);
    for source in [
        "import bag as bag;let value=bag.make(9);Out.println(value.value);",
        "import bag as bag;let value=bag.make(9);value.value=2;",
        "import bag as bag;let value=bag.Bag(9);",
        "import bag as bag;let value=bag.make(9);match value{bag.Bag{value:x}=>{Out.println(x);}}",
    ] {
        let result = run(&root, source);
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("private"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn expected_factories_multiple_bounds_and_where_survive_source_free_execution() {
    let root = project();
    fs::write(
        root.join("factory.rw"),
        "pub fn empty<T:Share>()->List<T> effects {} {return List<T>();}",
    )
    .unwrap();
    let source="import factory as factory;fn empty<T:Share>()->List<T> effects {} {return factory.empty();}fn order<T>(a:T,b:T)->Int effects {} where T:Ord+Share{return a.cmp(b);}let items:List<Int>=empty();items.add(9);assert_eq(items.get(0),9);assert_eq(order(1,2),-1);Out.println(\"ok\");publish;";
    let result = run(&root, source);
    success(&result);
    assert_eq!(result.stdout, b"ok\n");
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["compile", root.join("main.rw").to_str().unwrap()])
        .output()
        .unwrap();
    success(&result);
    fs::remove_file(root.join("main.rw")).unwrap();
    fs::remove_file(root.join("factory.rw")).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(root.join("main.rwc"))
        .output()
        .unwrap();
    success(&result);
    assert_eq!(result.stdout, b"ok\n");
    let result = run(
        &root,
        "fn check<T:Ord+Share>(a:T)->Unit effects {} {}let xs=List<Int>();check(&xs);",
    );
    assert!(!result.status.success());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn native_budget_covers_json_string_members_and_concat_before_output() {
    for source in [
        "jsonParse(\"abcdefghijklmnop\");",
        "let text=\"abcdefghijklmnop\";text.display();",
        "let text=\"abcdefgh\"+\"ijklmnop\";Out.println(text);publish;",
    ] {
        let root = project();
        fs::write(root.join("main.rw"), source).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
            .args([
                "run",
                root.join("main.rw").to_str().unwrap(),
                "--native-work",
                "4",
            ])
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("NativeWorkBudgetExceeded"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn compiler_rejects_deep_syntax_and_oversized_source_with_diagnostics() {
    for source in [
        format!("let x={}true;", "!".repeat(200)),
        " ".repeat(1024 * 1024 + 1),
    ] {
        let root = project();
        let result = run(&root, &source);
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("CompilerBudgetExceeded"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn explicit_host_step_ceiling_cannot_be_reset_by_source_runtime_directives() {
    let root = project();
    fs::write(root.join("main.rw"),"runtime {executionSteps=1000000;}var i=0;while i<100{runtime {executionSteps=1000000;}i+=1;}Out.println(i);publish;").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run",
            root.join("main.rw").to_str().unwrap(),
            "--steps",
            "100",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("ExecutionBudgetExceeded"));
    assert!(result.stdout.is_empty());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn collection_keeps_channel_payload_closure_and_deferred_function_arguments() {
    let root = project();
    let source="fn churn()->Unit effects {} {var i=0;while i<600{let scratch=List<Int>();scratch.add(i);i+=1;}}fn deferred(items:List<Int>)->Unit effects {output} {defer Out.println(items.get(0));churn();}let channel=Channel<List<Int>>(1);let sent=List<Int>();sent.add(7);assert_eq(await channel.send(move sent),Ok(()));let keep=List<Int>();keep.add(9);let getter=| |->Int{return keep.get(0);};churn();assert_eq(getter(),9);match await channel.receive(){Ok(value)=>{assert_eq(value.get(0),7);deferred(move value);},Err(_)=>{panic(\"channel\");}}publish;";
    let result = run(&root, source);
    success(&result);
    assert_eq!(result.stdout, b"7\n");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn public_api_diff_tracks_failure_and_cost_but_ignores_private_representation() {
    let root = project();
    fs::write(
        root.join("rewind.toml"),
        "language=\"1.0.0\"\nsource_root=\".\"\nentry=\"main.rw\"\neffects=\"\"\n",
    )
    .unwrap();
    let original="pub struct Box{private value:Int}pub fn make()->Box effects {} {return Box(7);}\n// @api make cost O(1)\n// @api make failure No recoverable errors; fatal allocation budget applies.\n";
    fs::write(root.join("main.rw"), original).unwrap();
    let cmd = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .args(args)
            .output()
            .unwrap()
    };
    success(&cmd(&["update", "--root", root.to_str().unwrap()]));
    let before = root.join("before.json");
    let after = root.join("after.json");
    success(&cmd(&[
        "api-snapshot",
        "--root",
        root.to_str().unwrap(),
        "--output",
        before.to_str().unwrap(),
    ]));
    fs::write(
        root.join("main.rw"),
        original
            .replace("private value:Int", "private value:Int,private extra:Bool")
            .replace("Box(7)", "Box(7,true)"),
    )
    .unwrap();
    success(&cmd(&[
        "api-snapshot",
        "--root",
        root.to_str().unwrap(),
        "--output",
        after.to_str().unwrap(),
    ]));
    success(&cmd(&[
        "api-diff",
        before.to_str().unwrap(),
        after.to_str().unwrap(),
        "--deny-breaking",
    ]));
    fs::write(root.join("main.rw"), original.replace("O(1)", "O(n)")).unwrap();
    success(&cmd(&[
        "api-snapshot",
        "--root",
        root.to_str().unwrap(),
        "--output",
        after.to_str().unwrap(),
    ]));
    assert!(!cmd(&[
        "api-diff",
        before.to_str().unwrap(),
        after.to_str().unwrap(),
        "--deny-breaking"
    ])
    .status
    .success());
    fs::remove_dir_all(root).unwrap();
}
