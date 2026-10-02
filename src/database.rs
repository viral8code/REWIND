//! Native database adapters. Workers own connections and never access VM values.
mod adapter;
mod postgres;
mod sqlite;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf, sync::mpsc, time::Duration};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Parameter {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    Private(String),
}
impl Parameter {
    fn sqlite(self) -> rusqlite::types::Value {
        use rusqlite::types::Value as V;
        match self {
            Self::Null => V::Null,
            Self::Bool(v) => V::Integer(v.into()),
            Self::Int(v) => V::Integer(v),
            Self::Float(v) => V::Real(v),
            Self::Text(v) => V::Text(v),
            Self::Bytes(v) => V::Blob(v),
            Self::Private(_) => unreachable!("private aliases must be resolved before binding"),
        }
    }
}
fn parameter_size(value: &Parameter) -> usize {
    match value {
        Parameter::Text(v) | Parameter::Private(v) => v.len() + 32,
        Parameter::Bytes(v) => v.len() + 32,
        _ => 32,
    }
}
fn canonical_location(path: &std::path::Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        path.parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .map(|parent| parent.join(path.file_name().unwrap_or_default()))
            .unwrap_or_else(|| path.to_path_buf())
    })
}

#[derive(Clone, Debug, serde::Serialize)]
pub enum Operation {
    Postgres {
        alias: String,
    },
    ExecuteMany {
        connection: usize,
        sql: String,
        parameters: Vec<Vec<Parameter>>,
    },
    Cleanup,
    Prepare {
        connection: usize,
        sql: String,
    },
    ExecuteStatement {
        statement: usize,
        parameters: Vec<Parameter>,
    },
    QueryStatement {
        statement: usize,
        parameters: Vec<Parameter>,
    },
    CloseStatement {
        statement: usize,
    },
    Sqlite {
        path: String,
        read_only: bool,
    },
    Execute {
        connection: usize,
        sql: String,
        parameters: Vec<Parameter>,
    },
    Query {
        connection: usize,
        sql: String,
        parameters: Vec<Parameter>,
    },
    Next {
        cursor: usize,
        rows: usize,
        bytes: usize,
    },
    CloseCursor {
        cursor: usize,
    },
    Begin {
        connection: usize,
    },
    Commit {
        connection: usize,
    },
    Rollback {
        connection: usize,
    },
    Close {
        connection: usize,
    },
}

enum Job {
    Cleanup(std::time::Instant),
    Open {
        receiver: mpsc::Receiver<std::result::Result<adapter::Worker, adapter::Error>>,
        deadline: std::time::Instant,
        reservation: usize,
    },
    Command {
        pending: adapter::Pending,
        connection: usize,
        cursor: Option<usize>,
        closing: bool,
    },
}

