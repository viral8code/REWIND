mod support;
#[test]
fn graph_cycles_components_negative_weights_and_forest() {
    support::ok(
        r#"
import std.graph as graph;
match graph.create(5,8){Ok(g)=>{graph.add(&mut g,0,1,2);graph.add(&mut g,1,2,-1);graph.add(&mut g,0,2,4);graph.add(&mut g,3,4,1);
match graph.topological(&g){Ok(order)=>{assert_eq(order.len(),5);},Err(_)=>{panic("topological");}}
match graph.bellmanFord(&g,0){Ok(d)=>{assert_eq(d.get(0),Some(0));assert_eq(d.get(2),Some(1));assert_eq(d.get(3),None);},Err(_)=>{panic("bellman");}}
match graph.minimumForest(&g){Ok(f)=>{assert_eq(f.weight,2);assert_eq(f.components,2);assert_eq(f.froms.len(),3);},Err(_)=>{panic("forest");}}
graph.add(&mut g,2,1,0);assert_eq(graph.topological(&g),Err("Cycle"));assert_eq(graph.bellmanFord(&g,0),Err("NegativeCycle"));
match graph.components(&g){Ok(ids)=>{assert_eq(ids.get(1),ids.get(2));assert(ids.get(0)!=ids.get(1));assert(ids.get(3)!=ids.get(4));},Err(_)=>{panic("components");}}
},Err(_)=>{panic("graph");}}
"#,
    );
}
#[test]
fn components_match_reachability_reference() {
    support::ok(
        r#"
import std.graph as graph;
var mask=0;while mask<64{match graph.create(3,6){Ok(g)=>{var bit=1;var a=0;while a<3{var b=0;while b<3{if a!=b{if stdBitAnd(mask,bit)!=0{graph.add(&mut g,a,b,1);}bit*=2;}b+=1;}a+=1;}
match graph.components(&g){Ok(ids)=>{a=0;while a<3{match graph.bfs(&g,a){Ok(da)=>{var b=0;while b<3{match graph.bfs(&g,b){Ok(db)=>{assert_eq(ids.get(a)==ids.get(b),da.get(b)>=0 && db.get(a)>=0);},Err(_)=>{panic("bfs");}}b+=1;}},Err(_)=>{panic("bfs");}}a+=1;}},Err(_)=>{panic("scc");}}
},Err(_)=>{panic("graph");}}mask+=1;}
"#,
    );
}
#[test]
fn lca_checks_tree_shape_and_ancestor_queries() {
    support::ok(
        r#"
import std.graph as graph;
match graph.create(5,4){Ok(g)=>{graph.add(&mut g,0,1,0);graph.add(&mut g,0,2,0);graph.add(&mut g,1,3,0);graph.add(&mut g,1,4,0);match graph.ancestors(&g,0){Ok(tree)=>{assert_eq(graph.lca(&tree,3,4),Ok(1));assert_eq(graph.lca(&tree,2,3),Ok(0));assert_eq(graph.lca(&tree,1,3),Ok(1));assert_eq(graph.lca(&tree,4,4),Ok(4));assert_eq(graph.lca(&tree,-1,0),Err("Range"));},Err(_)=>{panic("ancestors");}}},Err(_)=>{panic("graph");}}
"#,
    );
}
