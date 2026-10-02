use rewind::{
    database::{Operation, Parameter},
    Runtime,
};
use serde_json::Value;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn fixture() -> Option<(String, Vec<u8>)> {
    let dsn = std::env::var("REWIND_TEST_PG_DSN").ok();
    if std::env::var_os("REWIND_REQUIRE_PG").is_some() {
        assert!(dsn.is_some(), "PostgreSQL fixture is required");
    }
    dsn.map(|s| {
        (
            s,
            std::fs::read(std::env::var("REWIND_TEST_PG_CA").expect("fixture CA path")).unwrap(),
        )
    })
}
fn runtime(dsn: &str, ca: &[u8]) -> Runtime {
    let root = std::env::temp_dir().join(format!(
        "rewind-pg-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut runtime = Runtime::new(root).unwrap();
    runtime.enter_external(false).unwrap();
    assert_eq!(
        runtime
            .register_postgres_credentials("fixture", dsn, ca)
            .unwrap(),
        Ok(())
    );
    runtime.exit_external().unwrap();
    runtime
}
fn call_timeout(runtime: &mut Runtime, operation: Operation, timeout: u64) -> Value {
    runtime.enter_external(false).unwrap();
    let id = runtime.start_database(operation, timeout).unwrap();
    runtime.exit_external().unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(result) = runtime.poll_external(id).unwrap() {
            return result.unwrap();
        }
        assert!(Instant::now() < deadline, "DB operation did not complete");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn call(runtime: &mut Runtime, operation: Operation) -> Value {
    call_timeout(runtime, operation, 5000)
}
fn open(runtime: &mut Runtime) -> usize {
    let result = call(
        runtime,
        Operation::Postgres {
            alias: "fixture".into(),
        },
    );
    assert_eq!(result["backend"], "postgres", "{result}");
    result["connection"].as_u64().unwrap() as usize
}
fn execute(
    runtime: &mut Runtime,
    connection: usize,
    sql: &str,
    parameters: Vec<Parameter>,
) -> Value {
    call(
        runtime,
        Operation::Execute {
            connection,
            sql: sql.into(),
            parameters,
        },
    )
}
fn query(runtime: &mut Runtime, connection: usize, sql: &str, parameters: Vec<Parameter>) -> usize {
    let result = call(
        runtime,
        Operation::Query {
            connection,
            sql: sql.into(),
            parameters,
        },
    );
    assert!(result["cursor"].is_number(), "{result}");
    result["cursor"].as_u64().unwrap() as usize
}
fn next(runtime: &mut Runtime, cursor: usize, rows: usize) -> Value {
    call(
        runtime,
        Operation::Next {
            cursor,
            rows,
            bytes: 1048576,
        },
    )
}
#[test]
fn tls_parameter_types_sqlstate_and_bounded_cursor_roundtrip() {
    let Some((dsn, ca)) = fixture() else { return };
    let mut runtime = runtime(&dsn, &ca);
    let connection = open(&mut runtime);
    assert_eq!(
        execute(
            &mut runtime,
            connection,
            "CREATE TEMP TABLE sample (id BIGINT PRIMARY KEY, name TEXT)",
            vec![]
        )["changed"],
        0
    );
    let prepared = call(
        &mut runtime,
        Operation::Prepare {
            connection,
            sql: "INSERT INTO sample VALUES ($1,$2)".into(),
        },
    );
    assert_eq!(prepared["parameters"], 2);
    let statement = prepared["statement"].as_u64().unwrap() as usize;
    assert_eq!(
        call(
            &mut runtime,
            Operation::ExecuteStatement {
                statement,
                parameters: vec![Parameter::Int(1), Parameter::Text("Alice".into())]
            }
        )["changed"],
        1
    );
    let duplicate = execute(
        &mut runtime,
        connection,
        "INSERT INTO sample VALUES ($1,$2)",
        vec![Parameter::Int(1), Parameter::Text("another".into())],
    );
    assert_eq!(duplicate["error"]["sqlState"], "23505");
    assert!(duplicate["error"]["sqlCode"].is_null());
    let cursor=query(&mut runtime,connection,"SELECT $1::boolean AS flag,$2::bigint AS integer,$3::double precision AS number,$4::text AS name,$5::bytea AS binary,$6::text AS missing",vec![Parameter::Bool(true),Parameter::Int(i64::MAX),Parameter::Float(1.25),Parameter::Text("日本語".into()),Parameter::Bytes(vec![0,255]),Parameter::Null]);
    assert_eq!(
        execute(
            &mut runtime,
            connection,
            "INSERT INTO sample VALUES (2,'busy')",
            vec![]
        )["error"]["code"],
        "DbBusy"
    );
    let result = next(&mut runtime, cursor, 64);
    assert_eq!(result["done"], true);
    assert_eq!(result["rows"][0][0]["type"], "Bool");
    assert_eq!(result["rows"][0][0]["value"], true);
    assert_eq!(result["rows"][0][1]["value"], i64::MAX);
    assert_eq!(result["rows"][0][3]["value"], "日本語");
    assert_eq!(result["rows"][0][4]["value"], "AP8=");
    assert_eq!(result["rows"][0][5]["type"], "Null");
    let cursor = query(
        &mut runtime,
        connection,
        "SELECT generate_series(1,137)::bigint AS n",
        vec![],
    );
    let mut count = 0;
    loop {
        let batch = next(&mut runtime, cursor, 32);
        let rows = batch["rows"].as_array().unwrap();
        for row in rows {
            count += 1;
            assert_eq!(row[0]["value"], count);
        }
        if batch["done"] == true {
            break;
        }
    }
    assert_eq!(count, 137);
    assert_eq!(next(&mut runtime, cursor, 1)["error"]["code"], "DbClosed");
    assert_eq!(
        call(&mut runtime, Operation::Close { connection })["unit"],
        true
    );
}
#[test]
fn physical_transactions_and_partial_bulk_failures_are_independent_of_vm_revert() {
    let Some((dsn, ca)) = fixture() else { return };
    let mut runtime = runtime(&dsn, &ca);
    let connection = open(&mut runtime);
    execute(
        &mut runtime,
        connection,
        "CREATE TEMP TABLE sample (id BIGINT PRIMARY KEY)",
        vec![],
    );
    assert_eq!(
        call(&mut runtime, Operation::Begin { connection })["unit"],
        true
    );
    runtime.commit("before_insert").unwrap();
    let insert = Operation::Execute {
        connection,
        sql: "INSERT INTO sample VALUES ($1)".into(),
        parameters: vec![Parameter::Int(42)],
    };
    assert_eq!(call(&mut runtime, insert.clone())["changed"], 1);
    runtime.revert("before_insert").unwrap();
    assert_eq!(call(&mut runtime, insert)["changed"], 1);
    runtime.enter_external(true).unwrap();
    let rollback = runtime
        .start_database(Operation::Rollback { connection }, 5000)
        .unwrap();
    runtime.exit_external().unwrap();
    while runtime.poll_external(rollback).unwrap().is_none() {
        std::thread::sleep(Duration::from_millis(1));
    }
    let cursor = query(
        &mut runtime,
        connection,
        "SELECT count(*) FROM sample",
        vec![],
    );
    assert_eq!(next(&mut runtime, cursor, 64)["rows"][0][0]["value"], 0);
    let batch = call(
        &mut runtime,
        Operation::ExecuteMany {
            connection,
            sql: "INSERT INTO sample VALUES ($1)".into(),
            parameters: vec![
                vec![Parameter::Int(1)],
                vec![Parameter::Int(2)],
                vec![Parameter::Int(1)],
            ],
        },
    );
    assert_eq!(batch["error"]["sqlState"], "23505");
    let cursor = query(
        &mut runtime,
        connection,
        "SELECT count(*) FROM sample",
        vec![],
    );
    let result = next(&mut runtime, cursor, 64);
    assert_eq!(result["rows"][0][0]["value"], 2);
    assert_eq!(
        execute(
            &mut runtime,
            connection,
            "/* cannot bypass */ BEGIN",
            vec![]
        )["error"]["code"],
        "DbConfiguration"
    );
    call(&mut runtime, Operation::Close { connection });
}
#[test]
fn deadlines_cancel_active_queries_and_cleanup_does_not_leave_a_locked_connection() {
    let Some((dsn, ca)) = fixture() else { return };
    let mut runtime = runtime(&dsn, &ca);
    let connection = open(&mut runtime);
    let cursor = query(
        &mut runtime,
        connection,
        "SELECT 7::bigint FROM pg_sleep(30)",
        vec![],
    );
    let start = Instant::now();
    let result = call_timeout(
        &mut runtime,
        Operation::Next {
            cursor,
            rows: 1,
            bytes: 1048576,
        },
        20,
    );
    assert_eq!(result["error"]["code"], "DbDeadline");
    assert!(start.elapsed() < Duration::from_secs(3));
    call(&mut runtime, Operation::Close { connection });
    let cleanup = call(&mut runtime, Operation::Cleanup);
    assert_eq!(cleanup["unit"], true, "{cleanup}");
    let connection = open(&mut runtime);
    let cursor = query(&mut runtime, connection, "SELECT 9::bigint", vec![]);
    assert_eq!(next(&mut runtime, cursor, 64)["rows"][0][0]["value"], 9);
    call(&mut runtime, Operation::Close { connection });
}
#[test]
fn missing_trust_invalid_aliases_and_secret_results_are_not_recorded_as_public_data() {
    let Some((dsn, ca)) = fixture() else { return };
    let mut runtime = runtime(&dsn, &ca);
    runtime.enter_external(false).unwrap();
    assert_eq!(
        runtime
            .register_postgres_credentials(
                "fixture",
                &(dsn.clone() + " application_name=changed"),
                &ca
            )
            .unwrap(),
        Err("DbCredentialImmutable")
    );
    assert_eq!(
        runtime
            .register_postgres_credentials("untrusted", &dsn, &[])
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        runtime
            .register_database_parameter("hidden", Parameter::Text(dsn.clone()))
            .unwrap(),
        Ok(())
    );
    runtime.exit_external().unwrap();
    let untrusted = call(
        &mut runtime,
        Operation::Postgres {
            alias: "untrusted".into(),
        },
    );
    assert_eq!(untrusted["error"]["code"], "DbTls");
    assert_eq!(
        call(
            &mut runtime,
            Operation::Postgres {
                alias: "missing".into()
            }
        )["error"]["code"],
        "DbCredentialUnknown"
    );
    let connection = open(&mut runtime);
    let cursor = query(
        &mut runtime,
        connection,
        "SELECT $1::text",
        vec![Parameter::Private("hidden".into())],
    );
    let result = next(&mut runtime, cursor, 64);
    assert_eq!(result["error"]["code"], "DbSecretResult");
    call(&mut runtime, Operation::Close { connection });
    let exported = serde_json::to_string(&runtime.export_observations().unwrap()).unwrap();
    assert!(!exported.contains(&dsn));
}

#[test]
fn scram_authentication_failures_preserve_sqlstate_without_server_messages() {
    let Some((dsn, ca)) = fixture() else { return };
    // A wrong password is supplied through a private alias, never through an observation fingerprint.
    let wrong = dsn.replace("rewind-fixture-only-password", "wrong-fixture-password");
    assert_ne!(dsn, wrong, "The real SCRAM fixture must be used");
    let mut runtime = runtime(&wrong, &ca);
    let result = call(
        &mut runtime,
        Operation::Postgres {
            alias: "fixture".into(),
        },
    );
    assert_eq!(result["error"]["code"], "DbAuthentication");
    assert_eq!(result["error"]["sqlState"], "28P01");
    assert!(!result.to_string().contains("wrong-fixture-password"));
}
#[test]
fn scope_owned_connections_finish_rollback_and_closed_tokens_do_not_reopen() {
    let Some((dsn, ca)) = fixture() else { return };
    let mut runtime = runtime(&dsn, &ca);
    let connection = open(&mut runtime);
    assert_eq!(
        call(&mut runtime, Operation::Begin { connection })["unit"],
        true
    );
    execute(
        &mut runtime,
        connection,
        "CREATE TEMP TABLE closing (id BIGINT)",
        vec![],
    );
    runtime.close_native_resource(connection as u64).unwrap();
    assert_eq!(call(&mut runtime, Operation::Cleanup)["unit"], true);
    assert_eq!(
        execute(
            &mut runtime,
            connection,
            "INSERT INTO closing VALUES (1)",
            vec![]
        )["error"]["code"],
        "DbClosed"
    );
}

#[test]
fn committed_write_survives_revert_but_transaction_success_cannot_be_reused_as_uncommitted() {
    let Some((dsn, ca)) = fixture() else { return };
    let mut runtime = runtime(&dsn, &ca);
    let connection = open(&mut runtime);
    execute(
        &mut runtime,
        connection,
        "CREATE TEMP TABLE committed (id BIGINT PRIMARY KEY)",
        vec![],
    );
    call(&mut runtime, Operation::Begin { connection });
    runtime.commit("before_write").unwrap();
    let insert = Operation::Execute {
        connection,
        sql: "INSERT INTO committed VALUES ($1)".into(),
        parameters: vec![Parameter::Int(42)],
    };
    assert_eq!(call(&mut runtime, insert.clone())["changed"], 1);
    call(&mut runtime, Operation::Commit { connection });
    runtime.revert("before_write").unwrap();
    runtime.enter_external(false).unwrap();
    let error = runtime.start_database(insert, 5000).unwrap_err();
    runtime.exit_external().unwrap();
    assert!(error.to_string().contains("DbTransactionExpired"));
    runtime.enter_external(true).unwrap();
    runtime.exit_external().unwrap();
    let cursor = query(
        &mut runtime,
        connection,
        "SELECT sum(id)::bigint FROM committed",
        vec![],
    );
    assert_eq!(next(&mut runtime, cursor, 64)["rows"][0][0]["value"], 42);
    call(&mut runtime, Operation::Close { connection });
}
#[test]
fn abrupt_backend_disconnect_is_typed_and_does_not_reconnect() {
    let Some((dsn, ca)) = fixture() else { return };
    let mut runtime = runtime(&dsn, &ca);
    let connection = open(&mut runtime);
    let cursor = query(
        &mut runtime,
        connection,
        "SELECT 1::bigint WHERE pg_terminate_backend(pg_backend_pid())",
        vec![],
    );
    let result = next(&mut runtime, cursor, 64);
    assert!(
        matches!(
            result["error"]["code"].as_str(),
            Some("DbSql" | "DbDisconnected")
        ),
        "{result}"
    );
    assert_eq!(result["error"]["phase"], "Unknown");
    runtime.close_native_resource(connection as u64).unwrap();
    let cleanup = call(&mut runtime, Operation::Cleanup);
    assert_eq!(cleanup["error"]["code"], "DbCleanup");
    assert_eq!(
        execute(
            &mut runtime,
            connection,
            "CREATE TABLE must_not_reconnect (id BIGINT)",
            vec![]
        )["error"]["code"],
        "DbClosed"
    );
}

#[test]
fn numeric_protocol_is_exact_finite_and_preserves_declared_scale() {
    let Some((dsn, ca)) = fixture() else { return };
    let mut runtime = runtime(&dsn, &ca);
    let connection = open(&mut runtime);
    let cursor=query(&mut runtime,connection,"SELECT $1::numeric + $2::numeric AS amount, $3::numeric AS exact, NULL::numeric AS missing",vec![Parameter::Text("0.1".into()),Parameter::Text("0.2".into()),Parameter::Text("12345678901234567890.12340000".into())]);
    let batch = next(&mut runtime, cursor, 64);
    assert_eq!(batch["rows"][0][0]["type"], "Text");
    assert_eq!(batch["rows"][0][0]["value"], "0.3");
    assert_eq!(
        batch["rows"][0][1]["value"],
        "12345678901234567890.12340000"
    );
    assert_eq!(batch["rows"][0][2]["type"], "Null");
}

#[test]
fn timestamp_protocol_preserves_microseconds_and_rejects_submicroseconds() {
    let Some((dsn, ca)) = fixture() else { return };
    let mut runtime = runtime(&dsn, &ca);
    let connection = open(&mut runtime);
    let cursor=query(&mut runtime,connection,"SELECT $1::timestamptz AS exact, $2::timestamptz AS before_epoch, NULL::timestamptz AS missing",vec![Parameter::Text("2024-02-29T12:34:56.123456+09:00".into()),Parameter::Text("1969-12-31T23:59:59.999999Z".into())]);
    let batch = next(&mut runtime, cursor, 64);
    assert_eq!(batch["rows"][0][0]["value"], "2024-02-29T03:34:56.123456Z");
    assert_eq!(batch["rows"][0][1]["value"], "1969-12-31T23:59:59.999999Z");
    assert_eq!(batch["rows"][0][2]["type"], "Null");
    let result = execute(
        &mut runtime,
        connection,
        "SELECT $1::timestamptz",
        vec![Parameter::Text("2024-01-01T00:00:00.000000001Z".into())],
    );
    assert_eq!(result["error"]["code"], "DbType");
}
