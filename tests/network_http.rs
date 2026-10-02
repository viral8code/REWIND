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
        download: false,
        upload_limit: None,
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

#[test]
fn download_reads_incrementally_and_revert_reuses_chunks_after_close() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut head = Vec::new();
        let mut byte = [0];
        while !head.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        socket.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\na\xff\0\r\n").unwrap();
        socket.flush().unwrap();
        thread::sleep(Duration::from_millis(40));
        let _ = socket.write_all(b"3\r\nbcd\r\n0\r\n\r\n");
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let mut req = request(url);
    req.download = true;
    runtime.enter_external(false).unwrap();
    let open = runtime.start_http(req).unwrap();
    runtime.exit_external().unwrap();
    let head = result(&mut runtime, open);
    let stream = head["download"].as_u64().unwrap() as usize;
    runtime.commit("first").unwrap();
    let mut body = Vec::new();
    let mut ids = Vec::new();
    loop {
        runtime.enter_external(false).unwrap();
        let id = runtime.read_http(stream, 2).unwrap();
        runtime.exit_external().unwrap();
        let response = result(&mut runtime, id);
        ids.push((id, response.clone()));
        if response["body"].is_null() {
            break;
        }
        body.extend(STANDARD.decode(response["body"].as_str().unwrap()).unwrap());
    }
    assert_eq!(body, b"a\xff\0bcd");
    runtime.close_native_resource(stream as u64).unwrap();
    runtime.revert("first").unwrap();
    for (id, recorded) in ids {
        runtime.enter_external(false).unwrap();
        let replay = runtime.read_http(stream, 2).unwrap();
        runtime.exit_external().unwrap();
        assert_eq!(id, replay);
        assert_eq!(result(&mut runtime, replay), recorded);
    }
    runtime.enter_external(true).unwrap();
    let id = runtime.read_http(stream, 2).unwrap();
    runtime.exit_external().unwrap();
    assert_eq!(
        result(&mut runtime, id)["error"]["code"],
        "HttpDownloadClosed"
    );
    worker.join().unwrap();
}

#[test]
fn download_never_records_secret_split_between_wire_chunks() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut head = Vec::new();
        let mut byte = [0];
        while !head.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        socket.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n7\r\nprivate\r\n").unwrap();
        socket.flush().unwrap();
        thread::sleep(Duration::from_millis(20));
        let _ = socket.write_all(b"6\r\n-token\r\n0\r\n\r\n");
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    runtime.register_secret_value(&rewind::Value::Text("private-token".into()));
    let mut req = request(url);
    req.download = true;
    runtime.enter_external(false).unwrap();
    let open = runtime.start_http(req).unwrap();
    runtime.exit_external().unwrap();
    let stream = result(&mut runtime, open)["download"].as_u64().unwrap() as usize;
    runtime.enter_external(false).unwrap();
    let read = runtime.read_http(stream, 1).unwrap();
    runtime.exit_external().unwrap();
    assert_eq!(
        result(&mut runtime, read)["error"]["code"],
        "HttpSecretResponse"
    );
    worker.join().unwrap();
}

#[test]
fn upload_queues_binary_chunks_finishes_and_reuses_records_without_server() {
    use std::io::BufRead;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut reader = std::io::BufReader::new(socket.try_clone().unwrap());
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
        }
        let mut body = Vec::new();
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            let size = usize::from_str_radix(line.trim(), 16).unwrap();
            if size == 0 {
                break;
            }
            let mut bytes = vec![0; size];
            reader.read_exact(&mut bytes).unwrap();
            body.extend(bytes);
            let mut end = [0; 2];
            reader.read_exact(&mut end).unwrap();
        }
        assert_eq!(body, b"a\xff\0bcd");
        socket
            .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let mut req = request(url);
    req.method = "POST".into();
    req.upload_limit = Some(8);
    runtime.enter_external(false).unwrap();
    let open = runtime.start_http(req).unwrap();
    runtime.exit_external().unwrap();
    let stream = result(&mut runtime, open)["upload"].as_u64().unwrap() as usize;
    runtime.commit("upload").unwrap();
    let chunks = [Arc::new(b"a\xff\0".to_vec()), Arc::new(b"bcd".to_vec())];
    let mut results = Vec::new();
    for chunk in &chunks {
        runtime.enter_external(false).unwrap();
        let id = runtime.write_http(stream, chunk.clone()).unwrap();
        runtime.exit_external().unwrap();
        let r = result(&mut runtime, id);
        assert_eq!(r["written"], 3);
        results.push(r);
    }
    runtime.enter_external(false).unwrap();
    let done = runtime.finish_http(stream, 1024).unwrap();
    runtime.exit_external().unwrap();
    let response = result(&mut runtime, done);
    assert_eq!(response["status"], 201);
    worker.join().unwrap();
    runtime.revert("upload").unwrap();
    for (chunk, expected) in chunks.iter().zip(results) {
        runtime.enter_external(false).unwrap();
        let id = runtime.write_http(stream, chunk.clone()).unwrap();
        runtime.exit_external().unwrap();
        assert_eq!(result(&mut runtime, id), expected);
    }
    runtime.enter_external(false).unwrap();
    let done = runtime.finish_http(stream, 1024).unwrap();
    runtime.exit_external().unwrap();
    assert_eq!(result(&mut runtime, done), response);
}

