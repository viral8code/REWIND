use rewind::{database::Operation, external::LiveLease, network::Request, Runtime};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
fn result(runtime: &mut Runtime, id: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if let Some(result) = runtime.poll_external(id).unwrap() {
            return result.unwrap();
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}
fn req(url: String) -> Request {
    Request {
        method: "GET".into(),
        url,
        body: Arc::new(vec![]),
        timeout_ms: 2000,
        limit: 1024,
        headers: vec![],
        ca: Arc::new(vec![]),
        credential: String::new(),
        download: false,
        upload_limit: None,
    }
}
fn call(runtime: &mut Runtime, operation: Operation) -> (usize, Arc<LiveLease>, Value) {
    runtime.enter_external_live_task(0).unwrap();
    let id = runtime.start_database(operation, 5000).unwrap();
    let lease = runtime.live_external_lease(id).unwrap();
    runtime.exit_external().unwrap();
    let response = result(runtime, id);
    (id, lease, response)
}
#[test]
fn a_retained_live_future_reuses_its_receipt_but_a_new_factory_sends_again() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let request = req(format!("http://{}/", listener.local_addr().unwrap()));
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        for number in 1..=2 {
            let deadline = Instant::now() + Duration::from_secs(6);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut head = Vec::new();
            let mut byte = [0];
            while !head.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
                assert!(head.len() < 32768);
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n{number}"
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
        thread::sleep(Duration::from_millis(20));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.configure_live_external(true);
    runtime.commit("start").unwrap();
    let mut ids = Vec::new();
    for body in ["MQ==", "Mg=="] {
        runtime.enter_external_live_task(0).unwrap();
        let id = runtime.start_http(request.clone()).unwrap();
        let lease = runtime.live_external_lease(id).unwrap();
        runtime.exit_external().unwrap();
        let receipt = result(&mut runtime, id);
        assert_eq!(receipt["body"], body);
        runtime.revert("start").unwrap();
        assert_eq!(result(&mut runtime, id), receipt);
        ids.push(id);
        drop(lease);
        runtime.reclaim_live_external().unwrap();
        assert!(runtime.poll_external(id).is_err());
    }
    assert!(ids[1] > ids[0]);
    worker.join().unwrap();
}
#[test]
fn unclaimed_database_handles_close_but_claimed_handles_survive_receipt_collection() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.configure_live_external(true);
    for claimed in [false, true] {
        let (id, lease, response) = call(
            &mut runtime,
            Operation::Sqlite {
                path: ":memory:".into(),
                read_only: false,
            },
        );
        let connection = response["connection"].as_u64().unwrap() as usize;
        assert_eq!(runtime.native_resource_count(), 1);
        if claimed {
            runtime.claim_live_external(id);
        }
        drop(lease);
        runtime.reclaim_live_external().unwrap();
        assert_eq!(runtime.native_resource_count(), usize::from(claimed));
        let (_, lease, response) = call(
            &mut runtime,
            Operation::Execute {
                connection,
                sql: "CREATE TABLE value_table(value INTEGER)".into(),
                parameters: vec![],
            },
        );
        if claimed {
            assert_eq!(response["changed"], 0);
            drop(lease);
            runtime.reclaim_live_external().unwrap();
            runtime.commit("empty").unwrap();
            for _ in 0..2 {
                let (id, lease, response) = call(
                    &mut runtime,
                    Operation::Execute {
                        connection,
                        sql: "INSERT INTO value_table VALUES (1)".into(),
                        parameters: vec![],
                    },
                );
                assert_eq!(response["changed"], 1);
                runtime.revert("empty").unwrap();
                assert_eq!(result(&mut runtime, id), response);
                drop(lease);
                runtime.reclaim_live_external().unwrap();
            }
            let (id, lease, response) = call(
                &mut runtime,
                Operation::Query {
                    connection,
                    sql: "SELECT count(*) FROM value_table".into(),
                    parameters: vec![],
                },
            );
            let cursor = response["cursor"].as_u64().unwrap() as usize;
            runtime.claim_live_external(id);
            drop(lease);
            runtime.reclaim_live_external().unwrap();
            let (_, lease, response) = call(
                &mut runtime,
                Operation::Next {
                    cursor,
                    rows: 1,
                    bytes: 1048576,
                },
            );
            assert_eq!(response["rows"][0][0]["value"], 2, "{response}");
            drop(lease);
            runtime.reclaim_live_external().unwrap();
            runtime.close_native_resource(connection as u64).unwrap();
        } else {
            assert_eq!(response["error"]["code"], "DbClosed");
            drop(lease);
            runtime.reclaim_live_external().unwrap();
        }
    }
}
#[test]
fn dropping_a_pending_future_cancels_without_leaving_a_pollable_receipt() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.configure_live_external(true);
    runtime.enter_external_live_task(0).unwrap();
    let id = runtime
        .start_http(req(format!("http://{}/", listener.local_addr().unwrap())))
        .unwrap();
    let lease = runtime.live_external_lease(id).unwrap();
    runtime.exit_external().unwrap();
    drop(lease);
    runtime.reclaim_live_external().unwrap();
    assert!(runtime.poll_external(id).is_err());
}

