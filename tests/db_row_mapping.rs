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
