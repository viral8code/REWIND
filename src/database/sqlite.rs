//! SQLite cursor ownership stays on one bounded worker's stack.
//! No self-referential statement storage, eager result materialization, or SQL replay.
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    limits::Limit,
    types::Value,
    Connection, OpenFlags,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

const SQL_LIMIT: usize = 65536;
const VALUE_LIMIT: usize = 1024 * 1024;
const BATCH_LIMIT: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Closed,
    Busy,
    Limit,
    Deadline,
    Cancelled,
    Sql(i32),
    Worker,
}
type Result<T> = std::result::Result<T, Error>;

pub(super) enum Command {
    ExecuteMany {
        sql: String,
        params: Vec<Vec<Value>>,
    },
    Prepare {
        sql: String,
    },
    Execute {
        sql: String,
        params: Vec<Value>,
    },
    Query {
        sql: String,
        params: Vec<Value>,
    },
    Next {
        rows: usize,
        bytes: usize,
    },
    CloseCursor,
    Begin,
    Commit,
    Rollback,
    Close,
}

#[derive(Debug)]
pub(super) enum Reply {
    Statement {
        columns: Vec<String>,
        parameters: usize,
    },
    Unit,
    Changed(u64),
    Cursor(Vec<String>),
    Batch {
        rows: Vec<Vec<Value>>,
        done: bool,
    },
}

struct Request {
    command: Command,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    response: mpsc::Sender<Result<Reply>>,
}

pub(super) struct Pending {
    receiver: mpsc::Receiver<Result<Reply>>,
    cancelled: Arc<AtomicBool>,
}

impl Pending {
    pub(super) fn poll(&self) -> Option<Result<Reply>> {
        match self.receiver.try_recv() {
            Ok(value) => Some(value),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(Error::Worker)),
        }
    }
    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

pub(super) struct Worker {
    sender: mpsc::SyncSender<Request>,
    closed: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    transaction: Arc<AtomicU64>,
    completion: Arc<Mutex<Option<Result<()>>>>,
    interrupt: rusqlite::InterruptHandle,
}

impl Worker {
    /// `path` must already be authorized by Runtime's root/sidecar policy.
    pub(super) fn open(path: PathBuf, read_only: bool) -> Result<Self> {
        let flags = if read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        } | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let connection = Connection::open_with_flags(path, flags).map_err(sql_error)?;
        configure(&connection)?;
        let interrupt = connection.get_interrupt_handle();
        let (sender, receiver) = mpsc::sync_channel(1);
        let closed = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(AtomicBool::new(false));
        let worker_closed = closed.clone();
        let worker_busy = busy.clone();
        let done = Arc::new(AtomicBool::new(false));
        let worker_done = done.clone();
        let transaction = Arc::new(AtomicU64::new(0));
        let worker_transaction = transaction.clone();
        let completion = Arc::new(Mutex::new(None));
        let worker_completion = completion.clone();
        std::thread::Builder::new()
            .name("rewind-sqlite".into())
            .stack_size(512 * 1024)
            .spawn(move || {
                let result = run(
                    connection,
                    receiver,
                    worker_closed,
                    worker_busy,
                    worker_transaction,
                );
                *worker_completion.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
                worker_done.store(true, Ordering::Release);
            })
            .map_err(|_| Error::Worker)?;
        Ok(Self {
            sender,
            closed,
            busy,
            done,
            transaction,
            completion,
            interrupt,
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
        let request = Request {
            command,
            deadline: Instant::now() + timeout,
            cancelled: cancelled.clone(),
            response,
        };
        if self.sender.try_send(request).is_err() {
            self.busy.store(false, Ordering::Release);
            return Err(Error::Worker);
        }
        Ok(Pending {
            receiver,
            cancelled,
        })
    }

    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.interrupt.interrupt();
        // A queued request wakes the actor even if an active cursor is idle.
        let (response, _) = mpsc::channel();
        let _ = self.sender.try_send(Request {
            command: Command::Close,
            deadline: Instant::now(),
            cancelled: Arc::new(AtomicBool::new(false)),
            response,
        });
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

fn sql_error(error: rusqlite::Error) -> Error {
    match error {
        rusqlite::Error::SqliteFailure(code, _) => Error::Sql(code.extended_code),
        _ => Error::Sql(1),
    }
}

fn configure(connection: &Connection) -> Result<()> {
    connection
        .busy_timeout(Duration::from_millis(50))
        .map_err(sql_error)?;
    connection
        .execute_batch("PRAGMA foreign_keys=ON; PRAGMA temp_store=MEMORY; PRAGMA cache_size=-2048; PRAGMA hard_heap_limit=67108864;")
        .map_err(sql_error)?;
    connection.set_prepared_statement_cache_capacity(16);
    for (limit, value) in [
        (Limit::SQLITE_LIMIT_LENGTH, VALUE_LIMIT as i32),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, SQL_LIMIT as i32),
        (Limit::SQLITE_LIMIT_COLUMN, 256),
        (Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 1024),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_VDBE_OP, 100000),
        (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
    ] {
        connection.set_limit(limit, value).map_err(sql_error)?;
    }
    connection
        .authorizer(Some(|context: AuthContext<'_>| match context.action {
            AuthAction::Attach { .. } | AuthAction::Detach { .. } => Authorization::Deny,
            AuthAction::Function { function_name }
                if function_name.eq_ignore_ascii_case("load_extension") =>
            {
                Authorization::Deny
            }
            // Keep connection policy immutable; read-only introspection remains available.
            AuthAction::Pragma {
                pragma_name,
                pragma_value,
            } if pragma_value.is_some()
                || [
                    "temp_store_directory",
                    "data_store_directory",
                    "writable_schema",
                ]
                .iter()
                .any(|name| pragma_name.eq_ignore_ascii_case(name)) =>
            {
                Authorization::Deny
            }
            _ => Authorization::Allow,
        }))
        .map_err(sql_error)
}

