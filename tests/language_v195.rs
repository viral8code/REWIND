use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v195-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
const TAKE:&str="fn take<T,E>(r:Result<T,E>)->T effects {} {match move r {Ok(v)=>{return move v;},Err(_)=>{panic(\"unexpected error\");}}}";
#[test]
fn residual_flow_matches_exhaustive_cuts_and_preserves_conservation_and_replay() {
    let root = root();
    let mut source = format!("import std.flow as flow;{TAKE}\n");
    let mut seed = 7u64;
    for n in 2..=7 {
        for _ in 0..8 {
            let mut edges = Vec::new();
            for from in 0..n {
                for to in 0..n {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    if seed >> 61 < 3 {
                        edges.push((from, to, ((seed >> 32) % 7) as i64));
                    }
                }
            }
            // Duplicate and antiparallel pairs exercise separate residual edge identities.
            edges.extend([(0, n - 1, 3), (0, n - 1, 2), (n - 1, 0, 1)]);
            let expected = (0..(1usize << (n - 2)))
                .map(|mask| {
                    let contains = |v: usize| v == 0 || (v != n - 1 && mask & (1 << (v - 1)) != 0);
                    edges
                        .iter()
                        .filter(|&&(from, to, _)| contains(from) && !contains(to))
                        .map(|e| e.2)
                        .sum::<i64>()
                })
                .min()
                .unwrap();
            source.push_str(&format!(
                "{{let network=take(flow.create({n},{}));",
                edges.len()
            ));
            for (id, (from, to, capacity)) in edges.iter().enumerate() {
                source.push_str(&format!(
                    "assert_eq(flow.add(&mut network,{from},{to},{capacity}),Ok({id}));"
                ));
            }
            source.push_str(&format!(
                "assert_eq(flow.augment(&mut network,0,{},9223372036854775807),Ok({expected}));",
                n - 1
            ));
            source.push_str("let balance=List<Int>();");
            source.push_str(&format!("for i in 0..{n}{{balance.push(0);}}for i in 0..{}{{let e=take(flow.edge(&network,i));assert_eq(e.flow>=0 && e.flow<=e.capacity,true);balance.set(e.from,balance.get(e.from)-e.flow);balance.set(e.to,balance.get(e.to)+e.flow);}}",edges.len()));
            source.push_str(&format!("assert_eq(balance.get(0),-{expected});assert_eq(balance.get({}),{expected});for i in 1..{}{{assert_eq(balance.get(i),0);}}",n-1,n-1));
            source.push_str(&format!("let seen=take(flow.reachable(&network,0));assert_eq(seen.get({}),false);var cut=0;for i in 0..{}{{let e=take(flow.edge(&network,i));if seen.get(e.from) && !seen.get(e.to){{cut+=e.capacity;}}}}assert_eq(cut,{expected});}}\n",n-1,edges.len()));
        }
    }
    source.push_str("Out.println(1);publish;");
    fs::write(root.join("main.rw"), source).unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--native-work",
            "100000000",
            "--steps",
            "10000000",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"1\n");
    fs::write(root.join("main.rw"), format!("import std.flow as flow;{TAKE}\n") + r#"
let network=take(flow.create(3,3));flow.add(&mut network,0,1,4)?;flow.add(&mut network,1,2,3)?;flow.add(&mut network,0,2,2)?;
commit saved;assert_eq(flow.augment(&mut network,0,2,3),Ok(3));revert saved;
assert_eq(flow.augment(&mut network,0,2,99),Ok(5));drop saved;Out.println(1);publish;
"#).unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--record",
            "trace.json",
            "--native-work",
            "100000000",
        ],
    );
    ok(&out);
    let replay = call(&root, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn iterative_long_path_partial_flow_checkpoint_and_preflight_errors() {
    let root = root();
    let source = format!("import std.flow as flow;{TAKE}\n")
        + r#"
let network=take(flow.create(1024,1026));
for i in 0..1023 {flow.add(&mut network,i,i+1,7)?;}
assert_eq(flow.add(&mut network,0,0,9),Ok(1023));
assert_eq(flow.add(&mut network,-1,0,1),Err("Range"));
assert_eq(flow.add(&mut network,0,1,-1),Err("Range"));
assert_eq(flow.augment(&mut network,0,0,1),Err("Range"));
assert_eq(flow.edges(&network),1024);
commit saved;
assert_eq(flow.augment(&mut network,0,1023,3),Ok(3));
assert_eq(flow.augment(&mut network,0,1023,99),Ok(4));
assert_eq(flow.augment(&mut network,0,1023,99),Ok(0));
revert saved;
assert_eq(take(flow.edge(&network,0)).flow,0);
assert_eq(flow.augment(&mut network,0,1023,99),Ok(7));
drop saved;
let wide=take(flow.create(2,2));flow.add(&mut wide,0,1,9223372036854775807)?;flow.add(&mut wide,0,1,9223372036854775807)?;
assert_eq(flow.augment(&mut wide,0,1,9223372036854775807),Ok(9223372036854775807));
assert_eq(flow.augment(&mut wide,0,1,9223372036854775807),Ok(9223372036854775807));
assert_eq(flow.augment(&mut wide,0,1,1),Ok(0));
let full=take(flow.create(1,0));assert_eq(flow.add(&mut full,0,0,0),Err("Capacity"));assert_eq(flow.edges(&full),0);
assert_eq(flow.edge(&full,0),Err("Range"));
Out.println(1);publish;
"#;
    fs::write(root.join("main.rw"), source).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rw",
            "--native-work",
            "100000000",
            "--steps",
            "10000000",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"1\n");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn every_small_bipartite_graph_has_optimal_matching_and_vertex_cover() {
    let root = root();
    let source = format!("import std.matching as matching;import std.bits as bits;{TAKE}\n")
        + r#"
for mask in 0..64 {
 let pairs=List<Tuple<Int,Int>>();for l in 0..2 {for r in 0..3 {if bits.and(mask,take(bits.shiftLeft(1,l*3+r)))!=0 {pairs.push((l,r));}}}
 var expected=0;
 for first in -1..3 {for second in -1..3 {
  var valid=true;var count=0;
  if first>=0 {if bits.and(mask,take(bits.shiftLeft(1,first)))==0 {valid=false;}count+=1;}
  if second>=0 {if bits.and(mask,take(bits.shiftLeft(1,3+second)))==0 {valid=false;}count+=1;}
  if first>=0 && first==second {valid=false;}
  if valid && count>expected {expected=count;}
 }}
 let result=take(matching.maximum(2,3,&pairs));assert_eq(result.size,expected);var cover=0;var count=0;
 for i in 0..2 {if result.leftCover.get(i) {cover+=1;}match result.left.get(i){None=>{},Some(r)=>{count+=1;assert_eq(result.right.get(r),Some(i));assert_eq(bits.and(mask,take(bits.shiftLeft(1,i*3+r)))!=0,true);}}}
 for i in 0..3 {if result.rightCover.get(i) {cover+=1;}}
 assert_eq(cover,expected);assert_eq(count,expected);
 for i in 0..pairs.len() {let (l,r)=pairs.get(i);assert_eq(result.leftCover.get(l)||result.rightCover.get(r),true);}
}
let empty=List<Tuple<Int,Int>>();assert_eq(take(matching.maximum(0,0,&empty)).size,0);
empty.push((0,0));assert_eq(matching.maximum(0,0,&empty),Err("Range"));
Out.println(1);publish;
"#;
    fs::write(root.join("main.rw"), source).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rw",
            "--native-work",
            "100000000",
            "--steps",
            "10000000",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"1\n");
    fs::remove_dir_all(root).unwrap();
}
