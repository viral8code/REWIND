use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
    time::{Duration, Instant},
};
#[test]
fn compiled_tcp_records_once_replays_disconnected_and_live_reuses_pending_receipt() {
    for live in [false, true] {
        let root =
            std::env::temp_dir().join(format!("rewind-v1923-tcp-{}-{live}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(8);
            let mut socket = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut received = Vec::new();
            socket.read_to_end(&mut received).unwrap();
            assert_eq!(received, b"ping");
            socket.write_all(b"ok").unwrap();
        });
        let mut source = include_str!("../examples/tcp/main.rw").replace("PORT", &port.to_string());
        if live {
            source = source.replace("external {", "external live {").replace(
                "effects {external,network,tasks}",
                "effects {external,network,tasks,live}",
            );
        }
        fs::write(root.join("main.rw"), source).unwrap();
        let effects = if live {
            "external,network,tasks,live"
        } else {
            "external,network,tasks"
        };
        let call = |args: &[&str]| {
            Command::new(env!("CARGO_BIN_EXE_rewind"))
                .current_dir(&root)
                .args(args)
                .output()
                .unwrap()
        };
        let compiled = call(&["compile", "main.rw", "--allow-effects", effects]);
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        fs::remove_file(root.join("main.rw")).unwrap();
        let args = if live {
            vec!["profile", "main.rwc", "--allow-effects", effects]
        } else {
            vec![
                "run",
                "main.rwc",
                "--allow-effects",
                effects,
                "--record",
                "trace.json",
            ]
        };
        let result = call(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"tcp done\n");
        worker.join().unwrap();
        if live {
            let profile: serde_json::Value = String::from_utf8_lossy(&result.stderr)
                .lines()
                .filter_map(|v| serde_json::from_str(v).ok())
                .last()
                .unwrap();
            assert_eq!(
                profile["runtime"]["external_live"]["retained_operations"],
                0
            );
            assert_eq!(profile["runtime"]["external_live"]["native_resources"], 0);
            assert_eq!(
                profile["runtime"]["external_live"]["recorded_operations"],
                0
            );
        } else {
            let replay = call(&["replay", "trace.json", "--allow-effects", effects]);
            assert!(
                replay.status.success(),
                "{}",
                String::from_utf8_lossy(&replay.stderr)
            );
            assert_eq!(replay.stdout, result.stdout);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn tcp_requires_network_permission_external_boundary_and_affine_handle() {
    let root = std::env::temp_dir().join(format!("rewind-v1923-effects-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let call = |effects: &str| {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&root)
            .args(["check", "main.rw", "--allow-effects", effects])
            .output()
            .unwrap()
    };
    fs::write(
        root.join("main.rw"),
        "import std.tcp as tcp;external {let pending=tcp.connect(\"127.0.0.1\",9,100);}",
    )
    .unwrap();
    let denied = call("external,tasks");
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("network"));
    fs::write(
        root.join("main.rw"),
        "import std.tcp as tcp;let pending=tcp.connect(\"127.0.0.1\",9,100);",
    )
    .unwrap();
    let denied = call("external,tasks,network");
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("ExternalBoundary"));
    fs::write(root.join("main.rw"), "let fake=TcpSocket{peer:\"local\"};").unwrap();
    assert!(!call("external,tasks,network").status.success());
    fs::remove_dir_all(root).unwrap();
}