#[test]
fn live_postgres_tls_receipts_and_handles_survive_vm_revert_without_resending() {
    let dsn = std::env::var("REWIND_TEST_PG_DSN").ok();
    if std::env::var_os("REWIND_REQUIRE_PG").is_some() {
        assert!(dsn.is_some(), "PostgreSQL fixture required");
    }
    let Some(dsn) = dsn else { return };
    let ca = std::fs::read(std::env::var("REWIND_TEST_PG_CA").unwrap()).unwrap();
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.configure_live_external(true);
    runtime.enter_external_live_task(0).unwrap();
    assert_eq!(
        runtime
            .register_postgres_credentials("fixture", &dsn, &ca)
            .unwrap(),
        Ok(())
    );
    runtime.exit_external().unwrap();
    let (id, lease, response) = call(
        &mut runtime,
        Operation::Postgres {
            alias: "fixture".into(),
        },
    );
    assert_eq!(response["backend"], "postgres");
    let connection = response["connection"].as_u64().unwrap() as usize;
    runtime.claim_live_external(id);
    drop(lease);
    runtime.reclaim_live_external().unwrap();
    let (_, lease, response) = call(
        &mut runtime,
        Operation::Execute {
            connection,
            sql: "CREATE TEMP TABLE live_values(value INTEGER)".into(),
            parameters: vec![],
        },
    );
    assert_eq!(response["changed"], 0);
    drop(lease);
    runtime.reclaim_live_external().unwrap();
    runtime.commit("before").unwrap();
    for _ in 0..2 {
        let (id, lease, response) = call(
            &mut runtime,
            Operation::Execute {
                connection,
                sql: "INSERT INTO live_values VALUES (1)".into(),
                parameters: vec![],
            },
        );
        assert_eq!(response["changed"], 1);
        runtime.revert("before").unwrap();
        assert_eq!(result(&mut runtime, id), response);
        drop(lease);
        runtime.reclaim_live_external().unwrap();
    }
    let (id, lease, response) = call(
        &mut runtime,
        Operation::Query {
            connection,
            sql: "SELECT count(*) FROM live_values".into(),
            parameters: vec![],
        },
    );
    let cursor = response["cursor"].as_u64().unwrap() as usize;
    runtime.claim_live_external(id);
    drop(lease);
    runtime.reclaim_live_external().unwrap();
    let (_, lease, response) = call(
        &mut runtime,
        Operation::Next {
            cursor,
            rows: 1,
            bytes: 1048576,
        },
    );
    assert_eq!(response["rows"][0][0]["value"], 2);
    drop(lease);
    runtime.reclaim_live_external().unwrap();
    runtime.close_native_resource(connection as u64).unwrap();
    assert_eq!(runtime.native_resource_count(), 0);
}
