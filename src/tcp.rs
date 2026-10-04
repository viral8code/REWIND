//! Bounded TCP transport. VM observations and physical socket lifetime are separate.
use crate::{network::SecretMatcher, Error, Result, Runtime};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Mutex as StdMutex, OnceLock,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::Mutex,
    task::JoinHandle,
};
const FRAME: usize = 65536;
const SOCKET_MEMORY: usize = 1024 * 1024;
const JOB_MEMORY: usize = 2 * FRAME + 4096;
#[derive(Clone, Serialize, Deserialize)]
pub enum Operation {
    Connect { host: String, port: u16 },
    Read { socket: usize, limit: usize },
    Write { socket: usize, body: Arc<Vec<u8>> },
    ShutdownWrite { socket: usize },
    Close { socket: usize },
}
impl Operation {
    fn socket(&self) -> Option<usize> {
        match self {
            Self::Connect { .. } => None,
            Self::Read { socket, .. }
            | Self::Write { socket, .. }
            | Self::ShutdownWrite { socket }
            | Self::Close { socket } => Some(*socket),
        }
    }
    fn read(&self) -> bool {
        matches!(self, Self::Read { .. })
    }
    fn reserve(&self) -> usize {
        if let Self::Read { limit, .. } = self {
            limit.min(&FRAME).div_ceil(3) * 4 + 4096
        } else {
            4096
        }
    }
    fn size(&self) -> usize {
        match self {
            Self::Connect { host, .. } => host.len(),
            Self::Write { body, .. } => body.len(),
            _ => 32,
        }
    }
    fn validate(&self, timeout: u64) -> std::result::Result<(), &'static str> {
        if timeout == 0 || timeout > 120000 {
            return Err("TcpDeadline");
        }
        match self {
            Self::Connect { host, port }
                if host.is_empty()
                    || host.len() > 253
                    || *port == 0
                    || host.chars().any(|c| c.is_whitespace() || c == '\0') =>
            {
                Err("TcpAddress")
            }
            Self::Read { limit, .. } if *limit == 0 || *limit > FRAME => Err("TcpLimit"),
            Self::Write { body, .. } if body.len() > FRAME => Err("TcpLimit"),
            _ => Ok(()),
        }
    }
}
pub(crate) fn failure(code: &str, phase: &str, accepted: usize) -> Value {
    json!({"adapter":"tcp","error":{"code":code,"phase":phase,"acceptedBytes":accepted}})
}
struct Socket {
    read: Arc<Mutex<OwnedReadHalf>>,
    write: Arc<Mutex<OwnedWriteHalf>>,
    matcher: Arc<StdMutex<SecretMatcher>>,
    secret_count: usize,
    matcher_memory: usize,
    read_busy: bool,
    write_busy: bool,
    write_closed: bool,
}
struct Completed {
    wire: Value,
    opened: Option<Socket>,
}
struct Job {
    receiver: mpsc::Receiver<Completed>,
    handle: JoinHandle<()>,
    socket: Option<usize>,
    read: bool,
    accepted: Arc<AtomicUsize>,
    closed: bool,
    memory: usize,
}
pub(crate) struct Host {
    runtime: &'static tokio::runtime::Runtime,
    sockets: BTreeMap<usize, Socket>,
    jobs: BTreeMap<usize, Job>,
    retired: Vec<(JoinHandle<()>, usize)>,
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
                    .enable_all()
                    .build()
                    .map_err(|_| "TcpWorker")
            })
            .as_ref()
            .map_err(|e| *e)?;
        Ok(Self {
            runtime,
            sockets: BTreeMap::new(),
            jobs: BTreeMap::new(),
            retired: Vec::new(),
            admission: 0,
        })
    }
    pub(crate) fn contains_socket(&self, id: usize) -> bool {
        self.sockets.contains_key(&id)
    }
    pub(crate) fn owns_pending_socket(&self, id: usize) -> bool {
        self.jobs.contains_key(&id)
    }
    pub(crate) fn contains_job(&self, id: usize) -> bool {
        self.jobs.contains_key(&id)
    }
    pub(crate) fn resource_ids(&self) -> impl Iterator<Item = &usize> {
        self.sockets.keys()
    }
    pub(crate) fn reserved_bytes(&self) -> usize {
        8 * 1024 * 1024
            + self.admission
            + self
                .sockets
                .values()
                .map(|s| SOCKET_MEMORY + s.matcher_memory)
                .sum::<usize>()
            + self.jobs.values().map(|j| j.memory).sum::<usize>()
            + self.retired.iter().map(|(_, memory)| memory).sum::<usize>()
    }
    fn reap(&mut self) {
        self.retired.retain(|(task, _)| !task.is_finished());
    }
    pub(crate) fn submit(
        &mut self,
        id: usize,
        operation: Operation,
        timeout: u64,
        patterns: &[Vec<u8>],
    ) -> std::result::Result<(), &'static str> {
        self.reap();
        operation.validate(timeout)?;
        if self.jobs.len() + self.retired.len() >= 8 {
            return Err("TcpBusy");
        }
        let socket = operation.socket();
        let reading = operation.read();
        if matches!(operation, Operation::Connect { .. })
            && self.sockets.len() + self.jobs.values().filter(|j| j.socket.is_none()).count() >= 8
        {
            return Err("TcpSocketLimit");
        }
        let matcher_memory = patterns
            .iter()
            .map(Vec::len)
            .sum::<usize>()
            .saturating_mul(96)
            .saturating_add(4096);
        let transport = if let Some(socket) = socket {
            let connection = self.sockets.get_mut(&socket).ok_or("TcpClosed")?;
            if (reading && connection.read_busy) || (!reading && connection.write_busy) {
                return Err("TcpBusy");
            }
            if !reading
                && connection.write_closed
                && matches!(
                    operation,
                    Operation::Write { .. } | Operation::ShutdownWrite { .. }
                )
            {
                return Err("TcpWriteClosed");
            }
            if patterns.len() != connection.secret_count {
                self.close(socket);
                return Err("TcpSecretContextChanged");
            }
            if reading {
                connection.read_busy = true;
            } else {
                connection.write_busy = true;
            }
            Some((
                connection.read.clone(),
                connection.write.clone(),
                connection.matcher.clone(),
            ))
        } else {
            None
        };
        let (sender, receiver) = mpsc::channel();
        let accepted = Arc::new(AtomicUsize::new(0));
        let count = accepted.clone();
        let secret_count = patterns.len();
        // Only connection creation consumes a fresh matcher. A socket read retains its rolling state.
        let connect_matcher = if transport.is_none() {
            Some(
                crate::network::SecretAutomaton::new(patterns)
                    .map_err(|_| "TcpSecretLimit")?
                    .matcher(),
            )
        } else {
            None
        };
        let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout);
        let handle=self.runtime.spawn(async move {
            let future=async {
                match operation {
                    Operation::Connect{host,port}=>match TcpStream::connect((host.as_str(),port)).await {
                        Err(_)=>Completed{wire:failure("TcpConnect","NotConnected",0),opened:None},
                        Ok(stream)=>{
                            let peer=stream.peer_addr().map(|a|a.to_string()).unwrap_or_default();
                            let _=stream.set_nodelay(true);let (read,write)=stream.into_split();
                            Completed{wire:json!({"adapter":"tcp","socket":id,"peer":peer}),opened:Some(Socket{read:Arc::new(Mutex::new(read)),write:Arc::new(Mutex::new(write)),matcher:Arc::new(StdMutex::new(connect_matcher.unwrap())),secret_count,matcher_memory,read_busy:false,write_busy:false,write_closed:false})}
                        }
                    },
                    Operation::Read{socket,limit}=>{
                        let (read,_,matcher)=transport.unwrap();let mut stream=read.lock().await;let mut bytes=vec![0;limit];
                        match stream.read(&mut bytes).await {
                            Err(_)=>Completed{wire:failure("TcpRead","Connected",0),opened:None},
                            Ok(length)=>{bytes.truncate(length);let secret=(matcher.lock().unwrap_or_else(|e|e.into_inner()).scan)(&bytes);Completed{wire:if secret {failure("TcpSecretResponse","Received",0)} else {json!({"adapter":"tcp","stream":socket,"body":STANDARD.encode(bytes),"eof":length==0,"secret_count":secret_count})},opened:None}}
                        }
                    },
                    Operation::Write{body,..}=>{
                        let (_,write,_)=transport.unwrap();let mut stream=write.lock().await;
                        let wire=write_body(&mut *stream,&body,&count).await;
                        Completed{wire,opened:None}
                    },
                    Operation::ShutdownWrite{..}=>{
                        let (_,write,_)=transport.unwrap();let mut stream=write.lock().await;
                        Completed{wire:match stream.shutdown().await {Ok(())=>json!({"adapter":"tcp","shutdown":true}),Err(_)=>failure("TcpShutdown","Connected",0)},opened:None}
                    },
                    Operation::Close{..}=>unreachable!(),
                }
            };
            let completed=tokio::time::timeout_at(deadline,future).await.unwrap_or_else(|_|Completed{wire:failure("TcpDeadline",if socket.is_none(){"NotConnected"}else{"Unknown"},count.load(Ordering::Relaxed)),opened:None});
            let _=sender.send(completed);
        });
        self.jobs.insert(
            id,
            Job {
                receiver,
                handle,
                socket,
                read: reading,
                accepted,
                closed: false,
                memory: JOB_MEMORY
                    + matcher_memory
                    + if socket.is_none() { SOCKET_MEMORY } else { 0 },
            },
        );
        Ok(())
    }
    fn finish(&mut self, id: usize, completed: Completed) -> Value {
        let job = self.jobs.remove(&id).unwrap();
        if let Some(socket) = job.socket.and_then(|id| self.sockets.get_mut(&id)) {
            if job.read {
                socket.read_busy = false;
            } else {
                socket.write_busy = false;
            }
            if completed.wire["shutdown"] == true {
                socket.write_closed = true;
            }
        }
        self.retired.push((
            job.handle,
            job.memory.saturating_sub(if job.socket.is_none() {
                SOCKET_MEMORY
            } else {
                0
            }),
        ));
        if let Some(socket) = completed.opened {
            self.sockets.insert(id, socket);
        }
        if completed.wire.get("error").is_some() {
            if let Some(socket) = job.socket {
                self.close(socket);
            }
        }
        completed.wire
    }
    pub(crate) fn poll(&mut self, id: usize) -> Option<Value> {
        self.reap();
        let job = self.jobs.get(&id)?;
        let completed = match job.receiver.try_recv() {
            Ok(value) => value,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Completed {
                wire: failure(
                    if job.closed { "TcpClosed" } else { "TcpWorker" },
                    "Unknown",
                    job.accepted.load(Ordering::Relaxed),
                ),
                opened: None,
            },
        };
        Some(self.finish(id, completed))
    }
    pub(crate) fn cancel(&mut self, id: usize) -> Value {
        if let Some(value) = self.poll(id) {
            return value;
        }
        let Some(job) = self.jobs.remove(&id) else {
            return failure("TcpClosed", "NotSent", 0);
        };
        let value = failure(
            if job.closed {
                "TcpClosed"
            } else {
                "TcpCancelled"
            },
            if job.socket.is_none() {
                "NotConnected"
            } else {
                "Unknown"
            },
            job.accepted.load(Ordering::Relaxed),
        );
        job.handle.abort();
        self.retired.push((job.handle, job.memory));
        if let Some(socket) = job.socket {
            self.close(socket);
        }
        value
    }
    pub(crate) fn close(&mut self, socket: usize) -> Vec<usize> {
        self.sockets.remove(&socket);
        let mut affected = Vec::new();
        for (id, job) in &mut self.jobs {
            if job.socket == Some(socket) {
                job.closed = true;
                job.handle.abort();
                affected.push(*id);
            }
        }
        self.reap();
        affected
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
    }
}
impl Runtime {
    pub fn start_tcp(&mut self, operation: Operation, timeout: u64) -> Result<usize> {
        let size = operation.size();
        self.charge_native_work(size.saturating_mul(4).saturating_add(1))?;
        match &operation {
            Operation::Connect { host, .. } => self.protect_tcp_request(&[host.as_bytes()])?,
            Operation::Write { body, .. } => self.protect_tcp_request(&[body.as_slice()])?,
            _ => {}
        }
        self.check_native_allocation(size.saturating_mul(6).saturating_add(4096))?;
        let request = serde_json::to_vec(&(operation.clone(), timeout))
            .map_err(|_| Error::InvalidOperation("TcpEncoding".into()))?;
        let (id, fresh) =
            self.begin_async_external("tcp.operation.v1", &request, operation.reserve())?;
        if fresh {
            let submitted = (|| -> std::result::Result<(), &'static str> {
                operation.validate(timeout)?;
                if let Operation::Close { socket } = operation {
                    if !self
                        .tcp_host
                        .as_ref()
                        .is_some_and(|host| host.contains_socket(socket))
                    {
                        return Err("TcpClosed");
                    }
                    self.close_native_resource(socket as u64)
                        .map_err(|_| "TcpClose")?;
                    self.finish_async_external(id, Ok(json!({"adapter":"tcp","closed":true})))
                        .map_err(|_| "TcpRecording")?;
                    return Ok(());
                }
                let patterns = self.http_secret_patterns().map_err(|_| "TcpSecretLimit")?;
                if self.tcp_host.is_none() {
                    self.check_native_allocation(8 * 1024 * 1024)
                        .map_err(|_| "TcpMemoryLimit")?;
                    self.tcp_host = Some(Host::new()?);
                }
                self.tcp_host.as_mut().unwrap().reap();
                self.tcp_host.as_mut().unwrap().admission = JOB_MEMORY
                    + SOCKET_MEMORY
                    + 2 * patterns
                        .iter()
                        .map(Vec::len)
                        .sum::<usize>()
                        .saturating_mul(96)
                    + 8192;
                let budget = self.enforce_budget();
                self.tcp_host.as_mut().unwrap().admission = 0;
                budget.map_err(|_| "TcpMemoryLimit")?;
                self.tcp_host
                    .as_mut()
                    .unwrap()
                    .submit(id, operation, timeout, &patterns)
            })();
            if let Err(code) = submitted {
                self.finish_async_external(id, Ok(failure(code, "NotSent", 0)))?;
            }
        }
        Ok(id)
    }
    fn protect_tcp_request(&mut self, fields: &[&[u8]]) -> Result<()> {
        let pattern_bytes = self
            .sensitive_values
            .iter()
            .map(String::len)
            .sum::<usize>()
            .saturating_add(self.sensitive_bytes.iter().map(|b| b.len()).sum::<usize>());
        if pattern_bytes == 0 {
            return Ok(());
        }
        self.charge_native_work(
            pattern_bytes
                .saturating_mul(4)
                .saturating_add(fields.iter().map(|b| b.len()).sum::<usize>()),
        )?;
        self.check_native_allocation(pattern_bytes.saturating_mul(128).saturating_add(8192))?;
        let patterns = self
            .http_secret_patterns()
            .map_err(|_| Error::InvalidOperation("TcpSecretLimit".into()))?;
        let guard = crate::network::SecretAutomaton::new(&patterns)
            .map_err(|_| Error::InvalidOperation("TcpSecretLimit".into()))?;
        if fields.iter().any(|bytes| (guard.matcher().scan)(bytes)) {
            return Err(Error::InvalidOperation(
                "TcpSecretRequest: private bytes cannot be fingerprinted".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn sanitise_tcp_result(&mut self, result: Value) -> Value {
        let socket = result["stream"].as_u64();
        if let Some(socket) = socket {
            if result["secret_count"].as_u64()
                != Some((self.sensitive_values.len() + self.sensitive_bytes.len()) as u64)
            {
                let _ = self.close_native_resource(socket);
                return failure("TcpSecretContextChanged", "Received", 0);
            }
        }
        let protected = self.protect_http_response(&result);
        if !matches!(protected, Ok(false)) {
            if let Some(socket) = socket {
                let _ = self.close_native_resource(socket);
            }
        }
        match protected {
            Ok(false) => result,
            Ok(true) => failure("TcpSecretResponse", "Received", 0),
            Err(_) => failure("TcpMemoryLimit", "Received", 0),
        }
    }
}

async fn write_body<S: tokio::io::AsyncWrite + Unpin>(
    stream: &mut S,
    body: &[u8],
    accepted: &AtomicUsize,
) -> Value {
    while accepted.load(Ordering::Relaxed) < body.len() {
        let offset = accepted.load(Ordering::Relaxed);
        let end = (offset + 8192).min(body.len());
        match stream.write(&body[offset..end]).await {
            Ok(0) | Err(_) => {
                return failure("TcpWrite", "PartialWrite", accepted.load(Ordering::Relaxed))
            }
            Ok(length) => {
                accepted.fetch_add(length, Ordering::Relaxed);
            }
        }
    }
    json!({"adapter":"tcp","written":accepted.load(Ordering::Relaxed)})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backpressure_deadline_preserves_only_known_accepted_bytes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (mut sender, mut receiver) = tokio::io::duplex(16);
            let accepted = AtomicUsize::new(0);
            let bytes = vec![42; FRAME];
            assert!(tokio::time::timeout(
                Duration::from_millis(20),
                write_body(&mut sender, &bytes, &accepted)
            )
            .await
            .is_err());
            assert_eq!(accepted.load(Ordering::Relaxed), 16);
            drop(sender);
            let mut received = Vec::new();
            receiver.read_to_end(&mut received).await.unwrap();
            assert_eq!(received, vec![42; 16]);
        });
    }
    #[test]
    fn closed_peer_returns_partial_write_without_retry() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (mut sender, receiver) = tokio::io::duplex(16);
            drop(receiver);
            let accepted = AtomicUsize::new(0);
            let value = write_body(&mut sender, b"never sent", &accepted).await;
            assert_eq!(value["error"]["code"], "TcpWrite");
            assert_eq!(value["error"]["acceptedBytes"], 0);
        });
    }
}
