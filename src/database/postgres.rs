//! PostgreSQL connections, TLS and bounded server-side cursors live outside VM checkpoints.
use super::{
    adapter::{Command, Error, Reply},
    parameter_size, Parameter,
};
use bytes::BytesMut;
use futures_util::TryStreamExt;
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::TcpStream,
    sync::{mpsc as async_mpsc, Notify},
};
use tokio_postgres::{
    config::{ChannelBinding, Host, SslMode},
    types::{FromSql, IsNull, ToSql, Type},
    Client, Config, NoTls, Statement,
};
const VALUE_LIMIT: usize = 1024 * 1024;
const FRAME_LIMIT: usize = VALUE_LIMIT + 8192;
const CURSOR: &str = "rewind_bounded_cursor";
type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Credentials {
    config: Config,
    host: String,
    port: u16,
    certificate: Vec<u8>,
}
impl Credentials {
    pub(super) fn parse(dsn: &str, certificate: &[u8]) -> Result<Self> {
        if dsn.len() > 65536 || certificate.len() > 65536 {
            return Err(Error::Limit);
        }
        let config: Config = dsn.parse().map_err(|_| Error::Configuration)?;
        let [Host::Tcp(host)] = config.get_hosts() else {
            return Err(Error::Configuration);
        };
        if host.is_empty()
            || host.len() > 253
            || !config.get_hostaddrs().is_empty()
            || config.get_ports().len() > 1
            || config.get_options().is_some()
            || config.get_channel_binding() == ChannelBinding::Require
        {
            return Err(Error::Configuration);
        }
        if config.get_ssl_mode() != SslMode::Require
            && !(config.get_ssl_mode() == SslMode::Disable
                && (host == "localhost"
                    || host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|a| a.is_loopback())))
        {
            return Err(Error::Tls);
        }
        if !certificate.is_empty() {
            let mut roots = rustls::RootCertStore::empty();
            roots
                .add(rustls::pki_types::CertificateDer::from(
                    certificate.to_vec(),
                ))
                .map_err(|_| Error::Tls)?;
        }
        Ok(Self {
            host: host.clone(),
            port: config.get_ports().first().copied().unwrap_or(5432),
            config,
            certificate: certificate.to_vec(),
        })
    }
    pub(super) fn secret_password(&self) -> Option<&[u8]> {
        self.config.get_password()
    }
    pub(super) fn retained_bytes(&self) -> usize {
        self.certificate.len() + 65536
    }
    async fn transport(&self) -> Result<Box<dyn Io>> {
        let addresses = tokio::net::lookup_host((self.host.as_str(), self.port))
            .await
            .map_err(|_| Error::Disconnected)?;
        // One selected address, one connect attempt. No implicit failover or write retries.
        let address = addresses
            .into_iter()
            .find(|a| self.config.get_ssl_mode() != SslMode::Disable || a.ip().is_loopback())
            .ok_or(Error::Configuration)?;
        let mut stream = TcpStream::connect(address)
            .await
            .map_err(|_| Error::Disconnected)?;
        stream.set_nodelay(true).map_err(|_| Error::Disconnected)?;
        if self.config.get_ssl_mode() == SslMode::Disable {
            return Ok(Box::new(stream));
        }
        stream
            .write_all(&[0, 0, 0, 8, 4, 210, 22, 47])
            .await
            .map_err(|_| Error::Disconnected)?;
        if stream.read_u8().await.map_err(|_| Error::Disconnected)? != b'S' {
            return Err(Error::Tls);
        }
        let mut roots = rustls::RootCertStore::empty();
        if self.certificate.is_empty() {
            roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
        } else {
            roots
                .add(rustls::pki_types::CertificateDer::from(
                    self.certificate.clone(),
                ))
                .map_err(|_| Error::Tls)?;
        }
        if roots.is_empty() {
            return Err(Error::Tls);
        }
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| Error::Tls)?
        .with_root_certificates(roots)
        .with_no_client_auth();
        use tokio_postgres::tls::{MakeTlsConnect, TlsConnect};
        let mut connector = tokio_postgres_rustls::MakeRustlsConnect::new(tls);
        let connector=<tokio_postgres_rustls::MakeRustlsConnect as MakeTlsConnect<TcpStream>>::make_tls_connect(&mut connector,&self.host).map_err(|_|Error::Tls)?;
        Ok(Box::new(
            connector.connect(stream).await.map_err(|_| Error::Tls)?,
        ))
    }
}
trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}
/// Reject a PostgreSQL frame length before the protocol decoder can reserve its payload.
struct Bounded<S> {
    inner: S,
    header: [u8; 5],
    header_len: usize,
    remaining: usize,
}
impl<S> Bounded<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            header: [0; 5],
            header_len: 0,
            remaining: 0,
        }
    }
}
impl<S: AsyncRead + Unpin> AsyncRead for Bounded<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if out.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let header = self.remaining == 0;
        let wanted = out
            .remaining()
            .min(if header {
                5 - self.header_len
            } else {
                self.remaining
            })
            .min(8192);
        let mut temp = [0u8; 8192];
        let mut buf = ReadBuf::new(&mut temp[..wanted]);
        match Pin::new(&mut self.inner).poll_read(cx, &mut buf) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Ready(Ok(())) => {}
        }
        let bytes = buf.filled();
        if header {
            let start = self.header_len;
            self.header[start..start + bytes.len()].copy_from_slice(bytes);
            self.header_len += bytes.len();
            if self.header_len == 5 {
                let length = u32::from_be_bytes(self.header[1..5].try_into().unwrap()) as usize;
                if !(4..=FRAME_LIMIT).contains(&length) {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "DB frame limit",
                    )));
                }
                self.remaining = length - 4;
                self.header_len = 0;
            }
        } else {
            self.remaining -= bytes.len();
        }
        out.put_slice(bytes);
        Poll::Ready(Ok(()))
    }
}
impl<S: AsyncWrite + Unpin> AsyncWrite for Bounded<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
struct Request {
    command: Command,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    wake: Arc<Notify>,
    response: mpsc::Sender<Result<Reply>>,
}
pub(super) struct Pending {
    receiver: mpsc::Receiver<Result<Reply>>,
    cancelled: Arc<AtomicBool>,
    wake: Arc<Notify>,
}
impl Pending {
    pub(super) fn poll(&self) -> Option<Result<Reply>> {
        match self.receiver.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err(Error::Worker)),
        }
    }
    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.wake.notify_one();
    }
}
pub(super) struct Worker {
    sender: async_mpsc::Sender<Request>,
    closed: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    transaction: Arc<AtomicU64>,
    completion: Arc<Mutex<Option<Result<()>>>>,
    wake: Arc<Notify>,
}
impl Worker {
    pub(super) fn open(credentials: Credentials, timeout: Duration) -> Result<Self> {
        let (sender, receiver) = async_mpsc::channel(1);
        let closed = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let transaction = Arc::new(AtomicU64::new(0));
        let completion = Arc::new(Mutex::new(None));
        let wake = Arc::new(Notify::new());
        let (ready, opened) = mpsc::sync_channel(1);
        let (c, b, d, t, r, w) = (
            closed.clone(),
            busy.clone(),
            done.clone(),
            transaction.clone(),
            completion.clone(),
            wake.clone(),
        );
        std::thread::Builder::new()
            .name("rewind-postgres".into())
            .stack_size(512 * 1024)
            .spawn(move || {
                let mut published = false;
                let mut close_reply = None;
                let result = (|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|_| Error::Worker)?;
                    runtime.block_on(async {
                        let connected = tokio::time::timeout(timeout, async {
                            let transport = Bounded::new(credentials.transport().await?);
                            let mut config = credentials.config.clone();
                            config
                                .ssl_mode(SslMode::Disable)
                                .channel_binding(ChannelBinding::Disable);
                            config.connect_raw(transport, NoTls).await.map_err(pg_error)
                        })
                        .await
                        .unwrap_or(Err(Error::Deadline));
                        match connected {
                            Err(error) => Err(error),
                            Ok((client, connection)) => {
                                let driver = tokio::spawn(connection);
                                if ready.send(Ok(())).is_err() {
                                    driver.abort();
                                    return Err(Error::Cancelled);
                                }
                                published = true;
                                let (result, reply) =
                                    run(client, credentials, receiver, c.clone(), b, t, w).await;
                                close_reply = reply;
                                driver.abort();
                                let _ = driver.await;
                                result
                            }
                        }
                    })
                })();
                c.store(true, Ordering::Release);
                *r.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
                d.store(true, Ordering::Release);
                // Failed opens stay owned by Host's opener receiver until DNS and runtime cleanup finish.
                if !published {
                    let _ = ready.send(result);
                }
                if let Some((response, prior)) = close_reply {
                    let final_result = match prior {
                        Err(error) => Err(error),
                        Ok(_) => result.map(|_| Reply::Unit),
                    };
                    let _ = response.send(final_result);
                }
            })
            .map_err(|_| Error::Worker)?;
        opened.recv().map_err(|_| Error::Worker)??;
        Ok(Self {
            sender,
            closed,
            busy,
            done,
            transaction,
            completion,
            wake,
        })
    }
    pub(super) fn submit(&self, command: Command, timeout: Duration) -> Result<Pending> {
        if self.closed.load(Ordering::Acquire) {
            return Err(Error::Closed);
        }
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(Error::Limit);
        }
        validate(&command)?;
        if self.busy.swap(true, Ordering::AcqRel) {
            return Err(Error::Busy);
        }
        let (response, receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(Notify::new());
        if self
            .sender
            .try_send(Request {
                command,
                deadline: Instant::now() + timeout,
                cancelled: cancelled.clone(),
                wake: wake.clone(),
                response,
            })
            .is_err()
        {
            self.busy.store(false, Ordering::Release);
            return Err(Error::Worker);
        }
        Ok(Pending {
            receiver,
            cancelled,
            wake,
        })
    }
    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.wake.notify_one();
    }
    pub(super) fn released(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }
    pub(super) fn transaction(&self) -> u64 {
        self.transaction.load(Ordering::Acquire)
    }
    pub(super) fn completion(&self) -> Option<Result<()>> {
        *self.completion.lock().unwrap_or_else(|e| e.into_inner())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.close();
    }
}
fn pg_error(error: tokio_postgres::Error) -> Error {
    if let Some(db) = error.as_db_error() {
        let state: [u8; 5] = db.code().code().as_bytes().try_into().unwrap_or(*b"XXXXX");
        if &state == b"28P01" || &state == b"28000" {
            Error::Authentication(state)
        } else {
            Error::SqlState(state)
        }
    } else if error.is_closed() {
        Error::Disconnected
    } else {
        Error::Disconnected
    }
}
fn supported(ty: &Type) -> bool {
    matches!(
        *ty,
        Type::NUMERIC
            | Type::TIMESTAMPTZ
            | Type::BOOL
            | Type::INT2
            | Type::INT4
            | Type::INT8
            | Type::FLOAT4
            | Type::FLOAT8
            | Type::TEXT
            | Type::VARCHAR
            | Type::BPCHAR
            | Type::NAME
            | Type::BYTEA
    )
}
#[derive(Debug)]
struct Null;
impl ToSql for Null {
    fn to_sql(
        &self,
        _: &Type,
        _: &mut BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        Ok(IsNull::Yes)
    }
    fn accepts(ty: &Type) -> bool {
        supported(ty)
    }
    tokio_postgres::types::to_sql_checked!();
}
#[derive(Debug)]
struct Numeric(Vec<u8>);
impl ToSql for Numeric {
    fn to_sql(
        &self,
        _: &Type,
        out: &mut BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        out.extend_from_slice(&self.0);
        Ok(IsNull::No)
    }
    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }
    tokio_postgres::types::to_sql_checked!();
}
struct NumericText(String);
impl<'a> FromSql<'a> for NumericText {
    fn from_sql(
        _: &Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let value = crate::decimal::postgres::decode(raw).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid finite bounded PostgreSQL NUMERIC",
            )
        })?;
        Ok(Self(value.representation()))
    }
    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }
}
#[derive(Debug)]
struct Timestamp(i64);
impl ToSql for Timestamp {
    fn to_sql(
        &self,
        _: &Type,
        out: &mut BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        out.extend_from_slice(&self.0.to_be_bytes());
        Ok(IsNull::No)
    }
    fn accepts(ty: &Type) -> bool {
        *ty == Type::TIMESTAMPTZ
    }
    tokio_postgres::types::to_sql_checked!();
}
struct TimestampText(String);
impl<'a> FromSql<'a> for TimestampText {
    fn from_sql(
        _: &Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let raw: [u8; 8] = raw.try_into().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid timestamp size")
        })?;
        let value = crate::datetime::postgres::decode(i64::from_be_bytes(raw)).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid finite timestamp")
        })?;
        Ok(Self(value.format()))
    }
    fn accepts(ty: &Type) -> bool {
        *ty == Type::TIMESTAMPTZ
    }
}
fn bind(
    params: Vec<Parameter>,
    statement: &Statement,
) -> Result<Vec<Box<dyn ToSql + Sync + Send>>> {
    if params.len() != statement.params().len() {
        return Err(Error::Type);
    }
    params
        .into_iter()
        .zip(statement.params())
        .map(|(v, t)| {
            let value: Box<dyn ToSql + Sync + Send> = match v {
                Parameter::Null if supported(t) => Box::new(Null),
                Parameter::Bool(v) if *t == Type::BOOL => Box::new(v),
                Parameter::Int(v) if *t == Type::INT2 => {
                    Box::new(i16::try_from(v).map_err(|_| Error::Type)?)
                }
                Parameter::Int(v) if *t == Type::INT4 => {
                    Box::new(i32::try_from(v).map_err(|_| Error::Type)?)
                }
                Parameter::Int(v) if *t == Type::INT8 => Box::new(v),
                Parameter::Float(v) if *t == Type::FLOAT8 && v.is_finite() => Box::new(v),
                Parameter::Float(v)
                    if *t == Type::FLOAT4 && v.is_finite() && (v as f32).is_finite() =>
                {
                    Box::new(v as f32)
                }
                Parameter::Text(v) if *t == Type::TIMESTAMPTZ => {
                    let value = crate::datetime::Instant::parse(&v).map_err(|_| Error::Type)?;
                    Box::new(Timestamp(
                        crate::datetime::postgres::encode(value).map_err(|_| Error::Type)?,
                    ))
                }
                Parameter::Text(v) if *t == Type::NUMERIC => {
                    let value = crate::decimal::DecimalValue::parse(&v).map_err(|_| Error::Type)?;
                    Box::new(Numeric(
                        crate::decimal::postgres::encode(&value).map_err(|_| Error::Type)?,
                    ))
                }
                Parameter::Text(v)
                    if matches!(*t, Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME) =>
                {
                    Box::new(v)
                }
                Parameter::Bytes(v) if *t == Type::BYTEA => Box::new(v),
                _ => return Err(Error::Type),
            };
            Ok(value)
        })
        .collect()
}
fn row(row: tokio_postgres::Row) -> Result<Vec<Parameter>> {
    if row.len() > 256 || row.raw_size_bytes() > VALUE_LIMIT {
        return Err(Error::Limit);
    }
    row.columns()
        .iter()
        .enumerate()
        .map(|(i, column)| {
            macro_rules! get {
                ($ty:ty,$wrap:expr) => {
                    row.try_get::<_, Option<$ty>>(i)
                        .map_err(|_| Error::Type)?
                        .map($wrap)
                        .unwrap_or(Parameter::Null)
                };
            }
            Ok(match *column.type_() {
                Type::TIMESTAMPTZ => get!(TimestampText, |v| Parameter::Text(v.0)),
                Type::NUMERIC => get!(NumericText, |v| Parameter::Text(v.0)),
                Type::BOOL => get!(bool, Parameter::Bool),
                Type::INT2 => get!(i16, |v| Parameter::Int(i64::from(v))),
                Type::INT4 => get!(i32, |v| Parameter::Int(i64::from(v))),
                Type::INT8 => get!(i64, Parameter::Int),
                Type::FLOAT4 => get!(f32, |v| Parameter::Float(f64::from(v))),
                Type::FLOAT8 => get!(f64, Parameter::Float),
                Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
                    get!(String, Parameter::Text)
                }
                Type::BYTEA => get!(Vec<u8>, Parameter::Bytes),
                _ => return Err(Error::Type),
            })
        })
        .collect()
}
fn wire_size(row: &[Parameter]) -> usize {
    row.iter()
        .map(|v| match v {
            Parameter::Text(s) => {
                s.bytes()
                    .map(|b| match b {
                        b'"' | b'\\' => 2,
                        0..=31 => 6,
                        _ => 1,
                    })
                    .sum::<usize>()
                    + 64
            }
            Parameter::Bytes(b) => b.len().div_ceil(3) * 4 + 64,
            _ => 96,
        })
        .sum::<usize>()
        + 32
}
fn validate(command: &Command) -> Result<()> {
    let (sql, params) = match command {
        Command::Prepare { sql } => (Some(sql.as_str()), vec![]),
        Command::Execute { sql, params } | Command::Query { sql, params } => {
            (Some(sql.as_str()), params.iter().collect())
        }
        Command::ExecuteMany { sql, params } => {
            if params.len() > 1024 || params.iter().any(|r| r.len() > 1024) {
                return Err(Error::Limit);
            }
            (Some(sql.as_str()), params.iter().flatten().collect())
        }
        Command::Next { rows, bytes } => {
            if !(1..=64).contains(rows) || !(VALUE_LIMIT..=4 * VALUE_LIMIT).contains(bytes) {
                return Err(Error::Limit);
            }
            return Ok(());
        }
        _ => return Ok(()),
    };
    if sql.is_some_and(|s| s.len() > 65536)
        || params.len() > 1024 * 1024
        || params.iter().map(|v| parameter_size(v)).sum::<usize>() > VALUE_LIMIT
    {
        return Err(Error::Limit);
    }
    if sql.is_some_and(transaction_sql) {
        return Err(Error::Configuration);
    }
    Ok(())
}
fn transaction_sql(mut sql: &str) -> bool {
    loop {
        sql = sql.trim_start();
        if let Some(rest) = sql.strip_prefix("--") {
            sql = rest.split_once('\n').map_or("", |(_, r)| r);
        } else if let Some(rest) = sql.strip_prefix("/*") {
            let mut level = 1;
            let mut i = 0;
            let bytes = rest.as_bytes();
            while i + 1 < bytes.len() && level > 0 {
                if &bytes[i..i + 2] == b"/*" {
                    level += 1;
                    i += 2;
                } else if &bytes[i..i + 2] == b"*/" {
                    level -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if level != 0 {
                return true;
            }
            sql = &rest[i..];
        } else {
            break;
        }
    }
    let keyword = sql
        .split(|c: char| !c.is_ascii_alphabetic())
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(
        keyword.as_str(),
        "BEGIN"
            | "START"
            | "COMMIT"
            | "END"
            | "ROLLBACK"
            | "ABORT"
            | "SAVEPOINT"
            | "RELEASE"
            | "PREPARE"
    )
}
struct Cursor {
    deadline: Instant,
    implicit: bool,
    pending: VecDeque<Vec<Parameter>>,
    eof: bool,
}
struct Session {
    client: Client,
    cache: VecDeque<(String, Statement)>,
    cursor: Option<Cursor>,
    transaction: Arc<AtomicU64>,
}
impl Session {
    async fn statement(&mut self, sql: &str) -> Result<Statement> {
        if let Some(index) = self.cache.iter().position(|(s, _)| s == sql) {
            let entry = self.cache.remove(index).unwrap();
            let result = entry.1.clone();
            self.cache.push_back(entry);
            return Ok(result);
        }
        let statement = self.client.prepare(sql).await;
        // Unsupported user-defined types must not accumulate in the driver's unbounded type cache.
        self.client.clear_type_cache();
        let statement = statement.map_err(pg_error)?;
        if statement.params().len() > 1024
            || statement.columns().len() > 256
            || statement
                .columns()
                .iter()
                .map(|c| c.name().len() + 64)
                .sum::<usize>()
                > 65536
        {
            return Err(Error::Limit);
        }
        if statement
            .params()
            .iter()
            .chain(statement.columns().iter().map(|c| c.type_()))
            .any(|t| !supported(t))
        {
            return Err(Error::Type);
        }
        if self.cache.len() == 16 {
            self.cache.pop_front();
        }
        self.cache.push_back((sql.into(), statement.clone()));
        Ok(statement)
    }
    async fn close_cursor(&mut self) -> Result<()> {
        let Some(cursor) = self.cursor.take() else {
            return Err(Error::Closed);
        };
        self.client
            .batch_execute(&format!("CLOSE {CURSOR}"))
            .await
            .map_err(pg_error)?;
        if cursor.implicit {
            self.client
                .batch_execute("ROLLBACK")
                .await
                .map_err(pg_error)?;
        }
        Ok(())
    }
    async fn command(&mut self, command: Command, deadline: Instant) -> Result<Reply> {
        if self.cursor.is_some()
            && !matches!(
                command,
                Command::Next { .. } | Command::CloseCursor | Command::Close
            )
        {
            return Err(Error::Busy);
        }
        if matches!(command, Command::Close) {
            self.cursor = None;
            self.client
                .batch_execute("ROLLBACK")
                .await
                .map_err(pg_error)?;
            if self.transaction.load(Ordering::Acquire) & 1 != 0 {
                self.transaction.fetch_add(1, Ordering::AcqRel);
            }
            return Ok(Reply::Unit);
        }
        match command {
            Command::Prepare { sql } => {
                let s = self.statement(&sql).await?;
                Ok(Reply::Statement {
                    columns: s.columns().iter().map(|c| c.name().into()).collect(),
                    parameters: s.params().len(),
                })
            }
            Command::Execute { sql, params } => {
                let s = self.statement(&sql).await?;
                if !s.columns().is_empty() {
                    return Err(Error::Type);
                }
                let p = bind(params, &s)?;
                let n = self
                    .client
                    .execute(
                        &s,
                        &p.iter()
                            .map(|v| v.as_ref() as &(dyn ToSql + Sync))
                            .collect::<Vec<_>>(),
                    )
                    .await
                    .map_err(pg_error)?;
                Ok(Reply::Changed(n))
            }
            Command::ExecuteMany { sql, params } => {
                let s = self.statement(&sql).await?;
                if !s.columns().is_empty() {
                    return Err(Error::Type);
                }
                let mut changed = 0u64;
                for row in params {
                    let p = bind(row, &s)?;
                    changed = changed
                        .checked_add(
                            self.client
                                .execute(
                                    &s,
                                    &p.iter()
                                        .map(|v| v.as_ref() as &(dyn ToSql + Sync))
                                        .collect::<Vec<_>>(),
                                )
                                .await
                                .map_err(pg_error)?,
                        )
                        .ok_or(Error::Limit)?;
                }
                Ok(Reply::Changed(changed))
            }
            Command::Query { sql, params } => {
                let s = self.statement(&sql).await?;
                if s.columns().is_empty() {
                    return Err(Error::Type);
                }
                let columns = s.columns().iter().map(|c| c.name().into()).collect();
                let p = bind(params, &s)?;
                let implicit = self.transaction.load(Ordering::Acquire) & 1 == 0;
                if implicit {
                    self.client
                        .batch_execute("BEGIN READ ONLY")
                        .await
                        .map_err(pg_error)?;
                }
                let declared = async {
                    let declaration = self
                        .client
                        .prepare_typed(
                            &format!("DECLARE {CURSOR} NO SCROLL CURSOR FOR {sql}"),
                            s.params(),
                        )
                        .await
                        .map_err(pg_error)?;
                    self.client
                        .execute(
                            &declaration,
                            &p.iter()
                                .map(|v| v.as_ref() as &(dyn ToSql + Sync))
                                .collect::<Vec<_>>(),
                        )
                        .await
                        .map_err(pg_error)?;
                    Ok(())
                }
                .await;
                if let Err(error) = declared {
                    if implicit {
                        let _ = self.client.batch_execute("ROLLBACK").await;
                    }
                    return Err(error);
                }
                self.cursor = Some(Cursor {
                    deadline,
                    implicit,
                    pending: VecDeque::new(),
                    eof: false,
                });
                Ok(Reply::Cursor(columns))
            }
            Command::Next { rows, bytes } => {
                let mut cursor = self.cursor.take().ok_or(Error::Closed)?;
                let mut output = Vec::new();
                let mut size = 0;
                let result = async {
                    while output.len() < rows {
                        if let Some(next) = cursor.pending.pop_front() {
                            let n = wire_size(&next);
                            if n > bytes {
                                return Err(Error::Limit);
                            }
                            if size + n > bytes {
                                cursor.pending.push_front(next);
                                break;
                            }
                            size += n;
                            output.push(next);
                            continue;
                        }
                        if cursor.eof {
                            break;
                        }
                        let count = (rows - output.len()).min(8);
                        let stream = self
                            .client
                            .query_raw(
                                &format!("FETCH FORWARD {count} FROM {CURSOR}"),
                                std::iter::empty::<&(dyn ToSql + Sync)>(),
                            )
                            .await
                            .map_err(pg_error)?;
                        tokio::pin!(stream);
                        let mut received = 0;
                        while let Some(value) = stream.try_next().await.map_err(pg_error)? {
                            if received >= count {
                                return Err(Error::Limit);
                            }
                            cursor.pending.push_back(row(value)?);
                            received += 1;
                        }
                        cursor.eof = received < count;
                    }
                    Ok(())
                }
                .await;
                self.cursor = Some(cursor);
                result?;
                let done = self
                    .cursor
                    .as_ref()
                    .is_some_and(|c| c.eof && c.pending.is_empty());
                if done {
                    self.close_cursor().await?;
                }
                Ok(Reply::Batch { rows: output, done })
            }
            Command::CloseCursor => {
                self.close_cursor().await?;
                Ok(Reply::Unit)
            }
            Command::Begin => {
                if self.transaction.load(Ordering::Acquire) & 1 != 0 {
                    return Err(Error::Busy);
                }
                self.client.batch_execute("BEGIN").await.map_err(pg_error)?;
                self.transaction.fetch_add(3, Ordering::AcqRel);
                Ok(Reply::Unit)
            }
            Command::Commit | Command::Rollback => {
                if self.transaction.load(Ordering::Acquire) & 1 == 0 {
                    return Err(Error::Configuration);
                }
                self.client
                    .batch_execute(if matches!(command, Command::Commit) {
                        "COMMIT"
                    } else {
                        "ROLLBACK"
                    })
                    .await
                    .map_err(pg_error)?;
                self.transaction.fetch_add(1, Ordering::AcqRel);
                Ok(Reply::Unit)
            }
            Command::Close => unreachable!(),
        }
    }
}
type ClosingReply = (mpsc::Sender<Result<Reply>>, Result<Reply>);
async fn run(
    client: Client,
    credentials: Credentials,
    mut receiver: async_mpsc::Receiver<Request>,
    closed: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
    transaction: Arc<AtomicU64>,
    wake: Arc<Notify>,
) -> (Result<()>, Option<ClosingReply>) {
    let mut session = Session {
        client,
        cache: VecDeque::new(),
        cursor: None,
        transaction,
    };
    let mut closing_reply = None;
    loop {
        if closed.load(Ordering::Acquire) {
            break;
        }
        let expires = session
            .cursor
            .as_ref()
            .map_or(Instant::now() + Duration::from_secs(86400), |c| c.deadline);
        let request = tokio::select! {
            value=receiver.recv()=>{let Some(value)=value else{break};value},
            _=wake.notified()=>{if closed.load(Ordering::Acquire){break;}continue;},
            _=tokio::time::sleep_until(expires.into())=>{closed.store(true,Ordering::Release);break;},
        };
        let Request {
            command,
            deadline,
            cancelled,
            wake: cancel,
            response,
        } = request;
        let closing = matches!(command, Command::Close);
        let deadline = deadline.min(expires);
        let result = if cancelled.load(Ordering::Acquire) {
            Err(Error::Cancelled)
        } else {
            tokio::select! {
                result=tokio::time::timeout_at(deadline.into(),session.command(command,deadline))=>result.unwrap_or(Err(Error::Deadline)),
                _=cancel.notified()=>Err(Error::Cancelled),_=wake.notified()=>Err(Error::Cancelled),
            }
        };
        let terminate = closing
            || matches!(
                result,
                Err(Error::Deadline | Error::Cancelled | Error::Disconnected | Error::Worker)
            )
            || (session.cursor.is_some() && result.as_ref().is_err_and(|e| *e != Error::Busy));
        if (terminate || matches!(result, Err(Error::SqlState(_) | Error::Authentication(_))))
            && session.transaction.load(Ordering::Acquire) & 1 != 0
        {
            session.transaction.fetch_add(2, Ordering::AcqRel);
        }
        if terminate {
            closed.store(true, Ordering::Release);
        }
        busy.store(false, Ordering::Release);
        if closing {
            closing_reply = Some((response, result));
            break;
        }
        // Report deadlines before waiting for cancel transport / rollback. Cleanup retains the worker budget.
        let _ = response.send(result);
        if terminate {
            let token = session.client.cancel_token();
            let _ = tokio::time::timeout(Duration::from_secs(1), async {
                let transport = credentials.transport().await?;
                token
                    .cancel_query_raw(transport, NoTls)
                    .await
                    .map_err(pg_error)
            })
            .await;
            break;
        }
    }
    closed.store(true, Ordering::Release);
    session.cursor = None;
    let result = if closing_reply.as_ref().is_some_and(|(_, r)| r.is_ok()) {
        Ok(())
    } else {
        tokio::time::timeout(
            Duration::from_secs(1),
            session.client.batch_execute("ROLLBACK"),
        )
        .await
        .map_err(|_| Error::Deadline)
        .and_then(|r| r.map_err(pg_error))
    };
    if result.is_ok() && session.transaction.load(Ordering::Acquire) & 1 != 0 {
        session.transaction.fetch_add(1, Ordering::AcqRel);
    }
    (result, closing_reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transaction_control_cannot_bypass_physical_generation() {
        for sql in [
            "BEGIN",
            " -- hello\nCOMMIT",
            "/* a /* b */ c */ROLLBACK",
            "SAVEPOINT a",
        ] {
            assert!(transaction_sql(sql));
        }
        for sql in [
            "SELECT 1",
            "/* comment */ INSERT INTO t VALUES (1)",
            "SELECT 'COMMIT'",
        ] {
            assert!(!transaction_sql(sql));
        }
    }
    #[test]
    fn remote_plaintext_and_implicit_tls_fallback_are_rejected() {
        assert!(Credentials::parse("host=example.com sslmode=disable", &[]).is_err());
        assert!(Credentials::parse("host=localhost sslmode=prefer", &[]).is_err());
        assert!(Credentials::parse("host=localhost sslmode=disable", &[]).is_ok());
        assert!(Credentials::parse("host=localhost sslmode=require", &[]).is_ok());
    }
    #[tokio::test]
    async fn oversized_protocol_frames_are_rejected_before_payload_reservation() {
        let input = &[b'D', 0x7f, 0xff, 0xff, 0xff][..];
        let mut transport = Bounded::new(input);
        let mut output = Vec::new();
        assert!(transport.read_to_end(&mut output).await.is_err());
        assert!(output.capacity() < 8192);
        let bytes = &[b'D', 0, 0, 0, 7, 1, 2, 3, b'Z', 0, 0, 0, 5, b'I'][..];
        let mut transport = Bounded::new(bytes);
        let mut output = Vec::new();
        transport.read_to_end(&mut output).await.unwrap();
        assert_eq!(output, bytes);
    }
}
