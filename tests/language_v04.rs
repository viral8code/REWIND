use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn fixture(source: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "rewind-v04-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let path = root.join("main.rw");
    fs::write(&path, source).unwrap();
    (root, path)
}
fn command(path: &Path, root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(path)
        .arg("--root")
        .arg(root)
        .output()
        .unwrap()
}
fn successful(source: &str, expected: &[u8]) {
    let (root, path) = fixture(source);
    let output = command(&path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn async_tasks_execute_in_deterministic_order() {
    successful(
        r#"
    async fn worker(n:Int)->Int { Out.println(n); return n*2; }
    let a=spawn worker(1); let b=spawn worker(2);
    assert_eq(await b,Ok(4)); assert_eq(await a,Ok(2)); publish;
"#,
        b"1\n2\n",
    );
}
#[test]
fn channels_suspend_and_wake_tasks_fifo() {
    successful(
        r#"
    async fn producer(c:Channel<Int>)->Unit { assert_eq(await c.send(7),Ok(())); assert_eq(await c.send(8),Ok(())); c.close(); }
    async fn consumer(c:Channel<Int>)->Int {
        let a=await c.receive(); let b=await c.receive();
        assert_eq(a,Ok(7)); assert_eq(b,Ok(8));
        assert_eq(await c.receive(),Err("ChannelClosed")); return 15;
    }
    let c=Channel<Int>(1); let p=spawn producer(c); let result=spawn consumer(c);
    assert_eq(await result,Ok(15)); assert_eq(await p,Ok(()));
"#,
        b"",
    );
}
#[test]
fn rendezvous_channel_and_task_groups() {
    successful(
        r#"
    async fn producer(c:Channel<Int>)->Unit { assert_eq(await c.send(9),Ok(())); }
    let c=Channel<Int>(0); using group=TaskGroup(); let p=producer(c); group.add(p);
    assert_eq(await c.receive(),Ok(9)); assert_eq(await group.join(),Ok(()));
"#,
        b"",
    );
}
#[test]
fn checkpoint_restores_queued_values_and_completed_tasks() {
    successful(
        r#"
    async fn worker(c:Channel<Int>)->Int { assert_eq(await c.send(4),Ok(())); return 1; }
    let c=Channel<Int>(1); let task=spawn worker(c);
    commit before;
    assert_eq(await task,Ok(1)); assert_eq(await c.receive(),Ok(4));
    revert before;
    assert_eq(await task,Ok(1)); assert_eq(await c.receive(),Ok(4));
"#,
        b"",
    );
}
#[test]
fn deadlock_reports_wait_graph() {
    let (root, path) = fixture("let c=Channel<Int>(0); await c.receive();");
    let output = command(&path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("TaskDeadlock"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn cancelled_and_failed_tasks_return_errors() {
    successful(
        r#"
    async fn bad()->Int { panic("broken"); }
    async fn never()->Int { return 1; }
    let cancelled=spawn never(); cancelled.cancel(); assert_eq(await cancelled,Err("TaskCancelled"));
    let failure=spawn bad(); match await failure { Ok(_) => panic("unexpected"), Err(_) => assert(true), }
"#,
        b"",
    );
}

#[test]
fn record_replay_uses_observations_and_reports_divergence() {
    let (root, path) = fixture(
        r#"
        let content=File.readText("input.txt"); assert_eq(content,Ok("observed"));
        let env=Env.get("REWIND_V04_ENV"); assert_eq(env,Some("value"));
        let time=Time.now(); assert(time>0); commit observed;
        Out.println("done"); File.writeText("result.txt","virtual"); publish;
    "#,
    );
    fs::write(root.join("input.txt"), "observed").unwrap();
    let trace = root.join("record.json");
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .arg("--allow-env")
        .arg("REWIND_V04_ENV")
        .arg("--record")
        .arg(&trace)
        .env("REWIND_V04_ENV", "value")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(root.join("input.txt"), "changed").unwrap();
    fs::remove_file(root.join("result.txt")).unwrap();
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert_eq!(replay.stdout, b"done\n");
    assert!(!root.join("result.txt").exists());
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
    json["events"][0]["pc"] = serde_json::json!(999);
    fs::write(&trace, serde_json::to_vec(&json).unwrap()).unwrap();
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!replay.status.success());
    assert!(String::from_utf8_lossy(&replay.stderr).contains("ReplayMismatch: event 0"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn secret_values_are_redacted_from_record_and_debugger() {
    let (root,path)=fixture("let secret=Env.get(\"REWIND_V04_SECRET\"); commit saved; assert_eq(secret,Some(\"hidden-value\"));");
    let trace = root.join("record.json");
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("debug")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .arg("--secret-env")
        .arg("REWIND_V04_SECRET")
        .arg("--record")
        .arg(&trace)
        .env("REWIND_V04_SECRET", "hidden-value")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("hidden-value"));
    assert!(!fs::read_to_string(&trace).unwrap().contains("hidden-value"));
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!replay.status.success());
    assert!(String::from_utf8_lossy(&replay.stderr).contains("requires --secret-env"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn signed_packages_version_effects_and_reproducible_build() {
    let (root, path) = fixture("import math.api; assert_eq(answer(),42);");
    let package = root.join("vendor/math");
    fs::create_dir_all(&package).unwrap();
    fs::write(package.join("api.rw"), "pub fn answer()->Int {return 42;}").unwrap();
    fs::write(
        package.join("rewind.package.json"),
        r#"{"name":"math","version":"1.2.3","effects":[]}"#,
    )
    .unwrap();
    let seed = root.join("seed");
    fs::write(&seed, "07".repeat(32)).unwrap();
    let sign = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("sign")
        .arg(&package)
        .arg("--key")
        .arg(&seed)
        .output()
        .unwrap();
    assert!(sign.status.success());
    let public = String::from_utf8(sign.stdout).unwrap();
    fs::write(root.join("rewind.toml"),format!("language = \"0.4\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"\"\n[dependencies]\nmath = \"file:vendor/math\"\n[dependency_versions]\nmath = \"^1.2.0\"\n[dependency_signers]\nmath = \"author\"\n[trust]\nauthor = \"{}\"\n",public.trim())).unwrap();
    let update = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("update")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    let output = command(&path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let build = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("build")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let bytes = fs::read(root.join("rewind.build.json")).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(&root.to_string_lossy().to_string()));
    let build = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("build")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(build.status.success());
    assert_eq!(fs::read(root.join("rewind.build.json")).unwrap(), bytes);
    let (other, _) = fixture(&fs::read_to_string(&path).unwrap());
    fs::create_dir_all(other.join("vendor/math")).unwrap();
    for name in ["api.rw", "rewind.package.json", "rewind.signature"] {
        fs::copy(package.join(name), other.join("vendor/math").join(name)).unwrap();
    }
    for name in ["rewind.toml", "rewind.lock"] {
        fs::copy(root.join(name), other.join(name)).unwrap();
    }
    let build = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("build")
        .arg("--root")
        .arg(&other)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert_eq!(fs::read(other.join("rewind.build.json")).unwrap(), bytes);
    fs::remove_dir_all(other).unwrap();
    fs::write(package.join("api.rw"), "pub fn answer()->Int {return 43;}").unwrap();
    let output = command(&path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("signature verification failed"));
    let sign = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("sign")
        .arg(&package)
        .arg("--key")
        .arg(&seed)
        .output()
        .unwrap();
    assert!(sign.status.success());
    let output = command(&path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("rewind.lock mismatch"));
    let manifest = fs::read_to_string(root.join("rewind.toml")).unwrap();
    fs::write(
        root.join("rewind.toml"),
        manifest.replace("^1.2.0", "^2.0.0"),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("update")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not satisfy"));
    fs::write(
        root.join("rewind.toml"),
        manifest.replace("math = \"author\"", "math = \"unknown\""),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("update")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("untrusted package signer"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn undeclared_effect_and_library_publish_are_rejected() {
    let (root, path) = fixture("import helper; work();");
    fs::write(
        root.join("helper.rw"),
        "pub fn work()->Unit {Out.println(1);}",
    )
    .unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language = \"0.4\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    let update = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("update")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(update.status.success());
    let output = command(&path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("undeclared effect output"));
    fs::write(
        root.join("helper.rw"),
        "effects {output} pub fn work()->Unit {Out.println(1);}",
    )
    .unwrap();
    let output = command(&path, &root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(root.join("helper.rw"), "pub fn work()->Unit {publish;}").unwrap();
    let output = command(&path, &root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot publish"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn schedule_exploration_and_profile_report_tasks() {
    let (root, path) = fixture(
        r#"
        async fn producer(c:Channel<Int>,n:Int)->Unit {await c.send(n);}
        test fn choices()->Unit {
            let c=Channel<Int>(2);let a=spawn producer(c,1);let b=spawn producer(c,2);
            await a;await b;assert_eq(await c.receive(),Ok(1));
        }
    "#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("test")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .arg("--explore")
        .arg("20")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("schedule"));
    fs::write(
        &path,
        "async fn task()->Int {return 3;} let a=spawn task();assert_eq(await a,Ok(3));",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("profile")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(json["task_instructions"]["1"].as_u64().unwrap() > 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn suspended_cancellation_and_instruction_budget_run_cleanup() {
    successful(
        r#"
        async fn waiting(c:Channel<Int>)->Unit {
            defer Out.println("first"); defer Out.println("second");
            await c.send(1); await c.receive();
        }
        let c=Channel<Int>(0);let task=spawn waiting(c);
        assert_eq(await c.receive(),Ok(1)); task.cancel();
        assert_eq(await task,Err("TaskCancelled")); publish;
    "#,
        b"second\nfirst\n",
    );
    let (root, path) = fixture(
        r#"
        async fn forever()->Unit {defer Out.println("cleaned"); while true {}}
        let task=spawn forever(); match await task {Err(_) => assert(true),Ok(_) => panic("budget"),} publish;
    "#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .arg("--task-steps")
        .arg("50")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"cleaned\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn priority_function_values_and_task_data_are_isolated() {
    successful(
        r#"
        async fn worker(n:Int,items:List<Int>)->Int {Out.println(n);return items.len();}
        let items=List<Int>();items.add(1);let f=worker;
        let a=spawn f(1,items);let b=spawn f(2,items);b.setPriority(-1);items.add(2);
        assert_eq(await a,Ok(1));assert_eq(await b,Ok(1));assert_eq(items.len(),2);publish;
    "#,
        b"2\n1\n",
    );
    successful(
        r#"
        var n=1;
        async fn snapshot()->Int {n=7;return n;}
        let task=spawn snapshot();n=2;assert_eq(await task,Ok(7));assert_eq(n,2);
    "#,
        b"",
    );
    let (root,path)=fixture("async fn items()->List<Int> {let xs=List<Int>();xs.add(1);return xs;} let task=spawn items(); match await task {Ok(xs)=>xs.add(2),Err(_)=>panic(\"unexpected\"),}");
    let output = command(&path, &root);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("immutable List"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resume_restores_parked_tasks_and_branch_discards_tasks() {
    let (root, path) = fixture(
        r#"
        runtime {executionSteps=500;}
        async fn producer(c:Channel<Int>)->Unit {await c.send(1);await c.send(2);}
        let c=Channel<Int>(0);let task=spawn producer(c);
        assert_eq(await c.receive(),Ok(1));commit parked;
        assert_eq(await c.receive(),Ok(2));assert_eq(await task,Ok(()));resume parked;
    "#,
    );
    let output = command(&path, &root);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("ExecutionBudgetExceeded"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
    successful(
        r#"
        async fn abandoned()->Unit {panic("leaked branch task");}
        branch candidate {let task=spawn abandoned();}
    "#,
        b"",
    );
}

#[test]
fn debug_publication_and_saved_inspection_do_not_change_host() {
    let (root,path)=fixture("File.writeText(\"out.txt\",\"a\");Out.println(\"a\");commit saved;publish;assert_eq(File.readText(\"out.txt\"),Ok(\"a\"));");
    let trace = root.join("debug.json");
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("debug")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .arg("--record")
        .arg(&trace)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(!root.join("out.txt").exists());
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("debug")
        .arg(&trace)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!root.join("out.txt").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("saved"));
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&trace)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.join("out.txt").exists());
    fs::remove_dir_all(root).unwrap();
}
