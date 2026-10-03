use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v198-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
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
const TAKE:&str="fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic(\"unexpected error\");}}}";
fn execute(source: &str) {
    let root = root();
    fs::write(root.join("main.rw"), source).unwrap();
    ok(&call(
        &root,
        &[
            "run",
            "main.rw",
            "--steps",
            "20000000",
            "--native-work",
            "100000000",
        ],
    ));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn generic_recursion_retains_bounds_and_rejects_missing_constraints() {
    execute("fn recurse<T:Share>(n:Int,v:T)->T effects {} {if n==0{return v;}let next=recurse(n-1,v);return next;}assert_eq(recurse(5,42),42);");
    let root = root();
    fs::write(root.join("main.rw"),"fn share<T:Share>(v:T)->T effects {} {return v;}fn invalid<T>(v:T)->T effects {} {return share(v);}Out.println(invalid(7));").unwrap();
    let out = call(&root, &["check", "main.rw"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Share"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn lazy_affine_updates_match_naive_array_and_preserve_order() {
    let mut source=format!("import std.lazySegment as lazy;{TAKE}\nfn sum(a:Int,b:Int)->Int effects {{}} {{return a+b;}}fn apply(a:Int,t:Tuple<Int,Int>,n:Int)->Int effects {{}} {{return t._0*a+t._1*n;}}fn compose(a:Tuple<Int,Int>,b:Tuple<Int,Int>)->Tuple<Int,Int> effects {{}} {{return (b._0*a._0,b._0*a._1+b._1);}}let xs=List<Int>();for i in 0..31{{xs.add(i);}}let tree=take(lazy.build<Int,Tuple<Int,Int>>(&xs,0,sum));");
    let mut values: Vec<i64> = (0..31).collect();
    let mut seed = 19u64;
    for step in 0..128 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = (seed as usize) % 32;
        let b = ((seed >> 32) as usize) % 32;
        let (l, r) = (a.min(b), a.max(b));
        if step % 3 != 0 {
            let scale = ((seed >> 60) & 1) as i64;
            let offset = ((seed >> 48) % 21) as i64 - 10;
            for v in &mut values[l..r] {
                *v = scale * *v + offset;
            }
            source+=&format!("assert_eq(lazy.update(&mut tree,{l},{r},({scale},{offset}),sum,apply,compose),Ok(()));");
        } else {
            let expected: i64 = values[l..r].iter().sum();
            source += &format!(
                "assert_eq(lazy.query(&mut tree,{l},{r},sum,apply,compose),Ok({expected}));"
            );
        }
    }
    source+="assert_eq(lazy.update(&mut tree,-1,2,(1,9),sum,apply,compose),Err(\"Range\"));fn join(a:String,b:String)->String effects {} {return a+b;}fn unchanged(a:String,t:Int,n:Int)->String effects {} {return a;}fn tags(a:Int,b:Int)->Int effects {} {return 0;}let letters=List<String>();letters.add(\"a\");letters.add(\"b\");letters.add(\"c\");letters.add(\"d\");let text=take(lazy.build<String,Int>(&letters,\"\",join));assert_eq(lazy.query(&mut text,1,4,join,unchanged,tags),Ok(\"bcd\"));";
    execute(&source);
}
#[test]
fn trie_prunes_reuses_nodes_and_prevalidates_capacity() {
    let source = format!(
        r#"import std.trie as trie;{TAKE}
 let words=take(trie.createText<Int>(16));assert_eq(trie.textInsert(&mut words,"日本",1),Ok(None));assert_eq(trie.textInsert(&mut words,"日本語",2),Ok(None));assert_eq(trie.textPrefixCount(&words,"日"),Ok(2));assert_eq(trie.textInsert(&mut words,"日本",9),Ok(Some(1)));assert_eq(trie.textSize(&words),2);
 commit wordsSaved;take(trie.textRemove(&mut words,"日本"));assert_eq(trie.textGet(&words,"日本語"),Ok(Some(2)));revert wordsSaved;assert_eq(trie.textGet(&words,"日本"),Ok(Some(9)));drop wordsSaved;
 take(trie.textRemove(&mut words,"日本"));take(trie.textRemove(&mut words,"日本語"));assert_eq(trie.textNodes(&words),1);
 for i in 0..1000{{assert_eq(trie.textInsert(&mut words,"abcdefghijklmno",i),Ok(None));assert_eq(trie.textInsert(&mut words,"x",1),Err("Capacity"));assert_eq(trie.textRemove(&mut words,"abcdefghijklmno"),Ok(Some(i)));assert_eq(trie.textNodes(&words),1);}}
 assert_eq(trie.textInsert(&mut words,"",5),Ok(None));assert_eq(trie.textPrefixCount(&words,""),Ok(1));assert_eq(trie.textRemove(&mut words,""),Ok(Some(5)));
 let binary=take(trie.create<Int>(5));let bytes=take(stdEncode("a\0b"));assert_eq(trie.insert(&mut binary,bytes,3),Ok(None));assert_eq(trie.get(&binary,bytes),Some(3));assert_eq(trie.remove(&mut binary,bytes),Some(3));assert_eq(trie.nodes(&binary),1);
 "#
    );
    execute(&source);
}
#[test]
fn suffixes_match_independent_byte_order_lcp_and_search() {
    let mut source = format!("import std.suffix as suffix;import std.numeric as numeric;{TAKE}");
    for input in ["", "banana", "aaaaaa", "日本語日本", "a\0ba\0"] {
        let bytes = input.as_bytes();
        let mut order: Vec<usize> = (0..bytes.len()).collect();
        order.sort_by(|a, b| bytes[*a..].cmp(&bytes[*b..]));
        source+=&format!("{{let index=take(suffix.build(take(stdEncode({}))));let sa=suffix.order(&index);let lc=suffix.lcp(&index);let orders=take(numeric.valuesInt(&sa));let common=take(numeric.valuesInt(&lc));assert_eq(orders.len(),{});",serde_json::to_string(input).unwrap().replace("\\u0000", "\\0"),bytes.len());
        for (i, &p) in order.iter().enumerate() {
            let lcp = if i == 0 {
                0
            } else {
                bytes[p..]
                    .iter()
                    .zip(&bytes[order[i - 1]..])
                    .take_while(|(a, b)| a == b)
                    .count()
            };
            source += &format!("assert_eq(orders.get({i}),{p});assert_eq(common.get({i}),{lcp});");
        }
        for pattern in ["", "a", "ana", "日", "zzz", "\0"] {
            let pattern = pattern.as_bytes();
            let compare = |p: usize| {
                if bytes[p..].starts_with(pattern) {
                    std::cmp::Ordering::Equal
                } else {
                    bytes[p..].cmp(pattern)
                }
            };
            let l = order.partition_point(|&p| compare(p) == std::cmp::Ordering::Less);
            let r = order.partition_point(|&p| compare(p) != std::cmp::Ordering::Greater);
            source += &format!(
                "assert_eq(suffix.find(&index,take(stdEncode({}))),Ok(({l},{r})));",
                serde_json::to_string(std::str::from_utf8(pattern).unwrap())
                    .unwrap()
                    .replace("\\u0000", "\\0")
            );
        }
        source += "}";
    }
    execute(&source);
}
#[test]
fn geometry_exact_full_int_predicates_hull_and_float_tolerance() {
    let mut source = format!("import std.geometry as g;{TAKE}");
    let mut seed = 23u64;
    for _ in 0..80 {
        let mut coordinates = [0i64; 6];
        for v in &mut coordinates {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            *v = ((seed >> 32) % 2000001) as i64 - 1000000;
        }
        let [a, b, c, d, e, f] = coordinates;
        let det = (c as i128 - a as i128) * (f as i128 - b as i128)
            - (d as i128 - b as i128) * (e as i128 - a as i128);
        source += &format!(
            "assert_eq(g.orientation(g.Point({a},{b}),g.Point({c},{d}),g.Point({e},{f})),Ok({}));",
            det.signum()
        );
    }
    source += r#"let lo=(-9223372036854775807-1);let hi=9223372036854775807;assert_eq(g.orientation(g.Point(lo,lo),g.Point(hi,lo),g.Point(lo,hi)),Ok(1));assert_eq(g.orientation(g.Point(hi,hi),g.Point(lo,hi),g.Point(hi,lo)),Ok(1));assert_eq(g.intersects(g.Point(lo,lo),g.Point(hi,hi),g.Point(lo,hi),g.Point(hi,lo)),Ok(true));assert_eq(g.intersects(g.Point(0,0),g.Point(0,0),g.Point(0,0),g.Point(1,1)),Ok(true));assert_eq(g.intersects(g.Point(0,0),g.Point(1,0),g.Point(2,0),g.Point(3,0)),Ok(false));let points=List<g.Point>();points.add(g.Point(0,0));points.add(g.Point(2,0));points.add(g.Point(2,2));points.add(g.Point(0,2));points.add(g.Point(1,1));points.add(g.Point(1,0));points.add(g.Point(0,0));let hull=take(g.convexHull(&points,false));assert_eq(hull.len(),4);assert_eq(hull.get(0),g.Point(0,0));assert_eq(hull.get(1),g.Point(2,0));assert_eq(hull.get(2),g.Point(2,2));assert_eq(hull.get(3),g.Point(0,2));let boundary=take(g.convexHull(&points,true));assert_eq(boundary.len(),5);assert_eq(g.orientationFloat(g.Vector(0.0,0.0),g.Vector(1.0,0.0),g.Vector(1.0,0.001),0.01),Ok(0));assert_eq(g.orientationFloat(g.Vector(0.0,0.0),g.Vector(1.0,0.0),g.Vector(1.0,0.001),-0.01),Err(StdError("NumericDomain",0)));"#;
    execute(&source);
}
#[test]
fn advanced_algorithms_checkpoint_source_free_and_both_record_modes() {
    let root = root();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/advanced-algorithms/main.rw"),
    )
    .unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    for mode in ["debug", "compact"] {
        let out = call(
            &root,
            &[
                "run",
                "main.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(out.stdout, b"28\n2\n1\n1\n");
        let replay = call(&root, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn suffix_work_is_refused_before_large_kernel_and_empty_tree_is_valid() {
    let root = root();
    fs::write(root.join("main.rw"),format!("import std.suffix as suffix;{TAKE}let index=take(suffix.build(take(stdEncode(\"{}\"))));", "a".repeat(65536))).unwrap();
    let out = call(&root, &["run", "main.rw", "--native-work", "1000000"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("NativeWorkBudgetExceeded"));
    fs::remove_dir_all(root).unwrap();
    execute(&format!("import std.lazySegment as lazy;{TAKE}fn sum(a:Int,b:Int)->Int effects {{}} {{return a+b;}}fn apply(a:Int,b:Int,n:Int)->Int effects {{}} {{return a+b*n;}}let xs=List<Int>();let tree=take(lazy.build<Int,Int>(&xs,0,sum));assert_eq(lazy.query(&mut tree,0,0,sum,apply,sum),Ok(0));assert_eq(lazy.update(&mut tree,0,0,7,sum,apply,sum),Ok(()));assert_eq(lazy.update(&mut tree,0,1,7,sum,apply,sum),Err(\"Range\"));"));
}
