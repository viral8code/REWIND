//! HTTP/1 transport with bounded admission. Physical replies are not VM transactions.
use crate::{Error, Result, Runtime};
use base64::{engine::general_purpose::STANDARD, Engine};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{
    body::Incoming as Body, server::conn::http1, service::service_fn, Request, Response, StatusCode,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    convert::Infallible,
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as StdMutex, OnceLock,
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, Mutex, Semaphore},
    task::JoinHandle,
};
struct SharedBody(Arc<Vec<u8>>);
impl AsRef<[u8]> for SharedBody {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
const HEADERS: usize = 32768;
const WORKER: usize = 8 * 1024 * 1024;
const META: usize = 4096;
#[derive(Clone, Serialize, Deserialize)]
pub struct Limits {
    pub body_bytes: usize,
    pub connections: usize,
    pub lifetime_ms: u64,
}
impl Limits {
    fn valid(&self) -> bool {
        (1..=1024 * 1024).contains(&self.body_bytes)
            && (1..=8).contains(&self.connections)
            && (1..=120000).contains(&self.lifetime_ms)
    }
    fn memory(&self) -> usize {
        self.connections
            .saturating_mul(
                self.body_bytes
                    .saturating_mul(4)
                    .saturating_add(HEADERS * 2 + 65536),
            )
            .saturating_add(META)
    }
    fn result(&self) -> usize {
        self.body_bytes
            .saturating_mul(2)
            .saturating_add(HEADERS * 4 + META)
    }
}
pub(crate) const TLS_CONFIG_MEMORY: usize = 512 * 1024;
const TLS_CONNECTION_MEMORY: usize = 4 * 1024 * 1024;
#[derive(Clone)]
pub(crate) struct TlsCredential {
    certificate: Arc<Vec<u8>>,
    key_digest: [u8; 32],
    config: Arc<rustls::ServerConfig>,
}
trait ConnectionIo: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> ConnectionIo for T {}
#[derive(Clone, Serialize, Deserialize)]
pub enum Operation {
    ListenTls {
        address: String,
        port: i64,
        limits: Limits,
        credential: String,
    },
    Listen {
        address: String,
        port: i64,
        limits: Limits,
    },
    Next {
        server: usize,
        max_bytes: usize,
    },
    Respond {
        request: usize,
        status: i64,
        headers: Vec<(String, Vec<u8>)>,
        body: Arc<Vec<u8>>,
    },
    Close {
        resource: usize,
    },
}
impl Operation {
    fn size(&self) -> usize {
        match self {
            Self::Listen { address, .. } => address.len(),
            Self::ListenTls {
                address,
                credential,
                ..
            } => address.len() + credential.len(),
            Self::Respond { headers, body, .. } => {
                headers
                    .iter()
                    .map(|(n, v)| n.len() + v.len())
                    .sum::<usize>()
                    + body.len()
            }
            _ => 32,
        }
    }
    fn parent(&self) -> Option<usize> {
        match self {
            Self::Next { server, .. } => Some(*server),
            _ => None,
        }
    }
}
pub(crate) fn failure(code: &str, phase: &str) -> Value {
    json!({"adapter":"server","error":{"code":code,"phase":phase,"status":0}})
}
struct Packet {
    wire: Value,
    reply: oneshot::Sender<Response<Full<Bytes>>>,
}
struct Shared {
    closed: AtomicBool,
    next_busy: AtomicBool,
    connections: StdMutex<Vec<JoinHandle<()>>>,
}
impl Shared {
    fn close(&self) {
        let tasks = self.connections.lock().unwrap_or_else(|e| e.into_inner());
        self.closed.store(true, Ordering::SeqCst);
        for task in tasks.iter() {
            task.abort();
        }
    }
    fn reap(&self) {
        self.connections
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|task| !task.is_finished());
    }
    fn finished(&self) -> bool {
        self.connections
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .all(JoinHandle::is_finished)
    }
}
struct Listener {
    tls: Option<Arc<rustls::ServerConfig>>,
    limits: Limits,
    shared: Arc<Shared>,
    receiver: Arc<Mutex<mpsc::Receiver<Packet>>>,
    accept: Option<JoinHandle<()>>,
    socket: Option<TcpListener>,
    sender: Option<mpsc::Sender<Packet>>,
}
impl Listener {
    fn memory(&self) -> usize {
        self.limits.memory()
            + if self.tls.is_some() {
                self.limits.connections * TLS_CONNECTION_MEMORY
            } else {
                0
            }
    }
    // Binding alone cannot accept or spawn a connection. Activation happens only
    // after the completed listen operation is owned by the host.
    fn activate(&mut self, runtime: &tokio::runtime::Runtime) {
        let socket = self.socket.take().unwrap();
        let out = self.sender.take().unwrap();
        let peers = self.shared.clone();
        let cfg = self.limits.clone();
        let tls = self.tls.clone();
        let cap = Arc::new(Semaphore::new(cfg.connections));
        self.accept = Some(runtime.spawn(async move {
            loop {
                let Ok(permit) = cap.clone().acquire_owned().await else {
                    break;
                };
                let Ok((stream, _)) = socket.accept().await else {
                    break;
                };
                let mut tasks = peers.connections.lock().unwrap_or_else(|e| e.into_inner());
                if peers.closed.load(Ordering::SeqCst) {
                    break;
                }
                tasks.retain(|task| !task.is_finished());
                let out = out.clone();
                let cfg = cfg.clone();
                let tls = tls.clone();
                tasks.push(tokio::spawn(async move {
                    let _permit = permit;
                    let lifetime = Duration::from_millis(cfg.lifetime_ms);
                    let connection = async move {
                        let stream: Box<dyn ConnectionIo> = if let Some(config) = tls {
                            match tokio_rustls::TlsAcceptor::from(config).accept(stream).await {
                                Ok(stream) => Box::new(stream),
                                Err(_) => return,
                            }
                        } else {
                            Box::new(stream)
                        };
                        let service =
                            service_fn(move |request| dispatch(request, out.clone(), cfg.clone()));
                        let mut builder = http1::Builder::new();
                        builder
                            .timer(TokioTimer::new())
                            .header_read_timeout(lifetime)
                            .max_headers(128)
                            .max_buf_size(HEADERS);
                        let _ = builder
                            .serve_connection(TokioIo::new(stream), service)
                            .await;
                    };
                    let _ = tokio::time::timeout(lifetime, connection).await;
                }));
            }
        }));
    }
    fn shutdown(&mut self) {
        self.shared.close();
        if let Some(accept) = &self.accept {
            accept.abort();
        }
        self.socket.take();
        self.sender.take();
        if let Ok(mut receiver) = self.receiver.try_lock() {
            receiver.close();
            while receiver.try_recv().is_ok() {}
        }
    }
    fn finished(&self) -> bool {
        self.accept.as_ref().is_none_or(JoinHandle::is_finished) && self.shared.finished()
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.shutdown();
    }
}
struct ReplySlot {
    server: usize,
    reply: oneshot::Sender<Response<Full<Bytes>>>,
    limit: usize,
}
struct Completed {
    wire: Value,
    listener: Option<Listener>,
    request: Option<ReplySlot>,
}
struct Job {
    receiver: std::sync::mpsc::Receiver<Completed>,
    handle: JoinHandle<()>,
    parent: Option<usize>,
    shared: Option<Arc<Shared>>,
    memory: usize,
    closed: bool,
    creates_listener: bool,
}
pub(crate) struct Host {
    runtime: &'static tokio::runtime::Runtime,
    listeners: BTreeMap<usize, Listener>,
    requests: BTreeMap<usize, ReplySlot>,
    jobs: BTreeMap<usize, Job>,
    retired: Vec<(JoinHandle<()>, usize)>,
    retired_listeners: Vec<Listener>,
    pub(crate) admission: usize,
}
impl Host {
    pub(crate) fn new() -> std::result::Result<Self, &'static str> {
        static RUNTIME: OnceLock<std::result::Result<tokio::runtime::Runtime, &'static str>> =
            OnceLock::new();
        let runtime = RUNTIME
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .max_blocking_threads(1)
                    .thread_stack_size(1024 * 1024)
                    .enable_all()
                    .build()
                    .map_err(|_| "HttpServerWorker")
            })
            .as_ref()
            .map_err(|e| *e)?;
        Ok(Self {
            runtime,
            listeners: BTreeMap::new(),
            requests: BTreeMap::new(),
            jobs: BTreeMap::new(),
            retired: Vec::new(),
            retired_listeners: Vec::new(),
            admission: 0,
        })
    }
    pub(crate) fn contains_job(&self, id: usize) -> bool {
        self.jobs.contains_key(&id)
    }
    pub(crate) fn contains_resource(&self, id: usize) -> bool {
        self.listeners.contains_key(&id) || self.requests.contains_key(&id)
    }
    pub(crate) fn owns_pending_server(&self, id: usize) -> bool {
        self.jobs.values().any(|job| job.parent == Some(id))
    }
    pub(crate) fn resource_ids(&self) -> impl Iterator<Item = &usize> {
        self.listeners.keys().chain(self.requests.keys())
    }
    pub(crate) fn reserved_bytes(&self) -> usize {
        WORKER
            + self.admission
            + self.listeners.values().map(Listener::memory).sum::<usize>()
            + self.requests.len() * META
            + self.jobs.values().map(|j| j.memory).sum::<usize>()
            + self
                .retired
                .iter()
                .filter(|(h, _)| !h.is_finished())
                .map(|(_, bytes)| bytes)
                .sum::<usize>()
            + self.retired.len() * 64
            + self
                .retired_listeners
                .iter()
                .filter(|l| !l.finished())
                .map(Listener::memory)
                .sum::<usize>()
            + self.retired_listeners.len() * META
    }
    fn reap(&mut self) {
        self.retired.retain(|(task, _)| !task.is_finished());
        self.retired_listeners
            .retain(|listener| !listener.finished());
        for listener in self.listeners.values() {
            listener.shared.reap();
        }
    }

    #[cfg(test)]
    pub(crate) fn submit(
        &mut self,
        id: usize,
        operation: Operation,
        timeout: u64,
    ) -> std::result::Result<(), &'static str> {
        self.submit_with_tls(id, operation, timeout, None)
    }
    fn submit_with_tls(
        &mut self,
        id: usize,
        operation: Operation,
        timeout: u64,
        tls: Option<Arc<rustls::ServerConfig>>,
    ) -> std::result::Result<(), &'static str> {
        self.reap();
        if timeout == 0 || timeout > 120000 {
            return Err("HttpServerDeadline");
        }
        if self.jobs.len() + self.retired.len() >= 8 {
            return Err("HttpServerBusy");
        }
        let creates_listener = matches!(
            operation,
            Operation::Listen { .. } | Operation::ListenTls { .. }
        );
        let parent = operation.parent();
        let mut shared = None;
        let mut receiver = None;
        let mut limit = 0;
        let memory = match &operation {
            Operation::Listen {
                address,
                port,
                limits,
            }
            | Operation::ListenTls {
                address,
                port,
                limits,
                ..
            } => {
                if self.listeners.len()
                    + self.retired_listeners.len()
                    + self.jobs.values().filter(|j| j.creates_listener).count()
                    >= 4
                {
                    return Err("HttpServerLimit");
                }
                if !limits.valid() {
                    return Err("HttpServerLimit");
                }
                address.parse::<IpAddr>().map_err(|_| "HttpServerAddress")?;
                u16::try_from(*port).map_err(|_| "HttpServerAddress")?;
                if matches!(operation, Operation::ListenTls { .. }) && tls.is_none() {
                    return Err("HttpServerTlsCredentialUnknown");
                }
                limits.memory()
                    + META
                    + if tls.is_some() {
                        limits.connections * TLS_CONNECTION_MEMORY
                    } else {
                        0
                    }
            }
            Operation::Next { server, max_bytes } => {
                let listener = self.listeners.get(server).ok_or("HttpServerClosed")?;
                if *max_bytes != listener.limits.body_bytes {
                    return Err("HttpServerLimit");
                }
                if listener.shared.closed.load(Ordering::SeqCst) {
                    return Err("HttpServerClosed");
                }
                if listener.shared.next_busy.swap(true, Ordering::SeqCst) {
                    return Err("HttpServerBusy");
                }
                shared = Some(listener.shared.clone());
                receiver = Some(listener.receiver.clone());
                limit = listener.limits.body_bytes;
                listener.limits.result()
            }
            Operation::Respond {
                request,
                status,
                headers,
                body,
            } => {
                let slot = self
                    .requests
                    .get(request)
                    .ok_or("HttpServerRequestClosed")?;
                if slot.reply.is_closed() {
                    return Err("HttpServerRequestClosed");
                }
                if !(200..=599).contains(status)
                    || body.len() > slot.limit
                    || headers.len() > 128
                    || headers
                        .iter()
                        .map(|(n, v)| n.len() + v.len())
                        .sum::<usize>()
                        > HEADERS
                {
                    return Err("HttpServerResponse");
                }
                if (*status == 204 || *status == 304) && !body.is_empty() {
                    return Err("HttpServerResponse");
                }
                if headers.iter().any(|(name, _)| {
                    [
                        "connection",
                        "content-length",
                        "transfer-encoding",
                        "upgrade",
                    ]
                    .contains(&name.to_ascii_lowercase().as_str())
                }) {
                    return Err("HttpServerFraming");
                }
                for (name, value) in headers {
                    hyper::header::HeaderName::from_bytes(name.as_bytes())
                        .map_err(|_| "HttpServerHeader")?;
                    hyper::header::HeaderValue::from_bytes(value)
                        .map_err(|_| "HttpServerHeader")?;
                }
                // Sending consumes the one response slot; no historic request tombstone is kept.
                let slot = self.requests.remove(request).unwrap();
                let mut response =
                    Response::new(Full::new(Bytes::from_owner(SharedBody(body.clone()))));
                *response.status_mut() =
                    StatusCode::from_u16(*status as u16).map_err(|_| "HttpServerResponse")?;
                for (name, value) in headers {
                    response.headers_mut().append(
                        hyper::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                        hyper::header::HeaderValue::from_bytes(value).unwrap(),
                    );
                }
                let wire = match slot.reply.send(response) {
                    Ok(()) => json!({"adapter":"server","replied":true}),
                    Err(_) => failure("HttpServerRequestClosed", "NotSent"),
                };
                return self.ready(id, wire);
            }
            Operation::Close { .. } => return Err("HttpServerClosed"),
        };
        let (sender, rx) = std::sync::mpsc::channel();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout);
        let handle=self.runtime.spawn(async move {
   let waiting=async {
    match operation {
     Operation::Listen{address,port,limits}|Operation::ListenTls{address,port,limits,..}=>{
      let address=SocketAddr::new(address.parse::<IpAddr>().unwrap(),port as u16);
      match TcpListener::bind(address).await {
       Err(_)=>Completed{wire:failure("HttpServerBind","NotStarted"),listener:None,request:None},
       Ok(socket)=>{
        let address=socket.local_addr().unwrap();let (out,requests)=mpsc::channel(limits.connections);
        let shared=Arc::new(Shared{closed:AtomicBool::new(false),next_busy:AtomicBool::new(false),connections:StdMutex::new(Vec::new())});
        Completed{wire:json!({"adapter":"server","server":id,"address":address.ip().to_string(),"port":address.port(),"maxBytes":limits.body_bytes}),listener:Some(Listener{tls,limits,shared,receiver:Arc::new(Mutex::new(requests)),accept:None,socket:Some(socket),sender:Some(out)}),request:None}
       }
      }
     },
     Operation::Next{server,..}=>{
      let receiver=receiver.unwrap();let mut receiver=receiver.lock().await;
      match receiver.recv().await {
       None=>Completed{wire:failure("HttpServerClosed","NotReceived"),listener:None,request:None},
       Some(mut packet)=>{
        packet.wire["request"]=json!(id);
        Completed{wire:packet.wire,listener:None,request:Some(ReplySlot{server,reply:packet.reply,limit})}
       }
      }
     },
     _=>unreachable!(),
    }
   };
   let completed=tokio::time::timeout_at(deadline,waiting).await.unwrap_or_else(|_|Completed{wire:failure("HttpServerDeadline","Unknown"),listener:None,request:None});
   let _=sender.send(completed);
  });
        self.jobs.insert(
            id,
            Job {
                receiver: rx,
                handle,
                parent,
                shared,
                memory,
                closed: false,
                creates_listener,
            },
        );
        Ok(())
    }
    fn ready(&mut self, id: usize, wire: Value) -> std::result::Result<(), &'static str> {
        let (sender, receiver) = std::sync::mpsc::channel();
        sender
            .send(Completed {
                wire,
                listener: None,
                request: None,
            })
            .map_err(|_| "HttpServerWorker")?;
        self.jobs.insert(
            id,
            Job {
                receiver,
                handle: self.runtime.spawn(async {}),
                parent: None,
                shared: None,
                memory: META,
                closed: false,
                creates_listener: false,
            },
        );
        Ok(())
    }
    pub(crate) fn poll(&mut self, id: usize) -> Option<Value> {
        self.reap();
        let job = self.jobs.get(&id)?;
        let result = match job.receiver.try_recv() {
            Ok(value) => value,
            Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Completed {
                wire: failure(
                    if job.closed {
                        "HttpServerClosed"
                    } else {
                        "HttpServerWorker"
                    },
                    "Unknown",
                ),
                listener: None,
                request: None,
            },
        };
        let job = self.jobs.remove(&id).unwrap();
        if let Some(shared) = job.shared {
            shared.next_busy.store(false, Ordering::SeqCst);
        }
        let mut memory = job.memory;
        if let Some(mut listener) = result.listener {
            memory = META;
            listener.activate(self.runtime);
            self.listeners.insert(id, listener);
        }
        if let Some(request) = result.request {
            self.requests.insert(id, request);
        }
        self.retired.push((job.handle, memory));
        Some(result.wire)
    }
    pub(crate) fn cancel(&mut self, id: usize) -> Value {
        if let Some(value) = self.poll(id) {
            return value;
        }
        let Some(job) = self.jobs.remove(&id) else {
            return failure("HttpServerClosed", "NotStarted");
        };
        if let Some(shared) = job.shared {
            shared.next_busy.store(false, Ordering::SeqCst);
        }
        job.handle.abort();
        self.retired.push((job.handle, job.memory));
        failure(
            if job.closed {
                "HttpServerClosed"
            } else {
                "HttpServerCancelled"
            },
            "Unknown",
        )
    }
    pub(crate) fn close(&mut self, id: usize) -> Vec<usize> {
        if let Some(mut listener) = self.listeners.remove(&id) {
            listener.shutdown();
            self.retired_listeners.push(listener);
            self.requests.retain(|_, request| request.server != id);
            let mut jobs = Vec::new();
            for (job_id, job) in &mut self.jobs {
                if job.parent == Some(id) {
                    job.closed = true;
                    job.handle.abort();
                    jobs.push(*job_id);
                }
            }
            return jobs;
        }
        self.requests.remove(&id);
        Vec::new()
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        for job in self.jobs.values() {
            job.handle.abort();
        }
        for (task, _) in &self.retired {
            task.abort();
        }
        self.listeners.clear();
        self.retired_listeners.clear();
        self.requests.clear();
    }
}
fn response(status: StatusCode) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::new()));
    *response.status_mut() = status;
    response
}
async fn dispatch(
    request: Request<Body>,
    out: mpsc::Sender<Packet>,
    limits: Limits,
) -> std::result::Result<Response<Full<Bytes>>, Infallible> {
    let (parts, body) = request.into_parts();
    let target = parts.uri.to_string();
    if target.len() > 8192 || parts.method.as_str().len() > 32 {
        return Ok(response(StatusCode::URI_TOO_LONG));
    }
    if parts.headers.len() > 128
        || parts
            .headers
            .iter()
            .map(|(n, v)| n.as_str().len() + v.as_bytes().len())
            .sum::<usize>()
            > HEADERS
    {
        return Ok(response(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE));
    }
    let body = match tokio::time::timeout(
        Duration::from_millis(limits.lifetime_ms),
        Limited::new(body, limits.body_bytes).collect(),
    )
    .await
    {
        Err(_) => return Ok(response(StatusCode::REQUEST_TIMEOUT)),
        Ok(Err(_)) => return Ok(response(StatusCode::PAYLOAD_TOO_LARGE)),
        Ok(Ok(body)) => body.to_bytes(),
    };
    let headers = parts
        .headers
        .iter()
        .map(
            |(name, value)| json!({"name":name.as_str(),"value":STANDARD.encode(value.as_bytes())}),
        )
        .collect::<Vec<_>>();
    let (reply, answer) = oneshot::channel();
    let wire = json!({"adapter":"server","method":parts.method.as_str(),"target":target,"path":parts.uri.path(),"query":parts.uri.query().unwrap_or(""),"headers":headers,"body":STANDARD.encode(body)});
    let packet = Packet { wire, reply };
    if out.send(packet).await.is_err() {
        return Ok(response(StatusCode::SERVICE_UNAVAILABLE));
    }
    let value = match tokio::time::timeout(Duration::from_millis(limits.lifetime_ms), answer).await
    {
        Ok(Ok(response)) => response,
        _ => response(StatusCode::SERVICE_UNAVAILABLE),
    };
    Ok(value)
}
impl Runtime {
    fn protect_http_server_fields(&mut self, fields: &[&[u8]]) -> Result<()> {
        self.protect_tcp_request(fields)
            .map_err(|error| match error {
                Error::InvalidOperation(message) => {
                    Error::InvalidOperation(message.replace("TcpSecret", "HttpServerSecret"))
                }
                other => other,
            })
    }
    pub(crate) fn sanitise_http_server_result(&mut self, result: Value) -> Value {
        let Some(request) = result["request"].as_u64() else {
            return result;
        };
        if self.sensitive_values.is_empty() && self.sensitive_bytes.is_empty() {
            return result;
        }
        let checked = (|| -> Result<()> {
            let bytes = result["body"].as_str().map_or(0, str::len)
                + result["headers"].as_array().map_or(0, |h| {
                    h.iter()
                        .map(|h| h["value"].as_str().map_or(0, str::len))
                        .sum::<usize>()
                });
            self.check_native_allocation(bytes.saturating_mul(2).saturating_add(4096))?;
            let mut owned = Vec::new();
            owned.push(
                STANDARD
                    .decode(result["body"].as_str().unwrap_or(""))
                    .map_err(|_| Error::InvalidOperation("HttpServerEncoding".into()))?,
            );
            if let Some(headers) = result["headers"].as_array() {
                for header in headers {
                    owned.push(
                        STANDARD
                            .decode(header["value"].as_str().unwrap_or(""))
                            .map_err(|_| Error::InvalidOperation("HttpServerEncoding".into()))?,
                    );
                }
            }
            let mut fields = owned.iter().map(Vec::as_slice).collect::<Vec<_>>();
            for key in ["method", "target", "path", "query"] {
                fields.push(result[key].as_str().unwrap_or("").as_bytes());
            }
            if let Some(headers) = result["headers"].as_array() {
                for header in headers {
                    fields.push(header["name"].as_str().unwrap_or("").as_bytes());
                }
            }
            self.protect_http_server_fields(&fields)
        })();
        match checked {
            Ok(()) => result,
            Err(error) => {
                let _ = self.close_native_resource(request);
                failure(
                    if matches!(error, Error::InvalidOperation(_)) {
                        "HttpServerSecretRequest"
                    } else {
                        "HttpServerMemoryLimit"
                    },
                    "Received",
                )
            }
        }
    }
    /// Private material is retained outside VM checkpoints; operation fingerprints contain only an immutable alias.
    pub fn register_http_server_tls(
        &mut self,
        alias: &str,
        certificate: &[u8],
        key: &[u8],
    ) -> Result<std::result::Result<(), &'static str>> {
        use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
        use sha2::{Digest, Sha256};
        if self.external_depth != 1 {
            return Err(Error::InvalidOperation(
                "ExternalBoundary: server TLS credentials require external region".into(),
            ));
        }
        self.charge_native_work(
            certificate
                .len()
                .saturating_add(key.len())
                .saturating_mul(64)
                .saturating_add(1024),
        )?;
        if alias.is_empty()
            || alias.len() > 128
            || !alias
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            || certificate.is_empty()
            || certificate.len() > 65536
            || key.is_empty()
            || key.len() > 65536
        {
            return Ok(Err("HttpServerTlsCredential"));
        }
        self.protect_http_server_fields(&[alias.as_bytes()])?;
        let digest: [u8; 32] = Sha256::digest(key).into();
        if let Some(old) = self.http_server_tls_credentials.get(alias) {
            return Ok(
                if old.certificate.as_slice() == certificate && old.key_digest == digest {
                    Ok(())
                } else {
                    Err("HttpServerTlsCredentialImmutable")
                },
            );
        }
        if self.http_server_tls_credentials.len() >= 16 {
            return Ok(Err("HttpServerTlsCredentialLimit"));
        }
        self.check_native_allocation(TLS_CONFIG_MEMORY)?;
        let parsed = (|| {
            let certificates = CertificateDer::pem_slice_iter(certificate)
                .take(9)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| "HttpServerTlsCredential")?;
            if certificates.is_empty() || certificates.len() > 8 {
                return Err("HttpServerTlsCredential");
            }
            let key = PrivateKeyDer::from_pem_slice(key).map_err(|_| "HttpServerTlsCredential")?;
            let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|_| "HttpServerTlsCredential")?
            .with_no_client_auth()
            .with_single_cert(certificates, key)
            .map_err(|_| "HttpServerTlsCredential")?;
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            Ok(TlsCredential {
                certificate: Arc::new(certificate.to_vec()),
                key_digest: digest,
                config: Arc::new(config),
            })
        })();
        match parsed {
            Err(code) => Ok(Err(code)),
            Ok(credentials) => {
                self.http_server_tls_credentials
                    .insert(alias.into(), credentials);
                if let Err(error) = self.enforce_budget() {
                    self.http_server_tls_credentials.remove(alias);
                    return Err(error);
                }
                self.register_secret_value(&crate::Value::Bytes(Arc::new(key.to_vec())));
                Ok(Ok(()))
            }
        }
    }
    pub fn start_http_server(&mut self, operation: Operation, timeout: u64) -> Result<usize> {
        let size = operation.size();
        self.charge_native_work(size.saturating_mul(4).saturating_add(1))?;
        self.check_native_allocation(size.saturating_mul(6).saturating_add(8192))?;
        match &operation {
            Operation::Listen { address, .. } => {
                self.protect_http_server_fields(&[address.as_bytes()])?
            }
            Operation::ListenTls {
                address,
                credential,
                ..
            } => self.protect_http_server_fields(&[address.as_bytes(), credential.as_bytes()])?,
            Operation::Respond { headers, body, .. } => {
                let mut fields = Vec::with_capacity(headers.len() * 2 + 1);
                fields.push(body.as_slice());
                for (name, value) in headers {
                    fields.push(name.as_bytes());
                    fields.push(value.as_slice());
                }
                self.protect_http_server_fields(&fields)?;
            }
            _ => {}
        }
        let request = serde_json::to_vec(&(operation.clone(), timeout))
            .map_err(|_| Error::InvalidOperation("HttpServerEncoding".into()))?;
        let reserve = match &operation {
            Operation::Next { max_bytes, .. } => max_bytes
                .saturating_mul(2)
                .saturating_add(HEADERS * 4 + META),
            _ => META,
        };
        let (id, fresh) =
            self.begin_async_external("http.server.operation.v1", &request, reserve)?;
        if fresh {
            let submitted = (|| -> std::result::Result<(), &'static str> {
                if timeout == 0 || timeout > 120000 {
                    return Err("HttpServerDeadline");
                }
                match &operation {
                    Operation::Listen {
                        address,
                        port,
                        limits,
                    }
                    | Operation::ListenTls {
                        address,
                        port,
                        limits,
                        ..
                    } => {
                        if !limits.valid() {
                            return Err("HttpServerLimit");
                        }
                        address.parse::<IpAddr>().map_err(|_| "HttpServerAddress")?;
                        u16::try_from(*port).map_err(|_| "HttpServerAddress")?;
                    }
                    Operation::Next { max_bytes, .. } if !(1..=1024 * 1024).contains(max_bytes) => {
                        return Err("HttpServerLimit")
                    }
                    _ => {}
                }
                if let Operation::Close { resource } = operation {
                    if !self
                        .http_server_host
                        .as_ref()
                        .is_some_and(|host| host.contains_resource(resource))
                    {
                        return Err("HttpServerClosed");
                    }
                    self.close_native_resource(resource as u64)
                        .map_err(|_| "HttpServerClose")?;
                    self.finish_async_external(id, Ok(json!({"adapter":"server","closed":true})))
                        .map_err(|_| "HttpServerRecording")?;
                    return Ok(());
                }
                let tls = if let Operation::ListenTls { credential, .. } = &operation {
                    Some(
                        self.http_server_tls_credentials
                            .get(credential)
                            .ok_or("HttpServerTlsCredentialUnknown")?
                            .config
                            .clone(),
                    )
                } else {
                    None
                };
                if self.http_server_host.is_none() {
                    self.check_native_allocation(WORKER)
                        .map_err(|_| "HttpServerMemoryLimit")?;
                    self.http_server_host = Some(Host::new()?)
                }
                let admission = match &operation {
                    Operation::Listen { limits, .. } => limits.memory() + META,
                    Operation::ListenTls { limits, .. } => {
                        limits.memory() + META + limits.connections * TLS_CONNECTION_MEMORY
                    }
                    Operation::Respond { body, .. } => body.len() + META,
                    _ => reserve,
                };
                self.http_server_host.as_mut().unwrap().reap();
                self.http_server_host.as_mut().unwrap().admission = admission;
                let budget = self.enforce_budget();
                self.http_server_host.as_mut().unwrap().admission = 0;
                budget.map_err(|_| "HttpServerMemoryLimit")?;
                self.http_server_host
                    .as_mut()
                    .unwrap()
                    .submit_with_tls(id, operation, timeout, tls)
            })();
            if let Err(code) = submitted {
                self.finish_async_external(id, Ok(failure(code, "NotStarted")))?;
            }
        }
        Ok(id)
    }
}
#[cfg(test)]
mod tests;
