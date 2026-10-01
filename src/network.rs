//! Bounded asynchronous HTTP transport. VM values never cross worker threads.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc, Arc, OnceLock,
    },
    time::Duration,
};
use tokio::sync::oneshot;

impl crate::Runtime {
    pub fn start_http(&mut self, mut request: Request) -> crate::Result<usize> {
        self.charge_native_work(
            request
                .body
                .len()
                .saturating_add(request.url.len())
                .saturating_add(request.ca.len())
                .saturating_add(
                    request
                        .headers
                        .iter()
                        .map(|(n, v)| n.len() + v.len())
                        .sum::<usize>(),
                )
                .saturating_add(1),
        )?;
        if self.sensitive_values.iter().any(|s| {
            !s.is_empty()
                && (request.url.contains(s)
                    || request.body.windows(s.len()).any(|w| w == s.as_bytes())
                    || request
                        .headers
                        .iter()
                        .any(|(_, v)| v.windows(s.len()).any(|w| w == s.as_bytes())))
        }) {
            return Err(crate::Error::InvalidOperation(
                "SecretObservationUnrecordable: use an opaque credential".into(),
            ));
        }
        let (id, fresh) = self.begin_async_external(
            "http.request.v1",
            &request.fingerprint(),
            request.reservation(),
        )?;
        if fresh {
            let submitted = (|| {
                request.validate()?;
                if !request.credential.is_empty() {
                    let credential = self
                        .network_credentials
                        .get(&request.credential)
                        .ok_or("HttpCredentialUnknown")?;
                    request
                        .headers
                        .push(("Authorization".into(), credential.as_bytes().to_vec()));
                }
                if self.network_host.is_none() {
                    self.network_host = Some(Host::new()?);
                }
                // Admit native buffers before launching an irreversible operation.
                let estimate = request.body.len() + request.limit + MAX_HEADERS;
                self.network_host.as_mut().unwrap().admission = estimate;
                let budget = self.enforce_budget();
                self.network_host.as_mut().unwrap().admission = 0;
                if budget.is_err() {
                    return Err("HttpMemoryLimit");
                }
                self.network_host.as_mut().unwrap().submit(id, request)
            })();
            if let Err(code) = submitted {
                self.finish_async_external(id, Ok(failure(code, "NotSent", 0)))?;
            }
        }
        Ok(id)
    }
    pub fn register_http_credential(
        &mut self,
        alias: &str,
        value: &str,
    ) -> crate::Result<std::result::Result<(), &'static str>> {
        if self.external_depth != 1 {
            return Err(crate::Error::InvalidOperation(
                "ExternalBoundary: credential requires external region".into(),
            ));
        }
        self.charge_native_work(alias.len().saturating_add(value.len()).saturating_add(1))?;
        if alias.is_empty()
            || alias.len() > 128
            || !alias
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            || value.len() > 8192
            || reqwest::header::HeaderValue::from_str(value).is_err()
        {
            return Ok(Err("HttpCredential"));
        }
        if let Some(old) = self.network_credentials.get(alias) {
            return Ok(if old.as_ref() == value {
                Ok(())
            } else {
                Err("HttpCredentialImmutable")
            });
        }
        if self.network_credentials.len() >= 64 {
            return Ok(Err("HttpCredentialLimit"));
        }
        self.register_secret_value(&crate::Value::Text(value.into()));
        if let Some(token) = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("Basic "))
        {
            self.register_secret_value(&crate::Value::Text(token.into()));
        }
        self.network_credentials
            .insert(alias.into(), Arc::from(value));
        if let Err(e) = self.enforce_budget() {
            self.network_credentials.remove(alias);
            return Err(e);
        }
        Ok(Ok(()))
    }
    pub fn cancel_http(&mut self, id: usize) -> crate::Result<()> {
        if self.external_entries.get(id).is_some_and(|e| e.pending) {
            let mut result = self
                .network_host
                .as_mut()
                .map_or_else(|| failure("HttpCancelled", "NotSent", 0), |h| h.cancel(id));
            if protects_secret(&result, &self.sensitive_values) {
                result = failure(
                    "HttpSecretResponse",
                    "ResponseReceived",
                    result["status"].as_u64().unwrap_or(0) as u16,
                );
            }
            self.finish_async_external(id, Ok(result))?;
        }
        Ok(())
    }
}

