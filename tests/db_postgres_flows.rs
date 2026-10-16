//! DB-02/03 on a real PostgreSQL: the transaction helper and the migration
//! flow behave as on SQLite. Skipped without the CI fixture unless
//! `REWIND_REQUIRE_PG` is set.
use std::{fs, process::Command};

fn run_fixture(fixture: &str, prefix: &str, expected: &str) {
    let dsn = std::env::var("REWIND_TEST_PG_DSN").ok();
    if std::env::var_os("REWIND_REQUIRE_PG").is_some() {
        assert!(dsn.is_some(), "PostgreSQL fixture is required");
    }
    let Some(dsn) = dsn else { return };
    let dir = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    fs::copy(format!("tests/fixtures/{fixture}"), dir.join("main.rw")).unwrap();
    fs::copy(
        std::env::var("REWIND_TEST_PG_CA").unwrap(),
        dir.join("ca.der"),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .env("REWIND_PG_DSN", &dsn)
        .args([
            "run",
            "main.rw",
            "--allow-effects",
            "external,db,tasks,env,fileRead,output",
            "--secret-env",
            "REWIND_PG_DSN",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}

#[test]
fn postgres_transaction_helper_tracks_commit_rollback_and_rejects_stale_requests() {
    run_fixture(
        "db_transaction_postgres.rw",
        "rewind-pg-tx",
        "second commit rejected\n1\n1\nstale completion rejected\ntrue\n",
    );
}

/// PostgreSQL DDL is transactional, so a failing migration leaves no table behind.
#[test]
fn postgres_migrations_apply_once_detect_edits_and_roll_back_ddl() {
    run_fixture(
        "db_migration_postgres.rw",
        "rewind-pg-migration",
        "2\n0\n2\nchecksum mismatch 001_users\nbroken command failed\nrolled back table is gone\n",
    );
}
