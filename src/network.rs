//! Bounded asynchronous HTTP transport. VM values never cross worker threads.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc, Arc, OnceLock,
    },
    time::Duration,
};
use tokio::sync::{oneshot, Mutex};
mod client_pool;
mod upload;

impl crate::Runtime {
    pub(crate) fn http_secret_patterns(&self) -> Result<Vec<Vec<u8>>, &'static str> {
        let size = self.sensitive_values.iter().map(String::len).sum::<usize>()
            + self.sensitive_bytes.iter().map(|v| v.len()).sum::<usize>();
        if size > 65536 {
            return Err("HttpSecretLimit");
        }
        Ok(self
            .sensitive_values
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.as_bytes().to_vec())
            .chain(self.sensitive_bytes.iter().map(|v| v.as_ref().clone()))
            .collect())
    }
    pub(crate) fn protect_http_response(&mut self, response: &Value) -> crate::Result<bool> {
        if response.get("stream").is_some()
            && response.get("secret_count").and_then(Value::as_u64)
                == Some((self.sensitive_values.len() + self.sensitive_bytes.len()) as u64)
        {
            return Ok(false);
        }
        if self.sensitive_values.is_empty() && self.sensitive_bytes.is_empty() {
            return Ok(false);
        }
        let patterns = match self.http_secret_patterns() {
            Ok(p) => p,
            Err(_) => return Ok(true),
        };
        let size = patterns.iter().map(Vec::len).sum::<usize>();
        let body = response
            .get("body")
            .and_then(Value::as_str)
            .map_or(0, str::len);
        let estimate = body + body / 4 * 3 + size * 96 + 2 * MAX_HEADERS + 4096;
        if let Some(host) = self.network_host.as_mut() {
            host.admission = estimate;
        }
        let budget = self.enforce_budget();
        if let Some(host) = self.network_host.as_mut() {
            host.admission = 0;
        }
        budget?;
        self.charge_native_work(body.saturating_add(size).saturating_add(1))?;
        Ok(protect_patterns(response, &patterns))
    }
    pub fn start_http(&mut self, mut request: Request) -> crate::Result<usize> {
        self.charge_native_work(
            request
                .body
                .len()
                .saturating_add(request.method.len())
                .saturating_add(request.credential.len())
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
                    || request.method.contains(s)
                    || request.credential.contains(s)
                    || request.body.windows(s.len()).any(|w| w == s.as_bytes())
                    || request.headers.iter().any(|(name, v)| {
                        name.contains(s) || v.windows(s.len()).any(|w| w == s.as_bytes())
                    }))
        }) || self.sensitive_bytes.iter().any(|s| {
            request.body.windows(s.len()).any(|w| w == s.as_slice())
                || request
                    .headers
                    .iter()
                    .any(|(_, v)| v.windows(s.len()).any(|w| w == s.as_slice()))
        }) {
            return Err(crate::Error::InvalidOperation(
                "SecretObservationUnrecordable: use an opaque credential".into(),
            ));
        }
        let (id, fresh) = self.begin_async_external(
            if request.upload_limit.is_some() {
                "http.upload.v1"
            } else if request.download {
                "http.download.v1"
            } else {
                "http.request.v1"
            },
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
                let secrets = if request.download {
                    self.http_secret_patterns()?
                } else {
                    vec![]
                };
                let secret_bytes = secrets.iter().map(Vec::len).sum::<usize>();
                if secret_bytes > 65536 {
                    return Err("HttpSecretLimit");
                }
                let estimate = request.body.len()
                    + (if request.download {
                        download_reservation(secret_bytes)
                    } else {
                        response_reservation(request.limit)
                    })
                    + MAX_HEADERS;
                let estimate = estimate
                    + if request.upload_limit.is_some() {
                        2 * CHUNK + 8192
                    } else {
                        4096
                    };
                let client_estimate = self
                    .network_host
                    .as_mut()
                    .unwrap()
                    .clients
                    .admission(&request)?;
                self.network_host.as_mut().unwrap().admission = estimate + client_estimate;
                let budget = self.enforce_budget();
                self.network_host.as_mut().unwrap().admission = 0;
                if budget.is_err() {
                    return Err("HttpMemoryLimit");
                }
                self.network_host
                    .as_mut()
                    .unwrap()
                    .submit(id, request, secrets)
            })();
            if let Err(code) = submitted {
                self.finish_async_external(id, Ok(failure(code, "NotSent", 0)))?;
            }
        }
        Ok(id)
    }
    pub fn read_http(&mut self, stream: usize, limit: usize) -> crate::Result<usize> {
        self.charge_native_work(limit.saturating_add(1))?;
        let (id, fresh) = self.begin_async_external(
            "http.read.v1",
            &serde_json::to_vec(&(stream, limit)).unwrap(),
            CHUNK.div_ceil(3) * 4 + 4096,
        )?;
        if fresh {
            let submitted = (|| {
                let host = self.network_host.as_mut().ok_or("HttpDownloadClosed")?;
                host.admission = CHUNK * 2 + 4096;
                let budget = self.enforce_budget();
                self.network_host.as_mut().unwrap().admission = 0;
                if budget.is_err() {
                    return Err("HttpMemoryLimit");
                }
                self.network_host.as_mut().unwrap().read(
                    id,
                    stream,
                    limit,
                    self.sensitive_values.len() + self.sensitive_bytes.len(),
                )
            })();
            if let Err(code) = submitted {
                self.finish_async_external(id, Ok(failure(code, "ResponseReceived", 0)))?;
            }
        }
        Ok(id)
    }
    pub fn has_native_resources(&self) -> bool {
        if self
            .http_server_host
            .as_ref()
            .is_some_and(|h| h.resource_ids().next().is_some())
        {
            return true;
        }
        self.tcp_host
            .as_ref()
            .is_some_and(|h| h.resource_ids().next().is_some())
            || self
                .database_host
                .as_ref()
                .is_some_and(|h| h.resource_ids().next().is_some())
            || self
                .network_host
                .as_ref()
                .is_some_and(|h| !h.downloads.is_empty() || !h.uploads.is_empty())
    }
    pub fn native_resource_count(&self) -> usize {
        self.http_server_host
            .as_ref()
            .map_or(0, |h| h.resource_ids().count())
            + self
                .tcp_host
                .as_ref()
                .map_or(0, |h| h.resource_ids().count())
            + self
                .database_host
                .as_ref()
                .map_or(0, |h| h.resource_ids().count())
            + self
                .network_host
                .as_ref()
                .map_or(0, |h| h.downloads.len() + h.uploads.len())
    }
    pub fn http_cached_clients(&self) -> usize {
        self.network_host
            .as_ref()
            .map_or(0, |host| host.clients.cached())
    }
    pub fn http_retained_clients(&self) -> usize {
        self.network_host
            .as_ref()
            .map_or(0, |host| host.clients.retained())
    }
    pub fn http_client_reservation_bytes(&self) -> usize {
        self.network_host
            .as_ref()
            .map_or(0, |host| host.clients.reserved_bytes())
    }
    pub fn collect_native_resources(&mut self, roots: &[crate::Value]) -> crate::Result<()> {
        if let Some(host) = &self.http_server_host {
            let live = self.live_native_ids(roots);
            let abandoned = host
                .resource_ids()
                .filter(|id| !live.contains(&(**id as u64)) && !host.owns_pending_server(**id))
                .copied()
                .collect::<Vec<_>>();
            for id in abandoned {
                self.close_native_resource(id as u64)?;
                self.forget_native_owner(id as u64);
            }
        }
        if let Some(host) = &self.tcp_host {
            let live = self.live_native_ids(roots);
            let abandoned = host
                .resource_ids()
                .filter(|id| !live.contains(&(**id as u64)) && !host.owns_pending_socket(**id))
                .copied()
                .collect::<Vec<_>>();
            for id in abandoned {
                self.close_native_resource(id as u64)?;
                self.forget_native_owner(id as u64);
            }
        }
        if let Some(host) = &self.database_host {
            let live = self.live_native_ids(roots);
            let abandoned = host
                .resource_ids()
                .filter(|id| !live.contains(&(**id as u64)))
                .copied()
                .collect::<Vec<_>>();
            for id in abandoned {
                self.close_native_resource(id as u64)?;
                self.forget_native_owner(id as u64);
            }
        }
        if self
            .network_host
            .as_ref()
            .is_none_or(|h| h.downloads.is_empty() && h.uploads.is_empty())
        {
            return Ok(());
        }
        let live = self.live_native_ids(roots);
        let abandoned = self
            .network_host
            .as_ref()
            .unwrap()
            .downloads
            .keys()
            .chain(self.network_host.as_ref().unwrap().uploads.keys())
            .filter(|id| {
                !live.contains(&(**id as u64))
                    && !self.network_host.as_ref().unwrap().jobs.contains_key(id)
            })
            .copied()
            .collect::<Vec<_>>();
        for id in abandoned {
            self.close_native_resource(id as u64)?;
            self.forget_native_owner(id as u64);
        }
        Ok(())
    }
    pub fn close_http(&mut self, stream: usize) -> crate::Result<usize> {
        let (id, fresh) =
            self.begin_async_external("http.close.v1", &(stream as u64).to_le_bytes(), 4096)?;
        if fresh {
            self.close_native_resource(stream as u64)?;
            self.finish_async_external(id, Ok(json!({"closed":stream})))?;
        }
        Ok(id)
    }
    pub fn close_native_resource(&mut self, id: u64) -> crate::Result<()> {
        let stream = usize::try_from(id)
            .map_err(|_| crate::Error::InvalidOperation("NativeResourceInvalid".into()))?;
        if let Some(host) = self.database_host.as_mut() {
            host.close_resource(stream);
        }
        let server_jobs = self
            .http_server_host
            .as_mut()
            .map_or_else(Vec::new, |h| h.close(stream));
        for job in server_jobs {
            self.cancel_http(job)?;
        }
        let tcp_jobs = self
            .tcp_host
            .as_mut()
            .map_or_else(Vec::new, |h| h.close(stream));
        for job in tcp_jobs {
            self.cancel_http(job)?;
        }
        let jobs = self
            .network_host
            .as_mut()
            .map_or_else(Vec::new, |h| h.close(stream));
        for job in jobs {
            self.cancel_http(job)?;
        }
        Ok(())
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
    pub(crate) fn sanitise_http_result(&mut self, result: Value) -> Value {
        let status = result["status"].as_u64().unwrap_or(0) as u16;
        if let Some(stream) = result.get("stream").and_then(Value::as_u64) {
            if result.get("secret_count").and_then(Value::as_u64)
                != Some((self.sensitive_values.len() + self.sensitive_bytes.len()) as u64)
            {
                let _ = self.close_native_resource(stream);
                return failure("HttpSecretContextChanged", "ResponseReceived", status);
            }
        }
        match self.protect_http_response(&result) {
            Ok(false) => result,
            Ok(true) => failure("HttpSecretResponse", "ResponseReceived", status),
            Err(crate::Error::HistoryBudgetExceeded) => {
                failure("HttpMemoryLimit", "ResponseReceived", status)
            }
            Err(_) => failure("HttpWorkLimit", "ResponseReceived", status),
        }
    }
    pub fn cancel_http(&mut self, id: usize) -> crate::Result<()> {
        if self.external_operation_pending(id) {
            if self
                .http_server_host
                .as_ref()
                .is_some_and(|h| h.contains_job(id))
            {
                let result = self.http_server_host.as_mut().unwrap().cancel(id);
                self.capture_live_resources(id, &result);
                let result = self.sanitise_http_server_result(result);
                self.finish_async_external(id, Ok(result))?;
                return Ok(());
            }
            if self.tcp_host.as_ref().is_some_and(|h| h.contains_job(id)) {
                let result = self.tcp_host.as_mut().unwrap().cancel(id);
                self.capture_live_resources(id, &result);
                let result = self.sanitise_tcp_result(result);
                self.finish_async_external(id, Ok(result))?;
                return Ok(());
            }
            if self
                .database_host
                .as_ref()
                .is_some_and(|h| h.contains_job(id))
            {
                let result = self.database_host.as_mut().unwrap().cancel(id);
                self.finish_async_external(id, Ok(result))?;
                return Ok(());
            }
            let mut result = self
                .network_host
                .as_mut()
                .map_or_else(|| failure("HttpCancelled", "NotSent", 0), |h| h.cancel(id));
            self.capture_live_resources(id, &result);
            result = self.sanitise_http_result(result);
            self.finish_async_external(id, Ok(result))?;
        }
        Ok(())
    }
}

