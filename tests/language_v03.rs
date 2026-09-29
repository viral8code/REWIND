use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn primitive_standard_traits_and_generic_bounds() {
    let (root, path) = fixture(
        r#"
        fn display<T: Display>(value: T) -> String { return value.display(); }
        fn compare<T: Ord>(a: T, b: T) -> Int { return a.cmp(b); }
        assert_eq(display(42), "42");
        assert_eq(compare(1, 2), -1);
        assert_eq("a".cmp("b"), -1);
        assert(3.eq(3));
        assert_eq("same".hash(), "same".hash());
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_generics_and_higher_order_functions() {
    let (root, path) = fixture(
        r#"
        fn identity<T>(value: T) -> T { return value; }
        fn call(f: fn()->Int) -> Int { return f(); }
        fn twice(f: fn(fn()->Int)->Int, g: fn()->Int) -> Int { return f(g) + f(g); }
        let inner = List<Int>(); inner.add(5);
        let outer = List<List<Int>>(); outer.add(inner);
        let copy = identity(outer);
        assert_eq(copy.get(0).get(0),5);
        assert_eq(twice(call, || -> Int { return 7; }),14);
        fn captured<T>(value: T) -> fn()->T { return || -> T { return value; }; }
        let captured_value = captured(8);
        assert_eq(captured_value(),8);
        enum Wrapped<T> { Some(T), None, }
        let x = Wrapped::Some(copy);
        match x { Wrapped::Some(v) => assert_eq(v.get(0).get(0),5), Wrapped::None => panic("missing"), }
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn read_only_directory_observation_is_validated_before_publish() {
    let (root, _) = fixture("");
    let mut runtime = rewind::Runtime::new(&root).unwrap();
    let entries = runtime.directory_entries(".").unwrap();
    runtime.commit("observed").unwrap();
    fs::write(root.join("new.txt"), "external").unwrap();
    runtime.revert("observed").unwrap();
    assert_eq!(runtime.directory_entries(".").unwrap(), entries);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let error = runtime.publish(false, &mut out, &mut err).unwrap_err();
    assert!(
        matches!(error, rewind::Error::ExternalStateConflict(_)),
        "{error}"
    );
    assert!(out.is_empty());
    assert!(err.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn directory_entries_include_virtual_created_directories_and_files() {
    let (root, path) = fixture(
        r#"
        Directory.create("virtual");
        File.writeText("virtual/item.txt","value");
        assert_eq(Directory.entries("virtual/").get(0),"item.txt");
        assert_eq(Directory.entries(".").get(1),"virtual");
        commit saved;
        assert_eq(Directory.entries("virtual/").len(),1);
        revert saved;
        assert_eq(Directory.entries("virtual").len(),1);
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.join("virtual").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn public_constant_alias_and_private_constant_boundary() {
    let (root, path) = fixture(
        "import values as v; assert_eq(v.answer,42); let read = v.read; assert_eq(read(),7);",
    );
    fs::write(root.join("values.rw"),"pub const answer: Int = 42; const secret: Int = 7; pub fn read() -> Int { return secret; }").unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"0.3\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(&path, "import values; assert_eq(secret,7);").unwrap();
    let output = command("check", &path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("private"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn alias_types_in_public_signatures_and_annotations() {
    let (root,path) = fixture("import item as i; fn read(x: i.Item) -> Int { return x.n; } let x: i.Item = i.Item(9); assert_eq(read(x),9);");
    fs::write(root.join("item.rw"), "pub struct Item { n: Int, }").unwrap();
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn diamond_import_initializes_shared_module_once() {
    let (root, path) = fixture("import left; import right; publish;");
    fs::write(
        root.join("common.rw"),
        "Out.println(\"once\"); fn common() -> Int { return 1; }",
    )
    .unwrap();
    fs::write(
        root.join("left.rw"),
        "import common; fn left() -> Int { return common(); }",
    )
    .unwrap();
    fs::write(
        root.join("right.rw"),
        "import common; fn right() -> Int { return common(); }",
    )
    .unwrap();
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"once\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn map_snapshots_nested_mutable_keys() {
    let (root, path) = fixture(
        r#"
        struct Key { values: List<Int>, }
        impl Ord for Key { fn cmp(self: Self, other: Self) -> Int { return self.values.get(0) - other.values.get(0); } }
        let values = List<Int>(); values.add(1);
        let map = Map<Key,String>(); map.set(Key(values),"saved");
        values.set(0,9);
        let lookup = List<Int>(); lookup.add(1);
        assert_eq(map.get(Key(lookup)),Some("saved"));
        assert_eq(map.keys().get(0).values.get(0),1);
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resume_restores_completed_closure_frame_and_captured_cell() {
    let (root, path) = fixture(
        r#"
        runtime { executionSteps = 200; }
        var n = 0;
        let next = || -> Int { commit inside; n += 1; return n; };
        assert_eq(next(),1);
        assert_eq(n,1);
        resume inside;
    "#,
    );
    let output = command("run", &path, &root);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("ExecutionBudgetExceeded"), "{error}");
    assert!(!error.contains("assertion failed"), "{error}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn match_requires_full_payload_coverage_and_type_depth_is_bounded() {
    let (root, path) =
        fixture("enum E { One(Int), } let x = E::One(2); let y = match x { E::One(1) => 1 }; ");
    let output = command("check", &path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("non-exhaustive"));
    let deep = format!(
        "fn f(x: {}Int{}) -> Unit {{}}",
        "List<".repeat(80),
        ">".repeat(80)
    );
    fs::write(&path, deep).unwrap();
    let output = command("check", &path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("TypeExpansionBudgetExceeded"));
    fs::remove_dir_all(root).unwrap();
}

fn fixture(source: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "rewind-v03-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let path = root.join("main.rw");
    fs::write(&path, source).unwrap();
    (root, path)
}
fn command(mode: &str, path: &Path, root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(mode)
        .arg(path)
        .arg("--root")
        .arg(root)
        .output()
        .unwrap()
}

#[test]
fn generic_functions_and_structs() {
    let (root, path) = fixture(
        r#"
        fn identity<T>(value: T) -> T { let copy: T = value; return copy; }
        struct Pair<A,B> { first: A, second: B, }
        let pair = Pair(identity(42), "value");
        assert_eq(identity<Int>(7),7);
        fn empty<T>() -> List<T> { return List<T>(); }
        assert_eq(empty<Int>().len(),0);
        assert(pair.first == 42);
        assert(pair.second == "value");
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn trait_bound_and_method() {
    let (root, path) = fixture(
        r#"
        struct Item { n: Int, }
        trait Render { fn render(self: Self) -> String; }
        impl Render for Item {
            fn render(self: Self) -> String { return "value=" + self.n; }
        }
        fn show<T: Render>(value: T) -> String { return value.render(); }
        Out.println(show(Item(7)));
        publish;
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"value=7\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn enum_match_and_checkpoint() {
    let (root, path) = fixture(
        r#"
        enum Command { Write(String,Int), Quit, }
        fn describe(command: Command) -> String {
            match command {
                Command::Write(path, _) => return path,
                Command::Quit => return "quit",
            }
        }
        let before = Command::Write("a", 1);
        commit base;
        let after = Command::Quit;
        assert(describe(after) == "quit");
        revert base;
        assert(describe(before) == "a");
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn closure_cell_is_restored_with_checkpoint() {
    let (root, path) = fixture(
        r#"
        fn counter(start: Int) -> fn()->Int {
            var current = start;
            return || -> Int {
                current += 1;
                return current;
            };
        }
        let next = counter(10);
        assert(next() == 11);
        commit saved;
        assert(next() == 12);
        revert saved;
        assert(next() == 12);
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn environment_observations_and_directory_entries() {
    let (root, path) = fixture(
        r#"
        let args = Args.all();
        assert(args.get(0) == "alpha");
        assert(Locale.current() == "ja-JP");
        let entries = Directory.entries(".");
        assert(entries.get(0) == "main.rw");
        commit observed;
        assert(Env.get("REWIND_V03_TEST") == Some("sample"));
        revert observed;
        assert(Env.get("REWIND_V03_TEST") == Some("sample"));
    "#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .arg("--allow-env")
        .arg("REWIND_V03_TEST")
        .arg("--locale")
        .arg("ja-JP")
        .arg("--")
        .arg("alpha")
        .env("REWIND_V03_TEST", "sample")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deferred_closure_runs_on_return() {
    let (root, path) = fixture(
        r#"
        fn work() -> Unit {
            var n = 1;
            defer || -> Unit { Out.println(n); };
            n = 2;
            return;
        }
        work();
        publish;
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"2\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn using_closes_handle_before_return() {
    let (root, path) = fixture(
        r#"
        fn acquire() -> FileHandle {
            using handle = File.open("data.txt");
            return handle;
        }
        let closed = acquire();
        closed.read(1);
    "#,
    );
    fs::write(root.join("data.txt"), "content").unwrap();
    let output = command("run", &path, &root);
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("handle"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn manifest_lock_and_module_visibility() {
    let (root, _) = fixture("");
    let source = root.join("src");
    let dependency = root.join("vendor").join("dep");
    fs::create_dir(&source).unwrap();
    fs::create_dir_all(&dependency).unwrap();
    fs::write(root.join("rewind.toml"),"language = \"0.3\"\nsource_root = \"src\"\nentry = \"main.rw\"\n[dependencies]\ndep = \"vendor/dep\"\n").unwrap();
    fs::write(
        dependency.join("math.rw"),
        "fn hidden() -> Int { return 9; }\npub fn value() -> Int { return hidden(); }\n",
    )
    .unwrap();
    fs::write(
        source.join("main.rw"),
        "import dep.math;\nassert_eq(value(), 9);\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(root.join("rewind.lock").exists());
    fs::write(source.join("main.rw"), "import dep.math;\nhidden();\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("check")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("private"));
    fs::write(
        dependency.join("math.rw"),
        "pub fn value() -> Int { return 10; }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("check")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("lock"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn module_alias_and_selective_import() {
    let (root, _) = fixture("");
    let source = root.join("src");
    fs::create_dir(&source).unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"0.3\"\nsource_root = \"src\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(
        source.join("math.rw"),
        "pub fn twice(value: Int) -> Int { return value * 2; }\n",
    )
    .unwrap();
    fs::write(
        source.join("main.rw"),
        "import math as m;\nassert_eq(m.twice(3), 6);\n",
    )
    .unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .arg("run")
            .arg("--root")
            .arg(&root)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(
        source.join("main.rw"),
        "import math.{twice as double};\nassert_eq(double(3), 6);\n",
    )
    .unwrap();
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(source.join("main.rw"), "import math as m;\ntwice(3);\n").unwrap();
    let output = run();
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown function"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn aliases_resolve_colliding_exports() {
    let (root, _) = fixture("");
    let source = root.join("src");
    fs::create_dir(&source).unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"0.3\"\nsource_root = \"src\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(source.join("a.rw"), "pub fn value() -> Int { return 1; }\n").unwrap();
    fs::write(source.join("b.rw"), "pub fn value() -> Int { return 2; }\n").unwrap();
    fs::write(
        source.join("main.rw"),
        "import a as a;\nimport b as b;\nassert_eq(a.value() + b.value(), 3);\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn aliases_resolve_colliding_types_and_enum_variants() {
    let (root, _) = fixture("");
    let source = root.join("src");
    fs::create_dir(&source).unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"0.3\"\nsource_root = \"src\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    fs::write(source.join("a.rw"),"pub struct Item { n: Int, }\npub enum Choice { Yes, No, }\npub fn make() -> Item { return Item(1); }\n").unwrap();
    fs::write(
        source.join("b.rw"),
        "pub struct Item { n: Int, }\npub fn make() -> Item { return Item(2); }\n",
    )
    .unwrap();
    fs::write(source.join("main.rw"),"import a as a;\nimport b as b;\nassert_eq(a.make().n, 1);\nassert_eq(b.Item(2).n, 2);\nlet choice = a.Choice::Yes;\nlet text = match choice { a.Choice::Yes => \"yes\", a.Choice::No => \"no\", };\nassert_eq(text, \"yes\");\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn named_enum_match_expression_and_guard() {
    let (root, path) = fixture(
        r#"
        enum Command { Write { path: String, count: Int }, Quit, }
        let command = Command::Write { count: 2, path: "a" };
        let text = match command {
            Command::Write { path: value, count: _ } => value,
            Command::Quit => "quit",
        };
        assert_eq(text, "a");
        let range = match 4 {
            0..3 => "small",
            n if n > 3 => "large",
            _ => "middle",
        };
        assert_eq(range,"large");
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn structured_file_error_and_json_trace() {
    let (root, path) = fixture(
        r#"
        match File.readText("missing.txt") {
            Ok(value) => assert(false),
            Err(error) => assert_eq(error.code, "NotFound"),
        }
        commit point;
    "#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .arg("--trace-json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let trace = String::from_utf8_lossy(&output.stderr);
    assert!(trace.contains("\"checkpoint\":\"point\""), "{trace}");
    assert!(trace.contains("\"env\":0"), "{trace}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn formatter_and_public_api_documentation() {
    let (root,path) = fixture("/// Returns the answer.\n/// Example: `answer()` is 42.\npub fn answer()->Int {\nreturn 42;\n}\n");
    let fmt = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("fmt")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        fmt.status.success(),
        "{}",
        String::from_utf8_lossy(&fmt.stderr)
    );
    assert!(fs::read_to_string(&path)
        .unwrap()
        .contains("    return 42;"));
    let check = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("fmt")
        .arg(&path)
        .arg("--check")
        .output()
        .unwrap();
    assert!(check.status.success());
    let doc = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("doc")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        doc.status.success(),
        "{}",
        String::from_utf8_lossy(&doc.stderr)
    );
    assert!(String::from_utf8_lossy(&doc.stdout).contains("pub fn answer() -> Int"));
    assert!(String::from_utf8_lossy(&doc.stdout).contains("Example: `answer()` is 42."));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn map_uses_custom_ord_implementation() {
    let (root, path) = fixture(
        r#"
        struct Key { n: Int, }
        impl Ord for Key {
            fn cmp(self: Self, other: Self) -> Int { return other.n - self.n; }
        }
        let map = Map<Key,String>();
        map.set(Key(1), "one");
        map.set(Key(3), "three");
        map.set(Key(2), "two");
        let keys = map.keys();
        assert_eq(keys.get(0).n, 3);
        assert_eq(keys.get(1).n, 2);
        assert_eq(keys.get(2).n, 1);
        assert_eq(map.get(Key(2)), Some("two"));
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn orphan_trait_implementation_is_rejected() {
    let (root,path) = fixture("import item;\nimpl Ord for Item { fn cmp(self: Self, other: Self) -> Int { return 0; } }\n");
    fs::write(root.join("item.rw"), "pub struct Item { value: Int, }\n").unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"0.3\"\nsource_root = \".\"\nentry = \"main.rw\"\n",
    )
    .unwrap();
    let output = command("check", &path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("orphan impl"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deferred_closure_cannot_publish_indirectly() {
    let (root, path) = fixture(
        r#"
        fn forbidden() -> Unit { publish; }
        fn work() -> Unit {
            defer || -> Unit { forbidden(); };
        }
        work();
    "#,
    );
    let output = command("check", &path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("publish"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn match_guard_and_dynamic_cleanup_cannot_publish() {
    let (root, path) = fixture(
        r#"
        fn irreversible() -> Bool { publish; return true; }
        let x = match true { true if irreversible() => 1, _ => 2 };
    "#,
    );
    let output = command("check", &path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("match guard"));
    fs::write(
        &path,
        r#"
        fn irreversible() -> Unit { publish; }
        fn work() -> Unit { let action = irreversible; defer || -> Unit { action(); }; }
    "#,
    )
    .unwrap();
    let output = command("check", &path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cleanup"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn using_and_defer_run_at_scope_exit_and_question_mark() {
    let (root, path) = fixture(
        r#"
        fn failed() -> Result<Int,FileError> {
            using resource = File.open("data.txt");
            defer || -> Unit { Out.println("last"); };
            File.readText("missing.txt")?;
            return Ok(1);
        }
        fn scoped() -> Unit {
            { defer || -> Unit { Out.println("scope"); }; }
            Out.println("after");
        }
        File.writeText("data.txt","a");
        scoped();
        let result = failed();
        match result { Ok(_) => panic("unexpected"), Err(_) => Out.println("error"), }
        publish;
    "#,
    );
    let output = command("run", &path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"scope\nafter\nlast\nerror\n");
    fs::remove_dir_all(root).unwrap();
}
