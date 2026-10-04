use base64::{engine::general_purpose::STANDARD, Engine};
use rewind::{tcp::Operation, Runtime};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};
fn result(runtime: &mut Runtime, id: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if let Some(value) = runtime.poll_external(id).unwrap() {
            return value.unwrap();
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}
fn start(runtime: &mut Runtime, op: Operation, timeout: u64) -> usize {
    runtime.enter_external(false).unwrap();
    let id = runtime.start_tcp(op, timeout).unwrap();
    runtime.exit_external().unwrap();
    id
}
fn call(runtime: &mut Runtime, op: Operation) -> Value {
    let id = start(runtime, op, 2000);
    result(runtime, id)
}
fn connect(runtime: &mut Runtime, port: u16) -> usize {
    call(
        runtime,
        Operation::Connect {
            host: "127.0.0.1".into(),
            port,
        },
    )["socket"]
        .as_u64()
        .unwrap() as usize
}
#[test]
fn binary_half_close_eof_and_restored_receipt_do_not_repeat_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut received = Vec::new();
        stream.read_to_end(&mut received).unwrap();
        assert_eq!(received, vec![0, 255, 13, 10, 42]);
        stream.write_all(&[255, 0, 80]).unwrap();
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let socket = connect(&mut runtime, port);
    assert_eq!(runtime.native_resource_count(), 1);
    runtime.commit("before_write").unwrap();
    let op = Operation::Write {
        socket,
        body: Arc::new(vec![0, 255, 13, 10, 42]),
    };
    let id = start(&mut runtime, op.clone(), 2000);
    let sent = result(&mut runtime, id);
    assert_eq!(sent["written"], 5);
    runtime.revert("before_write").unwrap();
    assert_eq!(start(&mut runtime, op, 2000), id);
    assert_eq!(result(&mut runtime, id), sent);
    assert_eq!(
        call(&mut runtime, Operation::ShutdownWrite { socket })["shutdown"],
        true
    );
    let mut received = Vec::new();
    loop {
        let read = call(&mut runtime, Operation::Read { socket, limit: 2 });
        assert!(read.get("error").is_none(), "{read}");
        if read["eof"] == true {
            break;
        }
        received.extend(STANDARD.decode(read["body"].as_str().unwrap()).unwrap());
    }
    assert_eq!(received, vec![255, 0, 80]);
    worker.join().unwrap();
    assert_eq!(
        call(&mut runtime, Operation::Close { socket })["closed"],
        true
    );
    assert!(!runtime.has_native_resources());
}
#[test]
fn pending_read_is_exclusive_cancellable_and_cached_after_close() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 0);
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let socket = connect(&mut runtime, port);
    let first = start(&mut runtime, Operation::Read { socket, limit: 16 }, 5000);
    let second = call(&mut runtime, Operation::Read { socket, limit: 16 });
    assert_eq!(second["error"]["code"], "TcpBusy");
    runtime.cancel_http(first).unwrap();
    let cancelled = result(&mut runtime, first);
    assert_eq!(cancelled["error"]["code"], "TcpCancelled");
    assert_eq!(cancelled["error"]["phase"], "Unknown");
    assert_eq!(result(&mut runtime, first), cancelled);
    assert!(!runtime.has_native_resources());
    worker.join().unwrap();
}
#[test]
fn read_deadline_closes_physical_socket_and_stays_a_typed_receipt() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 0);
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let socket = connect(&mut runtime, port);
    let id = start(&mut runtime, Operation::Read { socket, limit: 16 }, 30);
    let value = result(&mut runtime, id);
    assert_eq!(value["error"]["code"], "TcpDeadline");
    assert_eq!(value["error"]["acceptedBytes"], 0);
    assert!(!runtime.has_native_resources());
    assert_eq!(result(&mut runtime, id), value);
    worker.join().unwrap();
}
#[test]
fn split_secret_response_is_rejected_and_socket_closed() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (go_tx, go_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(b"ab").unwrap();
        go_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        stream.write_all(b"cd").unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 0);
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.register_secret_value(&rewind::Value::Text("abcd".into()));
    let socket = connect(&mut runtime, port);
    let first = call(&mut runtime, Operation::Read { socket, limit: 2 });
    assert_eq!(
        STANDARD.decode(first["body"].as_str().unwrap()).unwrap(),
        b"ab"
    );
    go_tx.send(()).unwrap();
    let second = call(&mut runtime, Operation::Read { socket, limit: 2 });
    assert_eq!(second["error"]["code"], "TcpSecretResponse");
    assert!(!runtime.has_native_resources());
    worker.join().unwrap();
}
#[test]
fn live_connection_is_released_with_last_receipt_lease() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 0);
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.configure_live_external(true);
    runtime.enter_external_live_task(0).unwrap();
    let id = runtime
        .start_tcp(
            Operation::Connect {
                host: "127.0.0.1".into(),
                port,
            },
            2000,
        )
        .unwrap();
    let lease = runtime.live_external_lease(id).unwrap();
    runtime.exit_external().unwrap();
    let value = result(&mut runtime, id);
    assert!(value["socket"].is_u64());
    assert_eq!(runtime.native_resource_count(), 1);
    drop(lease);
    runtime.reclaim_live_external().unwrap();
    assert_eq!(runtime.native_resource_count(), 0);
    worker.join().unwrap();
}
#[test]
fn invalid_limits_and_secret_request_do_not_contact_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let id = start(
        &mut runtime,
        Operation::Connect {
            host: "127.0.0.1".into(),
            port,
        },
        0,
    );
    assert_eq!(result(&mut runtime, id)["error"]["code"], "TcpDeadline");
    let invalid = call(
        &mut runtime,
        Operation::Read {
            socket: 999,
            limit: 65537,
        },
    );
    assert_eq!(invalid["error"]["code"], "TcpLimit");
    runtime.register_secret_value(&rewind::Value::Text("127.0.0.1".into()));
    runtime.enter_external(false).unwrap();
    assert!(runtime
        .start_tcp(
            Operation::Connect {
                host: "127.0.0.1".into(),
                port
            },
            2000
        )
        .unwrap_err()
        .to_string()
        .contains("TcpSecretRequest"));
    runtime.exit_external().unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