pub(crate) struct Host {
    connections: BTreeMap<usize, adapter::Worker>,
    cursors: BTreeMap<usize, usize>,
    statements: BTreeMap<usize, (usize, String)>,
    preparing: BTreeMap<usize, (usize, String)>,
    jobs: BTreeMap<usize, Job>,
    cleanup: BTreeMap<usize, adapter::Pending>,
    retired: Vec<(usize, adapter::Worker)>,
    aborted_opens: Vec<(
        usize,
        mpsc::Receiver<std::result::Result<adapter::Worker, adapter::Error>>,
    )>,
    paths: BTreeMap<usize, PathBuf>,
    private_parameters: BTreeMap<String, Parameter>,
    postgres_credentials: BTreeMap<String, postgres::Credentials>,
    cleanup_errors: Vec<adapter::Error>,
    pub(crate) admission: usize,
}
impl Host {
    fn new() -> Self {
        Self {
            connections: BTreeMap::new(),
            cursors: BTreeMap::new(),
            statements: BTreeMap::new(),
            preparing: BTreeMap::new(),
            jobs: BTreeMap::new(),
            cleanup: BTreeMap::new(),
            retired: Vec::new(),
            aborted_opens: Vec::new(),
            paths: BTreeMap::new(),
            private_parameters: BTreeMap::new(),
            postgres_credentials: BTreeMap::new(),
            cleanup_errors: Vec::new(),
            admission: 0,
        }
    }
    pub(crate) fn contains_job(&self, id: usize) -> bool {
        self.jobs.contains_key(&id)
    }
    pub(crate) fn blocks_path(&self, path: &std::path::Path) -> bool {
        let path = canonical_location(path);
        self.paths
            .iter()
            .filter(|(id, _)| {
                !self
                    .retired
                    .iter()
                    .any(|(old, worker)| old == *id && worker.released())
            })
            .any(|(_, database)| {
                ["", "-wal", "-shm", "-journal"].iter().any(|suffix| {
                    let mut sidecar = database.as_os_str().to_os_string();
                    sidecar.push(suffix);
                    canonical_location(std::path::Path::new(&sidecar)) == path
                })
            })
    }
    pub(crate) fn resource_ids(&self) -> impl Iterator<Item = &usize> {
        self.connections
            .keys()
            .chain(self.cursors.keys())
            .chain(self.statements.keys())
    }
    pub(crate) fn reserved_bytes(&self) -> usize {
        // Conservative per-connection page/statement cache, thread stack and bounded rows.
        self.admission
            + self
                .private_parameters
                .iter()
                .map(|(alias, value)| alias.len() + parameter_size(value) + 128)
                .sum::<usize>()
            + self
                .postgres_credentials
                .values()
                .map(postgres::Credentials::retained_bytes)
                .sum::<usize>()
            + self
                .connections
                .values()
                .map(adapter::Worker::reserved_bytes)
                .sum::<usize>()
            + self
                .retired
                .iter()
                .map(|(_, worker)| worker.reserved_bytes())
                .sum::<usize>()
            + self.aborted_opens.len() * (192 * 1024 * 1024)
            + self
                .jobs
                .values()
                .map(|job| {
                    if let Job::Open { reservation, .. } = job {
                        *reservation
                    } else {
                        12 * 1024 * 1024
                    }
                })
                .sum::<usize>()
    }
    fn submit(
        &mut self,
        id: usize,
        operation: Operation,
        path: Option<PathBuf>,
        timeout: Duration,
    ) -> std::result::Result<(), &'static str> {
        self.reap();
        let operation = match operation {
            Operation::ExecuteStatement {
                statement,
                parameters,
            } => {
                let (connection, sql) = self.statements.get(&statement).ok_or("DbClosed")?;
                Operation::Execute {
                    connection: *connection,
                    sql: sql.clone(),
                    parameters,
                }
            }
            Operation::QueryStatement {
                statement,
                parameters,
            } => {
                let (connection, sql) = self.statements.get(&statement).ok_or("DbClosed")?;
                Operation::Query {
                    connection: *connection,
                    sql: sql.clone(),
                    parameters,
                }
            }
            other => other,
        };
        if self.jobs.len() >= 8 {
            return Err("DbBusy");
        }
        if matches!(operation, Operation::Cleanup) {
            self.jobs
                .insert(id, Job::Cleanup(std::time::Instant::now() + timeout));
            return Ok(());
        }
        if matches!(
            operation,
            Operation::Sqlite { .. } | Operation::Postgres { .. }
        ) {
            if self.connections.len()
                + self.retired.len()
                + self.aborted_opens.len()
                + self
                    .jobs
                    .values()
                    .filter(|j| matches!(j, Job::Open { .. }))
                    .count()
                >= 8
            {
                return Err("DbConnectionLimit");
            }
            let reservation = if matches!(operation, Operation::Postgres { .. }) {
                192 * 1024 * 1024
            } else {
                87 * 1024 * 1024
            };
            let opener: Box<
                dyn FnOnce() -> std::result::Result<adapter::Worker, adapter::Error> + Send,
            > = match operation {
                Operation::Sqlite { read_only, .. } => {
                    let path = path.ok_or("DbPath")?;
                    if path != std::path::Path::new(":memory:") {
                        self.paths.insert(id, path.clone());
                    }
                    Box::new(move || adapter::Worker::sqlite(path, read_only))
                }
                Operation::Postgres { alias } => {
                    let credentials = self
                        .postgres_credentials
                        .get(&alias)
                        .ok_or("DbCredentialUnknown")?
                        .clone();
                    Box::new(move || {
                        postgres::Worker::open(credentials, timeout).map(adapter::Worker::Postgres)
                    })
                }
                _ => unreachable!(),
            };
            let (sender, receiver) = mpsc::channel();
            std::thread::Builder::new()
                .name("rewind-db-open".into())
                .stack_size(512 * 1024)
                .spawn(move || {
                    let _ = sender.send(opener());
                })
                .map_err(|_| {
                    self.paths.remove(&id);
                    "DbWorker"
                })?;
            self.jobs.insert(
                id,
                Job::Open {
                    receiver,
                    deadline: std::time::Instant::now() + timeout,
                    reservation,
                },
            );
            return Ok(());
        }
        let (connection, cursor, command, closing) = match operation {
            Operation::ExecuteMany {
                connection,
                sql,
                parameters,
            } => (
                connection,
                None,
                adapter::Command::ExecuteMany {
                    sql,
                    params: parameters
                        .into_iter()
                        .map(|row| self.bind(row))
                        .collect::<std::result::Result<_, _>>()?,
                },
                false,
            ),
            Operation::Prepare { connection, sql } => {
                if self.statements.len() + self.preparing.len() >= 128 {
                    return Err("DbStatementLimit");
                }
                self.preparing.insert(id, (connection, sql.clone()));
                (connection, None, adapter::Command::Prepare { sql }, false)
            }
            Operation::Execute {
                connection,
                sql,
                parameters,
            } => (
                connection,
                None,
                adapter::Command::Execute {
                    sql,
                    params: self.bind(parameters)?,
                },
                false,
            ),
            Operation::Query {
                connection,
                sql,
                parameters,
            } => (
                connection,
                Some(id),
                adapter::Command::Query {
                    sql,
                    params: self.bind(parameters)?,
                },
                false,
            ),
            Operation::Next {
                cursor,
                rows,
                bytes,
            } => (
                *self.cursors.get(&cursor).ok_or("DbClosed")?,
                Some(cursor),
                adapter::Command::Next { rows, bytes },
                false,
            ),
            Operation::CloseCursor { cursor } => (
                *self.cursors.get(&cursor).ok_or("DbClosed")?,
                Some(cursor),
                adapter::Command::CloseCursor,
                false,
            ),
            Operation::Begin { connection } => (connection, None, adapter::Command::Begin, false),
            Operation::Commit { connection } => (connection, None, adapter::Command::Commit, false),
            Operation::Rollback { connection } => {
                (connection, None, adapter::Command::Rollback, false)
            }
            Operation::Close { connection } => (connection, None, adapter::Command::Close, true),
            _ => unreachable!(),
        };
        let pending = match self
            .connections
            .get(&connection)
            .ok_or("DbClosed")
            .and_then(|worker| worker.submit(command, timeout).map_err(error_code))
        {
            Ok(pending) => pending,
            Err(error) => {
                self.preparing.remove(&id);
                return Err(error);
            }
        };
        self.jobs.insert(
            id,
            Job::Command {
                pending,
                connection,
                cursor,
                closing,
            },
        );
        Ok(())
    }
    pub(crate) fn poll(&mut self, id: usize) -> Option<Value> {
        self.reap();
        if matches!(self.jobs.get(&id),Some(Job::Open {deadline,..}) if std::time::Instant::now() >= *deadline)
        {
            self.cancel(id);
            return Some(failure("DbDeadline", "Unknown", None));
        }
        let job = self.jobs.get(&id)?;
        let mut result = match job {
            Job::Cleanup(deadline) => {
                if !self.retired.is_empty()
                    || !self.cleanup.is_empty()
                    || !self.aborted_opens.is_empty()
                {
                    if std::time::Instant::now() < *deadline {
                        return None;
                    }
                    failure("DbDeadline", "Unknown", None)
                } else if let Some(error) = self.cleanup_errors.first() {
                    let mut result = error_failure(*error, "Unknown");
                    result["error"]["code"] = json!("DbCleanup");
                    result
                } else {
                    json!({"adapter":"db","unit":true})
                }
            }
            Job::Open { receiver, .. } => {
                let result = match receiver.try_recv() {
                    Ok(result) => result,
                    Err(mpsc::TryRecvError::Empty) => return None,
                    Err(_) => Err(adapter::Error::Worker),
                };
                match result {
                    Ok(worker) => {
                        let backend = worker.backend();
                        self.connections.insert(id, worker);
                        json!({"adapter":"db","connection":id,"backend":backend})
                    }
                    Err(error) => {
                        self.paths.remove(&id);
                        error_failure(error, "NotSent")
                    }
                }
            }
            Job::Command {
                pending,
                connection,
                cursor,
                closing,
            } => {
                let result = pending.poll()?;
                let (connection, cursor, closing) = (*connection, *cursor, *closing);
                if closing {
                    self.close_resource(connection);
                }
                match result {
                    Ok(adapter::Reply::Unit) => {
                        if let Some(cursor) = cursor {
                            self.cursors.remove(&cursor);
                        }
                        json!({"adapter":"db","unit":true})
                    }
                    Ok(adapter::Reply::Changed(count)) => json!({"adapter":"db","changed":count}),
                    Ok(adapter::Reply::Statement {
                        columns,
                        parameters,
                    }) => {
                        if let Some(prepared) = self.preparing.remove(&id) {
                            self.statements.insert(id, prepared);
                        }
                        json!({"adapter":"db","statement":id,"columns":columns,"parameters":parameters})
                    }
                    Ok(adapter::Reply::Cursor(columns)) => {
                        self.cursors.insert(id, connection);
                        json!({"adapter":"db","cursor":id,"columns":columns})
                    }
                    Ok(adapter::Reply::Batch { rows, done }) => {
                        if done {
                            if let Some(cursor) = cursor {
                                self.cursors.remove(&cursor);
                            }
                        }
                        let rows = rows
                            .into_iter()
                            .map(|row| row.into_iter().map(wire_value).collect::<Vec<_>>())
                            .collect::<Vec<_>>();
                        json!({"adapter":"db","rows":rows,"done":done})
                    }
                    Err(error) => {
                        if let Some(cursor) = cursor {
                            if !matches!(error, adapter::Error::Busy) {
                                self.cursors.remove(&cursor);
                            }
                        }
                        error_failure(error, "Unknown")
                    }
                }
            }
        };
        if result.get("error").is_none() {
            if let Some(Job::Command { connection, .. }) = self.jobs.get(&id) {
                if let Some(worker) = self.connections.get(connection) {
                    let transaction = worker.transaction();
                    if transaction & 1 != 0 {
                        result["dbConnection"] = json!(connection);
                        result["dbTransaction"] = json!(transaction);
                    }
                }
            }
        }
        self.jobs.remove(&id);
        self.preparing.remove(&id);
        Some(result)
    }
    pub(crate) fn close_resource(&mut self, id: usize) {
        if self.statements.remove(&id).is_some() {
            return;
        }
        self.reap();
        if let Some(worker) = self.connections.remove(&id) {
            worker.close();
            self.retired.push((id, worker));
            self.cursors.retain(|_, parent| *parent != id);
            self.statements.retain(|_, (parent, _)| *parent != id);
            self.preparing.retain(|_, (parent, _)| *parent != id);
        } else if let Some(parent) = self.cursors.remove(&id) {
            match self
                .connections
                .get(&parent)
                .map(|worker| worker.submit(adapter::Command::CloseCursor, Duration::from_secs(1)))
            {
                Some(Ok(pending)) => {
                    self.cleanup.insert(id, pending);
                }
                _ => self.close_resource(parent),
            }
        }
    }
    fn reap(&mut self) {
        self.aborted_opens
            .retain(|(id, receiver)| match receiver.try_recv() {
                Ok(Ok(worker)) => {
                    worker.close();
                    self.retired.push((*id, worker));
                    false
                }
                Err(mpsc::TryRecvError::Empty) => true,
                _ => {
                    self.paths.remove(id);
                    false
                }
            });
        self.retired.retain(|(id, worker)| {
            if worker.released() {
                if let Some(Err(error)) = worker.completion() {
                    if self.cleanup_errors.len() < 8 {
                        self.cleanup_errors.push(error);
                    }
                }
                self.paths.remove(id);
                false
            } else {
                true
            }
        });
        self.cleanup.retain(|_, pending| match pending.poll() {
            None => true,
            Some(Err(error)) => {
                if self.cleanup_errors.len() < 8 {
                    self.cleanup_errors.push(error);
                }
                false
            }
            Some(Ok(_)) => false,
        });
    }
    fn bind(
        &self,
        parameters: Vec<Parameter>,
    ) -> std::result::Result<Vec<Parameter>, &'static str> {
        parameters
            .into_iter()
            .map(|value| {
                let value = if let Parameter::Private(alias) = value {
                    self.private_parameters
                        .get(&alias)
                        .ok_or("DbPrivateParameterUnknown")?
                        .clone()
                } else {
                    value
                };
                Ok(value)
            })
            .collect()
    }
    pub(crate) fn cancel(&mut self, id: usize) -> Value {
        self.preparing.remove(&id);
        if let Some(job) = self.jobs.remove(&id) {
            if let Job::Command {
                pending,
                connection,
                ..
            } = job
            {
                pending.cancel();
                self.close_resource(connection);
            } else if let Job::Open { receiver, .. } = job {
                self.aborted_opens.push((id, receiver));
            }
        }
        failure("DbCancelled", "Unknown", None)
    }
}
fn error_code(error: adapter::Error) -> &'static str {
    match error {
        adapter::Error::Closed => "DbClosed",
        adapter::Error::Busy => "DbBusy",
        adapter::Error::Limit => "DbLimit",
        adapter::Error::Deadline => "DbDeadline",
        adapter::Error::Cancelled => "DbCancelled",
        adapter::Error::Sql(_) => "DbSql",
        adapter::Error::Worker => "DbWorker",
        adapter::Error::SqlState(_) => "DbSql",
        adapter::Error::Configuration => "DbConfiguration",
        adapter::Error::Tls => "DbTls",
        adapter::Error::Authentication(_) => "DbAuthentication",
        adapter::Error::Disconnected => "DbDisconnected",
        adapter::Error::Type => "DbType",
    }
}
fn wire_value(value: Parameter) -> Value {
    match value {
        Parameter::Null => json!({"type":"Null"}),
        Parameter::Bool(v) => json!({"type":"Bool","value":v}),
        Parameter::Int(v) => json!({"type":"Int","value":v}),
        Parameter::Float(v) => json!({"type":"Float","bits":v.to_bits()}),
        Parameter::Text(v) => json!({"type":"Text","value":v}),
        Parameter::Bytes(v) => json!({"type":"Bytes","value":STANDARD.encode(v)}),
        Parameter::Private(_) => unreachable!("native results cannot contain private aliases"),
    }
}
fn error_failure(error: adapter::Error, phase: &str) -> Value {
    let code = if let adapter::Error::Sql(code) = error {
        Some(code)
    } else {
        None
    };
    let mut result = failure(error_code(error), phase, code);
    if let adapter::Error::SqlState(state) | adapter::Error::Authentication(state) = error {
        result["error"]["sqlState"] = json!(std::str::from_utf8(&state).unwrap_or("XXXXX"));
    }
    result
}
fn failure(code: &str, phase: &str, sql_code: Option<i32>) -> Value {
    json!({"adapter":"db","error":{"code":code,"phase":phase,"sqlCode":sql_code,"sqlState":null}})
}

