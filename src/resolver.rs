//! Process-wide, non-queuing DNS admission shared by native transports.
//! OS getaddrinfo is not cancellable; its permit stays with the blocking job.
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::{Arc, OnceLock},
};
use tokio::{sync::Semaphore, task::JoinHandle};
const SLOTS: usize = 8;
const ADDRESSES: usize = 64;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolveError {
    Busy,
    Limit,
    Lookup,
}
impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Busy => "DNS admission exhausted",
            Self::Limit => "DNS capacity exceeded",
            Self::Lookup => "DNS resolution failed",
        })
    }
}
impl std::error::Error for ResolveError {}
fn slots() -> Arc<Semaphore> {
    static S: OnceLock<Arc<Semaphore>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Semaphore::new(SLOTS))).clone()
}
pub(crate) fn spawn_with_slots<F>(
    slots: Arc<Semaphore>,
    work: F,
) -> Result<JoinHandle<Result<Vec<SocketAddr>, ResolveError>>, ResolveError>
where
    F: FnOnce() -> Result<Vec<SocketAddr>, ResolveError> + Send + 'static,
{
    let permit = slots.try_acquire_owned().map_err(|_| ResolveError::Busy)?;
    Ok(tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    }))
}
pub(crate) async fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, ResolveError> {
    resolve_with_slots(host, port, slots()).await
}
async fn resolve_with_slots(
    host: &str,
    port: u16,
    slots: Arc<Semaphore>,
) -> Result<Vec<SocketAddr>, ResolveError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    if host.is_empty() || host.len() > 254 {
        return Err(ResolveError::Limit);
    }
    let host = host.to_owned();
    spawn_with_slots(slots, move || {
        let addresses = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|_| ResolveError::Lookup)?;
        bounded(addresses)
    })?
    .await
    .map_err(|_| ResolveError::Lookup)?
}
fn bounded(addresses: impl Iterator<Item = SocketAddr>) -> Result<Vec<SocketAddr>, ResolveError> {
    let addresses = addresses.take(ADDRESSES + 1).collect::<Vec<_>>();
    if addresses.len() > ADDRESSES {
        return Err(ResolveError::Limit);
    }
    if addresses.is_empty() {
        return Err(ResolveError::Lookup);
    }
    Ok(addresses)
}
pub(crate) struct HttpResolver;
impl reqwest::dns::Resolve for HttpResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addresses = resolve(&host, 0)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
    }
}
pub(crate) fn http_error(error: &reqwest::Error) -> Option<&'static str> {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(e) = source {
        if let Some(e) = e.downcast_ref::<ResolveError>() {
            return Some(match e {
                ResolveError::Busy => "HttpBusy",
                ResolveError::Limit => "HttpResolveLimit",
                ResolveError::Lookup => "HttpResolve",
            });
        }
        source = e.source();
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_and_queued_jobs_keep_their_permits_until_os_work_finishes() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        rt.block_on(async {
            let slots = Arc::new(Semaphore::new(8));
            let mut releases = Vec::new();
            for _ in 0..8 {
                let (tx, rx) = std::sync::mpsc::channel();
                releases.push(tx);
                let handle = spawn_with_slots(slots.clone(), move || {
                    rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
                    Ok(vec![SocketAddr::from(([127, 0, 0, 1], 80))])
                })
                .unwrap();
                drop(handle);
            }
            assert_eq!(slots.available_permits(), 0);
            assert!(matches!(
                spawn_with_slots(slots.clone(), || panic!("must not queue")),
                Err(ResolveError::Busy)
            ));
            for tx in releases {
                tx.send(()).unwrap()
            }
            let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while slots.available_permits() != 8 {
                assert!(std::time::Instant::now() < until);
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        });
    }
    #[test]
    fn reqwest_preserves_typed_dns_failures_without_connecting() {
        struct Busy;
        impl reqwest::dns::Resolve for Busy {
            fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
                Box::pin(async {
                    Err(Box::new(ResolveError::Busy) as Box<dyn std::error::Error + Send + Sync>)
                })
            }
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let client = reqwest::Client::builder()
                .no_proxy()
                .dns_resolver(Arc::new(Busy))
                .build()
                .unwrap();
            let error = client
                .get("http://resolver-rejected.invalid/")
                .send()
                .await
                .unwrap_err();
            assert!(error.is_connect());
            assert_eq!(http_error(&error), Some("HttpBusy"));
        });
    }
    #[test]
    fn numeric_addresses_bypass_exhausted_admission_and_address_results_are_bounded() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let slots = Arc::new(Semaphore::new(0));
            assert_eq!(
                resolve_with_slots("127.0.0.1", 123, slots.clone())
                    .await
                    .unwrap(),
                vec![SocketAddr::from(([127, 0, 0, 1], 123))]
            );
            assert_eq!(
                resolve_with_slots("::1", 123, slots.clone()).await.unwrap(),
                vec!["[::1]:123".parse().unwrap()]
            );
            assert_eq!(
                resolve_with_slots("localhost", 123, slots).await,
                Err(ResolveError::Busy)
            );
        });
        let address = SocketAddr::from(([127, 0, 0, 1], 80));
        assert_eq!(bounded(std::iter::empty()), Err(ResolveError::Lookup));
        assert_eq!(
            bounded(std::iter::repeat_n(address, 65)),
            Err(ResolveError::Limit)
        );
        assert_eq!(bounded(std::iter::repeat_n(address, 64)).unwrap().len(), 64);
    }
}