#[test]
fn a_pending_read_does_not_block_writes_on_the_same_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = [0; 2];
        stream.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"go");
        stream.write_all(b"yes").unwrap();
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let socket = connect(&mut runtime, port);
    let read = start(&mut runtime, Operation::Read { socket, limit: 3 }, 5000);
    assert_eq!(
        call(
            &mut runtime,
            Operation::Write {
                socket,
                body: Arc::new(b"go".to_vec())
            }
        )["written"],
        2
    );
    assert_eq!(
        STANDARD
            .decode(result(&mut runtime, read)["body"].as_str().unwrap())
            .unwrap(),
        b"yes"
    );
    runtime.close_native_resource(socket as u64).unwrap();
    worker.join().unwrap();
}
#[test]
fn native_memory_admission_happens_before_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime
        .set_budget(rewind::ResourceBudget {
            history_memory: 1024 * 1024,
            history_storage: 16 * 1024 * 1024,
            spill_threshold: 65536,
        })
        .unwrap();
    let response = call(
        &mut runtime,
        Operation::Connect {
            host: "127.0.0.1".into(),
            port,
        },
    );
    assert_eq!(response["error"]["code"], "TcpMemoryLimit");
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(!runtime.has_native_resources());
}
#[test]
fn socket_limit_is_bounded_and_a_closed_slot_can_be_reused() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let mut sockets = Vec::new();
    let mut peers = Vec::new();
    for _ in 0..8 {
        sockets.push(connect(&mut runtime, port));
        peers.push(listener.accept().unwrap().0);
    }
    assert_eq!(
        call(
            &mut runtime,
            Operation::Connect {
                host: "127.0.0.1".into(),
                port
            }
        )["error"]["code"],
        "TcpSocketLimit"
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    runtime.close_native_resource(sockets[0] as u64).unwrap();
    let new = connect(&mut runtime, port);
    peers.push(listener.accept().unwrap().0);
    assert_eq!(runtime.native_resource_count(), 8);
    for socket in sockets.into_iter().chain(std::iter::once(new)) {
        runtime.close_native_resource(socket as u64).unwrap();
    }
    assert!(!runtime.has_native_resources());
}
#[test]
fn a_new_secret_context_closes_the_socket_before_the_next_read() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        peer.write_all(b"ab").unwrap();
        let mut byte = [0];
        assert_eq!(peer.read(&mut byte).unwrap(), 0);
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let socket = connect(&mut runtime, port);
    assert_eq!(
        call(&mut runtime, Operation::Read { socket, limit: 2 })["body"],
        "YWI="
    );
    runtime.register_secret_value(&rewind::Value::Text("abcd".into()));
    let value = call(&mut runtime, Operation::Read { socket, limit: 2 });
    assert_eq!(value["error"]["code"], "TcpSecretContextChanged");
    assert!(!runtime.has_native_resources());
    worker.join().unwrap();
}
#[test]
fn a_new_secret_context_rejects_an_already_pending_read_across_chunk_boundaries() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        peer.write_all(b"ab").unwrap();
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        peer.write_all(b"cd").unwrap();
        let mut byte = [0];
        assert_eq!(peer.read(&mut byte).unwrap(), 0);
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let socket = connect(&mut runtime, port);
    assert_eq!(
        call(&mut runtime, Operation::Read { socket, limit: 2 })["body"],
        "YWI="
    );
    let pending = start(&mut runtime, Operation::Read { socket, limit: 2 }, 2000);
    runtime.register_secret_value(&rewind::Value::Text("abcd".into()));
    tx.send(()).unwrap();
    let value = result(&mut runtime, pending);
    assert_eq!(value["error"]["code"], "TcpSecretContextChanged");
    assert!(value.get("body").is_none());
    assert!(!runtime.has_native_resources());
    worker.join().unwrap();
}
