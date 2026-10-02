use rewind::{
    database::{Operation, Parameter},
    Runtime,
};
use serde_json::Value;
use std::time::{Duration, Instant};
fn call(runtime: &mut Runtime, operation: Operation) -> Value {
    runtime.enter_external(false).unwrap();
    let id = runtime.start_database(operation, 5000).unwrap();
    runtime.exit_external().unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if let Some(result) = runtime.poll_external(id).unwrap() {
            return result.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn committed_database_effect_survives_vm_revert_and_is_not_repeated() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let opened = call(
        &mut runtime,
        Operation::Sqlite {
            path: ":memory:".into(),
            read_only: false,
        },
    );
    let connection = opened["connection"].as_u64().unwrap() as usize;
    assert_eq!(
        call(
            &mut runtime,
            Operation::Execute {
                connection,
                sql: "CREATE TABLE values_table(value INTEGER)".into(),
                parameters: vec![]
            }
        )["changed"],
        0
    );
    runtime.commit("before_insert").unwrap();
    let insert = Operation::Execute {
        connection,
        sql: "INSERT INTO values_table VALUES (?1)".into(),
        parameters: vec![Parameter::Int(42)],
    };
    assert_eq!(call(&mut runtime, insert.clone())["changed"], 1);
    runtime.revert("before_insert").unwrap();
    assert_eq!(call(&mut runtime, insert)["changed"], 1);
    let query = call(
        &mut runtime,
        Operation::Query {
            connection,
            sql: "SELECT count(*), sum(value) FROM values_table".into(),
            parameters: vec![],
        },
    );
    let cursor = query["cursor"].as_u64().unwrap() as usize;
    let batch = call(
        &mut runtime,
        Operation::Next {
            cursor,
            rows: 64,
            bytes: 1048576,
        },
    );
    assert_eq!(batch["rows"][0][0]["value"], 1);
    assert_eq!(batch["rows"][0][1]["value"], 42);
    assert_eq!(
        call(&mut runtime, Operation::Close { connection })["unit"],
        true
    );
}
#[test]
fn closed_resources_and_invalid_paths_are_typed_recorded_errors() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    assert_eq!(
        call(
            &mut runtime,
            Operation::Sqlite {
                path: "../escape.sqlite".into(),
                read_only: false
            }
        )["error"]["code"],
        "DbPath"
    );
    let opened = call(
        &mut runtime,
        Operation::Sqlite {
            path: ":memory:".into(),
            read_only: false,
        },
    );
    let connection = opened["connection"].as_u64().unwrap() as usize;
    call(&mut runtime, Operation::Close { connection });
    assert_eq!(
        call(
            &mut runtime,
            Operation::Execute {
                connection,
                sql: "SELECT 1".into(),
                parameters: vec![]
            }
        )["error"]["code"],
        "DbClosed"
    );
}

#[test]
fn prepared_statement_reuse_and_parent_lifetime() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let connection = call(
        &mut runtime,
        Operation::Sqlite {
            path: ":memory:".into(),
            read_only: false,
        },
    )["connection"]
        .as_u64()
        .unwrap() as usize;
    call(
        &mut runtime,
        Operation::Execute {
            connection,
            sql: "CREATE TABLE data(value INTEGER)".into(),
            parameters: vec![],
        },
    );
    let statement = call(
        &mut runtime,
        Operation::Prepare {
            connection,
            sql: "INSERT INTO data VALUES (?1)".into(),
        },
    );
    assert_eq!(statement["parameters"], 1);
    let statement = statement["statement"].as_u64().unwrap() as usize;
    for n in [10, 20] {
        assert_eq!(
            call(
                &mut runtime,
                Operation::ExecuteStatement {
                    statement,
                    parameters: vec![Parameter::Int(n)]
                }
            )["changed"],
            1
        );
    }
    call(&mut runtime, Operation::Close { connection });
    assert_eq!(
        call(
            &mut runtime,
            Operation::ExecuteStatement {
                statement,
                parameters: vec![Parameter::Int(30)]
            }
        )["error"]["code"],
        "DbClosed"
    );
}

