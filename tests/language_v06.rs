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
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .arg(mode)
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
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
