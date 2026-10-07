//! Bound client/pool ownership by origin and trust configuration, preserving reuse.
use super::*;
use std::{collections::BTreeSet, sync::Weak, time::Instant};

pub(super) const MAX_CLIENTS: usize = 16;
const CLIENT_ALLOWANCE: usize = 1024 * 1024;
type Key = (String, String);
mod trust;
pub(super) struct Lease {
    pub client: reqwest::Client,
    bytes: usize,
    trust_slot: usize,
}
struct Cached {
    lease: Arc<Lease>,
    used: Instant,
}
#[derive(Default)]
pub(super) struct ClientPool {
    clients: BTreeMap<Key, Cached>,
    authorities: BTreeSet<String>,
    retained: Vec<Weak<Lease>>,
    trust: trust::TrustCache,
}
impl ClientPool {
    fn key(request: &Request) -> Result<Key, &'static str> {
        use sha2::{Digest, Sha256};
        let url = reqwest::Url::parse(&request.url).map_err(|_| "HttpUrl")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("HttpUrl");
        }
        let authority = if request.ca.is_empty() {
            String::new()
        } else {
            format!("{:x}", Sha256::digest(request.ca.as_slice()))
        };
        Ok((url.origin().ascii_serialization(), authority))
    }
    fn prune(&mut self) {
        let now = Instant::now();
        self.clients
            .retain(|_, entry| now.duration_since(entry.used) < Duration::from_secs(30));
        self.retained.retain(|entry| entry.strong_count() > 0);
    }
    fn allowance(request: &Request, key: &Key) -> usize {
        CLIENT_ALLOWANCE + 2 * request.ca.len() + key.0.len() + key.1.len() + 1024
    }
    pub fn admission(&mut self, request: &Request) -> Result<usize, &'static str> {
        self.prune();
        let key = Self::key(request)?;
        Ok(if self.clients.contains_key(&key) {
            0
        } else {
            Self::allowance(request, &key)
        })
    }
    pub fn reserved_bytes(&self) -> usize {
        let mut covered = [false; 9];
        let mut bytes = 0;
        for entry in self.retained.iter().filter_map(Weak::upgrade) {
            covered[entry.trust_slot] = true;
            bytes += entry.bytes;
        }
        bytes
            + self.trust.uncovered_bytes(&covered)
            + self
                .authorities
                .iter()
                .map(|s| s.len() + 128)
                .sum::<usize>()
            + self.retained.capacity() * std::mem::size_of::<Weak<Lease>>()
    }
    pub fn cached(&self) -> usize {
        self.clients.len()
    }
    pub fn retained(&self) -> usize {
        self.retained
            .iter()
            .filter(|entry| entry.strong_count() > 0)
            .count()
    }
    pub fn client(&mut self, request: &Request) -> Result<Arc<Lease>, &'static str> {
        self.prune();
        let key = Self::key(request)?;
        if let Some(entry) = self.clients.get_mut(&key) {
            entry.used = Instant::now();
            return Ok(entry.lease.clone());
        }
        if !key.1.is_empty() && !self.authorities.contains(&key.1) && self.authorities.len() >= 8 {
            return Err("HttpCaLimit");
        }
        let (config, trust_slot) = self.trust.configuration(&key.1, &request.ca)?;
        let builder = reqwest::Client::builder()
            .use_preconfigured_tls(config.as_ref().clone())
            .dns_resolver(Arc::new(crate::resolver::HttpResolver))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(30));
        let client = builder.build().map_err(|_| "HttpTls")?;
        if self.clients.len() == MAX_CLIENTS {
            let oldest = self
                .clients
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .unwrap()
                .0
                .clone();
            self.clients.remove(&oldest);
        }
        if !key.1.is_empty() {
            self.authorities.insert(key.1.clone());
        }
        let lease = Arc::new(Lease {
            client,
            bytes: Self::allowance(request, &key),
            trust_slot,
        });
        self.retained.push(Arc::downgrade(&lease));
        self.clients.insert(
            key,
            Cached {
                lease: lease.clone(),
                used: Instant::now(),
            },
        );
        Ok(lease)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(url: &str) -> Request {
        Request {
            method: "GET".into(),
            url: url.into(),
            body: Arc::new(vec![]),
            timeout_ms: 5000,
            limit: 1024,
            headers: vec![],
            ca: Arc::new(vec![]),
            credential: String::new(),
            download: false,
            upload_limit: None,
        }
    }
    #[test]
    fn shared_trust_survives_origin_eviction_without_unaccounted_cache_storage() {
        let mut pool = ClientPool::default();
        let first = request("http://127.0.0.1:10000/");
        pool.client(&first).unwrap();
        let (config, slot) = pool.trust.configuration("", &[]).unwrap();
        for port in 10001..10033 {
            pool.client(&request(&format!("https://127.0.0.1:{port}/")))
                .unwrap();
        }
        let (reused, reused_slot) = pool.trust.configuration("", &[]).unwrap();
        assert!(Arc::ptr_eq(&config, &reused));
        assert_eq!(slot, reused_slot);
        assert_eq!(pool.trust.len(), 1);
        assert_eq!(pool.cached(), MAX_CLIENTS);
        for entry in pool.clients.values_mut() {
            entry.used = Instant::now() - Duration::from_secs(31);
        }
        pool.admission(&first).unwrap();
        assert_eq!(pool.retained(), 0);
        assert_eq!(pool.cached(), 0);
        let unused = pool.reserved_bytes();
        assert!(
            unused >= CLIENT_ALLOWANCE,
            "trust remained allocated but unaccounted"
        );
        pool.client(&first).unwrap();
        assert_eq!(pool.trust.len(), 1);
        assert!(
            pool.reserved_bytes() < unused + CLIENT_ALLOWANCE,
            "shared configuration was counted twice"
        );
        let mut invalid = request("https://example.test/");
        invalid.ca =
            Arc::new(b"-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----\n".to_vec());
        let mut failed = ClientPool::default();
        assert!(matches!(failed.client(&invalid), Err("HttpTls")));
        assert_eq!(failed.trust.len(), 0);
        assert_eq!(
            failed.reserved_bytes(),
            0,
            "failed config retained trust storage"
        );
    }
    #[test]
    fn canonical_origin_reuses_client_without_merging_protocols_or_ports() {
        let mut pool = ClientPool::default();
        let first = pool
            .client(&request("https://EXAMPLE.test:443/a?x=1"))
            .unwrap();
        let reused = pool.client(&request("https://example.test/b?x=2")).unwrap();
        assert!(Arc::ptr_eq(&first, &reused));
        let http = pool.client(&request("http://example.test/a")).unwrap();
        let port = pool
            .client(&request("https://example.test:8443/a"))
            .unwrap();
        assert!(!Arc::ptr_eq(&first, &http));
        assert!(!Arc::ptr_eq(&first, &port));
        let mut invalid = request("https://example.test/a");
        invalid.ca = Arc::new(b"invalid certificate".to_vec());
        let invalid_result = pool.client(&invalid);
        assert!(
            matches!(invalid_result, Err("HttpCa" | "HttpTls")),
            "{}",
            invalid_result.err().unwrap_or("unexpected client")
        );
        assert_eq!(pool.cached(), 3);
    }
    #[test]
    fn evicted_active_lease_remains_accounted_and_last_owner_releases_it() {
        let mut pool = ClientPool::default();
        let first = request("http://127.0.0.1:10000/");
        let retained = pool.client(&first).unwrap();
        let weak = Arc::downgrade(&retained);
        for port in 10001..10257 {
            pool.client(&request(&format!("http://127.0.0.1:{port}/")))
                .unwrap();
        }
        assert_eq!(pool.cached(), MAX_CLIENTS);
        assert_eq!(pool.retained(), MAX_CLIENTS + 1);
        let before = pool.reserved_bytes();
        let bytes = retained.bytes;
        drop(retained);
        assert!(weak.upgrade().is_none());
        assert_eq!(before - pool.reserved_bytes(), bytes);
        assert_eq!(pool.retained(), MAX_CLIENTS);
        assert!(pool.retained.len() <= MAX_CLIENTS + 2);
        for entry in pool.clients.values_mut() {
            entry.used = Instant::now() - Duration::from_secs(31);
        }
        pool.admission(&first).unwrap();
        assert_eq!(pool.cached(), 0);
        assert_eq!(pool.retained(), 0);
    }
    #[test]
    fn custom_authority_limit_survives_origin_eviction_and_rejects_empty_bundles() {
        let mut pool = ClientPool::default();
        let pem = include_bytes!("../../tests/fixtures/tls/localhost-cert.pem");
        for suffix in 0..8 {
            let mut r = request("https://example.test/");
            let mut ca = pem.to_vec();
            ca.extend(std::iter::repeat_n(b'\n', suffix));
            r.ca = Arc::new(ca);
            pool.client(&r).unwrap();
        }
        for port in 10000..10032 {
            pool.client(&request(&format!("http://127.0.0.1:{port}/")))
                .unwrap();
        }
        let mut r = request("https://example.test:8443/");
        r.ca = Arc::new(pem.to_vec());
        pool.client(&r).unwrap();
        let mut ca = pem.to_vec();
        ca.extend_from_slice(b"\n\n\n\n\n\n\n\n");
        r.ca = Arc::new(ca);
        assert!(matches!(pool.client(&r), Err("HttpCaLimit")));
        let mut empty = request("https://example.test/");
        empty.ca = Arc::new(b"not a PEM certificate".to_vec());
        assert!(matches!(
            ClientPool::default().client(&empty),
            Err("HttpCa")
        ));
    }
    #[test]
    fn real_https_additive_ca_preserves_chain_and_hostname_verification() {
        use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mut host = Host::new().unwrap();
        let pem = include_bytes!("../../tests/fixtures/tls/localhost-cert.pem");
        let certificates = CertificateDer::pem_slice_iter(pem)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let key = PrivateKeyDer::from_pem_slice(include_bytes!(
            "../../tests/fixtures/tls/localhost-key.pem"
        ))
        .unwrap();
        let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .unwrap();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        // Bind both families: localhost resolution order differs on Windows.
        let ipv4 = host
            .runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let port = ipv4.local_addr().unwrap().port();
        let ipv6 = host
            .runtime
            .block_on(tokio::net::TcpListener::bind((
                std::net::Ipv6Addr::LOCALHOST,
                port,
            )))
            .ok();
        let served = Arc::new(AtomicUsize::new(0));
        let mut servers = Vec::new();
        for listener in std::iter::once(ipv4).chain(ipv6) {
            let acceptor = acceptor.clone();
            let served = served.clone();
            servers.push(host.runtime.spawn(async move {
                loop {
                    let Ok((socket, _)) = listener.accept().await else {
                        break;
                    };
                    let acceptor = acceptor.clone();
                    let served = served.clone();
                    tokio::spawn(async move {
                        let Ok(socket) = acceptor.accept(socket).await else {
                            return;
                        };
                        let service = hyper::service::service_fn(move |_| {
                            served.fetch_add(1, Ordering::SeqCst);
                            async {
                                Ok::<_, std::convert::Infallible>(hyper::Response::new(
                                    http_body_util::Full::new(bytes::Bytes::from_static(
                                        b"verified",
                                    )),
                                ))
                            }
                        });
                        let _ = hyper::server::conn::http1::Builder::new()
                            .serve_connection(hyper_util::rt::TokioIo::new(socket), service)
                            .await;
                    });
                }
            }));
        }
        let mut trusted = request(&format!("https://localhost:{port}/"));
        trusted.ca = Arc::new(include_bytes!("../../tests/fixtures/tls/localhost-ca.pem").to_vec());
        let lease = host.clients.client(&trusted).unwrap();
        for _ in 0..2 {
            let body = host.runtime.block_on(async {
                lease
                    .client
                    .get(&trusted.url)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                    .unwrap()
                    .text()
                    .await
                    .unwrap()
            });
            assert_eq!(body, "verified");
        }
        assert!(Arc::ptr_eq(&lease, &host.clients.client(&trusted).unwrap()));
        let untrusted = request(&trusted.url);
        let client = host.clients.client(&untrusted).unwrap();
        let error = host
            .runtime
            .block_on(async {
                client
                    .client
                    .get(&untrusted.url)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
            })
            .unwrap_err();
        assert!(
            error.is_connect(),
            "untrusted TLS failed in an unexpected phase"
        );
        let mut mismatch = request(&format!("https://127.0.0.1:{port}/"));
        mismatch.ca = trusted.ca.clone();
        let client = host.clients.client(&mismatch).unwrap();
        let error = host
            .runtime
            .block_on(async {
                client
                    .client
                    .get(&mismatch.url)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
            })
            .unwrap_err();
        assert!(
            error.is_connect(),
            "hostname rejection failed in an unexpected phase"
        );
        let mut invalid = trusted;
        invalid.ca = Arc::new(b"not a certificate".to_vec());
        assert!(matches!(host.clients.client(&invalid), Err("HttpCa")));
        assert_eq!(
            served.load(Ordering::SeqCst),
            2,
            "rejected requests reached the HTTP service"
        );
        for server in servers {
            server.abort();
        }
    }
    #[test]
    fn real_http_keepalive_reuses_connections_and_eviction_closes_idle_peer() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mut host = Host::new().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let ended = Arc::new(AtomicUsize::new(0));
        let mut listeners = Vec::new();
        let mut urls = Vec::new();
        for _ in 0..MAX_CLIENTS + 1 {
            let listener = host
                .runtime
                .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
                .unwrap();
            urls.push(format!("http://{}/", listener.local_addr().unwrap()));
            let accepted = accepted.clone();
            let ended = ended.clone();
            listeners.push(host.runtime.spawn(async move {
                loop {
                    let Ok((socket, _)) = listener.accept().await else {
                        break;
                    };
                    accepted.fetch_add(1, Ordering::SeqCst);
                    let ended = ended.clone();
                    tokio::spawn(async move {
                        let service = hyper::service::service_fn(|_| async {
                            Ok::<_, std::convert::Infallible>(hyper::Response::new(
                                http_body_util::Full::new(bytes::Bytes::from_static(b"ok")),
                            ))
                        });
                        let _ = hyper::server::conn::http1::Builder::new()
                            .serve_connection(hyper_util::rt::TokioIo::new(socket), service)
                            .await;
                        ended.fetch_add(1, Ordering::SeqCst);
                    });
                }
            }));
        }
        let mut id = 0;
        let mut fetch = |host: &mut Host, url: &str| {
            id += 1;
            host.submit(id, request(url), vec![]).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(response) = host.poll(id) {
                    assert_eq!(response["status"], 200, "{response}");
                    assert_eq!(response["body"], "b2s=");
                    break;
                }
                assert!(Instant::now() < deadline, "HTTP fixture did not complete");
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        fetch(&mut host, &urls[0]);
        fetch(&mut host, &urls[0]);
        assert_eq!(
            accepted.load(Ordering::SeqCst),
            1,
            "same-origin keepalive was lost"
        );
        for url in &urls[1..] {
            fetch(&mut host, url);
        }
        assert_eq!(host.clients.cached(), MAX_CLIENTS);
        let deadline = Instant::now() + Duration::from_secs(5);
        while ended.load(Ordering::SeqCst) == 0 {
            assert!(
                Instant::now() < deadline,
                "evicted idle connection remained open"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        fetch(&mut host, &urls[0]);
        assert_eq!(accepted.load(Ordering::SeqCst), MAX_CLIENTS + 2);
        drop(host);
        for listener in listeners {
            listener.abort();
        }
    }
    #[test]
    fn evicted_pending_request_and_upload_keep_lease_until_completion_or_cancel() {
        use http_body_util::BodyExt;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mut host = Host::new().unwrap();
        let listener = host
            .runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let started = Arc::new(AtomicUsize::new(0));
        let received = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(tokio::sync::Notify::new());
        let server = {
            let started = started.clone();
            let received = received.clone();
            let release = release.clone();
            host.runtime.spawn(async move {
                loop {
                    let Ok((socket, _)) = listener.accept().await else {
                        break;
                    };
                    let started = started.clone();
                    let received = received.clone();
                    let release = release.clone();
                    tokio::spawn(async move {
                        let service = hyper::service::service_fn(
                            move |request: hyper::Request<hyper::body::Incoming>| {
                                let started = started.clone();
                                let received = received.clone();
                                let release = release.clone();
                                async move {
                                    started.fetch_add(1, Ordering::SeqCst);
                                    if request.method() == hyper::Method::GET {
                                        release.notified().await;
                                    } else {
                                        let body = request.into_body().collect().await?.to_bytes();
                                        received.fetch_add(body.len(), Ordering::SeqCst);
                                    }
                                    Ok::<_, hyper::Error>(hyper::Response::new(
                                        http_body_util::Full::new(bytes::Bytes::from_static(b"ok")),
                                    ))
                                }
                            },
                        );
                        let _ = hyper::server::conn::http1::Builder::new()
                            .serve_connection(hyper_util::rt::TokioIo::new(socket), service)
                            .await;
                    });
                }
            })
        };
        let wait = |stage: &str, predicate: &mut dyn FnMut() -> bool| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !predicate() {
                assert!(
                    Instant::now() < deadline,
                    "HTTP worker did not complete: {stage}"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        let evict = |host: &mut Host| {
            for port in 10000..10016 {
                host.clients
                    .client(&request(&format!("http://127.0.0.1:{port}/")))
                    .unwrap();
            }
        };
        let old = Arc::downgrade(&host.clients.client(&request(&url)).unwrap());
        host.submit(1, request(&url), vec![]).unwrap();
        wait("pending request headers", &mut || {
            started.load(Ordering::SeqCst) == 1
        });
        evict(&mut host);
        assert_eq!(host.clients.retained(), MAX_CLIENTS + 1);
        assert!(host.poll(1).is_none());
        release.notify_one();
        wait("pending response", &mut || {
            host.poll(1).is_some_and(|v| {
                assert_eq!(v["status"], 200);
                true
            })
        });
        wait("pending lease drop", &mut || old.upgrade().is_none());
        let mut upload = request(&url);
        upload.method = "POST".into();
        upload.upload_limit = Some(1024);
        host.submit(2, upload.clone(), vec![]).unwrap();
        assert_eq!(host.poll(2).unwrap()["upload"], 2);
        let old = Arc::downgrade(&host.clients.client(&upload).unwrap());
        host.write_upload(3, 2, Arc::new(b"payload".to_vec()))
            .unwrap();
        wait("upload write", &mut || {
            host.poll(3).is_some_and(|v| {
                assert_eq!(v["written"], 7);
                true
            })
        });
        wait("upload headers", &mut || {
            started.load(Ordering::SeqCst) == 2
        });
        evict(&mut host);
        assert_eq!(host.clients.retained(), MAX_CLIENTS + 1);
        host.finish_upload(4, 2).unwrap();
        wait("upload response", &mut || {
            host.poll(4).is_some_and(|v| {
                assert_eq!(v["status"], 200);
                true
            })
        });
        assert_eq!(received.load(Ordering::SeqCst), 7);
        // A later submission collects the finishing stream's acknowledged owner.
        wait("finished upload acknowledgement", &mut || {
            host.retired_uploads
                .iter()
                .all(|s| Arc::strong_count(s) == 1)
        });
        host.submit(5, upload.clone(), vec![]).unwrap();
        assert_eq!(host.poll(5).unwrap()["upload"], 5);
        wait("finished upload lease drop", &mut || {
            old.upgrade().is_none()
        });
        let old = Arc::downgrade(&host.clients.client(&upload).unwrap());
        evict(&mut host);
        host.close_upload(5);
        wait("cancel acknowledgement", &mut || {
            host.retired_uploads
                .iter()
                .all(|s| Arc::strong_count(s) == 1)
        });
        host.retired_uploads.retain(|s| Arc::strong_count(s) > 1);
        wait("cancelled upload lease drop", &mut || {
            old.upgrade().is_none()
        });
        // Keep closing owners alive to test the quota without a worker scheduling race.
        let mut closing = Vec::new();
        for id in 10..18 {
            host.submit(id, upload.clone(), vec![]).unwrap();
            assert_eq!(host.poll(id).unwrap()["upload"], id);
            closing.push(host.uploads[&id].clone());
            host.close_upload(id);
        }
        assert_eq!(host.retired_uploads.len(), 8);
        assert_eq!(
            host.submit(18, upload.clone(), vec![]),
            Err("HttpUploadLimit")
        );
        drop(closing);
        wait("closing upload owners released", &mut || {
            host.retired_uploads
                .iter()
                .all(|s| Arc::strong_count(s) == 1)
        });
        host.submit(19, upload, vec![]).unwrap();
        assert_eq!(host.poll(19).unwrap()["upload"], 19);
        host.close_upload(19);
        drop(host);
        server.abort();
    }
    #[test]
    fn evicted_download_keeps_client_until_the_closed_stream_is_collected() {
        let mut host = Host::new().unwrap();
        let listener = host
            .runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = host.runtime.spawn(async move {
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let service = hyper::service::service_fn(|_| async {
                        Ok::<_, std::convert::Infallible>(hyper::Response::new(
                            http_body_util::Full::new(bytes::Bytes::from_static(b"data")),
                        ))
                    });
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(hyper_util::rt::TokioIo::new(socket), service)
                        .await;
                });
            }
        });
        let mut download = request(&url);
        download.download = true;
        host.submit(1, download, vec![]).unwrap();
        let poll = |host: &mut Host, id| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(result) = host.poll(id) {
                    return result;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        let result = poll(&mut host, 1);
        assert_eq!(result["download"], 1, "{result}");
        let old = Arc::downgrade(&host.downloads[&1]._client);
        for port in 10000..10016 {
            host.clients
                .client(&request(&format!("http://127.0.0.1:{port}/")))
                .unwrap();
        }
        assert!(old.upgrade().is_some());
        assert_eq!(host.clients.cached(), MAX_CLIENTS);
        assert_eq!(host.clients.retained(), MAX_CLIENTS + 1);
        host.close(1);
        let deadline = Instant::now() + Duration::from_secs(10);
        while host.retired.iter().any(|s| Arc::strong_count(s) > 1) {
            assert!(
                Instant::now() < deadline,
                "download close did not acknowledge"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        host.submit(2, request(&url), vec![]).unwrap();
        assert!(old.upgrade().is_none());
        assert_eq!(poll(&mut host, 2)["status"], 200);
        let mut closing = Vec::new();
        let mut download = request(&url);
        download.download = true;
        for id in 3..11 {
            host.submit(id, download.clone(), vec![]).unwrap();
            assert_eq!(poll(&mut host, id)["download"], id);
            closing.push(host.downloads[&id].clone());
            host.close(id);
        }
        assert_eq!(host.retired.len(), 8);
        assert_eq!(
            host.submit(11, download.clone(), vec![]),
            Err("HttpDownloadLimit")
        );
        drop(closing);
        let deadline = Instant::now() + Duration::from_secs(10);
        while host.retired.iter().any(|s| Arc::strong_count(s) > 1) {
            assert!(
                Instant::now() < deadline,
                "closing download owners were not released"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        host.submit(12, download, vec![]).unwrap();
        assert_eq!(poll(&mut host, 12)["download"], 12);
        host.close(12);
        drop(host);
        server.abort();
    }
}
