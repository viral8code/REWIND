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
const TAKE: &str = r#"fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}"#;
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
#[test]
fn native_adjacency_preserves_existing_algorithms_and_incremental_order() {
    let r = root();
    libraries(&r);
    fs::write(r.join("main.rw"),format!(r#"import graph as old;import graphLarge as large;import numericIndex as index;
{TAKE}
fn ints(a:&List<Int>,b:&List<Int>)->Unit effects {{}} {{assert_eq(a.len(),b.len());for i in 0..a.len(){{assert_eq(a.get(i),b.get(i));}}}}
fn options(a:&List<Option<Int>>,b:&List<Option<Int>>)->Unit effects {{}} {{assert_eq(a.len(),b.len());for i in 0..a.len(){{assert_eq(a.get(i),b.get(i));}}}}
let legacy=take(old.create(8,16));let graph=take(large.create(8,16));
for i in 0..7{{take(old.add(&mut legacy,i,i+1,1));take(large.add(&mut graph,i,i+1,1));}}
let bfs=take(large.bfs(&graph,0));let expected=take(old.bfs(&legacy,0));for i in 0..8{{assert_eq(take(index.getInt(&bfs,i)),expected.get(i));}}
let dfs=take(large.dfs(&graph,0));let oldDfs=take(old.dfs(&legacy,0));ints(&dfs,&oldDfs);let dij=take(large.dijkstra(&graph,0));let oldDij=take(old.dijkstra(&legacy,0));options(&dij,&oldDij);
let topo=take(large.topological(&graph));let oldTopo=take(old.topological(&legacy));ints(&topo,&oldTopo);let bf=take(large.bellmanFord(&graph,0));let oldBf=take(old.bellmanFord(&legacy,0));options(&bf,&oldBf);let comp=take(large.components(&graph));let oldComp=take(old.components(&legacy));ints(&comp,&oldComp);
let a=take(large.minimumForest(&graph));let b=take(old.minimumForest(&legacy));assert_eq(a.weight,b.weight);assert_eq(a.components,b.components);ints(&a.froms,&b.froms);ints(&a.tos,&b.tos);
let ancestor=take(large.ancestors(&graph,0));assert_eq(take(large.lca(&ancestor,2,7)),2);
commit original;take(large.add(&mut graph,7,0,-1));assert_eq(large.dijkstra(&graph,0),Err("NegativeWeight"));assert_eq(large.topological(&graph),Err("Cycle"));
revert original;assert_eq(large.edges(&graph),7);let restored=take(large.bfs(&graph,0));assert_eq(take(index.getInt(&restored,7)),7);drop original;Out.println(7);publish;
"#)).unwrap();
    let o = call(&r, &["run", "main.rw", "--native-work", "100000000"]);
    ok(&o);
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "7");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn old_language_keeps_graph_and_flat_index_api_and_rejects_new_kernels() {
    let r = root();
    libraries(&r);
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.48\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    fs::write(r.join("main.rw"),format!(r#"import graph as g;import numericIndex as index;{TAKE}let graph=take(g.create(2,1));take(g.add(&mut graph,0,1,1));Out.println(take(g.bfs(&graph,0)).get(1));publish;"#)).unwrap();
    ok(&call(&r, &["update"]));
    let o = call(&r, &["run", "main.rw"]);
    ok(&o);
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "1");
    fs::write(
        r.join("main.rw"),
        "fn newKernel()->Result<IntArray,StdError> effects {} {return stdNumericRangeInt(0,1,5);}",
    )
    .unwrap();
    let o = call(&r, &["check", "main.rw"]);
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("require language 1.9.49"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn large_native_chain_fits_bounded_memory_without_intermediate_vm_lists() {
    let r = root();
    libraries(&r);
    fs::write(r.join("main.rw"),format!(r#"import graphLarge as graph;import numericRange as range;import numericIndex as index;{TAKE}
let froms=take(range.integers(0,1,199999));let tos=take(range.integers(1,1,199999));let weights=take(range.integers(0,0,199999));
let chain=take(graph.fromEdges(200000,&froms,tos,weights));let distance=take(graph.bfs(&chain,0));assert_eq(take(index.getInt(&distance,199999)),199999);Out.println(199999);publish;
"#)).unwrap();
    let o = call(
        &r,
        &[
            "profile",
            "main.rw",
            "--native-work",
            "100000000",
            "--history-memory",
            "64MiB",
        ],
    );
    ok(&o);
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "199999");
    let stderr = String::from_utf8(o.stderr).unwrap();
    let p: serde_json::Value =
        serde_json::from_str(stderr.lines().rev().find(|s| s.starts_with('{')).unwrap()).unwrap();
    assert!(
        p["numeric_pages"]["live_bytes"].as_u64().unwrap() < 32 * 1024 * 1024,
        "{}",
        p["numeric_pages"]
    );
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn native_kernel_preflight_rejects_work_and_memory_before_publication() {
    let r = root();
    fs::write(r.join("main.rw"),format!(r#"{TAKE}let froms=take(stdNumericRangeInt(0,1,1));let tos=take(stdNumericRangeInt(0,1,1));let arrays=take(stdNumericGraphAdjacency(1,&froms,&tos));Out.println(1);publish;"#)).unwrap();
    let o = call(&r, &["run", "main.rw", "--native-work", "3000"]);
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("NativeWork"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.stdout.is_empty());
    fs::write(
        r.join("main.rw"),
        format!(r#"{TAKE}let large=take(stdNumericRangeInt(0,1,4096));Out.println(1);publish;"#),
    )
    .unwrap();
    let o = call(
        &r,
        &[
            "run",
            "main.rw",
            "--history-memory",
            "8KiB",
            "--native-work",
            "100000000",
        ],
    );
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("HistoryMemory"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.stdout.is_empty());
    fs::remove_dir_all(r).unwrap();
}
