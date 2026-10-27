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
