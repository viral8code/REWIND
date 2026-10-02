//! Shared DB actor protocol; SQLite-specific values never cross into other adapters.
use super::{postgres, sqlite, Parameter};
use std::{path::PathBuf, time::Duration};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Closed,
    Busy,
    Limit,
    Deadline,
    Cancelled,
    Sql(i32),
    SqlState([u8; 5]),
    Worker,
    Configuration,
    Tls,
    Authentication([u8; 5]),
    Disconnected,
    Type,
}
pub(super) enum Command {
    Prepare {
        sql: String,
    },
    Execute {
        sql: String,
        params: Vec<Parameter>,
    },
    ExecuteMany {
        sql: String,
        params: Vec<Vec<Parameter>>,
    },
    Query {
        sql: String,
        params: Vec<Parameter>,
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
        rows: Vec<Vec<Parameter>>,
        done: bool,
    },
}
fn sqlite_error(error: sqlite::Error) -> Error {
    match error {
        sqlite::Error::Closed => Error::Closed,
        sqlite::Error::Busy => Error::Busy,
        sqlite::Error::Limit => Error::Limit,
        sqlite::Error::Deadline => Error::Deadline,
        sqlite::Error::Cancelled => Error::Cancelled,
        sqlite::Error::Sql(n) => Error::Sql(n),
        sqlite::Error::Worker => Error::Worker,
    }
}
fn sqlite_reply(reply: sqlite::Reply) -> Reply {
    match reply {
        sqlite::Reply::Statement {
            columns,
            parameters,
        } => Reply::Statement {
            columns,
            parameters,
        },
        sqlite::Reply::Unit => Reply::Unit,
        sqlite::Reply::Changed(n) => Reply::Changed(n),
        sqlite::Reply::Cursor(v) => Reply::Cursor(v),
        sqlite::Reply::Batch { rows, done } => Reply::Batch {
            rows: rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|v| {
                            use rusqlite::types::Value;
                            match v {
                                Value::Null => Parameter::Null,
                                Value::Integer(n) => Parameter::Int(n),
                                Value::Real(n) => Parameter::Float(n),
                                Value::Text(s) => Parameter::Text(s),
                                Value::Blob(v) => Parameter::Bytes(v),
                            }
                        })
                        .collect()
                })
                .collect(),
            done,
        },
    }
}
pub(super) enum Pending {
    Sqlite(sqlite::Pending),
    Postgres(postgres::Pending),
}
impl Pending {
    pub(super) fn poll(&self) -> Option<Result<Reply, Error>> {
        match self {
            Self::Sqlite(p) => p.poll().map(|r| r.map(sqlite_reply).map_err(sqlite_error)),
            Self::Postgres(p) => p.poll(),
        }
    }
    pub(super) fn cancel(&self) {
        match self {
            Self::Sqlite(p) => p.cancel(),
            Self::Postgres(p) => p.cancel(),
        }
    }
}
pub(super) enum Worker {
    Sqlite(sqlite::Worker),
    Postgres(postgres::Worker),
}
impl Worker {
    pub(super) fn sqlite(path: PathBuf, read_only: bool) -> Result<Self, Error> {
        sqlite::Worker::open(path, read_only)
            .map(Self::Sqlite)
            .map_err(sqlite_error)
    }
    pub(super) fn backend(&self) -> &'static str {
        match self {
            Self::Sqlite(_) => "sqlite",
            Self::Postgres(_) => "postgres",
        }
    }
    pub(super) fn reserved_bytes(&self) -> usize {
        match self {
            Self::Sqlite(_) => 75 * 1024 * 1024,
            Self::Postgres(_) => 192 * 1024 * 1024,
        }
    }
    pub(super) fn submit(&self, command: Command, timeout: Duration) -> Result<Pending, Error> {
        match self {
            Self::Postgres(w) => w.submit(command, timeout).map(Pending::Postgres),
            Self::Sqlite(w) => {
                let c = match command {
                    Command::Prepare { sql } => sqlite::Command::Prepare { sql },
                    Command::Execute { sql, params } => sqlite::Command::Execute {
                        sql,
                        params: params.into_iter().map(Parameter::sqlite).collect(),
                    },
                    Command::ExecuteMany { sql, params } => sqlite::Command::ExecuteMany {
                        sql,
                        params: params
                            .into_iter()
                            .map(|r| r.into_iter().map(Parameter::sqlite).collect())
                            .collect(),
                    },
                    Command::Query { sql, params } => sqlite::Command::Query {
                        sql,
                        params: params.into_iter().map(Parameter::sqlite).collect(),
                    },
                    Command::Next { rows, bytes } => sqlite::Command::Next { rows, bytes },
                    Command::CloseCursor => sqlite::Command::CloseCursor,
                    Command::Begin => sqlite::Command::Begin,
                    Command::Commit => sqlite::Command::Commit,
                    Command::Rollback => sqlite::Command::Rollback,
                    Command::Close => sqlite::Command::Close,
                };
                w.submit(c, timeout)
                    .map(Pending::Sqlite)
                    .map_err(sqlite_error)
            }
        }
    }
    pub(super) fn close(&self) {
        match self {
            Self::Sqlite(w) => w.close(),
            Self::Postgres(w) => w.close(),
        }
    }
    pub(super) fn released(&self) -> bool {
        match self {
            Self::Sqlite(w) => w.released(),
            Self::Postgres(w) => w.released(),
        }
    }
    pub(super) fn transaction(&self) -> u64 {
        match self {
            Self::Sqlite(w) => w.transaction(),
            Self::Postgres(w) => w.transaction(),
        }
    }
    pub(super) fn completion(&self) -> Option<Result<(), Error>> {
        match self {
            Self::Sqlite(w) => w.completion().map(|r| r.map_err(sqlite_error)),
            Self::Postgres(w) => w.completion(),
        }
    }
}
