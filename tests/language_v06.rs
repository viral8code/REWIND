use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn fixture(source: &str, effects: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v06-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.rw"), source).unwrap();
    fs::write(root.join("rewind.toml"),format!("language = \"0.6\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"{effects}\"\n")).unwrap();
    let output = cmd("update", &root, &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    root
}
fn cmd(mode: &str, root: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rewind"));
    command.arg(mode);
    if mode == "replay" {
        command
            .arg(args[0])
            .arg("--root")
            .arg(root)
            .args(&args[1..]);
    } else {
        command.arg("--root").arg(root).args(args);
    }
    command.output().unwrap()
}
fn ok(source: &str, effects: &str, expected: &[u8]) {
    let root = fixture(source, effects);
    let output = cmd("run", &root, &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
    fs::remove_dir_all(root).unwrap();
}
fn rejected(source: &str, message: &str) {
    let root = fixture(source, "tasks,output,fileRead,fileWrite");
    let output = cmd("check", &root, &[]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_capture_modes_have_distinct_ownership() {
    rejected("var n=7;fn read()->Int effects {} {return n;}let snapshot=capture value ||->Int{return read();};","global dependencies");
    ok("let xs=List<Int>();xs.add(1);let read=capture value ||->Int{return xs.len();};xs.add(2);assert_eq(read(),1);assert_eq(xs.len(),2);","",b"");
    ok("struct Box{n:Int}trait Read{fn read(self:&Self,xs:&List<Int>)->Int effects {};}impl Read for Box{fn read(self:&Box,xs:&List<Int>)->Int effects {} {return self.n+xs.len();}}let box=Box(2);let xs=List<Int>();xs.add(5);{let read=capture borrow ||->Int{return box.read(&xs);};assert_eq(read(),3);}xs.add(6);","",b"");
    ok("var n=7;let read=capture value ||->Int{return n;};n=9;assert_eq(read(),7);commit saved;n=10;revert saved;assert_eq(read(),7);","",b"");
    ok("let xs=List<Int>();xs.add(7);let read=capture move ||->Int{return xs.len();};assert_eq(read(),1);","",b"");
    rejected(
        "let xs=List<Int>();let read=capture move ||->Int{return xs.len();};xs.len();",
        "use after move",
    );
    ok("let xs=List<Int>();xs.add(7);{let read=capture borrow ||->Int{return xs.len();};assert_eq(read(),1);}xs.add(8);assert_eq(xs.len(),2);","",b"");
    rejected(
        "let xs=List<Int>();let read=capture borrow ||->Int{return xs.len();};xs.add(8);",
        "while borrowed",
    );
    rejected(
        "let xs=List<Int>();let change=capture borrow ||->Unit{xs.add(1);};",
        "shared borrow",
    );
    rejected(
        "var n=1;let change=capture borrow ||->Unit{n=2;};",
        "shared capture",
    );
    rejected("fn leak()->fn()->Int effects {} {let xs=List<Int>();return capture borrow ||->Int{return xs.len();};}","cannot escape");
    ok("async fn invoke(f:fn()->Int effects {})->Int effects {} {return f();}var n=7;let task=spawn invoke(capture value ||->Int{return n;});n=8;assert_eq(await task,Ok(7));","tasks",b"");
    rejected("File.writeText(\"data\",\"x\");using h=File.open(\"data\");let read=capture value ||->String{return h.read(1);};","value capture requires transferable");
}

#[test]
fn tuples_enumerate_and_zip_preserve_types_and_cursor_state() {
    rejected(
        "let xs=List<Int>();let pair=(xs,1);",
        "requires move or freeze",
    );
    rejected("let xs=List<Int>();let left=xs.iter();let right=xs.iter();let view=&right;left.zip(right);","while borrowed");
    ok("let pair:(Int,String)=(7,\"seven\");assert_eq(pair._0,7);assert_eq(pair._1,\"seven\");let single:(Int,)=(4,);assert_eq(single._0,4);async fn sum(pair:(Int,Int))->Int effects {} {return pair._0+pair._1;}assert_eq(await sum((3,4)),Ok(7));","tasks",b"");
    rejected("let pair=(1,2);pair._0=3;", "tuple fields are immutable");
    rejected("let pair=(1,2);pair._2;", "out of range");
    ok("let xs=List<Int>();xs.add(5);xs.add(6);let enumerated=xs.iter().enumerate().collect();assert_eq(enumerated.get(0)._0,0);assert_eq(enumerated.get(1)._1,6);let ys=List<String>();ys.add(\"x\");let zipped=xs.iter().zip(ys.iter()).collect();assert_eq(zipped.len(),1);assert_eq(zipped.get(0)._0,5);assert_eq(zipped.get(0)._1,\"x\");let empty=List<Int>().iter().zip(List<String>().iter()).collect();assert_eq(empty.len(),0);","",b"");
    ok("let xs=List<Int>();xs.add(1);xs.add(2);let a=xs.iter();let b=xs.iter();commit saved;assert_eq(a.zip(b).collect().len(),2);revert saved;assert_eq(a.enumerate().next(),Some((0,1)));assert_eq(b.next(),Some(1));","",b"");
}

#[test]
fn pure_higher_order_effects_and_reassignment() {
    ok("fn apply<E:Effect>(f:fn(Int)->Int effects E,n:Int)->Int effects E {return f(n);}assert_eq(apply(|n:Int|->Int{return n+1;},4),5);","",b"");
    rejected("pub fn quiet()->Int effects {} {return 1;}pub fn loud()->Int effects {output} {Out.println(1);return 2;}var action=quiet;action=loud;action();","output");
    rejected(
        "pub fn apply<E:Effect>(f:fn(Int)->Int effects E,n:Int)->Int effects {} {return f(n);}",
        "undeclared effect E",
    );
}

#[test]
fn synchronous_borrow_contracts() {
    rejected("fn both(xs:&List<Int>,unit:Unit)->Unit effects {} {}let xs=List<Int>();both(&xs,xs.add(1));","borrowed during call");
    ok("fn count(xs:&List<Int>)->Int effects {} {return xs.len();}fn append(xs:&mut List<Int>,n:Int)->Unit effects {} {xs.add(n);}let xs=List<Int>();append(&mut xs,7);assert_eq(count(&xs),1);xs.add(8);assert_eq(xs.len(),2);","",b"");
    rejected(
        "fn change(xs:&List<Int>)->Unit effects {} {xs.add(1);}",
        "shared borrow",
    );
    rejected(
        "fn append(xs:&mut List<Int>)->Unit effects {} {}let xs=List<Int>();append(&xs);",
        "requires &mut",
    );
    rejected("fn both(a:&mut List<Int>,b:&List<Int>)->Unit effects {} {}let xs=List<Int>();both(&mut xs,&xs);","conflicting borrows");
    rejected(
        "fn leak(xs:&List<Int>)->&List<Int> effects {} {return xs;}",
        "borrowed return",
    );
    rejected(
        "async fn bad(xs:&List<Int>)->Unit effects {} {}",
        "not Send",
    );
}

#[test]
fn conversion_boundaries_and_result_adapters() {
    ok(
        r#"assert_eq("あ😀".byteLen(),7);assert_eq("あ😀".charLen(),2);assert_eq("あ😀".utf16Len(),3);
    assert_eq("あ😀".encodeUtf8().decodeUtf8(),Ok("あ😀"));assert_eq("あ".slice(1,2),Err("InvalidUtf8BoundaryOrRange"));
    assert_eq("42".parseInt().map(|n:Int|->Int{return n+1;}),Ok(43));
    assert_eq("no".parseInt().mapErr(|e:String|->String{return "failed";}),Err("failed"));
    assert_eq("42".parseInt().andThen(|n:Int|->Result<Int,String>{return Ok(n+2);}),Ok(44));
    let nested:Result<Result<Int,String>,String> = Ok(Ok(7));assert_eq(nested.flatten(),Ok(7));
    assert_eq(9007199254740993.toFloatChecked(),Err("InexactConversion"));
    assert_eq("9223372036854775808".parseInt(),Err("InvalidIntOrOverflow"));"#,
        "",
        b"",
    );
}

#[test]
fn inferred_callbacks_and_trait_contracts() {
    let root = fixture(
        "fn noisy()->fn()->Int effects {} {Out.println(1);return ||->Int{return 7;};}noisy()();",
        "",
    );
    let output = cmd("check", &root, &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("output"));
    fs::remove_dir_all(root).unwrap();
    ok("fn call(f:fn()->Int effects {})->Int{return f();}fn pure()->Int{return 7;}assert_eq(call(pure),7);","",b"");
    rejected("struct Box{n:Int}trait Read{fn read(self:Self)->Int effects {};}impl Read for Box{fn read(self:Self)->Int effects {output}{Out.println(1);return 1;}}","exceeds contract");
    let root = fixture(
        "fn pure()->Int{return 7;}let f=match true{true=>pure,false=>pure};assert_eq(f(),7);",
        "",
    );
    assert!(cmd("check", &root, &[]).status.success());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn iterator_adapters_use_checkpointed_cursors() {
    ok("let xs=List<Int>();xs.add(1);xs.add(2);xs.add(3);let iter=xs.iter();commit saved;let doubled=iter.map(|n:Int|->Int{return n*2;}).filter(|n:Int|->Bool{return n>2;}).take(1).collect();assert_eq(doubled.len(),1);assert_eq(doubled.get(0),4);revert saved;assert_eq(iter.fold(0,|sum:Int,n:Int|->Int{return sum+n;}),6);","",b"");
}

#[test]
fn task_diagnostics_keep_spans_and_cleanup_causes() {
    ok(
        r#"fn cleanup()->Unit effects {} {panic("cleanup failed");}
    async fn fail()->Int effects {} {defer ||->Unit{panic("cleanup failed");};panic("primary failed");return 1;}
    let task=spawn fail();match await task {Err(TaskError::Failed(d))=>{assert_eq(d.taskId,Some(1));assert_eq(d.causes.len(),1);assert_eq(d.source,"main.rw");},_=>{panic("missing diagnostic");}}"#,
        "tasks",
        b"",
    );
    let root = fixture(
        "test fn fail(){defer ||->Unit{panic(\"cleanup\");};panic(\"primary\");}",
        "",
    );
    let path = root.join("failed.json");
    let output = cmd("test", &root, &["--record", path.to_str().unwrap()]);
    assert!(!output.status.success());
    let bytes = fs::read(&path).unwrap();
    let trace: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        trace["result"]["diagnostic"]["causes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let replay = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("replay")
        .arg(&path)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!replay.status.success());
    assert!(
        !String::from_utf8_lossy(&replay.stderr).contains("ReplayMismatch"),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn field_moves_borrows_and_secret_methods() {
    ok("struct Pair{left:List<Int>,right:List<Int>}let p=Pair(List<Int>(),List<Int>());let left=move p.left;p.right.add(3);assert_eq(p.right.len(),1);","",b"");
    rejected("struct Pair{left:List<Int>,right:List<Int>}let p=Pair(List<Int>(),List<Int>());let left=move p.left;p.left.len();","use after move");
    rejected("struct Pair{left:List<Int>,right:List<Int>}let p=Pair(List<Int>(),List<Int>());let r=&p.left;p.left.add(1);","while borrowed");
    ok("struct Pair{left:List<Int>,right:List<Int>}let p=Pair(List<Int>(),List<Int>());let r=&p.left;p.right.add(1);assert_eq(r.len(),0);assert_eq(p.right.len(),1);let s=secret(\"classified\");assert_eq(reveal(s.charLen()),10);assert_eq(reveal(s.encodeUtf8().decodeUtf8()),Ok(\"classified\"));","",b"");
    rejected(
        "fn leak(xs:&List<List<Int>>)->List<Int> effects {} {return xs.get(0);}",
        "borrow-derived owner",
    );
}

#[test]
fn cancelled_tasks_and_group_failures_are_observable() {
    ok(
        r#"async fn count()->Int effects {} {return 7;}let t=count();t.requestCancel();assert_eq(await t,Err(TaskError::Cancelled));let t2=count();assert_eq(await t2.cancelAndJoin(),Err(TaskError::Cancelled));
    async fn fail()->Unit effects {} {panic("failure");}let group=TaskGroup();group.add(fail());group.add(fail());match await group.join(){Err(TaskError::Failed(d))=>{assert_eq(d.causes.len(),1);},_=>{panic("missing failures");}}"#,
        "tasks",
        b"",
    );
}

#[test]
fn artifacts_reinfer_effects_and_run_without_sources() {
    let root=fixture("fn apply<E:Effect>(f:fn(Int)->Int effects E,n:Int)->Int effects E{return f(n);}assert_eq(apply(|n:Int|->Int{return n+1;},3),4);","");
    let artifact = root.join("program.json");
    let built = cmd("build", &root, &["--output", artifact.to_str().unwrap()]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("run-artifact")
        .arg(&artifact)
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
fn lsp_applies_utf16_incremental_edits_without_writing_source() {
    use std::io::Write;
    use std::process::Stdio;
    let original = "let s=\"😀\";let n:Int=1;";
    let root = fixture(original, "");
    let uri = format!(
        "file:///{}",
        root.join("main.rw").to_string_lossy().replace('\\', "/")
    );
    let requests = [
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":original}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"range":{"start":{"line":0,"character":21},"end":{"line":0,"character":22}},"text":"\"bad\""}]}}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}),
        serde_json::json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("lsp")
        .arg("--root")
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for request in requests {
            let bytes = serde_json::to_vec(&request).unwrap();
            write!(stdin, "Content-Length: {}\r\n\r\n", bytes.len()).unwrap();
            stdin.write_all(&bytes).unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(text.contains("\"textDocumentSync\":2"));
    assert!(text.contains("type mismatch"), "{text}");
    assert_eq!(fs::read_to_string(root.join("main.rw")).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn tuple_patterns_aliases_and_default_methods() {
    ok("type Pair<T>=(T,String);type Row=Pair<Int>;let row:Row=(7,\"seven\");let (n,text)=row;assert_eq(n,7);assert_eq(text,\"seven\");var (a,(b,))=(1,(2,));a=3;assert_eq(a+b,5);assert_eq(match (1,(2,3)){(x,(y,z))=>x+y+z},6);", "", b"");
    rejected("let (a,b)=(1,2,3);", "expects 3 fields");
    rejected("let (a,a)=(1,2);", "duplicate pattern binding");
    rejected(
        "type A=B;type B=A;let a:A=1;",
        "TypeExpansionBudgetExceeded",
    );
    ok("struct Box{n:Int}trait Read{fn read(self:&Self)->Int effects {};fn twice(self:&Self)->Int effects {} {return self.read()+self.read();}}impl Read for Box{fn read(self:&Box)->Int effects {} {return self.n;}}let box=Box(4);assert_eq(box.twice(),8);", "", b"");
    ok("struct Box{n:Int}trait Read{fn read(self:&Self)->Int effects {} {return 2;}}impl Read for Box{}let box=Box(4);assert_eq(box.read(),2);", "", b"");
    ok("struct Box{n:Int}trait Read{fn read(self:&Self)->Int effects {} {return 2;}}impl Read for Box{fn read(self:&Box)->Int effects {} {return 9;}}let box=Box(4);assert_eq(box.read(),9);", "", b"");
    rejected("struct Box{n:Int}trait Read{fn read(self:&Self)->Int effects {} {Out.println(1);return 2;}}impl Read for Box{}", "undeclared effect output");
}

#[test]
fn effect_inclusion_and_temporary_borrow_capture() {
    ok("fn both<E:Effect,F:Effect>(a:fn()->Unit effects E,b:fn()->Unit effects F,c:fn()->Unit effects {E,F})->Unit effects {E,F} {a();b();c();}both(||->Unit{},||->Unit{},||->Unit{});", "", b"");
    ok("fn apply(f:&fn()->Int effects {})->Int effects {} {return f();}let xs=List<Int>();assert_eq(apply(capture borrow ||->Int{return xs.len();}),0);xs.add(1);{let read=capture borrow ||->Int{return xs.len();};assert_eq(apply(&read),1);}", "", b"");
    rejected(
        "fn bad(f:&fn()->Int effects {})->fn()->Int effects {} {return f;}",
        "cannot escape",
    );
    rejected(
        "let xs=List<Int>();commit saved;let view=&xs;revert saved;view.len();",
        "use after move",
    );
}

#[test]
fn property_tests_reproduce_and_shrink_int_inputs() {
    ok("assert_eq(propertyInt(7,100,-100,100,|n:Int|->Bool{return n+0==n;}),Ok(()));let failed=propertyInt(7,100,-100,100,|n:Int|->Bool{return false;});match failed{Err(f)=>{assert_eq(f.seed,7);assert_eq(f.input,0);assert_eq(f.case,0);},Ok(_)=>panic(\"expected failure\")}", "", b"");
    rejected(
        "propertyInt(1,10,0,100,|n:Int|->Bool{Out.println(n);return true;});",
        "predicate must be",
    );
}

#[test]
fn logical_timeouts_select_and_replay() {
    ok("async fn loopForever()->Int effects {} {while true {}return 1;}let timed=loopForever().timeout(20);assert_eq(await timed,Err(TaskError::TimedOut));", "tasks", b"");
    ok("let ch=Channel<Int>(0);let wait=ch.receive().timeout(3);assert_eq(await wait,Err(TaskError::TimedOut));", "tasks", b"");
    ok("async fn one()->Int effects {} {return 1;}async fn two()->Int effects {} {return 2;}let left=one();let right=two();let chosen=left.select(right);match await chosen{Ok(pair)=>{assert_eq(pair._0,0);assert_eq(pair._1,Ok(1));},Err(_)=>panic(\"select failed\")}right.cancel();", "tasks", b"");
    let root = fixture("async fn loopForever()->Int effects {} {while true {}return 1;}assert_eq(await loopForever().timeout(20),Err(TaskError::TimedOut));", "tasks");
    let trace = root.join("trace.json");
    assert!(cmd("run", &root, &["--record", trace.to_str().unwrap()])
        .status
        .success());
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
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn public_capture_contracts_and_specialized_effects() {
    ok("fn accept(f:fn()->Int effects {} captures {Send,Share})->Int effects {} {return f();}assert_eq(accept(||->Int{return 3;}),3);", "", b"");
    rejected("fn accept(f:fn()->Int effects {} captures {Share})->Unit effects {} {}var n=3;accept(||->Int{return n;});", "expected fn");
    ok("struct Box{n:Int}trait Read{fn read(self:&Self)->Int effects {output};}impl Read for Box{fn read(self:&Box)->Int effects {} {return self.n;}}fn read<T:Read>(x:&T)->Int effects {output}{return x.read();}let x=Box(5);assert_eq(read(&x),5);", "", b"");
}

fn interactive(mode: &str, root: &Path, args: &[&str], input: &[u8]) -> Output {
    use std::io::Write;
    let mut command = Command::new(env!("CARGO_BIN_EXE_rewind"));
    command.arg(mode);
    if mode == "debug-session" {
        command
            .arg(args[0])
            .arg("--root")
            .arg(root)
            .args(&args[1..]);
    } else {
        command.arg("--root").arg(root).args(args);
    }
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn lsp_messages(output: &Output) -> Vec<serde_json::Value> {
    let text = String::from_utf8_lossy(&output.stdout);
    text.split("Content-Length: ")
        .skip(1)
        .map(|part| serde_json::from_str(part.split_once("\r\n\r\n").unwrap().1.trim()).unwrap())
        .collect()
}
#[test]
fn persistent_cache_rebuilds_corruption_and_preserves_artifact_bytes() {
    let root = fixture(
        "fn answer(n:Int)->Int{return n+1;}assert_eq(answer(2),3);",
        "",
    );
    let artifact = root.join("compiled.json");
    success(&cmd(
        "build",
        &root,
        &["--output", artifact.to_str().unwrap()],
    ));
    let expected = fs::read(&artifact).unwrap();
    let entries = fs::read_dir(root.join(".rewind/cache"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert!(entries.iter().any(|p| p
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .contains("compiled")));
    for entry in entries {
        fs::write(entry, r#"{"payload":true,"auth":[]}"#).unwrap();
    }
    success(&cmd(
        "build",
        &root,
        &["--output", artifact.to_str().unwrap()],
    ));
    assert_eq!(fs::read(&artifact).unwrap(), expected);
    let clean = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .env("REWIND_NO_CACHE", "1")
        .arg("build")
        .arg("--root")
        .arg(&root)
        .arg("--output")
        .arg(&artifact)
        .output()
        .unwrap();
    success(&clean);
    assert_eq!(fs::read(&artifact).unwrap(), expected);
    fs::write(
        root.join("main.rw"),
        "fn answer(n:Int)->String{return n;}assert_eq(answer(2),3);",
    )
    .unwrap();
    assert!(!cmd("check", &root, &[]).status.success());
    let compatible = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg("compatibility")
        .arg(&artifact)
        .output()
        .unwrap();
    success(&compatible);
    let value: serde_json::Value = serde_json::from_slice(&compatible.stdout).unwrap();
    assert_eq!(value["readable"], true);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn signed_mirror_solver_backtracks_and_update_proposals_are_atomic() {
    let root = fixture("import math.api;assert_eq(answer(),42);", "");
    let seed = root.join("seed");
    fs::write(&seed, "07".repeat(32)).unwrap();
    let mut public = String::new();
    for (dir, name, version, deps, source) in [
        (
            "math1",
            "math",
            "1.0.0",
            r#"{"util":"^2"}"#,
            "pub fn answer()->Int effects {}{return 42;}",
        ),
        (
            "math2",
            "math",
            "1.1.0",
            r#"{"util":"^3"}"#,
            "pub fn answer()->Int effects {}{return 99;}",
        ),
        (
            "util2",
            "util",
            "2.0.0",
            "{}",
            "pub fn unused()->Unit effects {}{}",
        ),
    ] {
        let path = root.join(format!("vendor/{dir}"));
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("rewind.package.json"),
            format!(
                r#"{{"name":"{name}","version":"{version}","effects":[],"dependencies":{deps}}}"#
            ),
        )
        .unwrap();
        fs::write(path.join("api.rw"), source).unwrap();
        let sign = Command::new(env!("CARGO_BIN_EXE_rewind"))
            .arg("sign")
            .arg(&path)
            .arg("--key")
            .arg(&seed)
            .output()
            .unwrap();
        success(&sign);
        public = String::from_utf8(sign.stdout).unwrap();
    }
    let manifest=format!("language = \"0.6\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"\"\n[dependencies]\nmath = \"file:vendor/math2 | file:vendor/math1\"\n[dependency_versions]\nmath = \"^1\"\nutil = \"^2\"\n[registry]\nutil = \"file:vendor/util2\"\n[dependency_signers]\nmath = \"author\"\nutil = \"author\"\n[trust]\nauthor = \"{}\"\n",public.trim());
    fs::write(root.join("rewind.toml"), &manifest).unwrap();
    let old = fs::read(root.join("rewind.lock")).unwrap();
    let plan = root.join("plan.json");
    success(&cmd(
        "update",
        &root,
        &["--preview", "--output", plan.to_str().unwrap()],
    ));
    assert_eq!(fs::read(root.join("rewind.lock")).unwrap(), old);
    let proposal: serde_json::Value = serde_json::from_slice(&fs::read(&plan).unwrap()).unwrap();
    assert!(proposal["proposed_lock"]
        .as_str()
        .unwrap()
        .contains("vendor/math1"));
    success(&cmd("update", &root, &["--apply", plan.to_str().unwrap()]));
    success(&cmd("run", &root, &[]));
    let applied = fs::read(root.join("rewind.lock")).unwrap();
    let stale = cmd("update", &root, &["--apply", plan.to_str().unwrap()]);
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("StaleUpdateProposal"));
    assert_eq!(fs::read(root.join("rewind.lock")).unwrap(), applied);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn lsp_references_rename_signature_and_quickfix_use_overlay_scopes() {
    use serde_json::json;
    let source="fn plus(n:Int)->Int effects {} {let value=n;{let value=9;assert_eq(value,9);}return value;}\nassert_eq(plus(2),2);";
    let root = fixture(source, "");
    let uri = format!("file://{}", root.join("main.rw").display());
    let pos = json!({"line":0,"character":source.find("return value").unwrap()+8});
    let incomplete = format!("{source}\nplus(");
    let requests = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/references","params":{"textDocument":{"uri":uri},"position":pos,"context":{"includeDeclaration":true}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"textDocument/rename","params":{"textDocument":{"uri":uri},"position":pos,"newName":"result"}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":incomplete}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"textDocument/signatureHelp","params":{"textDocument":{"uri":uri},"position":{"line":2,"character":5}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":"pub fn noisy()->Unit effects {} {Out.println(1);}"}]}}),
        json!({"jsonrpc":"2.0","id":5,"method":"textDocument/codeAction","params":{"textDocument":{"uri":uri},"context":{"diagnostics":[{"range":{"start":{"line":0,"character":4},"end":{"line":0,"character":5}},"message":"undeclared effect output"}]}}}),
        json!({"jsonrpc":"2.0","id":6,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let mut input = Vec::new();
    for request in requests {
        let bytes = serde_json::to_vec(&request).unwrap();
        input.extend(format!("Content-Length: {}\r\n\r\n", bytes.len()).bytes());
        input.extend(bytes);
    }
    let output = interactive("lsp", &root, &[], &input);
    success(&output);
    let messages = lsp_messages(&output);
    let result = |id| messages.iter().find(|m| m["id"] == id).unwrap()["result"].clone();
    assert_eq!(result(2).as_array().unwrap().len(), 2);
    assert_eq!(result(3)["changes"][&uri].as_array().unwrap().len(), 2);
    assert!(result(4)["signatures"][0]["label"]
        .as_str()
        .unwrap()
        .contains("n: Int"));
    assert!(result(5)[0]["edit"]["changes"][&uri][0]["newText"]
        .as_str()
        .unwrap()
        .contains("output"));
    assert_eq!(fs::read_to_string(root.join("main.rw")).unwrap(), source);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn source_free_debugger_reverses_breakpoints_and_file_diffs() {
    let root = fixture(
        "var n=1;\ncommit saved;\nn=2;\nFile.writeText(\"out.txt\",\"new\");\npublish;",
        "fileWrite",
    );
    let trace = root.join("trace.json");
    success(&cmd("run", &root, &["--record", trace.to_str().unwrap()]));
    let bytes = fs::read(&trace).unwrap();
    fs::remove_file(root.join("main.rw")).unwrap();
    fs::remove_file(root.join("out.txt")).unwrap();
    let output=interactive("debug-session",&root,&[trace.to_str().unwrap()],b"break main.rw:4\ncontinue\nstate\nback\nstep\ncontinue\nclear main.rw:4\nbreak main.rw:5\ncontinue\nfiles\ndiff 0\nclear main.rw:5\ncontinue\nreverse-continue\nstate\nquit\n");
    success(&output);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("\"breakpoint\":true") && text.contains("new") && text.contains("\"from\":0"),
        "{text}"
    );
    assert_eq!(fs::read(&trace).unwrap(), bytes);
    assert!(!root.join("out.txt").exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn external_secret_inputs_require_reinjection_and_never_enter_trace() {
    let root=fixture("match In.readSecretLine(){Some(s)=>{assert_eq(reveal(s.charLen()),19);let plain=reveal(s);panic(plain);},None=>{panic(\"missing input\");}}","input");
    let secret = "external-classified";
    let input = root.join("secret.txt");
    fs::write(&input, format!("{secret}\n")).unwrap();
    let trace = root.join("trace.json");
    let run = cmd(
        "run",
        &root,
        &[
            "--secret-input",
            input.to_str().unwrap(),
            "--record",
            trace.to_str().unwrap(),
        ],
    );
    assert!(!run.status.success());
    assert!(!String::from_utf8_lossy(&run.stderr).contains(secret));
    let bytes = fs::read(&trace).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(secret));
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["observations"]["input"][0]["secret"], true);
    assert!(value["audit"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["kind"] == "reveal"));
    let missing = cmd("replay", &root, &[trace.to_str().unwrap()]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("secret input requires"));
    let replay = cmd(
        "replay",
        &root,
        &[
            trace.to_str().unwrap(),
            "--secret-input",
            input.to_str().unwrap(),
        ],
    );
    assert!(!replay.status.success());
    assert!(
        !String::from_utf8_lossy(&replay.stderr).contains("ReplayMismatch"),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert_eq!(fs::read(&trace).unwrap(), bytes);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn capture_contracts_follow_methods_and_patterns_preserve_owners() {
    rejected("let xs=List<Int>();struct Box{n:Int}trait Read{fn read(self:&Self)->Int effects {};}impl Read for Box{fn read(self:&Box)->Int effects {}{return xs.len();}}let box=Box(1);let read:fn()->Int effects {} captures {Share}=||->Int{return box.read();};", "type mismatch");
    rejected(
        "let xs=List<Int>();let pair=(move xs,1);match pair{(items,_)=>{items.add(1);}}",
        "shared borrow",
    );
    ok("let xs=List<Int>();xs.add(1);let pair=(move xs,2);match pair{(items,_)=>{assert_eq(items.len(),1);}}match move pair{(items,_)=>{items.add(2);assert_eq(items.len(),2);}}", "", b"");
    ok("struct Box{n:Int}type Wrapped=Box;trait Read{fn read(self:&Self)->Int effects {};}impl Read for Wrapped{fn read(self:&Wrapped)->Int effects {}{return self.n;}}let b:Wrapped=Box(7);assert_eq(b.read(),7);", "", b"");
    rejected(
        "trait Broken{fn wrong(self:&Self)->Int effects {}{return \"bad\";}}",
        "type mismatch",
    );
    ok("match propertyInt(4,100,10,100,|n:Int|->Bool{return n<25;}) {Err(f)=>{assert_eq(f.input,25);},Ok(_)=>{panic(\"expected failure\");}}", "", b"");
}

#[test]
fn lsp_trait_rename_and_effect_related_locations_follow_contracts() {
    use serde_json::json;
    let source="struct Box{n:Int}\ntrait Read{fn read(self:&Self)->Int effects {};}\nimpl Read for Box{fn read(self:&Box)->Int effects {}{return self.n;}}\nfn use<T:Read>(box:&T)->Int effects {}{return box.read();}\nlet b=Box(7);assert_eq(use(&b),7);assert_eq(b.read(),7);";
    let root = fixture(source, "");
    let uri = format!("file://{}", root.join("main.rw").display());
    let requests = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/rename","params":{"textDocument":{"uri":uri},"position":{"line":1,"character":15},"newName":"inspect"}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":"fn low()->Unit{Out.println(1);}\npub fn high()->Unit effects {}{low();}"}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let mut input = Vec::new();
    for request in requests {
        let bytes = serde_json::to_vec(&request).unwrap();
        input.extend(format!("Content-Length: {}\r\n\r\n", bytes.len()).bytes());
        input.extend(bytes);
    }
    let output = interactive("lsp", &root, &[], &input);
    success(&output);
    let messages = lsp_messages(&output);
    let rename = messages.iter().find(|m| m["id"] == 2).unwrap();
    assert_eq!(
        rename["result"]["changes"][&uri].as_array().unwrap().len(),
        4,
        "{rename}"
    );
    let diagnostic = &messages
        .iter()
        .find(|m| m["method"] == "textDocument/publishDiagnostics")
        .unwrap()["params"]["diagnostics"][0];
    assert_eq!(diagnostic["code"], "MissingEffect");
    assert!(
        diagnostic["relatedInformation"].as_array().unwrap().len() >= 2,
        "{diagnostic}"
    );
    fs::remove_dir_all(root).unwrap();
}
