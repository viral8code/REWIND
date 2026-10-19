use std::{fs, process::Command};

#[test]
fn std_crypto_limits_inputs_over_one_mebibyte() {
    let root = std::env::temp_dir().join(format!("rewind-std-crypto-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let file = root.join("main.rw");
    // The input is 2^20 + 1 bytes, one byte over the 1 MiB limit. It is built by doubling in the
    // language because a literal that size exceeds the 1 MiB module-source budget. The doubling
    // needs more native work than the default budget, so the run raises `--native-work`.
    let source = "import std.crypto as crypto;\n\
fn main() -> Int {\n\
    var s = \"x\";\n\
    for i in 0..20 {s = s + s;}\n\
    s = s + \"x\";\n\
    match crypto.sha256(Bytes(s)) {Ok(_)=>{Out.println(\"Ok\");},Err(e)=>{Out.println(e.code);}}\n\
    return 0;\n\
}\n";
    fs::write(&file, source).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .args([
            "run",
            file.to_str().unwrap(),
            "--steps",
            "10000000",
            "--native-work",
            "100000000",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"Limit\n");
    fs::remove_dir_all(root).unwrap();
}
