use super::*;
use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::Instant,
};
fn result(runtime: &mut Runtime, id: usize) -> Value {
    let end = Instant::now() + Duration::from_secs(6);
    loop {
        if let Some(value) = runtime.poll_external(id).unwrap() {
            return value.unwrap();
        };
        assert!(Instant::now() < end, "operation {id} did not finish");
        thread::sleep(Duration::from_millis(1));
    }
}
fn start(runtime: &mut Runtime, op: Operation, timeout: u64) -> usize {
    runtime.enter_external(false).unwrap();
    let id = runtime.start_http_server(op, timeout).unwrap();
    runtime.exit_external().unwrap();
    id
}
fn call(runtime: &mut Runtime, op: Operation) -> Value {
    let id = start(runtime, op, 3000);
    result(runtime, id)
}
fn listen(runtime: &mut Runtime) -> (usize, u16) {
    let value = call(
        runtime,
        Operation::Listen {
            address: "127.0.0.1".into(),
            port: 0,
            limits: Limits {
                body_bytes: 64,
                connections: 2,
                lifetime_ms: 4000,
            },
        },
    );
    assert!(value.get("error").is_none(), "{value}");
    (
        value["server"].as_u64().unwrap() as usize,
        value["port"].as_u64().unwrap() as u16,
    )
}
fn peer(port: u16, request: Vec<u8>) -> thread::JoinHandle<Vec<u8>> {
    peer_inner(port, request, false)
}
fn peer_inner(port: u16, request: Vec<u8>, allow_reset: bool) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut peer = TcpStream::connect(("127.0.0.1", port)).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(6))).unwrap();
        peer.write_all(&request).unwrap();
        let mut response = Vec::new();
        let result = peer.read_to_end(&mut response);
        if let Err(error) = result {
            assert!(
                allow_reset && error.kind() == std::io::ErrorKind::ConnectionReset,
                "{error}"
            );
        }
        response
    })
}
fn next(server: usize) -> Operation {
    Operation::Next {
        server,
        max_bytes: 64,
    }
}
fn reply(request: usize, body: &[u8]) -> Operation {
    Operation::Respond {
        request,
        status: 200,
        headers: vec![("content-type".into(), b"application/octet-stream".to_vec())],
        body: Arc::new(body.to_vec()),
    }
}
#[test]
fn binary_request_reply_and_checkpoint_receipt_do_not_resend() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let (server, port) = listen(&mut runtime);
    let mut request=b"POST /item?q=one HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\nConnection: close\r\n\r\n".to_vec();
    request.extend([0, 255, 42]);
    let worker = peer(port, request);
    let incoming = call(&mut runtime, next(server));
    assert_eq!(incoming["method"], "POST");
    assert_eq!(incoming["path"], "/item");
    assert_eq!(incoming["query"], "q=one");
    assert_eq!(
        STANDARD.decode(incoming["body"].as_str().unwrap()).unwrap(),
        [0, 255, 42]
    );
    let request = incoming["request"].as_u64().unwrap() as usize;
    runtime.commit("before_response").unwrap();
    let operation = reply(request, &[255, 0, 80]);
    let id = start(&mut runtime, operation.clone(), 3000);
    let response = result(&mut runtime, id);
    assert_eq!(response["replied"], true);
    let received = worker.join().unwrap();
    assert!(received.starts_with(b"HTTP/1.1 200"));
    assert!(received.ends_with(&[255, 0, 80]));
    runtime.revert("before_response").unwrap();
    assert_eq!(start(&mut runtime, operation, 3000), id);
    assert_eq!(result(&mut runtime, id), response);
    assert_eq!(runtime.native_resource_count(), 1);
    assert_eq!(
        call(&mut runtime, Operation::Close { resource: server })["closed"],
        true
    );
    assert!(!runtime.has_native_resources());
}
#[test]
fn next_wait_is_exclusive_and_cancellation_does_not_consume_a_later_request() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let (server, port) = listen(&mut runtime);
    let waiting = start(&mut runtime, next(server), 3000);
    assert_eq!(
        call(&mut runtime, next(server))["error"]["code"],
        "HttpServerBusy"
    );
    runtime.cancel_http(waiting).unwrap();
    assert_eq!(
        result(&mut runtime, waiting)["error"]["code"],
        "HttpServerCancelled"
    );
    let worker = peer(
        port,
        b"GET /later HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".to_vec(),
    );
    let incoming = call(&mut runtime, next(server));
    assert_eq!(incoming["path"], "/later");
    let request = incoming["request"].as_u64().unwrap() as usize;
    assert_eq!(call(&mut runtime, reply(request, b"ok"))["replied"], true);
    assert!(worker.join().unwrap().ends_with(b"ok"));
    runtime.close_native_resource(server as u64).unwrap();
}
#[test]
fn body_limit_is_enforced_without_delivering_an_application_request() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let (server, port) = listen(&mut runtime);
    let mut request =
        b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 65\r\nConnection: close\r\n\r\n"
            .to_vec();
    request.extend([42; 65]);
    let response = peer(port, request).join().unwrap();
    assert!(
        response.starts_with(b"HTTP/1.1 413"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    let id = start(&mut runtime, next(server), 30);
    assert_eq!(
        result(&mut runtime, id)["error"]["code"],
        "HttpServerDeadline"
    );
    runtime.close_native_resource(server as u64).unwrap();
}
#[test]
fn closing_server_interrupts_pending_next_and_physical_connections() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let (server, port) = listen(&mut runtime);
    let worker = peer_inner(
        port,
        b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".to_vec(),
        true,
    );
    let incoming = call(&mut runtime, next(server));
    assert!(incoming["request"].is_u64());
    let waiting = start(&mut runtime, next(server), 3000);
    runtime.close_native_resource(server as u64).unwrap();
    assert_eq!(
        result(&mut runtime, waiting)["error"]["code"],
        "HttpServerClosed"
    );
    assert!(worker.join().unwrap().is_empty());
    assert!(!runtime.has_native_resources());
    let host = runtime.http_server_host.as_ref().unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while host.reserved_bytes() > WORKER + META * 4 {
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn private_request_target_is_never_exposed_and_the_response_slot_is_released() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let (server, port) = listen(&mut runtime);
    runtime.register_secret_value(&crate::Value::Text("hidden-token".into()));
    let worker = peer(
        port,
        b"GET /hidden-token HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".to_vec(),
    );
    let incoming = call(&mut runtime, next(server));
    assert_eq!(incoming["error"]["code"], "HttpServerSecretRequest");
    assert!(!incoming.to_string().contains("hidden-token"));
    assert_eq!(runtime.native_resource_count(), 1);
    assert!(worker.join().unwrap().starts_with(b"HTTP/1.1 503"));
    runtime.close_native_resource(server as u64).unwrap();
}
#[test]
fn framing_injection_and_private_response_are_rejected_before_consuming_the_slot() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let (server, port) = listen(&mut runtime);
    let worker = peer(
        port,
        b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".to_vec(),
    );
    let incoming = call(&mut runtime, next(server));
    let request = incoming["request"].as_u64().unwrap() as usize;
    assert_eq!(
        call(
            &mut runtime,
            Operation::Respond {
                request,
                status: 200,
                headers: vec![("content-length".into(), b"2".to_vec())],
                body: Arc::new(b"ok".to_vec())
            }
        )["error"]["code"],
        "HttpServerFraming"
    );
    runtime.register_secret_value(&crate::Value::Text("private-response".into()));
    runtime.enter_external(false).unwrap();
    let error = runtime
        .start_http_server(reply(request, b"private-response"), 3000)
        .unwrap_err();
    runtime.exit_external().unwrap();
    assert!(error.to_string().contains("HttpServerSecretRequest"));
    assert_eq!(call(&mut runtime, reply(request, b"ok"))["replied"], true);
    assert!(worker.join().unwrap().ends_with(b"ok"));
    runtime.close_native_resource(server as u64).unwrap();
}
#[test]
fn unclaimed_live_listener_is_closed_when_its_last_future_lease_is_dropped() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.configure_live_external(true);
    runtime.enter_external_live_task(0).unwrap();
    let id = runtime
        .start_http_server(
            Operation::Listen {
                address: "127.0.0.1".into(),
                port: 0,
                limits: Limits {
                    body_bytes: 64,
                    connections: 1,
                    lifetime_ms: 3000,
                },
            },
            3000,
        )
        .unwrap();
    runtime.exit_external().unwrap();
    let lease = runtime.live_external_lease(id).unwrap();
    let incoming = result(&mut runtime, id);
    assert!(incoming["server"].is_u64());
    assert_eq!(runtime.native_resource_count(), 1);
    drop(lease);
    runtime.reclaim_live_external().unwrap();
    assert!(!runtime.has_native_resources());
    assert_eq!(runtime.external_live_entries.len(), 0);
}
#[test]
fn low_memory_budget_denies_listener_before_binding() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime
        .set_budget(crate::ResourceBudget {
            history_memory: 1024 * 1024,
            history_storage: 16 * 1024 * 1024,
            spill_threshold: 65536,
        })
        .unwrap();
    let value = call(
        &mut runtime,
        Operation::Listen {
            address: "127.0.0.1".into(),
            port: 0,
            limits: Limits {
                body_bytes: 64,
                connections: 1,
                lifetime_ms: 1000,
            },
        },
    );
    assert_eq!(value["error"]["code"], "HttpServerMemoryLimit");
    assert!(!runtime.has_native_resources());
}
#[test]
fn whole_connection_lifetime_closes_a_peer_stalled_in_headers() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let value = call(
        &mut runtime,
        Operation::Listen {
            address: "127.0.0.1".into(),
            port: 0,
            limits: Limits {
                body_bytes: 64,
                connections: 1,
                lifetime_ms: 100,
            },
        },
    );
    assert!(value["server"].is_u64(), "{value}");
    let mut stream =
        TcpStream::connect(("127.0.0.1", value["port"].as_u64().unwrap() as u16)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    stream.write_all(b"GET / HTTP/1.1\r\nHost:").unwrap();
    let mut response = Vec::new();
    let result = stream.read_to_end(&mut response);
    if let Err(error) = result {
        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
    }
    assert!(response.is_empty() || response.starts_with(b"HTTP/1.1 408"));
    runtime
        .close_native_resource(value["server"].as_u64().unwrap())
        .unwrap();
}
#[test]
fn invalid_capacity_is_a_typed_failure_without_worker_or_listener_allocation() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    for (body_bytes, connections) in [(usize::MAX, 1), (1, usize::MAX), (0, 1), (64, 0)] {
        let value = call(
            &mut runtime,
            Operation::Listen {
                address: "127.0.0.1".into(),
                port: 0,
                limits: Limits {
                    body_bytes,
                    connections,
                    lifetime_ms: 3000,
                },
            },
        );
        assert_eq!(value["error"]["code"], "HttpServerLimit");
        assert!(runtime.http_server_host.is_none());
    }
}
