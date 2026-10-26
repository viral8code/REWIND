use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1948-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(r: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(r)
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn libraries(r: &PathBuf) {
    fs::write(
        r.join("numeric.rw"),
        include_str!("../libraries/std/numeric.rw"),
    )
    .unwrap();
    fs::write(
        r.join("numericIndex.rw"),
        include_str!("../libraries/std/numericIndex.rw"),
    )
    .unwrap();
}
#[test]
fn flat_views_and_checkpoints_survive_source_free_debug_and_compact_replay() {
    let r = root();
    libraries(&r);
    let source = include_str!("../examples/numeric-index/main.rw")
        .replace("std.numeric as", "numeric as")
        .replace("std.numericIndex as", "numericIndex as");
    fs::write(r.join("main.rw"), source).unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    for name in ["main.rw", "numeric.rw", "numericIndex.rw"] {
        fs::remove_file(r.join(name)).unwrap();
    }
    let _ = fs::remove_dir_all(r.join(".rewind"));
    for mode in ["debug", "compact"] {
        let out = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--native-work",
                "10000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "99\n2\n"
        );
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
