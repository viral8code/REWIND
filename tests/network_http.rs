use rewind::{network::Request, Runtime};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
fn request(url: String) -> Request {
    Request {
        method: "GET".into(),
        url,
        body: Arc::new(vec![]),
        timeout_ms: 2000,
        limit: 1024,
        headers: vec![],
        ca: Arc::new(vec![]),
        credential: String::new(),
    }
}
fn result(runtime: &mut Runtime, id: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(r) = runtime.poll_external(id).unwrap() {
            return r.unwrap();
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}
fn tls_server() -> (String, thread::JoinHandle<()>) {
    use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
    let cert =
        CertificateDer::from_pem_slice(include_bytes!("fixtures/tls/localhost-cert.pem")).unwrap();
    let key =
        PrivateKeyDer::from_pem_slice(include_bytes!("fixtures/tls/localhost-key.pem")).unwrap();
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "https://localhost:{}/",
        listener.local_addr().unwrap().port()
    );
    let worker = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let conn = rustls::ServerConnection::new(Arc::new(config)).unwrap();
        let mut stream = rustls::StreamOwned::new(conn, socket);
        let mut bytes = Vec::new();
        let mut byte = [0];
        while !bytes.ends_with(b"\r\n\r\n") {
            if stream.read_exact(&mut byte).is_err() {
                return;
            }
            bytes.push(byte[0]);
            assert!(bytes.len() < 32768);
        }
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        let _ = stream.flush();
    });
    (url, worker)
}
#[test]
fn tls_verifies_certificates_and_explicit_ca_preserves_validation() {
    for trusted in [false, true] {
        let (url, worker) = tls_server();
        let mut req = request(url);
        if trusted {
            req.ca = Arc::new(include_bytes!("fixtures/tls/localhost-ca.pem").to_vec());
        }
        let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
        runtime.enter_external(false).unwrap();
        let id = runtime.start_http(req).unwrap();
        runtime.exit_external().unwrap();
        let response = result(&mut runtime, id);
        worker.join().unwrap();
        if trusted {
            assert_eq!(response["status"], 200);
            assert_eq!(response["body"], "b2s=");
        } else {
            assert_eq!(response["error"]["code"], "HttpTransport");
        }
    }
}
#[test]
fn cancellation_is_recorded_and_a_restored_operation_is_not_resubmitted() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let req = request(format!("http://{}/", listener.local_addr().unwrap()));
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.commit("start").unwrap();
    runtime.enter_external(false).unwrap();
    let id = runtime.start_http(req.clone()).unwrap();
    runtime.exit_external().unwrap();
    runtime.cancel_http(id).unwrap();
    let cancelled = result(&mut runtime, id);
    assert_eq!(cancelled["error"]["code"], "HttpCancelled");
    runtime.revert("start").unwrap();
    runtime.enter_external(false).unwrap();
    assert_eq!(runtime.start_http(req).unwrap(), id);
    runtime.exit_external().unwrap();
    assert_eq!(result(&mut runtime, id), cancelled);
    let tape = runtime.export_observations().unwrap();
    drop(listener);
    let mut replay = Runtime::new(std::env::temp_dir()).unwrap();
    replay.import_observations(&tape).unwrap();
    replay.enter_external(false).unwrap();
    let req = request("http://127.0.0.1:1".into());
    // A changed request must fail even when the host is unreachable.
    assert!(replay
        .start_http(req)
        .unwrap_err()
        .to_string()
        .contains("ExternalRequestMismatch"));
}
#[test]
fn invalid_limits_and_reserved_headers_fail_before_sending() {
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.enter_external(false).unwrap();
    let mut req = request("http://127.0.0.1:1".into());
    req.limit = usize::MAX;
    let id = runtime.start_http(req).unwrap();
    runtime.exit_external().unwrap();
    assert_eq!(result(&mut runtime, id)["error"]["phase"], "NotSent");
    runtime.enter_external(true).unwrap();
    let mut req = request("http://127.0.0.1:1".into());
    req.headers.push(("Content-Length".into(), b"100".to_vec()));
    let id = runtime.start_http(req).unwrap();
    runtime.exit_external().unwrap();
    assert_eq!(
        result(&mut runtime, id)["error"]["code"],
        "HttpHeaderReserved"
    );
}
#[test]
fn credentials_are_opaque_and_echoed_secrets_are_not_exported() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut req = request(format!("http://{}/", listener.local_addr().unwrap()));
    req.credential = "service".into();
    req.method = "POST".into();
    req.body = Arc::new(b"a\xff\0".to_vec());
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut data = Vec::new();
        let mut byte = [0];
        while !data.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            data.push(byte[0]);
        }
        assert!(String::from_utf8_lossy(&data)
            .to_lowercase()
            .contains("authorization: bearer test-private-token"));
        let mut body = [0; 3];
        stream.read_exact(&mut body).unwrap();
        assert_eq!(&body, b"a\xff\0");
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 18\r\nConnection: close\r\n\r\ntest-private-token").unwrap();
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.enter_external(false).unwrap();
    assert_eq!(
        runtime
            .register_http_credential("service", "Bearer test-private-token")
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        runtime
            .register_http_credential("service", "Bearer changed")
            .unwrap(),
        Err("HttpCredentialImmutable")
    );
    let id = runtime.start_http(req.clone()).unwrap();
    runtime.exit_external().unwrap();
    let response = result(&mut runtime, id);
    worker.join().unwrap();
    assert_eq!(response["error"]["code"], "HttpSecretResponse");
    let tape = runtime.export_observations().unwrap();
    let serialized = tape.to_string();
    assert!(!serialized.contains("test-private-token"));
    assert!(!serialized.contains("dGVzdC1wcml2YXRlLXRva2Vu"));
    let mut replay = Runtime::new(std::env::temp_dir()).unwrap();
    replay.import_observations(&tape).unwrap();
    replay.enter_external(false).unwrap();
    let id = replay.start_http(req).unwrap();
    replay.exit_external().unwrap();
    assert_eq!(result(&mut replay, id), response);
}