fn validate(command: &Command) -> Result<()> {
    match command {
        Command::ExecuteMany { sql, params } => {
            if sql.is_empty() || sql.len() > SQL_LIMIT || sql.contains('\0') || params.len() > 1024
            {
                return Err(Error::Limit);
            }
            let mut total = 0usize;
            for row in params {
                if row.len() > 1024 {
                    return Err(Error::Limit);
                }
                for value in row {
                    total = total.checked_add(value_size(value)).ok_or(Error::Limit)?;
                    if matches!(value,Value::Real(v) if !v.is_finite()) {
                        return Err(Error::Limit);
                    }
                }
            }
            if total > VALUE_LIMIT {
                return Err(Error::Limit);
            }
        }
        Command::Prepare { sql }
            if sql.is_empty() || sql.len() > SQL_LIMIT || sql.contains('\0') =>
        {
            return Err(Error::Limit)
        }
        Command::Execute { sql, params } | Command::Query { sql, params } => {
            if sql.is_empty() || sql.len() > SQL_LIMIT || sql.contains('\0') || params.len() > 1024
            {
                return Err(Error::Limit);
            }
            let bytes = params
                .iter()
                .try_fold(0usize, |total, value| total.checked_add(value_size(value)))
                .ok_or(Error::Limit)?;
            if bytes > VALUE_LIMIT
                || params
                    .iter()
                    .any(|v| matches!(v, Value::Real(n) if !n.is_finite()))
            {
                return Err(Error::Limit);
            }
        }
        Command::Next { rows, bytes }
            if *rows == 0 || *rows > 64 || *bytes < VALUE_LIMIT || *bytes > BATCH_LIMIT =>
        {
            return Err(Error::Limit)
        }
        _ => (),
    }
    Ok(())
}

fn value_size(value: &Value) -> usize {
    match value {
        Value::Text(v) => v.len() + 32,
        Value::Blob(v) => v.len() + 32,
        _ => 32,
    }
}
fn wire_size(value: &Value) -> usize {
    match value {
        Value::Text(value) => {
            64 + value
                .bytes()
                .map(|b| match b {
                    b'"' | b'\\' | b'\n' | b'\r' | b'\t' | 8 | 12 => 2,
                    0..=31 => 6,
                    _ => 1,
                })
                .sum::<usize>()
        }
        Value::Blob(value) => 64 + value.len().div_ceil(3) * 4,
        _ => 64,
    }
}

fn guard(connection: &Connection, request: &Request, closed: Arc<AtomicBool>) -> Result<()> {
    connection
        .busy_timeout(
            request
                .deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50)),
        )
        .map_err(sql_error)?;
    let deadline = request.deadline;
    let cancelled = request.cancelled.clone();
    connection
        .progress_handler(
            1000,
            Some(move || {
                closed.load(Ordering::Acquire)
                    || cancelled.load(Ordering::Acquire)
                    || Instant::now() >= deadline
            }),
        )
        .map_err(sql_error)
}

