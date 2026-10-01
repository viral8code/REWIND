use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind v094 {} {}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(args: &[&str], input: &[u8]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(input).unwrap();
    c.wait_with_output().unwrap()
}
fn ok(o: Output) -> Vec<u8> {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    o.stdout
}
fn source(root: &Path, text: &str) -> String {
    let p = root.join("main.rw");
    fs::write(&p, text).unwrap();
    p.to_string_lossy().into_owned()
}
#[test]
fn publish_keeps_confirmed_output_and_restores_pending_only() {
    let r = root();
    let p=source(&r,"var score=10;commit test1;score=99;Out.println(score);Err.println(score);commit test2;publish;revert test1;Out.println(score);publish;revert test2;publish;Out.println(99);publish;publish;");
    let trace = r.join("trace.json");
    let o = call(&["run", &p, "--record", trace.to_str().unwrap()], b"");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(o.stdout, b"99\n10\n99\n");
    assert_eq!(o.stderr, b"99\n");
    assert_eq!(
        ok(call(
            &[
                "replay",
                trace.to_str().unwrap(),
                "--root",
                r.to_str().unwrap()
            ],
            b""
        )),
        b"99\n10\n99\n"
    );
    let p = source(
        &r,
        "Out.println(1);commit base;Out.println(99);revert base;Out.println(10);publish;",
    );
    assert_eq!(ok(call(&["run", &p], b"")), b"1\n10\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn single_file_compile_std_generics_and_source_free_run() {
    let r = root();
    let p=source(&r,"import std.deque as deque;import std.sort as sort;fn main()->Int effects {output}{match deque.create<Int>(2){Ok(d)=>{deque.pushBack(&mut d,7);Out.println(deque.popFront(&mut d));},Err(_)=>{return 2;}}let xs=List<Int>();xs.add(3);xs.add(1);let ys=sort.integers(&xs);Out.println(ys.get(0));publish;return 0;}");
    assert_eq!(ok(call(&["run", &p], b"")), b"Some(7)\n1\n");
    assert!(!r.join("rewind.toml").exists());
    assert!(!r.join("rewind.lock").exists());
    assert!(!r.join("vendor").exists());
    ok(call(&["compile", &p], b""));
    let compiler = Command::new(env!("CARGO_BIN_EXE_rewindc"))
        .arg(&p)
        .output()
        .unwrap();
    ok(compiler);
    let artifact = r.join("main.rwc");
    fs::remove_file(p).unwrap();
    fs::remove_dir_all(r.join(".rewind")).unwrap();
    assert_eq!(
        ok(call(&[artifact.to_str().unwrap()], b"")),
        b"Some(7)\n1\n"
    );
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn byte_chunks_restore_and_replay_without_reading_host_again() {
    let r = root();
    let p=source(&r,"commit base;match In.readChunk(2){Some(bytes)=>{Out.writeBytes(bytes);},None=>{}}publish;revert base;match In.readChunk(2){Some(bytes)=>{Out.writeBytes(bytes);},None=>{}}publish;match In.readChunk(2){Some(bytes)=>{Out.writeBytes(bytes);},None=>{}}publish;");
    let trace = r.join("trace.json");
    assert_eq!(
        ok(call(
            &["run", &p, "--record", trace.to_str().unwrap()],
            b"abcd"
        )),
        b"ababcd"
    );
    assert_eq!(
        ok(call(
            &[
                "replay",
                trace.to_str().unwrap(),
                "--root",
                r.to_str().unwrap()
            ],
            b""
        )),
        b"ababcd"
    );
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn streaming_tokens_utf8_and_writer_handle_chunk_boundaries() {
    let r = root();
    let p = source(
        &r,
        r#"import std.stream as stream;
 fn bytes(text:String)->Bytes effects {} {match stdEncode(text){Ok(value)=>{return value;},Err(_)=>{panic("encode");}}}
 fn slice(value:Bytes,a:Int,b:Int)->Bytes effects {} {match stdBytesSlice(value,a,b){Ok(part)=>{return part;},Err(_)=>{panic("slice");}}}

 match stream.tokens(32){Ok(t)=>{stream.feed(&mut t,bytes("-922337203685"),false);stream.feed(&mut t,bytes("4775808 42"),true);assert_eq(stream.nextInt(&mut t),Ok(Some(-9223372036854775807-1)));assert_eq(stream.nextInt(&mut t),Ok(Some(42)));assert_eq(stream.nextInt(&mut t),Ok(None));},Err(_)=>{panic("tokens");}}
 let d=stream.utf8();let encoded=bytes("あ");assert_eq(stream.decode(&mut d,slice(encoded,0,1),false),Ok(""));assert_eq(stream.decode(&mut d,slice(encoded,1,3),true),Ok("あ"));
 let incomplete=stream.utf8();assert_eq(stream.decode(&mut incomplete,slice(encoded,0,1),false),Ok(""));assert_eq(stream.decode(&mut incomplete,bytes(""),true),Err(stream.StreamError("IncompleteUtf8",0)));assert_eq(stream.decode(&mut incomplete,slice(encoded,1,3),true),Ok("あ"));
 let invalid=stream.utf8();assert_eq(stream.decode(&mut invalid,slice(encoded,1,2),false),Err(stream.StreamError("Utf8",0)));assert_eq(stream.decode(&mut invalid,bytes("ok"),true),Ok("ok"));
 match stream.tokens(2){Ok(t)=>{assert_eq(stream.feed(&mut t,bytes("ab"),false),Ok(()));assert_eq(stream.feed(&mut t,bytes("c "),false),Err(stream.StreamError("TokenLimit",0)));assert_eq(stream.feed(&mut t,bytes(" "),true),Ok(()));assert_eq(stream.next(&mut t),Some(bytes("ab")));},Err(_)=>{panic("tokens");}}
 match stream.tokens(32){Ok(t)=>{stream.feed(&mut t,bytes("9223372036854775808"),true);match stream.nextInt(&mut t){Err(_)=>{},Ok(_)=>{panic("overflow accepted");}}},Err(_)=>{panic("tokens");}}
 match stream.writer(2){Ok(w)=>{assert_eq(stream.write(&mut w,bytes("ab")),Ok(()));assert_eq(stream.write(&mut w,bytes("c")),Err(stream.StreamError("Capacity",0)));assert_eq(stream.drain(&mut w),Ok(bytes("ab")));},Err(_)=>{panic("writer");}}
 "#,
    );
    ok(call(&["run", &p, "--task-steps", "2000000"], b""));
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn old_language_keeps_legacy_publish_contract() {
    let r = root();
    fs::write(
        r.join("rewind.toml"),
        "language=\"0.9.3\"\nsource_root=\".\"\nentry=\"main.rw\"\neffects=\"output\"\n",
    )
    .unwrap();
    let p = source(
        &r,
        "commit base;Out.println(99);publish;revert base;Out.println(10);publish;",
    );
    ok(call(&["update", "--root", r.to_str().unwrap()], b""));
    let o = call(&["run", &p], b"");
    assert!(!o.status.success());
    assert_eq!(o.stdout, b"99\n");
    assert!(String::from_utf8_lossy(&o.stderr).contains("predates an earlier publish"));
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn source_changes_recompile_and_host_effects_require_permission() {
    let r = root();
    let p = source(&r, "Out.println(1);publish;");
    ok(call(&["compile", &p], b""));
    source(&r, "Out.println(2);publish;");
    assert_eq!(ok(call(&["run", &p], b"")), b"2\n");
    source(&r, "File.writeText(\"data.txt\",\"value\");publish;");
    let denied = call(&["run", &p], b"");
    assert!(!denied.status.success());
    assert!(!r.join("data.txt").exists());
    ok(call(&["run", &p, "--allow-effects", "fileWrite"], b""));
    assert_eq!(fs::read_to_string(r.join("data.txt")).unwrap(), "value");
    ok(call(&["compile", &p, "--allow-effects", "fileWrite"], b""));
    fs::remove_file(r.join("data.txt")).unwrap();
    let artifact = r.join("main.rwc");
    assert!(!call(&["run", artifact.to_str().unwrap()], b"")
        .status
        .success());
    ok(call(
        &[
            "run",
            artifact.to_str().unwrap(),
            "--allow-effects",
            "fileWrite",
        ],
        b"",
    ));
    assert_eq!(fs::read_to_string(r.join("data.txt")).unwrap(), "value");
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn nearest_manifest_and_embedded_std_cache_preserve_validation() {
    let r = root();
    let p = source(
        &r,
        "import std.number as number;assert_eq(number.decimal(\"42\"),Ok(42));",
    );
    ok(call(&["run", &p], b""));
    let cached = r.join(".rewind/std-0.9.4/number.rw");
    fs::write(&cached, "invalid cached std").unwrap();
    ok(call(&["run", &p], b""));
    assert!(!fs::read_to_string(&cached)
        .unwrap()
        .contains("invalid cached std"));
    #[cfg(unix)]
    {
        let target = r.join("outside.txt");
        fs::write(&target, "keep").unwrap();
        fs::remove_file(&cached).unwrap();
        std::os::unix::fs::symlink(&target, &cached).unwrap();
        assert!(!call(&["run", &p], b"").status.success());
        assert_eq!(fs::read_to_string(&target).unwrap(), "keep");
        fs::remove_file(&cached).unwrap();
    }
    fs::write(
        r.join("rewind.toml"),
        "language=\"0.9.4\"\nsource_root=\".\"\nentry=\"main.rw\"\neffects=\"output\"\n",
    )
    .unwrap();
    source(&r, "File.writeText(\"data.txt\",\"value\");publish;");
    ok(call(&["update", "--root", r.to_str().unwrap()], b""));
    assert!(!call(&["run", &p, "--allow-effects", "fileWrite"], b"")
        .status
        .success());
    assert!(!r.join("data.txt").exists());
    source(&r, "Out.println(1);publish;");
    assert_eq!(ok(call(&["run", &p], b"")), b"1\n");
    let lock_path = r.join("rewind.lock");
    let mut lock: serde_json::Value =
        serde_json::from_slice(&fs::read(&lock_path).unwrap()).unwrap();
    lock["compiler"] = serde_json::json!("0.9.3");
    fs::write(&lock_path, serde_json::to_vec(&lock).unwrap()).unwrap();
    assert!(!call(&["run", &p], b"").status.success());
    fs::remove_dir_all(r).unwrap();
}
