use std::{fs, process::Command};

#[test]
fn std_bits_bytes_check_values_and_errors() {
    let root = std::env::temp_dir().join(format!("rewind-std-bits-bytes-{}", std::process::id()));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let file = root.join("main.rw");
    // Each line of output is one checked result. Errors print their StdError code.
    let source = "import std.bits as bits;\n\
import std.bytes as bytes;\n\
fn main() -> Int {\n\
    match bytes.fromHex(\"01020304\") {Ok(raw)=>{\n\
        match bytes.readInt(raw,0,4,true) {Ok(v)=>{Out.println(v);},Err(e)=>{Out.println(e.code);}}\n\
        match bytes.readInt(raw,0,4,false) {Ok(v)=>{Out.println(v);},Err(e)=>{Out.println(e.code);}}\n\
        match bytes.readInt(raw,0,3,true) {Ok(v)=>{Out.println(v);},Err(e)=>{Out.println(e.code);}}\n\
    },Err(e)=>{Out.println(e.code);}}\n\
    Out.println(bits.byteSwap(258));\n\
    match bits.rotateLeft(1,64) {Ok(v)=>{Out.println(v);},Err(e)=>{Out.println(e.code);}}\n\
    match bits.extract(255,4,4) {Ok(v)=>{Out.println(v);},Err(e)=>{Out.println(e.code);}}\n\
    return 0;\n\
}\n";
    fs::write(&file, source).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args(["run", file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout),
        "16909060\n67305985\nRange\n144396663052566528\nRange\n15\n"
    );
    fs::remove_dir_all(root).unwrap();
}