pub(crate) const MAX_BODY: usize = 4 * 1024 * 1024;
const MAX_HEADERS: usize = 32 * 1024;
const MAX_JOBS: usize = 8;
#[derive(Clone)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub body: Arc<Vec<u8>>,
    pub timeout_ms: u64,
    pub limit: usize,
    pub headers: Vec<(String, Vec<u8>)>,
    pub ca: Arc<Vec<u8>>,
    pub credential: String,
}
impl Request {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.url.len() > 8192
            || self.body.len() > MAX_BODY
            || self.limit > MAX_BODY
            || self.limit == 0
            || !(1..=120_000).contains(&self.timeout_ms)
        {
            return Err("HttpLimit");
        }
        if self.ca.len() > 64 * 1024
            || self.headers.len() > 256
            || self
                .headers
                .iter()
                .map(|(n, v)| n.len() + v.len())
                .sum::<usize>()
                > MAX_HEADERS
        {
            return Err("HttpHeaderLimit");
        }
        for (name, value) in &self.headers {
            reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| "HttpHeader")?;
            reqwest::header::HeaderValue::from_bytes(value).map_err(|_| "HttpHeader")?;
            if [
                "host",
                "content-length",
                "transfer-encoding",
                "connection",
                "proxy-authorization",
                "authorization",
                "cookie",
            ]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n))
            {
                return Err("HttpHeaderReserved");
            }
        }
        let url = reqwest::Url::parse(&self.url).map_err(|_| "HttpUrl")?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err("HttpUrl");
        }
        reqwest::Method::from_bytes(self.method.as_bytes()).map_err(|_| "HttpMethod")?;
        Ok(())
    }
    pub fn fingerprint(&self) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        json!([
            self.method,
            self.url,
            format!("{:x}", Sha256::digest(self.body.as_slice())),
            self.timeout_ms,
            self.limit,
            self.credential,
            self.headers
                .iter()
                .map(|(n, v)| json!([n, STANDARD.encode(v)]))
                .collect::<Vec<_>>(),
            format!("{:x}", Sha256::digest(self.ca.as_slice()))
        ])
        .to_string()
        .into_bytes()
    }
    pub fn reservation(&self) -> usize {
        self.limit.min(MAX_BODY).div_ceil(3) * 4 + MAX_HEADERS * 2 + 4096
    }
}
struct Job {
    receiver: mpsc::Receiver<Value>,
    cancel: Option<oneshot::Sender<()>>,
    phase: Arc<AtomicU8>,
    bytes: usize,
}
pub(crate) struct Host {
    runtime: &'static tokio::runtime::Runtime,
    client: reqwest::Client,
    jobs: BTreeMap<usize, Job>,
    admission: usize,
    ca_clients: BTreeMap<String, reqwest::Client>,
}
pub(crate) fn failure(code: &str, phase: &str, status: u16) -> Value {
    json!({"error":{"code":code,"phase":phase,"status":status}})
}
pub(crate) fn protects_secret(
    response: &Value,
    secrets: &std::collections::BTreeSet<String>,
) -> bool {
    let values = response
        .get("body")
        .and_then(Value::as_str)
        .into_iter()
        .chain(
            response
                .get("headers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|h| h["value"].as_str()),
        );
    values.filter_map(|v| STANDARD.decode(v).ok()).any(|bytes| {
        secrets
            .iter()
            .any(|s| !s.is_empty() && bytes.windows(s.len()).any(|w| w == s.as_bytes()))
    })
}
impl Host {
    pub fn new() -> Result<Self, &'static str> {
        static WORKERS: OnceLock<Result<tokio::runtime::Runtime, &'static str>> = OnceLock::new();
        let runtime = WORKERS
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .max_blocking_threads(2)
                    .enable_all()
                    .build()
                    .map_err(|_| "HttpWorker")
            })
            .as_ref()
            .map_err(|e| *e)?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "HttpTls")?;
        Ok(Self {
            runtime,
            client,
            jobs: BTreeMap::new(),
            admission: 0,
            ca_clients: BTreeMap::new(),
        })
    }
    pub fn reserved_bytes(&self) -> usize {
        self.admission + self.jobs.values().map(|j| j.bytes).sum::<usize>()
    }
    pub fn submit(&mut self, id: usize, request: Request) -> Result<(), &'static str> {
        // Request was validated before private credential injection.
        // Cancelled workers retain their slots until they acknowledge cancellation.
        self.jobs.retain(|_, j| {
            j.cancel.is_some() || matches!(j.receiver.try_recv(), Err(mpsc::TryRecvError::Empty))
        });
        if self.jobs.len() >= MAX_JOBS {
            return Err("HttpConcurrencyLimit");
        }
        if self.jobs.contains_key(&id) {
            return Err("HttpAlreadySubmitted");
        }
        let bytes = request.body.len() + request.limit + MAX_HEADERS;
        let (sender, receiver) = mpsc::channel();
        let (cancel, cancelled) = oneshot::channel();
        let phase = Arc::new(AtomicU8::new(0));
        let worker_phase = phase.clone();
        let client = if request.ca.is_empty() {
            self.client.clone()
        } else {
            use sha2::{Digest, Sha256};
            let key = format!("{:x}", Sha256::digest(request.ca.as_slice()));
            if !self.ca_clients.contains_key(&key) {
                if self.ca_clients.len() >= 8 {
                    return Err("HttpCaLimit");
                }
                let ca = reqwest::Certificate::from_pem(&request.ca).map_err(|_| "HttpCa")?;
                let client = reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .retry(reqwest::retry::never())
                    .connect_timeout(Duration::from_secs(10))
                    .pool_max_idle_per_host(2)
                    .pool_idle_timeout(Duration::from_secs(30))
                    .add_root_certificate(ca)
                    .build()
                    .map_err(|_| "HttpTls")?;
                self.ca_clients.insert(key.clone(), client);
            }
            self.ca_clients[&key].clone()
        };
        self.runtime.spawn(async move {
            let future = async {
                if worker_phase.compare_exchange(0,1,Ordering::SeqCst,Ordering::SeqCst).is_err() {return failure("HttpCancelled","NotSent",0);}
                execute(client, request, &worker_phase).await
            };
            let result = tokio::select! {
                biased;
                _ = cancelled => failure("HttpCancelled", if matches!(worker_phase.load(Ordering::SeqCst),0|3) {"NotSent"} else {"Unknown"}, 0),
                response = future => response,
            };
            let _ = sender.send(result);
        });
        self.jobs.insert(
            id,
            Job {
                receiver,
                cancel: Some(cancel),
                phase,
                bytes,
            },
        );
        Ok(())
    }
    pub fn poll(&mut self, id: usize) -> Option<Value> {
        let job = self.jobs.get(&id)?;
        let value = match job.receiver.try_recv() {
            Ok(v) => v,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => failure("HttpWorker", "Unknown", 0),
        };
        self.jobs.remove(&id);
        Some(value)
    }
    pub fn cancel(&mut self, id: usize) -> Value {
        if let Some(job) = self.jobs.get_mut(&id) {
            if let Ok(value) = job.receiver.try_recv() {
                self.jobs.remove(&id);
                return value;
            }
            let unsent = job
                .phase
                .compare_exchange(0, 3, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok();
            if let Some(cancel) = job.cancel.take() {
                let _ = cancel.send(());
            }
            failure(
                "HttpCancelled",
                if unsent { "NotSent" } else { "Unknown" },
                0,
            )
        } else {
            failure("HttpCancelled", "Unknown", 0)
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        for job in self.jobs.values_mut() {
            if let Some(cancel) = job.cancel.take() {
                let _ = cancel.send(());
            }
        }
    }
}
struct SharedBody(Arc<Vec<u8>>);
impl AsRef<[u8]> for SharedBody {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
async fn execute(client: reqwest::Client, request: Request, phase: &AtomicU8) -> Value {
    let deadline = Duration::from_millis(request.timeout_ms);
    let run = async {
        let method =
            reqwest::Method::from_bytes(request.method.as_bytes()).expect("validated method");
        let mut builder = client
            .request(method, &request.url)
            .body(bytes::Bytes::from_owner(SharedBody(request.body.clone())));
        let mut headers = reqwest::header::HeaderMap::new();
        for (name, value) in &request.headers {
            headers.append(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).expect("validated header"),
                reqwest::header::HeaderValue::from_bytes(value).expect("validated value"),
            );
        }
        builder = builder.headers(headers);
        let mut response = match builder.send().await {
            Ok(r) => r,
            Err(e) => {
                return failure(
                    if e.is_timeout() {
                        "HttpTimeout"
                    } else {
                        "HttpTransport"
                    },
                    if e.is_connect() { "NotSent" } else { "Unknown" },
                    0,
                )
            }
        };
        phase.store(2, Ordering::SeqCst);
        let status = response.status().as_u16();
        if response
            .content_length()
            .is_some_and(|n| n > request.limit as u64)
        {
            return failure("HttpBodyLimit", "ResponseReceived", status);
        }
        let mut headers = Vec::new();
        let mut header_bytes = 0usize;
        for (name, value) in response.headers() {
            header_bytes =
                header_bytes.saturating_add(name.as_str().len() + value.as_bytes().len());
            if header_bytes > MAX_HEADERS || headers.len() == 256 {
                return failure("HttpHeaderLimit", "ResponseReceived", status);
            }
            headers.push(json!({"name":name.as_str(),"value":STANDARD.encode(value.as_bytes())}));
        }
        let mut body = Vec::new();
        if body.try_reserve_exact(request.limit).is_err() {
            return failure("HttpAllocation", "ResponseReceived", status);
        }
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if chunk.len() > request.limit - body.len() {
                        return failure("HttpBodyLimit", "ResponseReceived", status);
                    }
                    body.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(e) => {
                    return failure(
                        if e.is_timeout() {
                            "HttpTimeout"
                        } else {
                            "HttpTransport"
                        },
                        "ResponseReceived",
                        status,
                    )
                }
            }
        }
        json!({"status":status,"headers":headers,"body":STANDARD.encode(body)})
    };
    match tokio::time::timeout(deadline, run).await {
        Ok(v) => v,
        Err(_) => failure(
            "HttpTimeout",
            if phase.load(Ordering::SeqCst) == 2 {
                "ResponseReceived"
            } else {
                "Unknown"
            },
            0,
        ),
    }
}
