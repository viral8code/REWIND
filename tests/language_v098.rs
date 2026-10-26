mod support;
#[test]
fn copying_scalar_fields_does_not_allow_borrowed_mutable_payload_escape() {
    let output = support::run(
        r#"
struct Bag{items:List<Int>}
fn bad(input:&Bag)->Result<Bag,String> effects {} {return Ok(Bag(input.items));}
let xs=List<Int>();xs.add(1);let bag=Bag(xs);bad(&bag);
"#,
    );
    assert!(!output.status.success());
}