fn respond(request: Request, result: Result<Reply>, busy: &AtomicBool) {
    let result = if matches!(result, Err(Error::Sql(_))) {
        match check(&request) {
            Err(error) => Err(error),
            Ok(()) => result,
        }
    } else {
        result
    };
    // Clear before notifying: the receiver may submit the next cursor read immediately.
    busy.store(false, Ordering::Release);
    let _ = request.response.send(result);
}

fn run(
    connection: Connection,
    receiver: mpsc::Receiver<Request>,
    closed: Arc<AtomicBool>,
    busy: Arc<AtomicBool>,
    transaction: Arc<AtomicU64>,
) -> Result<()> {
    let mut closing_request = None;
    while let Ok(request) = receiver.recv() {
        if closed.load(Ordering::Acquire) || matches!(request.command, Command::Close) {
            closed.store(true, Ordering::Release);
            closing_request = Some(request);
            break;
        }
        if let Err(error) = check(&request) {
            respond(request, Err(error), &busy);
            continue;
        }
        if let Err(error) = guard(&connection, &request, closed.clone()) {
            respond(request, Err(error), &busy);
            continue;
        }
        if let Command::Query {
            ref sql,
            ref params,
        } = request.command
        {
            let query = connection.prepare_cached(sql).map_err(sql_error);
            match query {
                Err(error) => respond(request, Err(error), &busy),
                Ok(mut statement) => {
                    if statement.column_count() == 0 {
                        respond(request, Err(Error::Sql(21)), &busy);
                        continue;
                    }
                    let columns = statement
                        .column_names()
                        .into_iter()
                        .map(str::to_owned)
                        .collect();
                    match statement
                        .query(rusqlite::params_from_iter(params.iter()))
                        .map_err(sql_error)
                    {
                        Err(error) => respond(request, Err(error), &busy),
                        Ok(mut rows) => {
                            let cursor_deadline = request.deadline;
                            respond(request, Ok(Reply::Cursor(columns)), &busy);
                            while !closed.load(Ordering::Acquire) {
                                let Some(remaining) =
                                    cursor_deadline.checked_duration_since(Instant::now())
                                else {
                                    break;
                                };
                                let Ok(mut next) = receiver.recv_timeout(remaining) else {
                                    break;
                                };
                                next.deadline = next.deadline.min(cursor_deadline);
                                if matches!(next.command, Command::Close) {
                                    closed.store(true, Ordering::Release);
                                    closing_request = Some(next);
                                    break;
                                }
                                if matches!(next.command, Command::CloseCursor) {
                                    respond(next, Ok(Reply::Unit), &busy);
                                    break;
                                }
                                let result = match check(&next)
                                    .and_then(|_| guard(&connection, &next, closed.clone()))
                                {
                                    Err(error) => Err(error),
                                    Ok(()) => match next.command {
                                        Command::Next { rows: count, bytes } => {
                                            batch(&mut rows, count, bytes)
                                        }
                                        _ => Err(Error::Busy),
                                    },
                                };
                                let terminal = (matches!(next.command, Command::Next { .. })
                                    && result.is_err())
                                    || matches!(result, Ok(Reply::Batch { done: true, .. }));
                                respond(next, result, &busy);
                                if terminal {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if closed.load(Ordering::Acquire) {
                break;
            }
            continue;
        }
        let result = match &request.command {
            Command::ExecuteMany { sql, params } => (|| {
                let mut statement = connection.prepare_cached(sql).map_err(sql_error)?;
                let mut changed = 0u64;
                for row in params {
                    check(&request)?;
                    if closed.load(Ordering::Acquire) {
                        return Err(Error::Closed);
                    }
                    changed = changed
                        .checked_add(
                            statement
                                .execute(rusqlite::params_from_iter(row.iter()))
                                .map_err(sql_error)? as u64,
                        )
                        .ok_or(Error::Limit)?;
                }
                Ok(Reply::Changed(changed))
            })(),
            Command::Prepare { sql } => {
                connection
                    .prepare_cached(sql)
                    .map_err(sql_error)
                    .map(|s| Reply::Statement {
                        columns: s.column_names().into_iter().map(str::to_owned).collect(),
                        parameters: s.parameter_count(),
                    })
            }
            Command::Execute { sql, params } => connection
                .prepare_cached(sql)
                .map_err(sql_error)
                .and_then(|mut s| {
                    s.execute(rusqlite::params_from_iter(params.iter()))
                        .map_err(sql_error)
                })
                .map(|n| Reply::Changed(n as u64)),
            Command::Begin => connection
                .execute("BEGIN", [])
                .map_err(sql_error)
                .map(|_| Reply::Unit),
            Command::Commit => connection
                .execute("COMMIT", [])
                .map_err(sql_error)
                .map(|_| Reply::Unit),
            Command::Rollback => connection
                .execute("ROLLBACK", [])
                .map_err(sql_error)
                .map(|_| Reply::Unit),
            Command::CloseCursor => Ok(Reply::Unit),
            _ => Err(Error::Closed),
        };
        let active = u64::from(!connection.is_autocommit());
        let previous = transaction.load(Ordering::Relaxed);
        if (previous & 1) != active {
            transaction.store(
                (previous & !1).saturating_add(2) | active,
                Ordering::Release,
            );
        }
        respond(request, result, &busy);
    }
    // Disable expired/cancelled query guard before rollback.
    let _ = connection.progress_handler(0, None::<fn() -> bool>);
    let rollback = if !connection.is_autocommit() {
        connection
            .execute("ROLLBACK", [])
            .map(|_| ())
            .map_err(sql_error)
    } else {
        Ok(())
    };
    let close = connection.close().map_err(|(_, e)| sql_error(e));
    closed.store(true, Ordering::Release);
    let result = rollback.and(close);
    if let Some(request) = closing_request {
        respond(request, result.map(|_| Reply::Unit), &busy);
    }
    result
}

fn check(request: &Request) -> Result<()> {
    if request.cancelled.load(Ordering::Acquire) {
        Err(Error::Cancelled)
    } else if Instant::now() >= request.deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}

fn batch(rows: &mut rusqlite::Rows<'_>, count: usize, max_bytes: usize) -> Result<Reply> {
    let mut output = Vec::with_capacity(count);
    let mut bytes = 0usize;
    for _ in 0..count {
        let Some(row) = rows.next().map_err(sql_error)? else {
            return Ok(Reply::Batch {
                rows: output,
                done: true,
            });
        };
        let mut values = Vec::with_capacity(row.as_ref().column_count());
        for index in 0..row.as_ref().column_count() {
            let value: Value = row.get(index).map_err(sql_error)?;
            bytes = bytes.checked_add(wire_size(&value)).ok_or(Error::Limit)?;
            if bytes > max_bytes {
                return Err(Error::Limit);
            }
            values.push(value);
        }
        output.push(values);
    }
    Ok(Reply::Batch {
        rows: output,
        done: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn call(worker: &Worker, command: Command) -> Result<Reply> {
        let pending = worker.submit(command, Duration::from_secs(5))?;
        pending
            .receiver
            .recv_timeout(Duration::from_secs(6))
            .map_err(|_| Error::Worker)?
    }
    fn memory() -> Worker {
        Worker::open(":memory:".into(), false).unwrap()
    }
    fn execute(worker: &Worker, sql: &str, params: Vec<Value>) -> Result<Reply> {
        call(
            worker,
            Command::Execute {
                sql: sql.into(),
                params,
            },
        )
    }
    fn query(worker: &Worker, sql: &str, params: Vec<Value>) -> Result<Reply> {
        call(
            worker,
            Command::Query {
                sql: sql.into(),
                params,
            },
        )
    }
    #[test]
    fn bound_parameters_binary_null_and_lazy_cursor() {
        let worker = memory();
        execute(
            &worker,
            "CREATE TABLE data (id INTEGER, text TEXT, body BLOB)",
            vec![],
        )
        .unwrap();
        let malicious = "'); DROP TABLE data; --";
        execute(
            &worker,
            "INSERT INTO data VALUES (?1, ?2, ?3)",
            vec![
                Value::Integer(7),
                Value::Text(malicious.into()),
                Value::Blob(vec![0, 255]),
            ],
        )
        .unwrap();
        execute(&worker, "INSERT INTO data VALUES (8, NULL, NULL)", vec![]).unwrap();
        assert!(
            matches!(query(&worker, "SELECT id, text, body FROM data ORDER BY id", vec![]).unwrap(), Reply::Cursor(c) if c == ["id", "text", "body"])
        );
        assert_eq!(
            execute(&worker, "DELETE FROM data", vec![]).unwrap_err(),
            Error::Busy
        );
        match call(
            &worker,
            Command::Next {
                rows: 1,
                bytes: VALUE_LIMIT,
            },
        )
        .unwrap()
        {
            Reply::Batch { rows, done } => {
                assert!(!done);
                assert_eq!(
                    rows[0],
                    vec![
                        Value::Integer(7),
                        Value::Text(malicious.into()),
                        Value::Blob(vec![0, 255])
                    ]
                );
            }
            _ => panic!("expected batch"),
        }
        match call(
            &worker,
            Command::Next {
                rows: 64,
                bytes: VALUE_LIMIT,
            },
        )
        .unwrap()
        {
            Reply::Batch { rows, done } => {
                assert!(done);
                assert_eq!(rows[0], vec![Value::Integer(8), Value::Null, Value::Null]);
            }
            _ => panic!("expected batch"),
        }
        assert!(matches!(
            execute(&worker, "DELETE FROM data", vec![]).unwrap(),
            Reply::Changed(2)
        ));
    }
    #[test]
    fn transaction_commit_rollback_and_multiple_statement_rejection() {
        let worker = memory();
        execute(&worker, "CREATE TABLE data (id INTEGER UNIQUE)", vec![]).unwrap();
        call(&worker, Command::Begin).unwrap();
        execute(&worker, "INSERT INTO data VALUES (1)", vec![]).unwrap();
        call(&worker, Command::Rollback).unwrap();
        call(&worker, Command::Begin).unwrap();
        execute(&worker, "INSERT INTO data VALUES (2)", vec![]).unwrap();
        call(&worker, Command::Commit).unwrap();
        assert!(execute(
            &worker,
            "INSERT INTO data VALUES (3); INSERT INTO data VALUES (4)",
            vec![]
        )
        .is_err());
        query(&worker, "SELECT id FROM data ORDER BY id", vec![]).unwrap();
        match call(
            &worker,
            Command::Next {
                rows: 64,
                bytes: VALUE_LIMIT,
            },
        )
        .unwrap()
        {
            Reply::Batch { rows, done } => {
                assert!(done);
                assert_eq!(rows, vec![vec![Value::Integer(2)]]);
            }
            _ => panic!("expected batch"),
        }
    }
    #[test]
    fn authorizer_blocks_host_escape_and_configuration_mutation() {
        let worker = memory();
        for sql in [
            "ATTACH DATABASE ':memory:' AS outside",
            "PRAGMA writable_schema=ON",
            "PRAGMA busy_timeout=999999",
            "SELECT load_extension('anything')",
        ] {
            assert!(execute(&worker, sql, vec![]).is_err(), "{sql}");
        }
        execute(&worker, "CREATE TABLE safe (id INTEGER)", vec![]).unwrap();
    }
    #[test]
    fn deadline_interrupts_expensive_query_and_connection_remains_usable() {
        let worker = memory();
        query(&worker, "WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT sum(x) FROM n", vec![]).unwrap();
        let pending = worker
            .submit(
                Command::Next {
                    rows: 1,
                    bytes: VALUE_LIMIT,
                },
                Duration::from_millis(5),
            )
            .unwrap();
        assert!(pending
            .receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .is_err());
        execute(&worker, "CREATE TABLE after_timeout (id INTEGER)", vec![]).unwrap();
        worker.close();
        assert_eq!(
            execute(&worker, "INSERT INTO after_timeout VALUES(1)", vec![]).unwrap_err(),
            Error::Closed
        );
    }
    #[test]
    fn explicit_cursor_close_releases_statement_and_cancellation_is_observable() {
        let worker = memory();
        query(&worker, "SELECT 1", vec![]).unwrap();
        call(&worker, Command::CloseCursor).unwrap();
        let pending = worker
            .submit(
                Command::Query {
                    sql: "SELECT 2".into(),
                    params: vec![],
                },
                Duration::from_secs(1),
            )
            .unwrap();
        // Cancellation can race completion; it must never leave a connection locked.
        pending.cancel();
        let result = pending
            .receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        if result.is_ok() {
            call(&worker, Command::CloseCursor).unwrap();
        }
        execute(&worker, "CREATE TABLE usable (id INTEGER)", vec![]).unwrap();
    }
}