#[test]
fn private_binary_parameter_is_bound_but_never_recorded_as_public_row() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let connection = call(
        &mut runtime,
        Operation::Sqlite {
            path: ":memory:".into(),
            read_only: false,
        },
    )["connection"]
        .as_u64()
        .unwrap() as usize;
    runtime.enter_external(false).unwrap();
    runtime
        .register_database_parameter("opaque", Parameter::Bytes(vec![0xff, 0, 0xfe]))
        .unwrap()
        .unwrap();
    assert_eq!(
        runtime
            .register_database_parameter("opaque", Parameter::Bytes(vec![9]))
            .unwrap(),
        Err("DbPrivateParameterImmutable")
    );
    runtime.exit_external().unwrap();
    let query = call(
        &mut runtime,
        Operation::Query {
            connection,
            sql: "SELECT ?1".into(),
            parameters: vec![Parameter::Private("opaque".into())],
        },
    );
    let cursor = query["cursor"].as_u64().unwrap() as usize;
    assert_eq!(
        call(
            &mut runtime,
            Operation::Next {
                cursor,
                rows: 64,
                bytes: 1048576
            }
        )["error"]["code"],
        "DbSecretResult"
    );
    runtime.enter_external(false).unwrap();
    let error = runtime
        .start_database(
            Operation::Execute {
                connection,
                sql: "SELECT ?1".into(),
                parameters: vec![Parameter::Bytes(vec![0xff, 0, 0xfe])],
            },
            5000,
        )
        .unwrap_err();
    assert!(error.to_string().contains("SecretObservationUnrecordable"));
    runtime.exit_external().unwrap();
}

#[test]
fn late_secret_registration_rejects_export_of_previously_recorded_binary_row() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let connection = call(
        &mut runtime,
        Operation::Sqlite {
            path: ":memory:".into(),
            read_only: false,
        },
    )["connection"]
        .as_u64()
        .unwrap() as usize;
    let cursor = call(
        &mut runtime,
        Operation::Query {
            connection,
            sql: "SELECT X'ff00fe'".into(),
            parameters: vec![],
        },
    )["cursor"]
        .as_u64()
        .unwrap() as usize;
    assert_eq!(
        call(
            &mut runtime,
            Operation::Next {
                cursor,
                rows: 64,
                bytes: 1048576
            }
        )["done"],
        true
    );
    runtime.register_secret_value(&rewind::Value::Bytes(std::sync::Arc::new(vec![
        0xff, 0, 0xfe,
    ])));
    assert!(runtime
        .export_observations()
        .unwrap_err()
        .to_string()
        .contains("private database value"));
}

#[test]
fn offline_replay_returns_recorded_rows_without_opening_a_database() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let operations = [
        Operation::Sqlite {
            path: ":memory:".into(),
            read_only: false,
        },
        Operation::Query {
            connection: 0,
            sql: "SELECT ?1 AS label, NULL AS missing, X'00ff' AS bytes".into(),
            parameters: vec![Parameter::Text("public".into())],
        },
        Operation::Next {
            cursor: 1,
            rows: 64,
            bytes: 1048576,
        },
        Operation::Close { connection: 0 },
    ];
    let expected = operations
        .iter()
        .cloned()
        .map(|op| call(&mut runtime, op))
        .collect::<Vec<_>>();
    let tape = runtime.export_observations().unwrap();
    drop(runtime);
    let mut replay = Runtime::new(std::env::temp_dir()).unwrap();
    replay.import_observations(&tape).unwrap();
    for (operation, expected) in operations.into_iter().zip(expected) {
        assert_eq!(call(&mut replay, operation), expected);
    }
    assert_eq!(replay.native_resource_count(), 0);
}

#[test]
fn rollback_invalidates_cached_success_from_the_old_transaction() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let connection = call(
        &mut runtime,
        Operation::Sqlite {
            path: ":memory:".into(),
            read_only: false,
        },
    )["connection"]
        .as_u64()
        .unwrap() as usize;
    call(
        &mut runtime,
        Operation::Execute {
            connection,
            sql: "CREATE TABLE data(value INTEGER)".into(),
            parameters: vec![],
        },
    );
    call(&mut runtime, Operation::Begin { connection });
    runtime.commit("transaction").unwrap();
    let insert = Operation::Execute {
        connection,
        sql: "INSERT INTO data VALUES(42)".into(),
        parameters: vec![],
    };
    assert_eq!(call(&mut runtime, insert.clone())["changed"], 1);
    call(&mut runtime, Operation::Rollback { connection });
    runtime.revert("transaction").unwrap();
    runtime.enter_external(false).unwrap();
    assert!(runtime
        .start_database(insert, 5000)
        .unwrap_err()
        .to_string()
        .contains("DbTransactionExpired"));
    runtime.exit_external().unwrap();
}

