mod support;
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