#[test]
fn later_registered_secret_blocks_export_even_if_split_between_recorded_reads() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut head = Vec::new();
        let mut byte = [0];
        while !head.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: close\r\n\r\nprivate-token",
            )
            .unwrap();
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let mut req = request(url);
    req.download = true;
    runtime.enter_external(false).unwrap();
    let open = runtime.start_http(req).unwrap();
    runtime.exit_external().unwrap();
    let stream = result(&mut runtime, open)["download"].as_u64().unwrap() as usize;
    for _ in 0..14 {
        runtime.enter_external(false).unwrap();
        let id = runtime.read_http(stream, 1).unwrap();
        runtime.exit_external().unwrap();
        let response = result(&mut runtime, id);
        assert!(response.get("error").is_none());
    }
    runtime.register_secret_value(&rewind::Value::Text("private-token".into()));
    assert!(runtime
        .export_observations()
        .unwrap_err()
        .to_string()
        .contains("SecretObservationUnrecordable"));
    worker.join().unwrap();
}

#[test]
fn dropped_runtime_cancels_an_open_upload_instead_of_waiting_for_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut data = Vec::new();
        let mut byte = [0];
        while !data.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            data.push(byte[0]);
        }
        let mut remainder = Vec::new();
        socket.read_to_end(&mut remainder).is_ok()
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let mut req = request(url);
    req.method = "POST".into();
    req.timeout_ms = 120000;
    req.upload_limit = Some(1024);
    runtime.enter_external(false).unwrap();
    let open = runtime.start_http(req).unwrap();
    runtime.exit_external().unwrap();
    let stream = result(&mut runtime, open)["upload"].as_u64().unwrap() as usize;
    runtime.enter_external(false).unwrap();
    let sent = runtime
        .write_http(stream, Arc::new(b"one chunk".to_vec()))
        .unwrap();
    runtime.exit_external().unwrap();
    assert_eq!(result(&mut runtime, sent)["written"], 9);
    thread::sleep(Duration::from_millis(30));
    drop(runtime);
    assert!(worker.join().unwrap());
}

#[test]
fn download_deadline_and_truncated_body_close_the_physical_slot() {
    for timeout in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut head = Vec::new();
            let mut byte = [0];
            while !head.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\n")
                .unwrap();
            socket.flush().unwrap();
            if timeout {
                thread::sleep(Duration::from_millis(250));
            } else {
                let _ = socket.write_all(b"a");
            }
        });
        let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
        let mut req = request(url);
        req.download = true;
        req.timeout_ms = if timeout { 100 } else { 1000 };
        runtime.enter_external(false).unwrap();
        let open = runtime.start_http(req).unwrap();
        runtime.exit_external().unwrap();
        let stream = result(&mut runtime, open)["download"].as_u64().unwrap() as usize;
        runtime.enter_external(false).unwrap();
        let read = runtime.read_http(stream, 64).unwrap();
        runtime.exit_external().unwrap();
        let mut failure = result(&mut runtime, read);
        if failure.get("error").is_none() {
            runtime.enter_external(false).unwrap();
            let read = runtime.read_http(stream, 64).unwrap();
            runtime.exit_external().unwrap();
            failure = result(&mut runtime, read);
        }
        assert_eq!(
            failure["error"]["code"],
            if timeout {
                "HttpTimeout"
            } else {
                "HttpTransport"
            }
        );
        assert_eq!(failure["error"]["phase"], "ResponseReceived");
        assert_eq!(runtime.native_resource_count(), 0);
        worker.join().unwrap();
    }
}

#[test]
fn binary_secrets_are_protected_for_whole_responses_and_streams() {
    for download in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut head = Vec::new();
            let mut byte = [0];
            while !head.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
            }
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\na\xff\0xb",
                )
                .unwrap();
        });
        let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
        runtime.register_secret_value(&rewind::Value::Bytes(Arc::new(b"\xff\0x".to_vec())));
        let mut req = request(url);
        req.download = download;
        runtime.enter_external(false).unwrap();
        let open = runtime.start_http(req).unwrap();
        runtime.exit_external().unwrap();
        let mut response = result(&mut runtime, open);
        if download {
            let stream = response["download"].as_u64().unwrap() as usize;
            runtime.enter_external(false).unwrap();
            let read = runtime.read_http(stream, 1).unwrap();
            runtime.exit_external().unwrap();
            response = result(&mut runtime, read);
        }
        assert_eq!(response["error"]["code"], "HttpSecretResponse");
        runtime.export_observations().unwrap();
        worker.join().unwrap();
    }
}

#[test]
fn tiny_reads_keep_only_actual_record_bytes_instead_of_maximum_reserved_capacity() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut head = Vec::new();
        let mut byte = [0];
        while !head.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 512\r\nConnection: close\r\n\r\n")
            .unwrap();
        socket.write_all(&[b'x'; 512]).unwrap();
    });
    let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
    let mut req = request(url);
    req.download = true;
    runtime.enter_external(false).unwrap();
    let open = runtime.start_http(req).unwrap();
    runtime.exit_external().unwrap();
    let stream = result(&mut runtime, open)["download"].as_u64().unwrap() as usize;
    for _ in 0..250 {
        runtime.enter_external(false).unwrap();
        let read = runtime.read_http(stream, 1).unwrap();
        runtime.exit_external().unwrap();
        assert_eq!(result(&mut runtime, read)["body"], "eA==");
    }
    assert!(runtime.export_observations().unwrap().to_string().len() < 128 * 1024);
    runtime.close_native_resource(stream as u64).unwrap();
    assert_eq!(runtime.native_resource_count(), 0);
    worker.join().unwrap();
}
