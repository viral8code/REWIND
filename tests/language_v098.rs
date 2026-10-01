mod support;
#[test]
fn csv_quotes_newlines_utf8_roundtrip_and_failures() {
    support::ok(
        r#"
import std.csv as csv;
fn bytes(s:String)->Bytes effects {} {match stdEncode(s){Ok(v)=>{return v;},Err(_)=>{panic("encode");}}}
match csv.parse(bytes("a,\"b,c\",\"d\"\"e\"\r\n\"あ\nい\",,z\n")){Ok(table)=>{assert_eq(csv.rows(&table),2);assert_eq(csv.width(&table,0),Ok(3));assert_eq(csv.get(&table,0,1),Ok("b,c"));assert_eq(csv.get(&table,0,2),Ok("d\"e"));assert_eq(csv.get(&table,1,0),Ok("あ\nい"));assert_eq(csv.get(&table,1,1),Ok(""));match csv.format(&table){Ok(encoded)=>{match csv.parse(encoded){Ok(copy)=>{assert_eq(csv.get(&copy,1,0),Ok("あ\nい"));assert_eq(csv.rows(&copy),2);},Err(_)=>{panic("roundtrip");}}},Err(_)=>{panic("format");}}},Err(_)=>{panic("csv");}}
assert_eq(csv.parse(bytes("\"x")),Err(csv.CsvError("UnclosedQuote",2)));assert_eq(csv.parse(bytes("\"x\"y")),Err(csv.CsvError("TrailingText",3)));match csv.parse(bytes("")){Ok(empty)=>{assert_eq(csv.rows(&empty),0);},Err(_)=>{panic("empty");}}
"#,
    );
}
#[test]
fn checked_matrix_powers_and_dynamic_programs() {
    support::ok(
        r#"
import std.matrix as matrix;import std.dp as dp;
let values=List<Int>();values.add(1);values.add(1);values.add(1);values.add(0);
match matrix.create(2,2,&values){Ok(input)=>{match matrix.power(&input,10){Ok(result)=>{assert_eq(matrix.get(&result,0,1),Ok(55));},Err(_)=>{panic("power");}}},Err(_)=>{panic("matrix");}}
let xs=List<Int>();xs.add(3);xs.add(1);xs.add(2);xs.add(2);xs.add(4);assert_eq(dp.lisLength(&xs),Ok(3));let weights=List<Int>();weights.add(0);weights.add(2);weights.add(3);let costs=List<Int>();costs.add(1);costs.add(4);costs.add(5);assert_eq(dp.knapsack(&weights,&costs,5),Ok(10));assert_eq(dp.knapsack(&weights,&xs,5),Err("Dimension"));
"#,
    );
}
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

#[test]
fn modular_inverse_covers_full_positive_int64_modulus() {
    support::ok(
        r#"
import std.modular as modular;
assert_eq(modular.inverse(2,9223372036854775807),Ok(4611686018427387904));
match modular.inverse(9223372036854775805,9223372036854775807){Ok(inverse)=>{assert_eq(modular.multiply(9223372036854775805,inverse,9223372036854775807),Ok(1));},Err(_)=>{panic("inverse");}}
assert_eq(modular.inverse(6,15),Err("NonInvertible"));
"#,
    );
}