pub(crate) const MAX_BODY: usize = 4 * 1024 * 1024;
const MAX_HEADERS: usize = 32 * 1024;
const MAX_JOBS: usize = 8;
const FRAME: usize = 256 * 1024;
const CHUNK: usize = 64 * 1024;
fn response_reservation(limit: usize) -> usize {
    limit + limit.div_ceil(3) * 4 + 2 * MAX_HEADERS + 4096
}
fn download_reservation(pattern_bytes: usize) -> usize {
    2 * FRAME + MAX_HEADERS + 65536 + pattern_bytes * 96 + 4096
}
pub(crate) struct SecretMatcher {
    pub(crate) scan: Box<dyn FnMut(&[u8]) -> bool + Send>,
}
pub(crate) struct SecretAutomaton(Arc<aho_corasick::nfa::contiguous::NFA>);
impl SecretAutomaton {
    pub(crate) fn new(patterns: &[Vec<u8>]) -> Result<Self, &'static str> {
        use aho_corasick::automaton::Automaton;
        let size = patterns.iter().map(Vec::len).sum::<usize>();
        if size > 65536 {
            return Err("HttpSecretLimit");
        }
        let nfa = aho_corasick::nfa::contiguous::NFA::builder()
            .dense_depth(0)
            .prefilter(false)
            .build(patterns.iter().filter(|p| !p.is_empty()))
            .map_err(|_| "HttpSecretLimit")?;
        if nfa.memory_usage() > size * 96 + 4096 {
            return Err("HttpSecretLimit");
        }
        Ok(Self(Arc::new(nfa)))
    }
    pub(crate) fn matcher(&self) -> SecretMatcher {
        use aho_corasick::{automaton::Automaton, Anchored};
        let nfa = self.0.clone();
        let mut state = nfa.start_state(Anchored::No).expect("unanchored NFA");
        SecretMatcher {
            scan: Box::new(move |bytes| {
                for b in bytes {
                    state = nfa.next_state(Anchored::No, state, *b);
                    if nfa.is_match(state) {
                        return true;
                    }
                }
                false
            }),
        }
    }
}
impl SecretMatcher {
    fn new(patterns: &[Vec<u8>]) -> Result<Self, &'static str> {
        Ok(SecretAutomaton::new(patterns)?.matcher())
    }
}
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
    pub download: bool,
    pub upload_limit: Option<usize>,
}
impl Request {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.method.len() > 32
            || self.url.len() > 8192
            || self.body.len() > MAX_BODY
            || self.limit
                > if self.download {
                    1024 * 1024 * 1024
                } else {
                    MAX_BODY
                }
            || self.upload_limit.is_some_and(|n| {
                n == 0 || n > 1024 * 1024 * 1024 || self.download || !self.body.is_empty()
            })
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
        let mut value = json!([
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
        ]);
        // Preserve the v1.6 request fingerprint for existing traces.
        if self.download {
            value.as_array_mut().unwrap().push(json!(true));
        }
        if let Some(limit) = self.upload_limit {
            value.as_array_mut().unwrap().push(json!(["upload", limit]));
        }
        value.to_string().into_bytes()
    }
    pub fn reservation(&self) -> usize {
        (if self.download || self.upload_limit.is_some() {
            0
        } else {
            self.limit.min(MAX_BODY).div_ceil(3) * 4
        }) + MAX_HEADERS * 2
            + 4096
    }
}
struct Job {
    receiver: mpsc::Receiver<Value>,
    cancel: Option<oneshot::Sender<()>>,
    phase: Arc<AtomicU8>,
    bytes: usize,
    resource: Option<usize>,
    finishing: bool,
}
struct DownloadData {
    response: Option<reqwest::Response>,
    pending: VecDeque<u8>,
    eof: bool,
    total: usize,
    limit: usize,
    deadline: tokio::time::Instant,
    secrets: Vec<Vec<u8>>,
    hold: usize,
    status: u16,
    matcher: SecretMatcher,
}
struct Download {
    _client: Arc<client_pool::Lease>,
    data: Mutex<DownloadData>,
    busy: AtomicBool,
    closed: AtomicBool,
    secret_count: usize,
    reserved: usize,
}
pub(crate) struct Host {
    runtime: &'static tokio::runtime::Runtime,
    clients: client_pool::ClientPool,
    jobs: BTreeMap<usize, Job>,
    admission: usize,
    downloads: BTreeMap<usize, Arc<Download>>,
    retired: Vec<Arc<Download>>,
    uploads: BTreeMap<usize, Arc<upload::Upload>>,
    retired_uploads: Vec<Arc<upload::Upload>>,
}
pub(crate) fn failure(code: &str, phase: &str, status: u16) -> Value {
    json!({"error":{"code":code,"phase":phase,"status":status}})
}
pub(crate) fn protect_patterns(response: &Value, patterns: &[Vec<u8>]) -> bool {
    let Ok(automaton) = SecretAutomaton::new(patterns) else {
        return true;
    };
    protect_with_automaton(response, &automaton)
}
pub(crate) fn protect_with_automaton(response: &Value, automaton: &SecretAutomaton) -> bool {
    for header in response
        .get("headers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if header
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|s| (automaton.matcher().scan)(s.as_bytes()))
        {
            return true;
        }
        if header
            .get("value")
            .and_then(Value::as_str)
            .and_then(|s| STANDARD.decode(s).ok())
            .is_some_and(|bytes| (automaton.matcher().scan)(&bytes))
        {
            return true;
        }
    }
    response
        .get("body")
        .and_then(Value::as_str)
        .and_then(|s| STANDARD.decode(s).ok())
        .is_some_and(|bytes| (automaton.matcher().scan)(&bytes))
}
impl Host {
    pub fn new() -> Result<Self, &'static str> {
        static WORKERS: OnceLock<Result<tokio::runtime::Runtime, &'static str>> = OnceLock::new();
        let runtime = WORKERS
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .max_blocking_threads(2)
                    .thread_stack_size(1024 * 1024)
                    .enable_all()
                    .build()
                    .map_err(|_| "HttpWorker")
            })
            .as_ref()
            .map_err(|e| *e)?;
        Ok(Self {
            runtime,
            clients: client_pool::ClientPool::default(),
            jobs: BTreeMap::new(),
            admission: 0,
            downloads: BTreeMap::new(),
            retired: Vec::new(),
            uploads: BTreeMap::new(),
            retired_uploads: Vec::new(),
        })
    }
    pub fn reserved_bytes(&self) -> usize {
        self.admission
            + self.clients.reserved_bytes()
            + self.jobs.values().map(|j| j.bytes).sum::<usize>()
            + self
                .downloads
                .values()
                .chain(self.retired.iter())
                .map(|d| d.reserved)
                .sum::<usize>()
            + self
                .uploads
                .values()
                .chain(self.retired_uploads.iter())
                .map(|u| u.reserved())
                .sum::<usize>()
    }
    pub fn submit(
        &mut self,
        id: usize,
        request: Request,
        secrets: Vec<Vec<u8>>,
    ) -> Result<(), &'static str> {
        self.retired.retain(|slot| Arc::strong_count(slot) > 1);
        self.retired_uploads
            .retain(|slot| Arc::strong_count(slot) > 1);
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
        if request.download && self.downloads.len() + self.retired.len() >= 8 {
            return Err("HttpDownloadLimit");
        }
        let bytes = request.body.len()
            + (if request.download {
                0
            } else {
                response_reservation(request.limit)
            })
            + MAX_HEADERS;
        let client = self.clients.client(&request)?;
        let download = if request.download {
            let hold = secrets
                .iter()
                .map(Vec::len)
                .max()
                .unwrap_or(0)
                .saturating_sub(1);
            let slot = Arc::new(Download {
                _client: client.clone(),
                data: Mutex::new(DownloadData {
                    response: None,
                    pending: VecDeque::new(),
                    eof: false,
                    total: 0,
                    limit: request.limit,
                    deadline: tokio::time::Instant::now()
                        + Duration::from_millis(request.timeout_ms),
                    secrets: secrets.clone(),
                    hold,
                    status: 0,
                    matcher: SecretMatcher::new(&secrets)?,
                }),
                busy: AtomicBool::new(false),
                closed: AtomicBool::new(false),
                secret_count: secrets.len(),
                reserved: download_reservation(secrets.iter().map(Vec::len).sum()),
            });
            Some(slot)
        } else {
            None
        };
        let (sender, receiver) = mpsc::channel();
        let (cancel, cancelled) = oneshot::channel();
        let phase = Arc::new(AtomicU8::new(0));
        let worker_phase = phase.clone();
        if request.upload_limit.is_some() {
            return self.submit_upload(id, request, client);
        }
        if let Some(slot) = &download {
            self.downloads.insert(id, slot.clone());
        }
        self.runtime.spawn(async move {
            let future = async {
                if worker_phase.compare_exchange(0,1,Ordering::SeqCst,Ordering::SeqCst).is_err() {return failure("HttpCancelled","NotSent",0);}
                execute(client.client.clone(), request, &worker_phase,download.map(|d|(id,d))).await
            };
            let result = tokio::select! {
                biased;
                _ = cancelled => failure("HttpCancelled", if matches!(worker_phase.load(Ordering::SeqCst),0|3) {"NotSent"} else {"Unknown"}, 0),
                response = future => response,
            };
            let _ = sender.send(result);
            drop(client);
        });
        self.jobs.insert(
            id,
            Job {
                receiver,
                cancel: Some(cancel),
                phase,
                bytes,
                resource: None,
                finishing: false,
            },
        );
        Ok(())
    }
    pub fn read(
        &mut self,
        id: usize,
        stream: usize,
        limit: usize,
        secret_count: usize,
    ) -> Result<(), &'static str> {
        if !(1..=CHUNK).contains(&limit) {
            return Err("HttpChunkLimit");
        }
        if self.jobs.len() >= MAX_JOBS {
            return Err("HttpConcurrencyLimit");
        }
        let slot = self
            .downloads
            .get(&stream)
            .ok_or("HttpDownloadClosed")?
            .clone();
        if slot.closed.load(Ordering::SeqCst) {
            return Err("HttpDownloadClosed");
        }
        if slot.secret_count != secret_count {
            self.close(stream);
            return Err("HttpSecretContextChanged");
        }
        if slot.busy.swap(true, Ordering::SeqCst) {
            return Err("HttpDownloadBusy");
        }
        let (sender, receiver) = mpsc::channel();
        let (cancel, cancelled) = oneshot::channel();
        let phase = Arc::new(AtomicU8::new(2));
        self.runtime.spawn(async move {
            let result=tokio::select! {biased; _=cancelled=>failure("HttpCancelled","ResponseReceived",0), value=read_download(&slot,stream,limit)=>value};
            slot.busy.store(false,Ordering::SeqCst);let _=sender.send(result);
        });
        self.jobs.insert(
            id,
            Job {
                receiver,
                cancel: Some(cancel),
                phase,
                bytes: CHUNK * 2 + 4096,
                resource: Some(stream),
                finishing: false,
            },
        );
        Ok(())
    }
    pub fn close(&mut self, stream: usize) -> Vec<usize> {
        self.close_upload(stream);
        if let Some(slot) = self.downloads.remove(&stream) {
            slot.closed.store(true, Ordering::SeqCst);
            self.retired.push(slot.clone());
            self.runtime.spawn(async move {
                let mut data = slot.data.lock().await;
                data.response = None;
                data.pending.clear();
            });
        }
        self.jobs
            .iter()
            .filter_map(|(id, j)| (j.resource == Some(stream)).then_some(*id))
            .collect()
    }
    pub fn poll(&mut self, id: usize) -> Option<Value> {
        let job = self.jobs.get(&id)?;
        let finished = job.finishing.then_some(job.resource).flatten();
        let resource = job.resource;
        let value = match job.receiver.try_recv() {
            Ok(v) => v,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => failure("HttpWorker", "Unknown", 0),
        };
        self.jobs.remove(&id);
        if let Some(stream) = finished {
            self.close_upload(stream);
        }
        if value.get("error").is_some() {
            if let Some(stream) = resource {
                self.close(stream);
            }
            self.close(id);
        }
        Some(value)
    }
    pub fn cancel(&mut self, id: usize) -> Value {
        if let Some(stream) = self.jobs.get(&id).and_then(|j| j.resource) {
            if self.uploads.contains_key(&stream) {
                self.close_upload(stream);
            }
        }
        if self.downloads.contains_key(&id) || self.uploads.contains_key(&id) {
            self.close(id);
        }
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
        for slot in self.uploads.values().chain(self.retired_uploads.iter()) {
            slot.cancel_driver();
        }
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
async fn execute_with_body(
    client: reqwest::Client,
    request: Request,
    phase: &AtomicU8,
    download: Option<(usize, Arc<Download>)>,
    body: Option<reqwest::Body>,
) -> Value {
    let deadline = Duration::from_millis(request.timeout_ms);
    let run = async {
        let method =
            reqwest::Method::from_bytes(request.method.as_bytes()).expect("validated method");
        let mut builder = client
            .request(method, &request.url)
            .body(body.unwrap_or_else(|| {
                reqwest::Body::from(bytes::Bytes::from_owner(SharedBody(request.body.clone())))
            }));
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
                        crate::resolver::http_error(&e).unwrap_or("HttpTransport")
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
        if let Some((id, slot)) = download {
            let mut data = slot.data.lock().await;
            if slot.closed.load(Ordering::SeqCst) {
                return failure("HttpDownloadClosed", "ResponseReceived", status);
            }
            if response.headers().iter().any(|(n, v)| {
                data.secrets.iter().any(|s| {
                    !s.is_empty()
                        && (n.as_str().as_bytes().windows(s.len()).any(|w| w == s)
                            || v.as_bytes().windows(s.len()).any(|w| w == s))
                })
            }) {
                return failure("HttpSecretResponse", "ResponseReceived", status);
            }
            data.status = status;
            data.response = Some(response);
            return json!({"download":id,"status":status,"headers":headers});
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

async fn read_download(slot: &Download, stream: usize, limit: usize) -> Value {
    let mut data = slot.data.lock().await;
    let status = data.status;
    let deadline = data.deadline;
    let run = async {
        loop {
            if slot.closed.load(Ordering::SeqCst) {
                return failure("HttpDownloadClosed", "ResponseReceived", status);
            }
            let available = if data.eof {
                data.pending.len()
            } else {
                data.pending.len().saturating_sub(data.hold)
            };
            if available > 0 {
                let count = available.min(limit);
                let body =
                    STANDARD.encode(data.pending.iter().take(count).copied().collect::<Vec<_>>());
                data.pending.drain(..count);
                return json!({"stream":stream,"body":body,"secret_count":slot.secret_count});
            }
            if data.eof {
                return json!({"stream":stream,"body":null,"secret_count":slot.secret_count});
            }
            let Some(response) = data.response.as_mut() else {
                return failure("HttpDownloadClosed", "ResponseReceived", status);
            };
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if chunk.len() > data.limit.saturating_sub(data.total) {
                        return failure("HttpBodyLimit", "ResponseReceived", status);
                    }
                    if chunk.len() + data.pending.len() > FRAME {
                        return failure("HttpFrameLimit", "ResponseReceived", status);
                    }
                    data.total += chunk.len();
                    if (data.matcher.scan)(&chunk) {
                        return failure("HttpSecretResponse", "ResponseReceived", status);
                    }
                    data.pending.extend(chunk.iter().copied());
                }
                Ok(None) => {
                    data.eof = true;
                    data.response = None;
                }
                Err(_) => return failure("HttpTransport", "ResponseReceived", status),
            }
        }
    };
    let result = match tokio::time::timeout_at(deadline, run).await {
        Ok(v) => v,
        Err(_) => failure("HttpTimeout", "ResponseReceived", status),
    };
    if result.get("error").is_some() {
        data.response = None;
        data.pending.clear();
        slot.closed.store(true, Ordering::SeqCst);
    }
    result
}

async fn execute(
    client: reqwest::Client,
    request: Request,
    phase: &AtomicU8,
    download: Option<(usize, Arc<Download>)>,
) -> Value {
    execute_with_body(client, request, phase, download, None).await
}
