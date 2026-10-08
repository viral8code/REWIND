use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(result: Output) -> Output {
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result
}
#[test]
fn direct_numeric_pages_keep_old_language_and_source_free_restore_contracts() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for language in ["1.9.53", "1.9.54"] {
        for mode in ["debug", "compact"] {
            let root = std::env::temp_dir().join(format!(
                "rewind-pages-{}-{language}-{mode}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            fs::write(
                root.join("main.rw"),
                fs::read_to_string(repo.join("examples/numeric-pages/main.rw"))
                    .unwrap()
                    .replace("import std.numeric", "import numeric"),
            )
            .unwrap();
            fs::copy(
                repo.join("libraries/std/numeric.rw"),
                root.join("numeric.rw"),
            )
            .unwrap();
            fs::write(root.join("rewind.toml"), format!("language = \"{language}\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n")).unwrap();
            ok(call(&root, &["update", "--root", "."]));
            ok(call(&root, &["compile", "main.rw"]));
            fs::remove_file(root.join("main.rw")).unwrap();
            fs::remove_file(root.join("numeric.rw")).unwrap();
            if root.join(".rewind").exists() {
                fs::remove_dir_all(root.join(".rewind")).unwrap();
            }
            let run = ok(call(
                &root,
                &[
                    "run",
                    "main.rwc",
                    "--steps",
                    "20000000",
                    "--native-work",
                    "100000000",
                    "--record",
                    "trace.json",
                    "--record-mode",
                    mode,
                ],
            ));
            assert_eq!(
                String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n"),
                "pages done\nrestore done\n"
            );
            assert_eq!(
                ok(call(&root, &["replay", "trace.json"])).stdout,
                run.stdout
            );
            fs::remove_dir_all(root).unwrap();
        }
    }
}