#[test]
fn file_database_scope_close_rolls_back_and_releases_virtual_file_lock() {
    let root = std::env::temp_dir().join(format!(
        "rewind-sqlite-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut runtime = Runtime::new(&root).unwrap();
    let connection = call(
        &mut runtime,
        Operation::Sqlite {
            path: "data.sqlite".into(),
            read_only: false,
        },
    )["connection"]
        .as_u64()
        .unwrap() as usize;
    call(
        &mut runtime,
        Operation::Execute {
            connection,
            sql: "CREATE TABLE data(value INTEGER)".into(),
            parameters: vec![],
        },
    );
    call(&mut runtime, Operation::Begin { connection });
    call(
        &mut runtime,
        Operation::Execute {
            connection,
            sql: "INSERT INTO data VALUES(99)".into(),
            parameters: vec![],
        },
    );
    for path in [
        "data.sqlite",
        "data.sqlite-wal",
        "data.sqlite-shm",
        "data.sqlite-journal",
    ] {
        assert!(runtime
            .write_file(path, b"unsafe")
            .unwrap_err()
            .to_string()
            .contains("DbFileBusy"));
    }
    runtime.close_native_resource(connection as u64).unwrap();
    assert_eq!(call(&mut runtime, Operation::Cleanup)["unit"], true);
    let db = rusqlite::Connection::open(root.join("data.sqlite")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM data", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);
    runtime.write_file("data.sqlite", b"released").unwrap();
    drop(runtime);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn virtual_pending_database_file_is_rejected_before_host_open() {
    let root = std::env::temp_dir().join(format!(
        "rewind-pending-db-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut runtime = Runtime::new(&root).unwrap();
    runtime
        .write_file("pending.sqlite", b"unpublished")
        .unwrap();
    assert_eq!(
        call(
            &mut runtime,
            Operation::Sqlite {
                path: "pending.sqlite".into(),
                read_only: false
            }
        )["error"]["code"],
        "DbPendingFile"
    );
    assert!(!root.join("pending.sqlite").exists());
    drop(runtime);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn bulk_execution_prepares_once_and_partial_failure_can_be_rolled_back() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let connection = call(
        &mut runtime,
        Operation::Sqlite {
            path: ":memory:".into(),
            read_only: false,
        },
    )["connection"]
        .as_u64()
        .unwrap() as usize;
    call(
        &mut runtime,
        Operation::Execute {
            connection,
            sql: "CREATE TABLE data(value INTEGER UNIQUE)".into(),
            parameters: vec![],
        },
    );
    let parameters = (0..100).map(|n| vec![Parameter::Int(n)]).collect();
    assert_eq!(
        call(
            &mut runtime,
            Operation::ExecuteMany {
                connection,
                sql: "INSERT INTO data VALUES (?1)".into(),
                parameters
            }
        )["changed"],
        100
    );
    call(&mut runtime, Operation::Begin { connection });
    let failed = call(
        &mut runtime,
        Operation::ExecuteMany {
            connection,
            sql: "INSERT INTO data VALUES (?1)".into(),
            parameters: vec![
                vec![Parameter::Int(101)],
                vec![Parameter::Int(102)],
                vec![Parameter::Int(102)],
            ],
        },
    );
    assert_eq!(failed["error"]["code"], "DbSql");
    assert_eq!(failed["error"]["phase"], "Unknown");
    call(&mut runtime, Operation::Rollback { connection });
    let cursor = call(
        &mut runtime,
        Operation::Query {
            connection,
            sql: "SELECT count(*) FROM data".into(),
            parameters: vec![],
        },
    )["cursor"]
        .as_u64()
        .unwrap() as usize;
    assert_eq!(
        call(
            &mut runtime,
            Operation::Next {
                cursor,
                rows: 64,
                bytes: 1048576
            }
        )["rows"][0][0]["value"],
        100
    );
}
