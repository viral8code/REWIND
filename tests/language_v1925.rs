use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    thread,
    time::{Duration, Instant},
};
fn connect(port: u16) -> TcpStream {
    let end = Instant::now() + Duration::from_secs(12);
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(8)))
                    .unwrap();
                return stream;
            }
            Err(error) => {
                assert!(Instant::now() < end, "{error}");
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
#[test]
fn source_free_http_server_has_one_reply_and_disconnected_replay_or_live_reclamation() {
    for mode in ["debug", "compact", "live"] {
        let root = std::env::temp_dir().join(format!("rewind-v1925-{mode}-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let mut source =
            include_str!("../examples/http-server/main.rw").replace("PORT", &port.to_string());
        let live = mode == "live";
        if live {
            source = source.replace("external {", "external live {");
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
        let _ = fs::remove_dir_all(root.join(".rewind"));
        let worker = thread::spawn(move || {
            let mut stream = connect(port);
            stream.write_all(b"POST /echo?q=one HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 4\r\n\r\nping").unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            assert!(response.starts_with(b"HTTP/1.1 200"));
            let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
            assert_eq!(&response[split + 4..], b"ok");
            let mut shutdown = connect(port);
            shutdown
                .write_all(
                    b"GET /shutdown HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
            let mut response = Vec::new();
            let result = shutdown.read_to_end(&mut response);
            if let Err(error) = result {
                assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset)
            }
            assert!(response.is_empty());
        });
        let result = if live {
            call(&["profile", "main.rwc", "--allow-effects", effects])
        } else {
            call(&[
                "run",
                "main.rwc",
                "--allow-effects",
                effects,
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ])
        };
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        worker.join().unwrap();
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).replace("\r\n", "\n"),
            "http server done\n"
        );
        if live {
            let stderr = String::from_utf8(result.stderr).unwrap();
            let profile: serde_json::Value = serde_json::from_str(
                stderr
                    .lines()
                    .rev()
                    .find(|line| line.starts_with('{'))
                    .unwrap(),
            )
            .unwrap();
            for field in [
                "native_resources",
                "retained_operations",
                "recorded_operations",
                "recorded_polls",
            ] {
                assert_eq!(
                    profile["runtime"]["external_live"][field], 0,
                    "{field}: {profile}"
                );
            }
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
fn pure_router_and_server_capability_checks() {
    let root = std::env::temp_dir().join(format!("rewind-v1925-router-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let call = |source: &str, args: &[&str]| {
        fs::write(root.join("main.rw"), source).unwrap();
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap()
    };
    let source = r#"import std.httpRouter as router;
 let routes=List<router.Route>();routes.add(router.Route("GET","/items","get"));routes.add(router.Route("*","/wild","any"));let frozen=freeze(move routes);
 assert_eq(router.find(frozen,"GET","/items"),Some(router.Route("GET","/items","get")));
 assert_eq(router.find(frozen,"POST","/items"),None);assert_eq(router.allows(frozen,"/items"),true);
 assert_eq(router.find(frozen,"DELETE","/wild"),Some(router.Route("*","/wild","any")));
 assert_eq(router.find(frozen,"GET","/items/1"),None);assert_eq(router.find(frozen,"GET","/%69tems"),None);assert_eq(router.allows(frozen,"/missing"),false);
 publish;"#;
    let result = call(source, &["run", "main.rw"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let denied = call(
        "import std.httpServer as server;external {let task=server.listen(\"127.0.0.1\",0,1000);}",
        &["compile", "main.rw", "--allow-effects", "external,tasks"],
    );
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("network"));
    let forged = call(
        "let server=HttpServer(\"127.0.0.1\",1234,64);",
        &["compile", "main.rw"],
    );
    assert!(!forged.status.success());
    fs::remove_dir_all(root).unwrap();
}
