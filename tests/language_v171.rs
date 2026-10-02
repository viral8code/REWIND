use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn run(root: &Path, args: &[&str], dsn: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .env("REWIND_PG_DSN", dsn)
        .args(args)
        .output()
        .unwrap()
}
fn success(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn secret_aliases_typed_pg_rows_and_source_free_replay_do_not_reconnect() {
    let dsn = std::env::var("REWIND_TEST_PG_DSN").ok();
    if std::env::var_os("REWIND_REQUIRE_PG").is_some() {
        assert!(dsn.is_some(), "PostgreSQL fixture is required");
    }
    let Some(dsn) = dsn else { return };
    let root = std::env::temp_dir().join(format!("rewind-pg-cli-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/postgres/main.rw"),
    )
    .unwrap();
    fs::copy(
        std::env::var("REWIND_TEST_PG_CA").unwrap(),
        root.join("ca.der"),
    )
    .unwrap();
    let effects = "external,db,tasks,env,fileRead,output";
    success(&run(
        &root,
        &["compile", "main.rw", "--allow-effects", effects],
        &dsn,
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = run(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            effects,
            "--secret-env",
            "REWIND_PG_DSN",
            "--record",
            "trace.json",
        ],
        &dsn,
    );
    success(&out);
    assert_eq!(out.stdout, b"true\n42\nAlice\n");
    let trace = fs::read_to_string(root.join("trace.json")).unwrap();
    assert!(!trace.contains(&dsn));
    assert!(!trace.contains("rewind-fixture-only-password"));
    fs::remove_file(root.join("ca.der")).unwrap();
    let offline = "host=127.0.0.1 port=1 user=replay dbname=postgres sslmode=require";
    let replay = run(
        &root,
        &[
            "replay",
            "trace.json",
            "--allow-effects",
            effects,
            "--secret-env",
            "REWIND_PG_DSN",
        ],
        offline,
    );
    success(&replay);
    assert_eq!(out.stdout, replay.stdout);
    assert!(!root.join("ca.der").exists());
    fs::remove_dir_all(root).unwrap();
}