impl crate::Runtime {
    /// Register an immutable connection alias outside checkpoints. Neither DSN nor password enters replay fingerprints.
    pub fn register_postgres_credentials(
        &mut self,
        alias: &str,
        dsn: &str,
        certificate: &[u8],
    ) -> crate::Result<std::result::Result<(), &'static str>> {
        if self.external_depth != 1 {
            return Err(crate::Error::InvalidOperation(
                "ExternalBoundary: DB credentials require external region".into(),
            ));
        }
        if alias.is_empty()
            || alias.len() > 128
            || !alias
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Ok(Err("DbCredentialAlias"));
        }
        self.charge_native_work(
            dsn.len()
                .saturating_add(certificate.len())
                .saturating_add(alias.len())
                .saturating_add(1),
        )?;
        let credentials = match postgres::Credentials::parse(dsn, certificate) {
            Ok(c) => c,
            Err(e) => return Ok(Err(error_code(e))),
        };
        let host = self.database_host.get_or_insert_with(Host::new);
        if let Some(old) = host.postgres_credentials.get(alias) {
            return Ok(if old == &credentials {
                Ok(())
            } else {
                Err("DbCredentialImmutable")
            });
        }
        if host.postgres_credentials.len() >= 16
            || host
                .postgres_credentials
                .values()
                .map(postgres::Credentials::retained_bytes)
                .sum::<usize>()
                + credentials.retained_bytes()
                > 2 * 1024 * 1024
        {
            return Ok(Err("DbCredentialLimit"));
        }
        self.register_secret_value(&crate::Value::Text(dsn.into()));
        if let Some(password) = credentials.secret_password() {
            self.register_secret_value(&crate::Value::Bytes(std::sync::Arc::new(
                password.to_vec(),
            )));
        }
        self.database_host
            .as_mut()
            .unwrap()
            .postgres_credentials
            .insert(alias.into(), credentials);
        if let Err(error) = self.enforce_budget() {
            self.database_host
                .as_mut()
                .unwrap()
                .postgres_credentials
                .remove(alias);
            return Err(error);
        }
        Ok(Ok(()))
    }
    pub fn register_database_parameter(
        &mut self,
        alias: &str,
        value: Parameter,
    ) -> crate::Result<std::result::Result<(), &'static str>> {
        if self.external_depth != 1 {
            return Err(crate::Error::InvalidOperation(
                "ExternalBoundary: DB parameter registration requires external region".into(),
            ));
        }
        if alias.is_empty()
            || alias.len() > 128
            || !alias
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            || parameter_size(&value) > 65536
            || matches!(value, Parameter::Private(_))
            || matches!(value, Parameter::Float(v) if !v.is_finite())
        {
            return Ok(Err("DbPrivateParameter"));
        }
        self.charge_native_work(
            parameter_size(&value)
                .saturating_add(alias.len())
                .saturating_add(1),
        )?;
        let host = self.database_host.get_or_insert_with(Host::new);
        if let Some(old) = host.private_parameters.get(alias) {
            return Ok(if old == &value {
                Ok(())
            } else {
                Err("DbPrivateParameterImmutable")
            });
        }
        if host.private_parameters.len() >= 64
            || host
                .private_parameters
                .values()
                .map(parameter_size)
                .sum::<usize>()
                + parameter_size(&value)
                > 65536
        {
            return Ok(Err("DbPrivateParameterLimit"));
        }
        let secret = match &value {
            Parameter::Null => crate::Value::Null,
            Parameter::Bool(v) => crate::Value::Int(i64::from(*v)),
            Parameter::Int(v) => crate::Value::Int(*v),
            Parameter::Float(v) => crate::Value::Float(v.to_bits()),
            Parameter::Text(v) => crate::Value::Text(v.clone()),
            Parameter::Bytes(v) => crate::Value::Bytes(std::sync::Arc::new(v.clone())),
            Parameter::Private(_) => unreachable!(),
        };
        host.private_parameters.insert(alias.into(), value);
        self.register_secret_value(&secret);
        if let Err(error) = self.enforce_budget() {
            self.database_host
                .as_mut()
                .unwrap()
                .private_parameters
                .remove(alias);
            return Err(error);
        }
        Ok(Ok(()))
    }
    /// Submit a hot, journaled DB operation. Awaiting/polling is outside the external region.
    pub fn start_database(
        &mut self,
        operation: Operation,
        timeout_ms: u64,
    ) -> crate::Result<usize> {
        let parameters: Vec<&Parameter> = match &operation {
            Operation::Execute { parameters, .. }
            | Operation::Query { parameters, .. }
            | Operation::ExecuteStatement { parameters, .. }
            | Operation::QueryStatement { parameters, .. } => {
                if parameters.len() > 1024 {
                    return Err(crate::Error::InvalidOperation(
                        "DbLimit: parameter count".into(),
                    ));
                }
                parameters.iter().collect()
            }
            Operation::ExecuteMany { parameters, .. } => {
                if parameters.len() > 1024 || parameters.iter().any(|row| row.len() > 1024) {
                    return Err(crate::Error::InvalidOperation(
                        "DbLimit: batch count".into(),
                    ));
                }
                if parameters
                    .iter()
                    .flatten()
                    .map(parameter_size)
                    .sum::<usize>()
                    > 1048576
                {
                    return Err(crate::Error::InvalidOperation(
                        "DbLimit: batch bytes".into(),
                    ));
                }
                parameters.iter().flatten().collect()
            }
            _ => vec![],
        };
        let scalars = parameters
            .iter()
            .filter_map(|p| match p {
                Parameter::Int(v) => Some(v.to_string()),
                Parameter::Bool(v) => Some(i64::from(*v).to_string()),
                Parameter::Float(v) => Some(v.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut fields: Vec<&[u8]> = match &operation {
            Operation::Sqlite { path, .. } => vec![path.as_bytes()],
            Operation::Postgres { alias } => vec![alias.as_bytes()],
            Operation::Prepare { sql, .. } => vec![sql.as_bytes()],
            Operation::ExecuteMany {
                sql, parameters, ..
            } => {
                let mut fields = vec![sql.as_bytes()];
                fields.extend(parameters.iter().flatten().filter_map(|p| match p {
                    Parameter::Text(v) | Parameter::Private(v) => Some(v.as_bytes()),
                    Parameter::Bytes(v) => Some(v.as_slice()),
                    _ => None,
                }));
                fields
            }
            Operation::Execute {
                sql, parameters, ..
            }
            | Operation::Query {
                sql, parameters, ..
            } => {
                let mut fields = vec![sql.as_bytes()];
                fields.extend(parameters.iter().filter_map(|p| match p {
                    Parameter::Text(v) | Parameter::Private(v) => Some(v.as_bytes()),
                    Parameter::Bytes(v) => Some(v.as_slice()),
                    _ => None,
                }));
                fields
            }
            Operation::ExecuteStatement { parameters, .. }
            | Operation::QueryStatement { parameters, .. } => parameters
                .iter()
                .filter_map(|p| match p {
                    Parameter::Text(v) | Parameter::Private(v) => Some(v.as_bytes()),
                    Parameter::Bytes(v) => Some(v.as_slice()),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        };
        fields.extend(scalars.iter().map(|s| s.as_bytes()));
        let input_bytes = fields
            .iter()
            .try_fold(0usize, |total, value| total.checked_add(value.len()))
            .ok_or_else(|| crate::Error::InvalidOperation("DbLimit".into()))?;
        if input_bytes > 2 * 1024 * 1024 {
            return Err(crate::Error::InvalidOperation(
                "DbLimit: request bytes".into(),
            ));
        }
        self.charge_native_work(input_bytes.saturating_mul(8).saturating_add(1))?;
        let patterns = self
            .http_secret_patterns()
            .map_err(|_| crate::Error::InvalidOperation("DbSecretLimit".into()))?;
        if !patterns.is_empty() {
            let automaton = crate::network::SecretAutomaton::new(&patterns)
                .map_err(|_| crate::Error::InvalidOperation("DbSecretLimit".into()))?;
            if fields
                .into_iter()
                .any(|bytes| (automaton.matcher().scan)(bytes))
            {
                return Err(crate::Error::InvalidOperation(
                    "SecretObservationUnrecordable: use a DB private parameter alias".into(),
                ));
            }
        }
        use sha2::Digest;
        struct Fingerprint(sha2::Sha256);
        impl std::io::Write for Fingerprint {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.update(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut digest = Fingerprint(sha2::Sha256::new());
        serde_json::to_writer(&mut digest, &(&operation, timeout_ms))
            .map_err(|_| crate::Error::InvalidOperation("DbRequest".into()))?;
        let fingerprint = digest.0.finalize();
        let reservation = if matches!(operation, Operation::Next { .. }) {
            8 * 1024 * 1024
        } else {
            65536
        };
        let (id, fresh) = self.begin_async_external("db.sqlite.v1", &fingerprint, reservation)?;
        if !fresh && !self.replaying {
            if let Some(outcome) = self.external_entries[id].outcome.as_deref() {
                let outcome: Value = serde_json::from_str(outcome)
                    .map_err(|_| crate::Error::InvalidOperation("DbRecording".into()))?;
                if let (Some(connection), Some(transaction)) = (
                    outcome["Ok"]["dbConnection"].as_u64(),
                    outcome["Ok"]["dbTransaction"].as_u64(),
                ) {
                    if self
                        .database_host
                        .as_ref()
                        .and_then(|host| host.connections.get(&(connection as usize)))
                        .is_none_or(|worker| {
                            worker.transaction() != transaction || worker.released()
                        })
                    {
                        return Err(crate::Error::InvalidOperation("DbTransactionExpired: recorded success belongs to a finished DB transaction; use external fresh for new work".into()));
                    }
                }
            }
        }
        if fresh {
            let submit = (|| -> std::result::Result<(), &'static str> {
                if timeout_ms == 0 || timeout_ms > 120000 {
                    return Err("DbDeadline");
                }
                if let Operation::CloseStatement { statement } = operation {
                    let host = self.database_host.as_mut().ok_or("DbClosed")?;
                    if host.statements.remove(&statement).is_none() {
                        return Err("DbClosed");
                    }
                    self.finish_async_external(id, Ok(json!({"adapter":"db","unit":true})))
                        .map_err(|_| "DbRecording")?;
                    return Ok(());
                }
                let path = if let Operation::Sqlite { path, .. } = &operation {
                    if path == ":memory:" {
                        Some(PathBuf::from(path))
                    } else {
                        for suffix in ["", "-wal", "-shm", "-journal"] {
                            let sidecar = format!("{path}{suffix}");
                            self.checked_path(&sidecar).map_err(|_| "DbPath")?;
                            if self.state.files.contains_key(&sidecar)
                                || self.virtual_files.contains_key(&sidecar)
                            {
                                return Err("DbPendingFile");
                            }
                        }
                        Some(self.checked_path(path).map_err(|_| "DbPath")?)
                    }
                } else {
                    None
                };
                let host = self.database_host.get_or_insert_with(Host::new);
                host.admission = match operation {
                    Operation::Postgres { .. } => 192 * 1024 * 1024,
                    Operation::Sqlite { .. } => 87 * 1024 * 1024,
                    _ => 12 * 1024 * 1024,
                };
                let budget = self.enforce_budget();
                self.database_host.as_mut().unwrap().admission = 0;
                budget.map_err(|_| "DbMemoryLimit")?;
                self.database_host.as_mut().unwrap().submit(
                    id,
                    operation,
                    path,
                    Duration::from_millis(timeout_ms),
                )
            })();
            if let Err(code) = submit {
                self.finish_async_external(id, Ok(failure(code, "NotSent", None)))?;
            }
        }
        Ok(id)
    }
    pub(crate) fn sanitise_database_result(&mut self, result: Value) -> Value {
        let patterns = match self.http_secret_patterns() {
            Ok(patterns) => patterns,
            Err(_) => return failure("DbSecretLimit", "ResponseReceived", None),
        };
        if patterns.is_empty() {
            return result;
        }
        let bytes = result
            .get("rows")
            .and_then(Value::as_array)
            .map_or(0, |rows| {
                rows.iter()
                    .filter_map(Value::as_array)
                    .flatten()
                    .map(|v| v["value"].as_str().map_or(32, str::len))
                    .sum::<usize>()
            });
        let estimate =
            bytes.saturating_mul(2) + patterns.iter().map(Vec::len).sum::<usize>() * 96 + 4096;
        if let Some(host) = &mut self.database_host {
            host.admission = estimate;
        }
        let budget = self.enforce_budget();
        if let Some(host) = &mut self.database_host {
            host.admission = 0;
        }
        if budget.is_err() {
            return failure("DbMemoryLimit", "ResponseReceived", None);
        }
        if self
            .charge_native_work(bytes + patterns.iter().map(Vec::len).sum::<usize>() + 1)
            .is_err()
        {
            return failure("DbWorkLimit", "ResponseReceived", None);
        }
        let automaton = match crate::network::SecretAutomaton::new(&patterns) {
            Ok(automaton) => automaton,
            Err(_) => return failure("DbSecretLimit", "ResponseReceived", None),
        };
        match protect_result(&result, &automaton) {
            Ok(false) => result,
            Ok(true) => failure("DbSecretResult", "ResponseReceived", None),
            Err(_) => failure("DbDecode", "ResponseReceived", None),
        }
    }
}

pub(crate) fn protect_result(
    result: &Value,
    automaton: &crate::network::SecretAutomaton,
) -> std::result::Result<bool, &'static str> {
    if let Some(columns) = result.get("columns").and_then(Value::as_array) {
        for column in columns {
            if (automaton.matcher().scan)(column.as_str().ok_or("DbDecode")?.as_bytes()) {
                return Ok(true);
            }
        }
    }
    if let Some(rows) = result.get("rows").and_then(Value::as_array) {
        for row in rows.iter().filter_map(Value::as_array) {
            for value in row {
                let bytes = match value["type"].as_str() {
                    Some("Bytes") => STANDARD
                        .decode(value["value"].as_str().ok_or("DbDecode")?)
                        .map_err(|_| "DbDecode")?,
                    Some("Text") => value["value"]
                        .as_str()
                        .ok_or("DbDecode")?
                        .as_bytes()
                        .to_vec(),
                    Some("Bool") => {
                        if value["value"].as_bool().ok_or("DbDecode")? {
                            b"1".to_vec()
                        } else {
                            b"0".to_vec()
                        }
                    }
                    Some("Int") => value["value"]
                        .as_i64()
                        .ok_or("DbDecode")?
                        .to_string()
                        .into_bytes(),
                    Some("Float") => f64::from_bits(value["bits"].as_u64().ok_or("DbDecode")?)
                        .to_string()
                        .into_bytes(),
                    Some("Null") => continue,
                    _ => return Err("DbDecode"),
                };
                if (automaton.matcher().scan)(&bytes) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
