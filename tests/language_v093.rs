use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn cmd(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(args)
        .arg("--root")
        .arg(root)
        .output()
        .unwrap()
}
fn success(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn fixture(source: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v093-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    for item in fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/std")).unwrap() {
        let path = item.unwrap().path();
        if path.extension().is_some_and(|e| e == "rw") && path.file_name().unwrap() != "tests.rw" {
            fs::copy(&path, root.join(path.file_name().unwrap())).unwrap();
        }
    }
    fs::write(
        root.join("main.rw"),
        source.replace("import lib.", "import "),
    )
    .unwrap();
    fs::write(
        root.join("rewind.toml"),
        "language=\"0.9.3\"\nsource_root=\".\"\nentry=\"main.rw\"\neffects=\"\"\n",
    )
    .unwrap();
    success(&cmd(&root, &["update"]));
    root
}
#[test]
fn sorting_search_and_modular_arithmetic_match_reference_values() {
    let mut source=String::from("import lib.sort as sort;import lib.search as search;import lib.sequence as sequence;import lib.integer as integer;import lib.modular as modular;let cmp=|a:Int,b:Int|->Int{if a<b{return -1;}if a>b{return 1;}return 0;};");
    let mut random = 42u64;
    for case in 0..24 {
        let mut values = vec![];
        for _ in 0..case % 13 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            values.push((random % 21) as i64 - 10);
        }
        source += &format!(
            "{{let xs=List<Int>();{}
let sorted=sort.stable(&xs,cmp);",
            values
                .iter()
                .map(|v| format!("xs.add({v});"))
                .collect::<String>()
        );
        values.sort();
        for (i, v) in values.iter().enumerate() {
            source += &format!("assert_eq(sorted.get({i}),{v});");
        }
        for key in [-11, -3, 0, 4, 11] {
            let lower = values.partition_point(|v| *v < key);
            let upper = values.partition_point(|v| *v <= key);
            source+=&format!("assert_eq(search.lowerBound(&sorted,{key},cmp),{lower});assert_eq(search.upperBound(&sorted,{key},cmp),{upper});");
        }
        source += "}";
    }
    source+="assert_eq(integer.gcd(0,0),Ok(0));assert_eq(integer.gcd(-9223372036854775807-1,-1),Ok(1));assert_eq(integer.gcd(-9223372036854775807-1,0),Err(\"Overflow\"));assert_eq(integer.combination(67,33),Err(\"Overflow\"));assert_eq(modular.add(9223372036854775806,9223372036854775806,9223372036854775807),Ok(9223372036854775805));";
    let root = fixture(&source);
    success(&cmd(&root, &["run", "--task-steps", "2000000"]));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn owners_returned_from_borrows_and_data_structures_restore_checkpoints() {
    let root = fixture(
        r#"import lib.disjointSet as dsu;import lib.fenwick as fw;import lib.heap as heap;import lib.segment as segment;import lib.deque.{create as createDeque,pushFront,pushBack,popFront,popBack};
 fn make()->Int effects {} {return 1;}
 fn owned(xs:&List<Int>)->List<Int> effects {} {let out=List<Int>();var i=0;while i<xs.len(){out.add(xs.get(i));i+=1;}return out;}
 let xs=List<Int>();xs.add(2);let copy=owned(&xs);xs.set(0,3);assert_eq(copy.get(0),2);
 match dsu.create(5){Ok(d)=>{commit beforeUnion;assert_eq(dsu.unite(&mut d,0,4),Ok(true));assert_eq(dsu.size(&mut d,4),Ok(2));revert beforeUnion;assert_eq(dsu.same(&mut d,0,4),Ok(false));},Err(_)=>{panic("dsu");}}
 match fw.create(4){Ok(t)=>{assert_eq(fw.add(&mut t,0,9223372036854775807),Ok(()));assert_eq(fw.add(&mut t,1,1),Err("Overflow"));assert_eq(fw.range(&t,0,1),Ok(9223372036854775807));assert_eq(fw.range(&t,1,2),Ok(0));commit beforeAdd;assert_eq(fw.add(&mut t,0,-2),Ok(()));revert beforeAdd;assert_eq(fw.prefix(&t,4),Ok(9223372036854775807));},Err(_)=>{panic("fenwick");}}
 match createDeque<Int>(2){Ok(d)=>{assert_eq(pushBack(&mut d,1),Ok(()));commit beforeDeque;assert_eq(pushFront(&mut d,2),Ok(()));assert_eq(pushBack(&mut d,3),Err("Capacity"));assert_eq(popBack(&mut d),Some(1));revert beforeDeque;assert_eq(popFront(&mut d),Some(1));assert_eq(popBack(&mut d),None);},Err(_)=>{panic("deque");}}
 let compare=|a:Int,b:Int|->Int{if a>b{return -1;}if a<b{return 1;}return 0;};let h=List<Int>();heap.push(&mut h,2,3,compare);heap.push(&mut h,3,3,compare);commit beforeHeap;assert_eq(heap.pop(&mut h,compare),Some(3));revert beforeHeap;assert_eq(heap.pop(&mut h,compare),Some(3));assert_eq(heap.pop(&mut h,compare),Some(2));
 "#,
    );
    let trace = root.join("trace.json");
    success(&cmd(
        &root,
        &[
            "run",
            "--record",
            trace.to_str().unwrap(),
            "--task-steps",
            "2000000",
        ],
    ));
    success(&cmd(&root, &["replay", trace.to_str().unwrap()]));
    let artifact = root.join("artifact.json");
    success(&cmd(
        &root,
        &["build", "--output", artifact.to_str().unwrap()],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rw") {
            fs::remove_file(path).unwrap();
        }
    }
    success(&cmd(
        &root,
        &[
            "run-artifact",
            artifact.to_str().unwrap(),
            "--task-steps",
            "2000000",
        ],
    ));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn graph_shortest_paths_match_relaxation_and_reject_invalid_weights() {
    let mut source = String::from("import lib.graph as graph;");
    for case in 0..8 {
        let n = 6;
        let mut edges = vec![];
        for a in 0..n {
            for b in 0..n {
                if a != b && (a * 7 + b * 3 + case) % 4 == 0 {
                    edges.push((a, b, ((a + b + case) % 9) as i64));
                }
            }
        }
        let mut distances = vec![None; n];
        distances[0] = Some(0i64);
        for _ in 0..n {
            for &(a, b, w) in &edges {
                if let Some(d) = distances[a] {
                    if distances[b].is_none_or(|x| x > d + w) {
                        distances[b] = Some(d + w);
                    }
                }
            }
        }
        source += &format!("match graph.create({n},{}){{Ok(g)=>{{", edges.len());
        for (a, b, w) in edges {
            source += &format!("assert_eq(graph.add(&mut g,{a},{b},{w}),Ok(()));");
        }
        source += "match graph.dijkstra(&g,0){Ok(d)=>{";
        for (i, d) in distances.into_iter().enumerate() {
            source += &format!(
                "assert_eq(d.get({i}),{});",
                d.map(|v| format!("Some({v})")).unwrap_or("None".into())
            );
        }
        source += "},Err(_)=>{panic(\"dijkstra\");}}},Err(_)=>{panic(\"graph\");}}";
    }
    source+="match graph.create(2,1){Ok(g)=>{graph.add(&mut g,0,1,-1);assert_eq(graph.dijkstra(&g,0),Err(\"NegativeWeight\"));assert_eq(graph.add(&mut g,0,2,0),Err(\"Range\"));},Err(_)=>{panic(\"graph\");}}";
    let root = fixture(&source);
    success(&cmd(&root, &["run", "--task-steps", "2000000"]));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn borrowed_mutable_payloads_and_readonly_pop_are_rejected() {
    for source in [
        "fn leak(xs:&List<List<Int>>)->List<Int> effects {} {return xs.get(0);}",
        "fn pop(xs:&List<Int>)->Option<Int> effects {} {return xs.pop();}",
    ] {
        let root = fixture(source);
        assert!(!cmd(&root, &["check"]).status.success());
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn scanners_are_byte_based_and_report_overflow_without_input_values() {
    let root = fixture(
        r#"import lib.scanner as scanner;import lib.bytes as bytes;
 match scanner.fromBytes(Bytes("9223372036854775808")){Ok(s)=>{assert_eq(scanner.nextInt(&mut s),Err("Overflow"));},Err(_)=>{panic("scanner");}}
 match scanner.fromBytes(Bytes("+ ")){Ok(s)=>{assert_eq(scanner.nextInt(&mut s),Err("InvalidNumber"));},Err(_)=>{panic("scanner");}}
 match scanner.fromBytes(Bytes("日本\t42")){Ok(s)=>{match scanner.nextToken(&mut s){Ok(Some(b))=>{assert_eq(bytes.decode(b),Ok("日本"));},_=>{panic("token");}}assert_eq(scanner.nextInt(&mut s),Ok(Some(42)));},Err(_)=>{panic("scanner");}}
 "#,
    );
    success(&cmd(&root, &["run"]));
    fs::remove_dir_all(root).unwrap();
}
