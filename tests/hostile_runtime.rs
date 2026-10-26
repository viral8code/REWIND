//! Hostile programs that run with the default budgets must be stopped by those budgets, quickly and
//! with an ordinary failure code (71 for a budget), never by a hang, a crash or a memory blow-up.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const LIMIT: Duration = Duration::from_secs(20);

const HELPERS: &str = "fn dbl(s: String, k: Int) -> String {
    var o = s;
    var i = 0;
    while i < k { o = o + o; i = i + 1; }
    return o;
}
";

fn run(imports: &str, body: &str) -> (i32, String) {
    let root: PathBuf = std::env::temp_dir().join(format!(
        "rewind-hostile-rt-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let source = format!("{imports}{HELPERS}fn main() -> Int {{\n{body}\n    return 0;\n}}\n");
    fs::write(root.join("main.rw"), source).unwrap();
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&root)
        .args(["run", "main.rw"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("program did not finish within {LIMIT:?}: {body}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut bytes = Vec::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_end(&mut bytes);
    }
    let _ = fs::remove_dir_all(&root);
    (status.code().unwrap_or(-1), String::from_utf8_lossy(&bytes).into_owned())
}

fn stopped(imports: &str, body: &str) {
    let (code, stderr) = run(imports, body);
    assert_eq!(code, 71, "expected a budget stop, got {code}: {stderr}\n{body}");
}

#[test]
fn endless_loops_and_recursion_hit_the_budget() {
    stopped("", "while true { }");
    stopped("", "var n = 0; while true { n = n + 1; }");
    stopped("fn f(n: Int) -> Int { return f(n + 1) + 1; }\n", "let r = f(0);");
}

#[test]
fn memory_growth_hits_the_budget() {
    stopped("", "var s = \"a\"; while true { s = s + s; }");
    stopped("", "let xs = List<Int>(); while true { xs.add(1); }");
    stopped("", "let m = Map<Int, Int>(); var i = 0; while true { m.set(i, i); i = i + 1; }");
}

#[test]
fn closure_chains_stay_linear() {
    // Each new closure captures the previous one; creating them must not re-walk the whole chain.
    let started = Instant::now();
    let (code, stderr) = run(
        "",
        "var f = |a: Int| a; var n = 0; while n < 6000 { let g = f; f = |a: Int| g(a) + 1; n = n + 1; }",
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(started.elapsed() < Duration::from_secs(10), "closure chain took {:?}", started.elapsed());
    stopped("", "var f = |a: Int| a; while true { let g = f; f = |a: Int| g(a) + 1; }");
}

#[test]
fn std_natives_are_bounded_by_the_native_work_budget() {
    stopped(
        "import std.bigint as big;\n",
        "match big.fromInt(2) { Ok(b) => { let r = big.pow(&b, 1000000000); }, Err(_) => { } }",
    );
    stopped(
        "import std.json as json;\n",
        "let s = dbl(\"[\", 20) + dbl(\"]\", 20); match json.parse(s) { Ok(j) => { }, Err(_) => { } }",
    );
    stopped(
        "import std.regex as regex;\n",
        "match regex.compile(\"(a+)+$\") { Ok(re) => { let s = dbl(\"a\", 20) + \"!\"; let m = regex.isMatch(&re, &s); }, Err(_) => { } }",
    );
    stopped(
        "import std.url as url;\n",
        "let u = url.parse(\"http://\" + dbl(\"a\", 20));",
    );
}
