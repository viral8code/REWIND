use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
#[test]
fn compiled_tls_keeps_certificate_validation_and_replays_without_the_peer() {
    use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
    let root = std::env::temp_dir().join(format!("rewind-v1924-tls-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![
            CertificateDer::from_pem_slice(include_bytes!("fixtures/tls/localhost-cert.pem"))
                .unwrap(),
        ],
        PrivateKeyDer::from_pem_slice(include_bytes!("fixtures/tls/localhost-key.pem")).unwrap(),
    )
    .unwrap();
    let worker = thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(10);
        let socket = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < end);
                    thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("{e}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let connection = rustls::ServerConnection::new(Arc::new(config)).unwrap();
        let mut stream = rustls::StreamOwned::new(connection, socket);
        let mut received = [0; 4];
        stream.read_exact(&mut received).unwrap();
        assert_eq!(&received, b"ping");
        stream.write_all(b"ok").unwrap();
        stream.conn.send_close_notify();
        stream.flush().unwrap();
    });
    let certificate = serde_json::to_string(include_str!("fixtures/tls/localhost-ca.pem")).unwrap();
    let source = include_str!("../examples/tcp/main.rw")
        .replace(
            "tcp.connect(\"127.0.0.1\",PORT,5000)",
            &format!(
                "tcp.configuredTls(\"localhost\",{port},take(bytes.encode({certificate})),5000)"
            ),
        )
        .replace("take(take(await shutdown(&mut socket)));", "");
    fs::write(root.join("main.rw"), source).unwrap();
    let call = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap()
    };
    let effects = "external,network,tasks";
    let compiled = call(&["compile", "main.rw", "--allow-effects", effects]);
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    fs::remove_file(root.join("main.rw")).unwrap();
    let result = call(&[
        "run",
        "main.rwc",
        "--allow-effects",
        effects,
        "--record",
        "trace.json",
        "--record-mode",
        "compact",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"tcp done\n");
    worker.join().unwrap();
    let replay = call(&["replay", "trace.json", "--allow-effects", effects]);
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert_eq!(replay.stdout, result.stdout);
    fs::remove_dir_all(root).unwrap();
}
