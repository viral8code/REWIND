use std::{fs, path::Path, process::Command};

const SOURCE: &str =
    "effects {external,random};import std.crypto as crypto;import std.bytes as bytes;\n\
fn main() -> Int {\n\
    var low=\"\";\n\
    var high=\"\";\n\
    var a=\"\";\n\
    var b=\"\";\n\
    external {\n\
        match crypto.randomBytes(0) {Ok(_)=>{low=\"Ok\";},Err(e)=>{low=e.code;}}\n\
        match crypto.randomBytes(65537) {Ok(_)=>{high=\"Ok\";},Err(e)=>{high=e.code;}}\n\
        match crypto.randomBytes(32) {Ok(x)=>{a=bytes.toHex(x);},Err(e)=>{a=e.code;}}\n\
        match crypto.randomBytes(32) {Ok(x)=>{b=bytes.toHex(x);},Err(e)=>{b=e.code;}}\n\
    }\n\
    Out.println(low);\n\
    Out.println(high);\n\
    Out.println(a);\n\
    Out.println(b);\n\
    return 0;\n\
}\n";

fn root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("rewind-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    root
}

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn std_crypto_random_bytes_checks_length_and_draws_fresh_bytes() {
    let root = root("std-crypto-random");
    fs::write(root.join("main.rw"), SOURCE).unwrap();
    let result = run(
        &root,
        &["run", "main.rw", "--allow-effects", "external,random"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = String::from_utf8(result.stdout).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], "Range");
    assert_eq!(lines[1], "Range");
    assert_eq!(lines[2].len(), 64);
    assert_eq!(lines[3].len(), 64);
    assert_ne!(lines[2], lines[3]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn std_crypto_random_bytes_is_refused_without_external_permission() {
    let root = root("std-crypto-random-denied");
    fs::write(root.join("main.rw"), SOURCE).unwrap();
    let result = run(&root, &["run", "main.rw", "--allow-effects", "random"]);
    assert!(!result.status.success());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recorded_random_bytes_replays_the_same_value() {
    let root = root("std-crypto-random-replay");
    fs::write(root.join("main.rw"), SOURCE).unwrap();
    success(&run(
        &root,
        &["compile", "main.rw", "--allow-effects", "external,random"],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = run(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            "external,random",
            "--record",
            "trace.json",
        ],
    );
    success(&output);
    let replay = run(
        &root,
        &["replay", "trace.json", "--allow-effects", "external,random"],
    );
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}

fn success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
