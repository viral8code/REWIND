mod support;
#[test]
fn exact_byte_search_matches_naive_reference_and_boundaries() {
    support::ok(
        r#"
import std.stringSearch as search;
fn bytes(s:String)->Bytes effects {} {match stdEncode(s){Ok(v)=>{return v;},Err(_)=>{panic("encode");}}}
let patterns=List<String>();patterns.add("");patterns.add("a");patterns.add("aa");patterns.add("aba");patterns.add("あ");
let input=bytes("aababaあaa");var p=0;while p<patterns.len(){let pattern=bytes(patterns.get(p));match search.kmp(input,pattern){Ok(found)=>{var cursor=0;var i=0;while i<=stdBytesLength(input)-stdBytesLength(pattern){var same=true;var j=0;while j<stdBytesLength(pattern){if stdBytesGet(input,i+j)!=stdBytesGet(pattern,j){same=false;}j+=1;}if same{assert_eq(found.get(cursor),i);cursor+=1;}i+=1;}assert_eq(found.len(),cursor);},Err(_)=>{panic("search");}}p+=1;}
match search.z(bytes("aaaaa")){Ok(z)=>{assert_eq(z.get(0),5);assert_eq(z.get(1),4);assert_eq(z.get(4),1);},Err(_)=>{panic("z");}}
match search.prefix(bytes("ababaca")){Ok(pi)=>{assert_eq(pi.get(4),3);assert_eq(pi.get(5),0);assert_eq(pi.get(6),1);},Err(_)=>{panic("prefix");}}
"#,
    );
}
#[test]
fn sparse_range_bitset_and_rollback_restore_invariants() {
    support::ok(
        r#"
import std.range as range;import std.bitset as bits;import std.rollbackSet as rollback;
let xs=List<Int>();xs.add(4);xs.add(-2);xs.add(7);xs.add(1);
match range.sparseMin(&xs){Ok(table)=>{var a=0;while a<4{var b=a+1;var expected=xs.get(a);while b<=4{if xs.get(b-1)<expected{expected=xs.get(b-1);}assert_eq(range.minimum(&table,a,b),Ok(expected));b+=1;}a+=1;}assert_eq(range.minimum(&table,0,0),Err("Range"));},Err(_)=>{panic("range");}}
match bits.create(130){Ok(set)=>{bits.set(&mut set,62,true);bits.set(&mut set,63,true);bits.set(&mut set,129,true);assert_eq(bits.count(&set),3);bits.set(&mut set,63,false);assert_eq(bits.count(&set),2);match bits.shifted(&set,1){Ok(next)=>{assert_eq(bits.get(&next,63),Ok(true));assert_eq(bits.count(&next),1);},Err(_)=>{panic("shift");}}},Err(_)=>{panic("bits");}}
match rollback.create(4){Ok(set)=>{rollback.unite(&mut set,0,1);let mark=rollback.mark(&set);rollback.unite(&mut set,1,2);assert_eq(rollback.same(&set,0,2),Ok(true));rollback.rollback(&mut set,mark);assert_eq(rollback.same(&set,0,2),Ok(false));assert_eq(rollback.same(&set,0,1),Ok(true));},Err(_)=>{panic("rollback");}}
"#,
    );
}
#[test]
fn nested_field_pop_rejects_read_only_owner_borrows() {
    let output = support::run(
        r#"
struct Bag{items:List<Int>}
fn bad(bag:&Bag)->Unit effects {} {bag.items.pop();}
let items=List<Int>();items.add(1);let bag=Bag(items);bad(&bag);
"#,
    );
    assert!(!output.status.success());
}
