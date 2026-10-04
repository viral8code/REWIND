use crate::{tcp::Operation, Runtime};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
fn result(runtime: &mut Runtime, id: usize) -> Value {
    let limit = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(value) = runtime.poll_external(id).unwrap() {
            return value.unwrap();
        }
        assert!(Instant::now() < limit);
        thread::sleep(Duration::from_millis(1));
    }
}
fn call(runtime: &mut Runtime, op: Operation) -> Value {
    runtime.enter_external(false).unwrap();
    let id = runtime.start_tcp(op, 3000).unwrap();
    runtime.exit_external().unwrap();
    result(runtime, id)
}
fn server(bind: &str) -> (u16, Arc<AtomicUsize>, thread::JoinHandle<()>) {
    use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![CertificateDer::from_pem_slice(include_bytes!(
            "../../tests/fixtures/tls/localhost-cert.pem"
        ))
        .unwrap()],
        PrivateKeyDer::from_pem_slice(include_bytes!("../../tests/fixtures/tls/localhost-key.pem"))
            .unwrap(),
    )
    .unwrap();
    let listener = TcpListener::bind((bind, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = seen.clone();
    let worker = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let connection = rustls::ServerConnection::new(Arc::new(config)).unwrap();
        let mut stream = rustls::StreamOwned::new(connection, socket);
        let mut body = [0; 4];
        if stream.read_exact(&mut body).is_err() {
            return;
        }
        assert_eq!(&body, b"ping");
        counter.store(4, Ordering::SeqCst);
        stream.write_all(b"ok").unwrap();
        stream.conn.send_close_notify();
        stream.flush().unwrap();
    });
    (port, seen, worker)
}
#[test]
fn trusted_tls_is_verified_and_checkpoint_receipts_do_not_repeat_plaintext() {
    let (port, seen, worker) = server("127.0.0.1");
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let socket = call(
        &mut runtime,
        Operation::Tls {
            host: "localhost".into(),
            port,
            ca: Arc::new(include_bytes!("../../tests/fixtures/tls/localhost-ca.pem").to_vec()),
        },
    )["socket"]
        .as_u64()
        .unwrap() as usize;
    runtime.commit("before_write").unwrap();
    let write = Operation::Write {
        socket,
        body: Arc::new(b"ping".to_vec()),
    };
    assert_eq!(call(&mut runtime, write.clone())["written"], 4);
    runtime.revert("before_write").unwrap();
    assert_eq!(call(&mut runtime, write)["written"], 4);
    let mut bytes = Vec::new();
    loop {
        let read = call(&mut runtime, Operation::Read { socket, limit: 1 });
        assert!(read.get("error").is_none(), "{read}");
        if read["eof"] == true {
            break;
        }
        use base64::{engine::general_purpose::STANDARD, Engine};
        bytes.extend(STANDARD.decode(read["body"].as_str().unwrap()).unwrap());
    }
    assert_eq!(bytes, b"ok");
    worker.join().unwrap();
    assert_eq!(seen.load(Ordering::SeqCst), 4);
    runtime.close_native_resource(socket as u64).unwrap();
    assert!(!runtime.has_native_resources());
}
#[test]
fn untrusted_certificate_and_wrong_hostname_reject_before_application_bytes() {
    for wrong_name in [false, true] {
        let (port, seen, worker) = server(if wrong_name { "127.0.0.2" } else { "127.0.0.1" });
        let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
        let value = call(
            &mut runtime,
            Operation::Tls {
                host: if wrong_name { "127.0.0.2" } else { "localhost" }.into(),
                port,
                ca: Arc::new(if wrong_name {
                    include_bytes!("../../tests/fixtures/tls/localhost-ca.pem").to_vec()
                } else {
                    vec![]
                }),
            },
        );
        assert_eq!(value["error"]["code"], "TcpTls", "{value}");
        assert_eq!(value["error"]["phase"], "NotConnected");
        assert_eq!(value["error"]["acceptedBytes"], 0);
        worker.join().unwrap();
        assert_eq!(seen.load(Ordering::SeqCst), 0);
        assert!(!runtime.has_native_resources());
    }
}
#[test]
fn malformed_ca_is_a_typed_error_without_connecting() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let value = call(
        &mut runtime,
        Operation::Tls {
            host: "localhost".into(),
            port: listener.local_addr().unwrap().port(),
            ca: Arc::new(b"not a certificate".to_vec()),
        },
    );
    assert_eq!(value["error"]["code"], "TcpTlsCa");
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
#[test]
fn handshake_deadline_is_cached_and_the_tls_peer_is_not_recontacted() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(5);
        let mut peer = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < end);
                    thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("{e}"),
            }
        };
        peer.set_nonblocking(false).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut hello = Vec::new();
        peer.read_to_end(&mut hello).unwrap();
        assert!(!hello.is_empty());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.commit("before").unwrap();
    let operation = Operation::Tls {
        host: "localhost".into(),
        port,
        ca: Arc::new(include_bytes!("../../tests/fixtures/tls/localhost-ca.pem").to_vec()),
    };
    runtime.enter_external(false).unwrap();
    let id = runtime.start_tcp(operation.clone(), 100).unwrap();
    runtime.exit_external().unwrap();
    let value = result(&mut runtime, id);
    assert_eq!(value["error"]["code"], "TcpDeadline");
    assert_eq!(value["error"]["phase"], "NotConnected");
    worker.join().unwrap();
    runtime.revert("before").unwrap();
    runtime.enter_external(false).unwrap();
    assert_eq!(runtime.start_tcp(operation, 100).unwrap(), id);
    runtime.exit_external().unwrap();
    assert_eq!(result(&mut runtime, id), value);
    assert!(!runtime.has_native_resources());
}
#[test]
fn cancelling_a_tls_handshake_releases_buffers_without_a_new_tcp_operation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut first = [0; 1];
        peer.read_exact(&mut first).unwrap();
        tx.send(()).unwrap();
        let mut rest = Vec::new();
        peer.read_to_end(&mut rest).unwrap();
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.configure_live_external(true);
    runtime.enter_external_live_task(0).unwrap();
    let id = runtime
        .start_tcp(
            Operation::Tls {
                host: "localhost".into(),
                port,
                ca: Arc::new(include_bytes!("../../tests/fixtures/tls/localhost-ca.pem").to_vec()),
            },
            5000,
        )
        .unwrap();
    let lease = runtime.live_external_lease(id).unwrap();
    runtime.exit_external().unwrap();
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    runtime.cancel_http(id).unwrap();
    let value = result(&mut runtime, id);
    assert_eq!(value["error"]["code"], "TcpCancelled");
    assert_eq!(value["error"]["acceptedBytes"], 0);
    worker.join().unwrap();
    drop(lease);
    runtime.reclaim_live_external().unwrap();
    let host = runtime.tcp_host.as_ref().unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    while !host.retired.iter().all(|(handle, _)| handle.is_finished()) {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(1));
    }
    assert!(host.reserved_bytes() < super::WORKER_MEMORY + 1024);
    assert!(!runtime.has_native_resources());
}
