use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1949-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(r: &PathBuf, a: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(r)
        .args(a)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn libraries(r: &PathBuf) {
    for name in [
        "graph",
        "graphLarge",
        "numericRange",
        "numericIndex",
        "sort",
        "disjointSet",
        "heap",
        "integer",
    ] {
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("libraries/std")
                .join(format!("{name}.rw")),
            r.join(format!("{name}.rw")),
        )
        .unwrap();
    }
}
#[test]
fn bulk_graph_views_and_checkpoint_survive_both_source_free_trace_modes() {
    let r = root();
    libraries(&r);
    let mut s = include_str!("../examples/graph-large/main.rw").to_string();
    for n in ["graphLarge", "numericRange", "numericIndex"] {
        s = s.replace(&format!("std.{n} as"), &format!("{n} as"));
    }
    fs::write(r.join("main.rw"), s).unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    for p in fs::read_dir(&r).unwrap() {
        let p = p.unwrap().path();
        if p.extension().is_some_and(|e| e == "rw") {
            fs::remove_file(p).unwrap();
        }
    }
    let _ = fs::remove_dir_all(r.join(".rewind"));
    for mode in ["debug", "compact"] {
        let o = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--native-work",
                "100000000",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&o);
        assert_eq!(
            String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n"),
            "5\n-1\n"
        );
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(o.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
