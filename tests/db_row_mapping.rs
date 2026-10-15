use std::{fs, process::Command};

/// DB-01 acceptance on SQLite: NULL cells, 64-bit integers, mapper failures with
/// the batch row index, duplicate and missing column names, and reuse of the
/// borrowed batch after a failed mapping.
#[test]
fn sqlite_row_mapping_reports_typed_failures_and_keeps_the_batch() {
    let dir = std::env::temp_dir().join(format!("rewind-db-mapping-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    fs::copy("tests/fixtures/db_row_mapping.rw", dir.join("main.rw")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .args(["run", "main.rw", "--allow-effects", "external,db,tasks"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "2\n9223372036854775807\nmissing column rejected\n0\nDbType\nduplicate rejected\n2\n"
    );
}

/// DB-02: the tracked state follows real SQLite commits and rollbacks, rejects
/// duplicate and stale requests, and does not revive a finished transaction.
#[test]
fn sqlite_transaction_helper_tracks_commit_rollback_and_rejects_stale_requests() {
    let dir = std::env::temp_dir().join(format!("rewind-db-tx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    fs::copy("tests/fixtures/db_transaction.rw", dir.join("main.rw")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .args(["run", "main.rw", "--allow-effects", "external,db,tasks"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "second commit rejected\n1\n1\nstale completion rejected\ntrue\n"
    );
}

/// DB-03: plan, apply, re-plan, detect an edited migration, and roll back a
/// migration whose second command fails, on a real SQLite database.
#[test]
fn sqlite_migrations_apply_once_detect_edits_and_roll_back_failures() {
    let dir = std::env::temp_dir().join(format!("rewind-db-migration-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    fs::copy("tests/fixtures/db_migration.rw", dir.join("main.rw")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .args(["run", "main.rw", "--allow-effects", "external,db,tasks"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "2\n0\n2\nchecksum mismatch 001_users\nbroken command failed\nrolled back table is gone\n"
    );
}

/// APP-01: the notes example saves through a transaction, lists rows by column
/// name, rolls back a failing save, and keeps the committed count.
#[test]
fn sqlite_notes_example_saves_lists_and_rolls_back_a_failed_save() {
    let dir = std::env::temp_dir().join(format!("rewind-app-notes-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    fs::copy("tests/fixtures/notes_sqlite.rw", dir.join("main.rw")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .args(["run", "main.rw", "--allow-effects", "external,db,tasks"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "title required\nsaved 2\n1 first\n2 second\nrolled back\ncount 2\n"
    );
}

/// DB-01 on SQLite: datetime text keeps its offset and nanoseconds through a
/// round trip, and a malformed stored value reports its row and column.
#[test]
fn sqlite_datetime_round_trip_keeps_offset_and_nanos_and_reports_bad_rows() {
    run_fixture(
        "db_datetime_sqlite.rw",
        "rewind-db-datetime",
        "column at rejected\ntrue\n2024-02-29T12:34:56.123456789+09:00\ntrue\ntrue\n",
    );
}

/// DB-01 on SQLite: a Decimal survives a BLOB round trip exactly, and a float
/// column is rejected instead of being accepted as an exact decimal.
#[test]
fn sqlite_decimal_blob_round_trip_is_exact_and_rejects_floats() {
    run_fixture(
        "db_decimal_blob_sqlite.rw",
        "rewind-db-decimal-blob",
        "9007199254740993.25\ntrue\nfloat rejected\n",
    );
}

/// DB-01 on SQLite: after a mapper failure mid-batch, closing the cursor lets
/// the same connection run another query.
#[test]
fn sqlite_cursor_is_closed_after_mapper_failure_and_connection_is_reusable() {
    run_fixture(
        "db_cursor_cleanup_sqlite.rw",
        "rewind-db-cursor-cleanup",
        "b rejected\ntrue\nok\n",
    );
}

fn run_fixture(fixture: &str, prefix: &str, expected: &str) {
    let dir = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    fs::copy(format!("tests/fixtures/{fixture}"), dir.join("main.rw")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .args(["run", "main.rw", "--allow-effects", "external,db,tasks"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}
