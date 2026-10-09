//! REWIND's transactional state and I/O core.
//!
//! Checkpoints share immutable roots. A mutation clones only the map it changes;
//! file contents are reference counted. The external input and time observations
//! live outside checkpoints, while their cursors live inside them.

pub mod gui;
pub mod journal;
pub mod map_storage;
pub mod storage;
pub mod transforms;
use map_storage::PersistentMap;
use storage::{HeapStore, PagedValues};
pub mod bigint;
pub mod csv_stream;
pub mod database;
pub mod datetime;
pub mod decimal;
pub mod external;
pub mod http_server;
pub mod json_stream;
pub mod native_resources;
pub mod network;
pub mod numeric;
pub mod regular;
mod replay;
mod resolver;
mod shared_payload;
pub mod suffix;
pub mod tcp;
pub mod text_storage;
pub mod unicode;
use journal::{Journal, Segment};

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MapKey {
    Regex(regular::Pattern),
    Instant(datetime::Instant),
    Duration(datetime::Duration),
    BigInt(bigint::IntegerValue),
    Decimal(#[serde(deserialize_with = "decimal_key")] decimal::DecimalValue),
    Bool(bool),
    Int(i64),
    Float(u64),
    Text(String),
    Bytes(Vec<u8>),
}
fn decimal_key<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<decimal::DecimalValue, D::Error> {
    let value = <decimal::DecimalValue as serde::Deserialize>::deserialize(d)?;
    let canonical = value.canonical();
    if value.scale() != canonical.scale() || value.coefficient() != canonical.coefficient() {
        return Err(serde::de::Error::custom("noncanonical decimal map key"));
    }
    Ok(value)
}
impl PartialOrd for MapKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for MapKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn rank(k: &MapKey) -> u8 {
            match k {
                MapKey::Bool(_) => 0,
                MapKey::Int(_) => 1,
                MapKey::Float(_) => 2,
                MapKey::Text(_) => 3,
                MapKey::Bytes(_) => 4,
                MapKey::BigInt(_) => 5,
                MapKey::Decimal(_) => 6,
                MapKey::Instant(_) => 7,
                MapKey::Duration(_) => 8,
                MapKey::Regex(_) => 9,
            }
        }
        rank(self)
            .cmp(&rank(other))
            .then_with(|| match (self, other) {
                (MapKey::Regex(a), MapKey::Regex(b)) => a.cmp(b),
                (MapKey::Instant(a), MapKey::Instant(b)) => a.cmp(b),
                (MapKey::Duration(a), MapKey::Duration(b)) => a.cmp(b),
                (MapKey::Decimal(a), MapKey::Decimal(b)) => a.cmp(b),
                (MapKey::BigInt(a), MapKey::BigInt(b)) => a.cmp(b),
                (MapKey::Bool(a), MapKey::Bool(b)) => a.cmp(b),
                (MapKey::Int(a), MapKey::Int(b)) => a.cmp(b),
                (MapKey::Float(a), MapKey::Float(b)) => {
                    f64::from_bits(*a).total_cmp(&f64::from_bits(*b))
                }
                (MapKey::Text(a), MapKey::Text(b)) => a.cmp(b),
                (MapKey::Bytes(a), MapKey::Bytes(b)) => a.cmp(b),
                _ => std::cmp::Ordering::Equal,
            })
    }
}
impl MapKey {
    pub fn from_value(value: &Value) -> Option<Self> {
        Some(match value {
            Value::Regex(v) => Self::Regex(v.clone()),
            Value::Instant(v) => Self::Instant(*v),
            Value::Duration(v) => Self::Duration(*v),
            Value::Decimal(v) => Self::Decimal(v.canonical()),
            Value::BigInt(v) => Self::BigInt(v.clone()),
            Value::Bool(v) => Self::Bool(*v),
            Value::Int(v) => Self::Int(*v),
            Value::Float(v) => Self::Float(*v),
            Value::Text(v) => Self::Text(v.to_string()),
            Value::Bytes(v) => Self::Bytes(v.as_ref().clone()),
            _ => return None,
        })
    }
    pub fn value(&self) -> Value {
        match self {
            Self::Regex(v) => Value::Regex(v.clone()),
            Self::Instant(v) => Value::Instant(*v),
            Self::Duration(v) => Value::Duration(*v),
            Self::Decimal(v) => Value::Decimal(v.clone().into()),
            Self::BigInt(v) => Value::BigInt(v.clone()),
            Self::Bool(v) => Value::Bool(*v),
            Self::Int(v) => Value::Int(*v),
            Self::Float(v) => Value::Float(*v),
            Self::Text(v) => Value::Text(v.clone().into()),
            Self::Bytes(v) => Value::Bytes(v.clone().into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileFailure {
    pub code: String,
    pub path: String,
    pub causes: Vec<String>,
}
impl FileFailure {
    pub fn from_error(error: &Error, path: &str) -> Self {
        let code = match error {
            Error::MissingFile(_) => "NotFound".into(),
            Error::InvalidPath(_) => "InvalidPath".into(),
            Error::ExternalStateConflict(_) => "Conflict".into(),
            Error::Io(io) => format!("Io::{:?}", io.kind()),
            _ => "OperationFailed".into(),
        };
        let mut causes = vec![error.to_string()];
        let mut source = std::error::Error::source(error);
        while let Some(cause) = source {
            causes.push(cause.to_string());
            source = cause.source();
        }
        Self {
            code,
            path: path.into(),
            causes,
        }
    }
}
impl fmt::Display for FileFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({})",
            self.code,
            self.path,
            self.causes.join(" -> ")
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(u64),
    Text(text_storage::Text),
    Bytes(Arc<Vec<u8>>),
    NumericArray(numeric::Array),
    Regex(regular::Pattern),
    CsvStream(csv_stream::Reader),
    JsonStream(json_stream::Reader),
    Instant(datetime::Instant),
    Duration(datetime::Duration),
    BigInt(bigint::IntegerValue),
    Decimal(decimal::StoredDecimal),
    FileError(FileFailure),
    List(Vec<Value>),
    TypedList(String, PagedValues),
    Map(#[serde(with = "value_map_pairs")] PersistentMap),
    TypedMap(
        String,
        String,
        #[serde(with = "value_map_pairs")] PersistentMap,
    ),
    OrderedMap(String, String, Vec<(Value, Value)>),
    Struct(String, BTreeMap<String, Value>),
    Enum(String, String, Vec<(String, Value)>),
    Function(String, String),
    Closure(String, String, BTreeMap<String, Value>),
    CellRef(u64),
    Option(Option<Box<Value>>),
    Result(std::result::Result<Box<Value>, Box<Value>>),
    HeapRef(u64),
    Handle(u64),
    Null,
}

/// Borrowed, bounded mark traversal; identities are scoped to one collection.
pub(crate) struct HeapTraversal<'a> {
    pending: Vec<&'a Value>,
    seen: HashSet<usize>,
    work: usize,
    limit: usize,
}
impl<'a> HeapTraversal<'a> {
    fn failure() -> Error {
        Error::InvalidOperation("NativeWorkBudgetExceeded: heap collection".into())
    }
    pub(crate) fn charge(&mut self, units: usize) -> Result<()> {
        let next = self.work.checked_add(units).ok_or_else(Self::failure)?;
        if next > self.limit || self.pending.len() > self.limit - next {
            return Err(Self::failure());
        }
        self.work = next;
        Ok(())
    }
    pub(crate) fn visit(&mut self, identity: usize) -> Result<bool> {
        self.charge(1)?;
        Ok(self.seen.insert(identity))
    }
    pub(crate) fn push(&mut self, value: &'a Value) -> Result<()> {
        if self.pending.len() >= self.limit.saturating_sub(self.work) {
            return Err(Self::failure());
        }
        self.pending.push(value);
        Ok(())
    }
}

mod value_map_pairs {
    use super::*;
    use serde::{Deserialize, Serialize};
    pub fn serialize<S: serde::Serializer>(
        m: &PersistentMap,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        m.iter().collect::<Vec<_>>().serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<PersistentMap, D::Error> {
        let entries = Vec::<(MapKey, Value)>::deserialize(d)?;
        let len = entries.len();
        let map = entries.into_iter().collect::<BTreeMap<_, _>>();
        if map.len() != len {
            return Err(serde::de::Error::custom("duplicate value map key"));
        }
        Ok(map.into())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(v) => write!(f, "{v}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(bits) => write!(f, "{}", f64::from_bits(*bits)),
            Value::Text(s) => write!(f, "{s}"),
            Value::Bytes(v) => write!(f, "{v:?}"),
            Value::NumericArray(v) => write!(f, "{v:?}"),
            Value::BigInt(v) => write!(f, "{v}"),
            Value::Regex(v) => write!(f, "{v}"),
            Value::CsvStream(v) => write!(f, "{v}"),
            Value::JsonStream(v) => write!(f, "{v}"),
            Value::Instant(v) => write!(f, "{v}"),
            Value::Duration(v) => write!(f, "{v}"),
            Value::Decimal(v) => write!(f, "{v}"),
            Value::FileError(v) => write!(f, "{v}"),
            Value::List(v) => {
                write!(f, "[")?;
                for (i, item) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::TypedList(_, v) => {
                write!(f, "[")?;
                for (i, item) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::HeapRef(id) => write!(f, "<object:{id}>"),
            Value::Map(v) => write!(f, "{v:?}"),
            Value::TypedMap(_, _, v) => write!(f, "{v:?}"),
            Value::OrderedMap(_, _, v) => write!(f, "{v:?}"),
            Value::Struct(name, v) => write!(f, "{name}{v:?}"),
            Value::Enum(name, variant, fields) => {
                write!(f, "{name}::{variant}")?;
                if !fields.is_empty() {
                    write!(f, "{fields:?}")?;
                }
                Ok(())
            }
            Value::Function(name, _) => write!(f, "<fn:{name}>"),
            Value::Closure(name, _, _) => write!(f, "<closure:{name}>"),
            Value::CellRef(id) => write!(f, "<cell:{id}>"),
            Value::Option(Some(v)) => write!(f, "Some({v})"),
            Value::Option(None) => write!(f, "None"),
            Value::Result(Ok(v)) => write!(f, "Ok({v})"),
            Value::Result(Err(v)) => write!(f, "Err({v})"),
            Value::Handle(id) => write!(f, "<handle:{id}>"),
            Value::Null => write!(f, "null"),
        }
    }
}

/// Terminal publish failure. A failed Host call may itself have changed state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PublishFailure {
    pub phase: String,
    pub path: Option<String>,
    pub applied: Vec<String>,
    pub cause: String,
    pub retryable: bool,
}

#[derive(Debug)]
pub enum Error {
    Diagnostic(Box<DiagnosticRecord>),
    Io(io::Error),
    InvalidPath(String),
    MissingCheckpoint(String),
    MissingFile(String),
    MissingHandle(u64),
    ExternalStateConflict(String),
    TaintedCheckpoint(String),
    HistoryBudgetExceeded,
    PublishPartiallyApplied(String),
    InvalidOperation(String),
}

impl Error {
    pub fn publish_report(&self) -> Option<PublishFailure> {
        let text = match self {
            Self::PublishPartiallyApplied(text) => text.as_str(),
            Self::Diagnostic(d) if d.code == "PublishPartiallyApplied" => {
                d.message.strip_prefix("publish partially applied: ")?
            }
            _ => return None,
        };
        serde_json::from_str(text).ok()
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Diagnostic(d) => {
                write!(
                    f,
                    "invalid operation: {}{}:{}: {}",
                    if d.source.is_empty() {
                        String::new()
                    } else {
                        format!("{}:", d.source)
                    },
                    d.line,
                    d.column,
                    d.message
                )?;
                for cause in &d.causes {
                    write!(f, "; cleanup: {}", cause.message)?;
                }
                Ok(())
            }
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::InvalidPath(s) => write!(f, "invalid path: {s}"),
            Error::MissingCheckpoint(s) => write!(f, "unknown checkpoint: {s}"),
            Error::MissingFile(s) => write!(f, "file not found: {s}"),
            Error::MissingHandle(n) => write!(f, "unknown file handle: {n}"),
            Error::ExternalStateConflict(s) => write!(f, "external state conflict: {s}"),
            Error::TaintedCheckpoint(s) => write!(f, "tainted checkpoint: {s}"),
            Error::HistoryBudgetExceeded => write!(f, "history budget exceeded"),
            Error::PublishPartiallyApplied(s) => write!(f, "publish partially applied: {s}"),
            Error::InvalidOperation(s) => write!(f, "invalid operation: {s}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(error) = self {
            Some(error)
        } else {
            None
        }
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticRecord {
    pub code: String,
    pub message: String,
    pub source: String,
    pub line: usize,
    pub column: usize,
    pub task_id: Option<u64>,
    /// Innermost failure first, followed by caller locations. No local values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<DiagnosticFrame>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub frames_truncated: bool,
    pub causes: Vec<DiagnosticRecord>,
    pub wait_edges: Vec<WaitEdge>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticFrame {
    pub function: String,
    pub source: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum WaitTarget {
    Task(u64),
    Channel(u64),
    Group(u64),
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WaitEdge {
    pub task: u64,
    pub target: WaitTarget,
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
const FILE_PAGE_SIZE: usize = 4096;

struct StagedFiles(Vec<(PathBuf, PathBuf)>);
impl Drop for StagedFiles {
    fn drop(&mut self) {
        for (temp, _) in &self.0 {
            let _ = fs::remove_file(temp);
        }
    }
}

#[derive(Clone)]
struct PagedFile {
    len: usize,
    pages: Arc<BTreeMap<usize, Arc<Segment>>>,
}

impl fmt::Debug for PagedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PagedFile")
            .field("len", &self.len)
            .field("pages", &self.pages.len())
            .finish()
    }
}

impl PagedFile {
    fn from_bytes(bytes: &[u8]) -> Self {
        let pages = bytes
            .chunks(FILE_PAGE_SIZE)
            .enumerate()
            .map(|(index, chunk)| (index, Arc::new(Segment::new(chunk.to_vec()))))
            .collect();
        Self {
            len: bytes.len(),
            pages: Arc::new(pages),
        }
    }
    fn to_vec(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(self.len);
        for page in self.pages.values() {
            page.write_to(&mut bytes)?;
        }
        bytes.truncate(self.len);
        Ok(bytes)
    }
    /// Materialize only the requested pages, including when pages are spilled.
    fn read_range(&self, offset: usize, count: usize) -> Result<Vec<u8>> {
        if offset >= self.len || count == 0 {
            return Ok(Vec::new());
        }
        let end = offset.saturating_add(count).min(self.len);
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(end - offset)
            .map_err(|_| Error::InvalidOperation("FileAllocation".into()))?;
        let first = offset / FILE_PAGE_SIZE;
        let last = (end - 1) / FILE_PAGE_SIZE;
        for index in first..=last {
            let page = self
                .pages
                .get(&index)
                .ok_or_else(|| Error::InvalidOperation("FilePageMissing".into()))?
                .bytes()?;
            let start = if index == first {
                offset % FILE_PAGE_SIZE
            } else {
                0
            };
            let stop = if index == last {
                (end - 1) % FILE_PAGE_SIZE + 1
            } else {
                FILE_PAGE_SIZE
            };
            let slice = page
                .get(start..stop)
                .ok_or_else(|| Error::InvalidOperation("FilePageCorrupt".into()))?;
            bytes.extend_from_slice(slice);
        }
        Ok(bytes)
    }
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        let pages = Arc::make_mut(&mut self.pages);
        let mut rest = bytes;
        if self.len % FILE_PAGE_SIZE != 0 && !rest.is_empty() {
            let index = self.len / FILE_PAGE_SIZE;
            let mut page = pages.get(&index).expect("last page exists").bytes()?;
            let count = rest.len().min(FILE_PAGE_SIZE - page.len());
            page.extend_from_slice(&rest[..count]);
            pages.insert(index, Arc::new(Segment::new(page)));
            rest = &rest[count..];
        }
        let mut index = self.len.div_ceil(FILE_PAGE_SIZE);
        for chunk in rest.chunks(FILE_PAGE_SIZE) {
            pages.insert(index, Arc::new(Segment::new(chunk.to_vec())));
            index += 1;
        }
        self.len += bytes.len();
        Ok(())
    }
    fn write_at(&mut self, offset: usize, bytes: &[u8]) -> Result<()> {
        if offset > self.len {
            self.truncate(offset)?;
        }
        let pages = Arc::make_mut(&mut self.pages);
        let mut cursor = offset;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let index = cursor / FILE_PAGE_SIZE;
            let within = cursor % FILE_PAGE_SIZE;
            let count = remaining.len().min(FILE_PAGE_SIZE - within);
            let mut page = pages
                .get(&index)
                .map(|p| p.bytes())
                .transpose()?
                .unwrap_or_default();
            if page.len() < within + count {
                page.resize(within + count, 0);
            }
            page[within..within + count].copy_from_slice(&remaining[..count]);
            pages.insert(index, Arc::new(Segment::new(page)));
            cursor += count;
            remaining = &remaining[count..];
        }
        self.len = self.len.max(cursor);
        Ok(())
    }
    fn truncate(&mut self, len: usize) -> Result<()> {
        if len >= self.len {
            let zeros = [0u8; FILE_PAGE_SIZE];
            while self.len < len {
                self.append(&zeros[..(len - self.len).min(FILE_PAGE_SIZE)])?;
            }
            return Ok(());
        }
        let pages = Arc::make_mut(&mut self.pages);
        pages.retain(|index, _| index.saturating_mul(FILE_PAGE_SIZE) < len);
        if len % FILE_PAGE_SIZE != 0 {
            let index = len / FILE_PAGE_SIZE;
            let mut page = pages.get(&index).expect("last page exists").bytes()?;
            page.truncate(len % FILE_PAGE_SIZE);
            pages.insert(index, Arc::new(Segment::new(page)));
        }
        self.len = len;
        Ok(())
    }
}

#[cfg(test)]
mod page_tests {
    use super::*;
    #[test]
    fn file_range_reads_skip_unrequested_spilled_pages() {
        let mut data = vec![b'A'; FILE_PAGE_SIZE];
        data.extend(vec![b'B'; FILE_PAGE_SIZE]);
        data.extend(vec![b'C'; FILE_PAGE_SIZE]);
        let file = PagedFile::from_bytes(&data);
        let last = &file.pages[&2];
        last.spill().unwrap();
        let path = std::env::temp_dir().join(format!(
            "rewind-journal-{}-{}.tmp",
            std::process::id(),
            last.id
        ));
        std::fs::remove_file(path).unwrap();
        assert_eq!(file.read_range(FILE_PAGE_SIZE - 2, 4).unwrap(), b"AABB");
        assert!(file.read_range(FILE_PAGE_SIZE * 2, 1).is_err());
        assert!(file.read_range(usize::MAX, usize::MAX).unwrap().is_empty());
        assert!(file.read_range(0, 0).unwrap().is_empty());
    }
    #[test]
    fn copy_on_write_reuses_unchanged_file_pages() {
        let mut original = PagedFile::from_bytes(&vec![b'A'; FILE_PAGE_SIZE * 2]);
        let snapshot = original.clone();
        original.truncate(FILE_PAGE_SIZE + 2).unwrap();
        assert!(Arc::ptr_eq(&original.pages[&0], &snapshot.pages[&0]));
        assert!(!Arc::ptr_eq(&original.pages[&1], &snapshot.pages[&1]));
        assert_eq!(snapshot.to_vec().unwrap().len(), FILE_PAGE_SIZE * 2);
        assert_eq!(original.to_vec().unwrap().len(), FILE_PAGE_SIZE + 2);
    }
    #[test]
    fn file_pages_spill_and_replay_after_checkpoint() {
        let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
        runtime
            .set_budget(ResourceBudget {
                history_memory: FILE_PAGE_SIZE,
                history_storage: FILE_PAGE_SIZE * 4,
                spill_threshold: FILE_PAGE_SIZE,
            })
            .unwrap();
        let path = format!(
            "rewind-page-test-{}-{}.bin",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        );
        runtime
            .write_file(&path, vec![b'A'; FILE_PAGE_SIZE * 2])
            .unwrap();
        let file = runtime.state.files[&path].as_ref().unwrap();
        assert!(file.pages.values().any(|page| page.usage().1 > 0));
        runtime.commit("base").unwrap();
        runtime.truncate_file(&path, FILE_PAGE_SIZE + 1).unwrap();
        runtime.revert("base").unwrap();
        assert_eq!(
            runtime.read_file(&path).unwrap(),
            vec![b'A'; FILE_PAGE_SIZE * 2]
        );
    }
}

#[derive(Clone, Debug)]
pub struct FileHandle {
    pub path: String,
    pub mode: FileMode,
    pub position: usize,
    pub buffer: Vec<u8>,
    pub virtual_file_version: u64,
    snapshot: Option<Arc<PagedFile>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallFrame {
    pub return_pc: usize,
    pub locals: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileMode {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResourceBudget {
    pub history_memory: usize,
    pub history_storage: usize,
    pub spill_threshold: usize,
}
impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            history_memory: 512 * 1024 * 1024,
            history_storage: 8 * 1024 * 1024 * 1024,
            spill_threshold: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub struct State {
    pub program_counter: usize,
    pub stack: Arc<Vec<Value>>,
    pub call_frames: Arc<Vec<CallFrame>>,
    pub globals: Arc<BTreeMap<String, Value>>,
    pub heap: Arc<HeapStore>,
    files: Arc<BTreeMap<String, Option<Arc<PagedFile>>>>,
    directories: Arc<BTreeMap<String, bool>>,
    pub handles: Arc<BTreeMap<u64, FileHandle>>,
    pub stdout: Journal,
    pub stderr: Journal,
    pub stdin_cursor: usize,
    gui_cursor: usize,
    gui_windows_cursor: usize,
    external_cursor: usize,
    external_poll_cursor: usize,
    native_owners: Arc<BTreeMap<u64, u64>>,
    gui_pending: Option<gui::Request>,
    gui_windows_pending: Arc<BTreeMap<String, gui::Request>>,
    pub byte_cursor: usize,
    pub time_cursor: usize,
    pub args_cursor: usize,
    pub env_cursor: usize,
    pub directory_cursor: usize,
    pub file_epoch: u64,
    file_operations: Arc<BTreeMap<String, u64>>,
    directory_operations: Arc<BTreeMap<String, u64>>,
    pub random_state: u64,
    pub next_heap_id: u64,
    pub next_handle_id: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            program_counter: 0,
            stack: Arc::new(Vec::new()),
            call_frames: Arc::new(Vec::new()),
            globals: Arc::new(BTreeMap::new()),
            heap: Arc::new(HeapStore::default()),
            files: Arc::new(BTreeMap::new()),
            directories: Arc::new(BTreeMap::new()),
            handles: Arc::new(BTreeMap::new()),
            stdout: Journal::default(),
            stderr: Journal::default(),
            stdin_cursor: 0,
            gui_cursor: 0,
            gui_windows_cursor: 0,
            external_cursor: 0,
            external_poll_cursor: 0,
            native_owners: Arc::new(BTreeMap::new()),
            gui_pending: None,
            gui_windows_pending: Arc::new(BTreeMap::new()),
            byte_cursor: 0,
            time_cursor: 0,
            args_cursor: 0,
            env_cursor: 0,
            directory_cursor: 0,
            file_epoch: 0,
            file_operations: Arc::new(BTreeMap::new()),
            directory_operations: Arc::new(BTreeMap::new()),
            random_state: 0x4d595df4d0f33173,
            next_heap_id: 1,
            next_handle_id: 1,
        }
    }
}

#[derive(Clone)]
struct Checkpoint {
    state: State,
    parent: Option<String>,
    tainted: bool,
}

#[derive(Clone)]
pub struct BranchAnchor {
    state: Arc<State>,
    parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HostVersion {
    len: u64,
    modified: Option<SystemTime>,
    content_hash: u128,
}

/// Host metadata is captured on open. Blocks are copied only when read through
/// a handle. A whole-file read intentionally loads every block.
#[derive(Clone)]
struct Observation {
    version: Option<HostVersion>,
    blocks: BTreeMap<usize, Arc<Segment>>,
}

/// Optional host measurements. Never checkpointed, observed, or included in VM state digests.
#[derive(Default, Clone, Debug, serde::Serialize)]
pub struct GcMetrics {
    pub attempts: u64,
    pub completed: u64,
    pub failed: u64,
    pub duration_nanos: u64,
    pub reclaimed_objects: usize,
    pub work: usize,
}
pub struct Runtime {
    root: PathBuf,
    state: State,
    checkpoints: BTreeMap<String, Checkpoint>,
    current_parent: Option<String>,
    observations: BTreeMap<(u64, String), Observation>,
    directory_observations: BTreeMap<(u64, String), Option<BTreeSet<String>>>,
    input: Vec<String>,
    gui_host: Option<gui::Host>,
    gui_dialogs: BTreeMap<usize, gui::dialog::Session>,
    gui_ime_enabled: bool,
    gui_accessibility_enabled: bool,
    gui_clipboard_enabled: bool,
    gui_command_keys_enabled: bool,
    gui_displayed: Option<Arc<gui::Frame>>,
    gui_observations: Vec<gui::Event>,
    gui_high_water: usize,
    gui_window_hosts: BTreeMap<String, gui::Host>,
    gui_window_frames: BTreeMap<String, Arc<gui::Frame>>,
    gui_window_observations: Vec<gui::WindowInput>,
    gui_window_observation_ends: Vec<usize>,
    gui_window_observation_bytes: usize,
    gui_window_scripted_bytes: usize,
    gui_window_high_water: usize,
    gui_window_scripted: Option<std::collections::VecDeque<gui::WindowEvent>>,
    gui_window_last_polled: Option<String>,
    external_entries: Vec<external::Entry>,
    external_high_water: usize,
    external_memory_bytes: usize,
    external_pending: usize,
    external_poll_high_water: usize,
    external_polls: Vec<external::Poll>,
    external_buffers: BTreeMap<usize, Vec<u8>>,
    network_host: Option<network::Host>,
    tcp_host: Option<tcp::Host>,
    http_server_host: Option<http_server::Host>,
    http_server_tls_credentials: BTreeMap<String, http_server::TlsCredential>,
    http_server_bearer_credentials: BTreeMap<String, Arc<http_server::BearerCredential>>,
    database_host: Option<database::Host>,
    network_credentials: BTreeMap<String, Arc<str>>,
    sensitive_bytes: BTreeSet<Arc<Vec<u8>>>,
    external_depth: usize,
    external_owner: u64,
    external_live: bool,
    external_live_allowed: bool,
    external_live_used: bool,
    external_live_entries: BTreeMap<usize, external::LiveEntry>,
    external_live_next: usize,
    external_live_unattached: BTreeMap<usize, Arc<external::LiveLease>>,
    external_live_releases: Arc<std::sync::Mutex<std::collections::VecDeque<usize>>>,
    next_native_lease: u64,
    gui_scripted: Option<std::collections::VecDeque<gui::Event>>,
    byte_input: Vec<(usize, Arc<Segment>)>,
    branch_roots: RefCell<Vec<Weak<State>>>,
    byte_eof: bool,
    times: Vec<u128>,
    arguments: Vec<String>,
    env_allowed: BTreeSet<String>,
    env_secrets: BTreeSet<String>,
    sensitive_values: BTreeSet<String>,
    sensitive_text_bytes: usize,
    sensitive_accounting: bool,
    secret_input_indices: BTreeSet<usize>,
    supplied_secret_input: Option<Vec<String>>,
    history_budget_kind: std::cell::Cell<&'static str>,
    env_observations: Vec<(String, Option<String>)>,
    entry_observations: Vec<(String, Vec<String>)>,
    locale: String,
    published_stdout: Journal,
    published_stderr: Journal,
    budget: ResourceBudget,
    publish_failure: Option<String>,
    publish_failure_detail: Option<PublishFailure>,
    replaying: bool,
    input_eof: bool,
    virtual_publish: bool,
    incremental_publish: bool,
    next_operation: u64,
    published_operations: BTreeSet<u64>,
    published_epoch: u64,
    virtual_files: BTreeMap<String, Option<Arc<PagedFile>>>,
    virtual_directories: BTreeMap<String, bool>,
    storage_start: (usize, (usize, usize)),
    gc_metrics: Option<GcMetrics>,
    allocations_since_gc: usize,
    allocation_bytes_since_gc: usize,
    native_remaining: Option<usize>,
    allocation_accounting: bool,
    cached_heap_traversal: bool,
    cached_heap_entries: bool,
    numeric_accounting: Option<numeric::Accounting>,
    numeric_checked_generation: std::cell::Cell<usize>,
    numeric_gc_start: usize,
    shared_accounting: Option<shared_payload::Accounting>,
    shared_checked_generation: std::cell::Cell<usize>,
    shared_gc_start: usize,
    execution_remaining: Option<usize>,
}

impl Runtime {
    pub fn register_secret_value(&mut self, value: &Value) {
        fn visit(
            rt: &Runtime,
            v: &Value,
            out: &mut BTreeSet<String>,
            binary: &mut BTreeSet<Arc<Vec<u8>>>,
            seen: &mut BTreeSet<u64>,
            depth: usize,
        ) {
            if depth > 64 {
                return;
            }
            match v {
                Value::HeapRef(id) | Value::CellRef(id) => {
                    if seen.insert(*id) {
                        if let Some(v) = rt.heap_get(*id) {
                            visit(rt, v, out, binary, seen, depth + 1);
                        }
                    }
                }
                Value::Struct(_, fields) | Value::Closure(_, _, fields) => {
                    for v in fields.values() {
                        visit(rt, v, out, binary, seen, depth + 1);
                    }
                }
                Value::List(items) => {
                    for v in items.iter() {
                        visit(rt, v, out, binary, seen, depth + 1);
                    }
                }
                Value::TypedList(_, items) => {
                    for v in items.iter() {
                        visit(rt, v, out, binary, seen, depth + 1);
                    }
                }
                Value::Map(items) | Value::TypedMap(_, _, items) => {
                    for (k, v) in items {
                        visit(rt, &k.value(), out, binary, seen, depth + 1);
                        visit(rt, v, out, binary, seen, depth + 1);
                    }
                }
                Value::OrderedMap(_, _, items) => {
                    for (k, v) in items {
                        visit(rt, k, out, binary, seen, depth + 1);
                        visit(rt, v, out, binary, seen, depth + 1);
                    }
                }
                Value::Enum(_, _, items) => {
                    for (_, v) in items {
                        visit(rt, v, out, binary, seen, depth + 1);
                    }
                }
                Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                    visit(rt, v, out, binary, seen, depth + 1)
                }
                Value::Null | Value::Option(None) | Value::Handle(_) | Value::Function(_, _) => {}
                _ => {
                    out.insert(v.to_string());
                    if let Value::Bytes(bytes) = v {
                        if !bytes.is_empty() {
                            binary.insert(bytes.clone());
                        }
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            out.insert(text);
                        }
                    }
                }
            }
        }
        let mut values = BTreeSet::new();
        let mut binary = BTreeSet::new();
        visit(
            self,
            value,
            &mut values,
            &mut binary,
            &mut BTreeSet::new(),
            0,
        );
        for bytes in binary {
            if self.sensitive_bytes.insert(bytes.clone()) && self.sensitive_accounting {
                self.shared_accounting
                    .as_ref()
                    .expect("sensitive ledger")
                    .register_bytes(&bytes);
            }
        }
        for text in values.into_iter().filter(|s| !s.is_empty()) {
            self.register_sensitive_text(text);
        }
    }
    fn register_sensitive_text(&mut self, text: String) {
        let charge = text.capacity().saturating_add(96);
        if self.sensitive_values.insert(text) {
            self.sensitive_text_bytes = self.sensitive_text_bytes.saturating_add(charge);
        }
    }
    /// Security patterns remain strong execution-wide roots, including after
    /// revert/drop. This charges retained storage; it does not forget patterns.
    pub fn enable_sensitive_accounting(&mut self) {
        self.enable_container_accounting();
        self.sensitive_accounting = true;
        for bytes in &self.sensitive_bytes {
            self.shared_accounting
                .as_ref()
                .expect("sensitive ledger")
                .register_bytes(bytes);
        }
    }
    pub fn enable_gui_command_keys(&mut self) {
        self.gui_command_keys_enabled = true;
    }
    pub fn enable_gui_accessibility(&mut self) {
        self.gui_accessibility_enabled = true;
    }
    pub(crate) fn gui_host_reservation(&self) -> usize {
        let accessibility = if self.gui_accessibility_enabled {
            // Includes up to 4096 retired/current AT-SPI paths and bounded COM handles.
            16 * 1024 * 1024
        } else {
            0
        };
        accessibility
            + if self.gui_ime_enabled {
                gui::clipboard::HOST_RESERVATION + gui::composition::HOST_RESERVATION
            } else if self.gui_clipboard_enabled {
                gui::clipboard::HOST_RESERVATION
            } else {
                0
            }
    }
    pub fn enable_gui_ime(&mut self) {
        self.gui_ime_enabled = true;
    }
    pub fn enable_gui_clipboard(&mut self) {
        self.gui_clipboard_enabled = true;
    }
    pub fn check_sensitive_accounting(&mut self) -> Result<()> {
        if self.sensitive_accounting {
            self.check_native_allocation(0)?;
        }
        Ok(())
    }
    pub fn sensitive_registry_metrics(&self) -> Option<serde_json::Value> {
        self.sensitive_accounting.then(|| {
            serde_json::json!({
                "text_patterns":self.sensitive_values.len(),
                "text_bytes":self.sensitive_text_bytes,
                "binary_patterns":self.sensitive_bytes.len(),
                "binary_entry_bytes":self.sensitive_bytes.len().saturating_mul(96)
            })
        })
    }
    pub fn mask_debug_json(&self, value: &serde_json::Value) -> serde_json::Value {
        use serde_json::Value as Json;
        match value {
            Json::String(s) => Json::String(self.masked_value(&Value::Text(s.clone().into()))),
            Json::Array(a) => Json::Array(a.iter().map(|v| self.mask_debug_json(v)).collect()),
            Json::Object(o) => Json::Object(
                o.iter()
                    .map(|(k, v)| {
                        (
                            self.masked_value(&Value::Text(k.clone().into())),
                            self.mask_debug_json(v),
                        )
                    })
                    .collect(),
            ),
            v => v.clone(),
        }
    }
    pub fn history_budget_kind(&self) -> &'static str {
        self.history_budget_kind.get()
    }
    pub fn configure_environment(
        &mut self,
        args: Vec<String>,
        allowed: BTreeSet<String>,
        secrets: BTreeSet<String>,
        locale: String,
    ) -> Result<()> {
        if !secrets.is_subset(&allowed) {
            return Err(Error::InvalidOperation(
                "secret environment names must be allowed".into(),
            ));
        }
        if locale.is_empty() {
            return Err(Error::InvalidOperation("locale must be explicit".into()));
        }
        self.arguments = args;
        self.env_allowed = allowed;
        self.env_secrets = secrets;
        self.locale = locale;
        Ok(())
    }
    pub fn arguments(&mut self) -> Vec<String> {
        self.state.args_cursor += 1;
        self.arguments.clone()
    }
    pub fn configure_secret_input(&mut self, lines: Vec<String>) {
        self.supplied_secret_input = Some(lines);
    }
    pub fn secret_input_line(&mut self, source: &mut impl io::BufRead) -> Result<Option<String>> {
        let cursor = self.state.stdin_cursor;
        let value = if cursor == self.input.len()
            && !self.replaying
            && self.supplied_secret_input.is_some()
        {
            let index = self
                .secret_input_indices
                .iter()
                .filter(|i| **i < cursor)
                .count();
            let text = self
                .supplied_secret_input
                .as_ref()
                .and_then(|v| v.get(index))
                .map(|s| format!("{s}\n"))
                .unwrap_or_default();
            self.input_line(&mut io::Cursor::new(text))?
        } else {
            self.input_line(source)?
        };
        if let Some(text) = &value {
            self.secret_input_indices.insert(cursor);
            self.register_secret_value(&Value::Text(text.clone().into()));
        }
        Ok(value)
    }
    pub fn secret_environment_value(&mut self, name: &str) -> Result<Option<String>> {
        if !self.env_allowed.contains(name) {
            return Err(Error::InvalidOperation(format!(
                "environment name {name} is not allowed"
            )));
        }
        self.env_secrets.insert(name.into());
        let value = self.environment_value(name)?;
        if let Some(text) = &value {
            self.register_secret_value(&Value::Text(text.clone().into()));
        }
        Ok(value)
    }
    pub fn environment_value(&mut self, name: &str) -> Result<Option<String>> {
        if !self.env_allowed.contains(name) {
            return Err(Error::InvalidOperation(format!(
                "environment name {name} is not allowed"
            )));
        }
        let cursor = self.state.env_cursor;
        let value = if let Some((previous, value)) = self.env_observations.get(cursor) {
            if previous != name {
                return Err(Error::InvalidOperation(format!(
                    "environment observation mismatch at {cursor}"
                )));
            }
            value.clone()
        } else {
            if self.replaying {
                return Err(Error::InvalidOperation(format!(
                    "ReplayMismatch: env observation {name} at {cursor}"
                )));
            }
            let value = std::env::var(name).ok();
            self.env_observations.push((name.into(), value.clone()));
            value
        };
        self.state.env_cursor += 1;
        Ok(value)
    }
    pub fn locale(&self) -> &str {
        &self.locale
    }
    pub fn directory_entries(&mut self, path: &str) -> Result<Vec<String>> {
        let normalized = if path == "." {
            ".".to_string()
        } else {
            self.checked_path(path)?;
            Path::new(path)
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        };
        let path = normalized.as_str();
        let cursor = self.state.directory_cursor;
        if let Some((previous, entries)) = self.entry_observations.get(cursor) {
            if previous != path {
                return Err(Error::InvalidOperation(format!(
                    "directory observation mismatch at {cursor}"
                )));
            }
            let entries = entries.clone();
            self.state.directory_cursor += 1;
            return Ok(entries);
        }
        let mut entries = if path == "." {
            let key = (self.state.file_epoch, ".".into());
            if let Some(cached) = self.directory_observations.get(&key) {
                cached.clone()
            } else {
                if self.replaying {
                    return Err(Error::InvalidOperation(format!(
                        "ReplayMismatch: directory observation {path}"
                    )));
                }
                let found = Self::host_directory_entries(&self.root)?;
                self.directory_observations.insert(key, found.clone());
                found
            }
        } else {
            self.observe_directory(path)?
        }
        .or_else(|| {
            (self
                .state
                .directories
                .get(path)
                .or_else(|| self.virtual_directories.get(path))
                == Some(&true))
            .then(BTreeSet::new)
        })
        .ok_or_else(|| Error::MissingFile(path.into()))?;
        if self
            .state
            .directories
            .get(path)
            .or_else(|| self.virtual_directories.get(path))
            == Some(&false)
        {
            return Err(Error::MissingFile(path.into()));
        }
        let prefix = if path.is_empty() || path == "." {
            String::new()
        } else {
            format!("{}/", path.trim_end_matches('/'))
        };
        for (name, content) in self.virtual_files.iter().chain(self.state.files.iter()) {
            if let Some(child) = name.strip_prefix(&prefix) {
                if !child.is_empty() && !child.contains('/') {
                    if content.is_some() {
                        entries.insert(child.into());
                    } else {
                        entries.remove(child);
                    }
                }
            }
        }
        for (name, exists) in self
            .virtual_directories
            .iter()
            .chain(self.state.directories.iter())
        {
            if let Some(child) = name.strip_prefix(&prefix) {
                if !child.is_empty() && !child.contains('/') {
                    if *exists {
                        entries.insert(child.into());
                    } else {
                        entries.remove(child);
                    }
                }
            }
        }
        let entries = entries.into_iter().collect::<Vec<_>>();
        self.entry_observations.push((path.into(), entries.clone()));
        self.state.directory_cursor += 1;
        Ok(entries)
    }
    /// Conservative admission units, not a measurement of allocator or RSS bytes.
    pub fn allocation_bytes(value: &Value) -> usize {
        let children = match value {
            Value::TypedList(_, v) => v.allocation_bytes(),
            Value::Map(v) | Value::TypedMap(_, _, v) => v.allocation_bytes(),
            Value::List(v) => v.iter().fold(v.capacity().saturating_mul(16), |n, v| {
                n.saturating_add(Self::allocation_bytes(v))
            }),
            Value::OrderedMap(_, _, v) => {
                v.iter().fold(v.capacity().saturating_mul(16), |n, (k, v)| {
                    n.saturating_add(Self::allocation_bytes(k))
                        .saturating_add(Self::allocation_bytes(v))
                })
            }
            Value::Struct(_, v) | Value::Closure(_, _, v) => v.iter().fold(0usize, |n, (k, v)| {
                n.saturating_add(96)
                    .saturating_add(k.len())
                    .saturating_add(Self::allocation_bytes(v))
            }),
            Value::Enum(_, _, v) => v.iter().fold(0usize, |n, (k, v)| {
                n.saturating_add(32)
                    .saturating_add(k.len())
                    .saturating_add(Self::allocation_bytes(v))
            }),
            Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                Self::allocation_bytes(v)
            }
            _ => Self::value_bytes(value),
        };
        let extra = match value {
            Value::Text(text) => text.owned_capacity_extra(),
            Value::TypedList(name, _) | Value::Struct(name, _) => name.capacity(),
            Value::TypedMap(k, v, _) => k.capacity().saturating_add(v.capacity()),
            Value::Closure(name, ty, _) => name.capacity().saturating_add(ty.capacity()),
            Value::Function(name, ty) => name
                .capacity()
                .saturating_sub(name.len())
                .saturating_add(ty.capacity().saturating_sub(ty.len())),
            Value::Enum(name, variant, fields) => name
                .capacity()
                .saturating_add(variant.capacity())
                .saturating_add(
                    fields
                        .capacity()
                        .saturating_sub(fields.len())
                        .saturating_mul(std::mem::size_of::<(String, Value)>()),
                )
                .saturating_add(
                    fields
                        .iter()
                        .map(|(k, _)| k.capacity().saturating_sub(k.len()))
                        .sum::<usize>(),
                ),
            Value::List(values) => values
                .capacity()
                .saturating_sub(values.len())
                .saturating_mul(std::mem::size_of::<Value>().saturating_sub(16)),
            Value::OrderedMap(k, v, values) => {
                k.capacity().saturating_add(v.capacity()).saturating_add(
                    values
                        .capacity()
                        .saturating_sub(values.len())
                        .saturating_mul(std::mem::size_of::<(Value, Value)>().saturating_sub(16)),
                )
            }
            _ => 0,
        };
        let key_extra = match value {
            Value::Struct(_, values) | Value::Closure(_, _, values) => values
                .keys()
                .map(|k| k.capacity().saturating_sub(k.len()))
                .sum(),
            _ => 0usize,
        };
        children
            .saturating_add(extra)
            .saturating_add(key_extra)
            .saturating_add(std::mem::size_of::<Value>())
            .saturating_add(32)
    }
    /// Check bounded native scratch/output before allocating it. The VM is single-threaded;
    /// native numeric routines do not yield while this reservation is being used.
    pub fn check_native_allocation(&mut self, bytes: usize) -> Result<()> {
        let previous = self.external_memory_bytes;
        self.external_memory_bytes = previous
            .checked_add(bytes)
            .ok_or(Error::HistoryBudgetExceeded)?;
        let result = self.enforce_budget();
        self.external_memory_bytes = previous;
        result
    }
    /// Skip persistent subtrees without heap edges; retain bounded mark/sweep.
    /// Resolve current heap edges from immutable entry summaries.
    pub fn enable_cached_heap_entries(&mut self) {
        self.enable_cached_heap_traversal();
        self.cached_heap_entries = true;
    }
    pub fn enable_cached_heap_traversal(&mut self) {
        self.cached_heap_traversal = true;
    }
    pub(crate) fn contains_heap_refs(value: &Value) -> bool {
        match value {
            Value::HeapRef(_) | Value::CellRef(_) => true,
            Value::TypedList(_, v) => v.has_heap_refs(),
            Value::Map(v) | Value::TypedMap(_, _, v) => v.has_heap_refs(),
            Value::List(v) => v.iter().any(Self::contains_heap_refs),
            Value::OrderedMap(_, _, v) => v
                .iter()
                .any(|(k, v)| Self::contains_heap_refs(k) || Self::contains_heap_refs(v)),
            Value::Struct(_, v) | Value::Closure(_, _, v) => {
                v.values().any(Self::contains_heap_refs)
            }
            Value::Enum(_, _, v) => v.iter().any(|(_, v)| Self::contains_heap_refs(v)),
            Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                Self::contains_heap_refs(v)
            }
            _ => false,
        }
    }
    pub fn enable_allocation_accounting(&mut self) {
        self.allocation_accounting = true;
    }
    pub fn enable_numeric_accounting(&mut self) {
        self.numeric_accounting.get_or_insert_with(Default::default);
    }
    pub fn enable_shared_payload_accounting(&mut self) {
        self.shared_accounting.get_or_insert_with(Default::default);
    }
    pub fn enable_container_accounting(&mut self) {
        self.enable_allocation_accounting();
        self.enable_numeric_accounting();
        if !self
            .shared_accounting
            .as_ref()
            .is_some_and(|a| a.includes_containers())
        {
            self.shared_accounting = Some(shared_payload::Accounting::with_containers());
            self.shared_checked_generation.set(0);
            self.shared_gc_start = 0;
        }
    }
    pub fn enable_byte_payload_accounting(&mut self) {
        if !self
            .shared_accounting
            .as_ref()
            .is_some_and(|a| a.includes_bytes())
        {
            self.shared_accounting = Some(shared_payload::Accounting::with_bytes());
            self.shared_gc_start = 0;
            self.shared_checked_generation.set(0);
        }
    }
    pub fn shared_payload_metrics(&self) -> Option<serde_json::Value> {
        self.shared_accounting.as_ref().map(|a|{a.prune_bytes(); serde_json::json!({"live_bytes":a.bytes(),"allocated_bytes":a.allocated_bytes(),"registration_visits":a.visits()})})
    }
    pub fn check_numeric_accounting(&self) -> Result<()> {
        if self
            .numeric_accounting
            .as_ref()
            .is_some_and(|a| a.generation() != self.numeric_checked_generation.get())
            || self
                .shared_accounting
                .as_ref()
                .is_some_and(|a| a.generation() != self.shared_checked_generation.get())
        {
            self.enforce_budget()?;
        }
        Ok(())
    }
    pub fn retained_numeric_bytes(&self) -> usize {
        self.numeric_accounting
            .as_ref()
            .map_or(0, numeric::Accounting::bytes)
    }
    pub fn retained_payload_bytes(&self, value: &Value) -> usize {
        self.retained_bytes(value, Self::value_bytes(value), false)
    }
    fn retained_value_bytes(&self, value: &Value) -> usize {
        self.retained_bytes(
            value,
            if self.allocation_accounting {
                Self::allocation_bytes(value)
            } else {
                Self::value_bytes(value)
            },
            self.allocation_accounting,
        )
    }
    fn retained_bytes(&self, value: &Value, mut bytes: usize, allocations: bool) -> usize {
        if let Some(a) = &self.numeric_accounting {
            Self::register_numeric_value(value, a);
            bytes = bytes.saturating_sub(Self::numeric_payload_bytes(value));
        }
        if let Some(a) = &self.shared_accounting {
            Self::register_shared_value(value, a);
            bytes = bytes.saturating_sub(Self::shared_payload_bytes(value));
            if a.includes_bytes() {
                bytes = bytes.saturating_sub(Self::byte_payload_info(value).0);
            }
            if allocations && a.includes_containers() {
                bytes = bytes.saturating_sub(Self::container_discount(value));
            }
        }
        bytes
    }
    pub(crate) fn numeric_payload_bytes(value: &Value) -> usize {
        match value {
            Value::NumericArray(array) => array.storage_bytes(),
            Value::TypedList(_, values) => values.numeric_bytes(),
            Value::Map(values) | Value::TypedMap(_, _, values) => values.numeric_bytes(),
            Value::List(values) => values.iter().fold(0usize, |n, v| {
                n.saturating_add(Self::numeric_payload_bytes(v))
            }),
            Value::OrderedMap(_, _, values) => values.iter().fold(0usize, |n, (k, v)| {
                n.saturating_add(Self::numeric_payload_bytes(k))
                    .saturating_add(Self::numeric_payload_bytes(v))
            }),
            Value::Struct(_, values) | Value::Closure(_, _, values) => {
                values.values().fold(0usize, |n, v| {
                    n.saturating_add(Self::numeric_payload_bytes(v))
                })
            }
            Value::Enum(_, _, values) => values.iter().fold(0usize, |n, (_, v)| {
                n.saturating_add(Self::numeric_payload_bytes(v))
            }),
            Value::Option(Some(value)) | Value::Result(Ok(value)) | Value::Result(Err(value)) => {
                Self::numeric_payload_bytes(value)
            }
            _ => 0,
        }
    }
    pub(crate) fn register_numeric_value(value: &Value, accounting: &numeric::Accounting) {
        match value {
            Value::NumericArray(array) => accounting.register(array),
            Value::TypedList(_, values) => values.register_numerics(accounting),
            Value::Map(values) | Value::TypedMap(_, _, values) => {
                values.register_numerics(accounting)
            }
            Value::List(values) => {
                for value in values {
                    Self::register_numeric_value(value, accounting);
                }
            }
            Value::OrderedMap(_, _, values) => {
                for (key, value) in values {
                    Self::register_numeric_value(key, accounting);
                    Self::register_numeric_value(value, accounting);
                }
            }
            Value::Struct(_, values) | Value::Closure(_, _, values) => {
                for value in values.values() {
                    Self::register_numeric_value(value, accounting);
                }
            }
            Value::Enum(_, _, values) => {
                for (_, value) in values {
                    Self::register_numeric_value(value, accounting);
                }
            }
            Value::Option(Some(value)) | Value::Result(Ok(value)) | Value::Result(Err(value)) => {
                Self::register_numeric_value(value, accounting)
            }
            _ => {}
        }
    }
    pub(crate) fn shared_payload_bytes(value: &Value) -> usize {
        match value {
            Value::Text(text) => text.shared_payload_bytes(),
            Value::TypedList(_, values) => values.shared_bytes(),
            Value::Map(values) | Value::TypedMap(_, _, values) => values.shared_bytes(),
            Value::List(values) => values.iter().fold(0usize, |n, v| {
                n.saturating_add(Self::shared_payload_bytes(v))
            }),
            Value::OrderedMap(_, _, values) => values.iter().fold(0usize, |n, (k, v)| {
                n.saturating_add(Self::shared_payload_bytes(k))
                    .saturating_add(Self::shared_payload_bytes(v))
            }),
            Value::Struct(_, values) | Value::Closure(_, _, values) => {
                values.values().fold(0usize, |n, v| {
                    n.saturating_add(Self::shared_payload_bytes(v))
                })
            }
            Value::Enum(_, _, values) => values.iter().fold(0usize, |n, (_, v)| {
                n.saturating_add(Self::shared_payload_bytes(v))
            }),
            Value::Option(Some(value)) | Value::Result(Ok(value)) | Value::Result(Err(value)) => {
                Self::shared_payload_bytes(value)
            }
            _ => 0,
        }
    }
    pub(crate) fn byte_payload_info(value: &Value) -> (usize, usize) {
        fn add(a: (usize, usize), b: (usize, usize)) -> (usize, usize) {
            (a.0.saturating_add(b.0), a.1.saturating_add(b.1))
        }
        match value {
            Value::Bytes(bytes) => (bytes.len(), 1),
            Value::TypedList(_, values) => values.byte_payload_info(),
            Value::Map(values) | Value::TypedMap(_, _, values) => values.byte_payload_info(),
            Value::List(values) => values
                .iter()
                .fold((0, 0), |n, v| add(n, Self::byte_payload_info(v))),
            Value::OrderedMap(_, _, values) => values.iter().fold((0, 0), |n, (k, v)| {
                add(
                    add(n, Self::byte_payload_info(k)),
                    Self::byte_payload_info(v),
                )
            }),
            Value::Struct(_, values) | Value::Closure(_, _, values) => values
                .values()
                .fold((0, 0), |n, v| add(n, Self::byte_payload_info(v))),
            Value::Enum(_, _, values) => values
                .iter()
                .fold((0, 0), |n, (_, v)| add(n, Self::byte_payload_info(v))),
            Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                Self::byte_payload_info(v)
            }
            _ => (0, 0),
        }
    }
    /// Existing per-root native contribution replaced by unique owners. Payload
    /// totals have already been removed separately, so never subtract them twice.
    pub(crate) fn container_discount(value: &Value) -> usize {
        match value {
            Value::TypedList(_, values) => values
                .allocation_bytes()
                .saturating_sub(values.numeric_bytes())
                .saturating_sub(values.shared_bytes())
                .saturating_sub(values.byte_payload_info().0),
            Value::Map(values) | Value::TypedMap(_, _, values) => values
                .allocation_bytes()
                .saturating_sub(values.numeric_bytes())
                .saturating_sub(values.shared_bytes())
                .saturating_sub(values.byte_payload_info().0),
            Value::List(values) => values
                .iter()
                .fold(0usize, |n, v| n.saturating_add(Self::container_discount(v))),
            Value::OrderedMap(_, _, values) => values.iter().fold(0usize, |n, (k, v)| {
                n.saturating_add(Self::container_discount(k))
                    .saturating_add(Self::container_discount(v))
            }),
            Value::Struct(_, values) | Value::Closure(_, _, values) => values
                .values()
                .fold(0usize, |n, v| n.saturating_add(Self::container_discount(v))),
            Value::Enum(_, _, values) => values.iter().fold(0usize, |n, (_, v)| {
                n.saturating_add(Self::container_discount(v))
            }),
            Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                Self::container_discount(v)
            }
            _ => 0,
        }
    }
    pub(crate) fn container_logical_discount(value: &Value) -> usize {
        match value {
            Value::TypedList(_, values) => values
                .logical_bytes()
                .saturating_sub(values.numeric_bytes())
                .saturating_sub(values.shared_bytes())
                .saturating_sub(values.byte_payload_info().0),
            Value::Map(values) | Value::TypedMap(_, _, values) => values
                .logical_bytes()
                .saturating_sub(values.numeric_bytes())
                .saturating_sub(values.shared_bytes())
                .saturating_sub(values.byte_payload_info().0),
            Value::List(values) => values.iter().fold(0usize, |n, v| {
                n.saturating_add(Self::container_logical_discount(v))
            }),
            Value::OrderedMap(_, _, values) => values.iter().fold(0usize, |n, (k, v)| {
                n.saturating_add(Self::container_logical_discount(k))
                    .saturating_add(Self::container_logical_discount(v))
            }),
            Value::Struct(_, values) | Value::Closure(_, _, values) => {
                values.values().fold(0usize, |n, v| {
                    n.saturating_add(Self::container_logical_discount(v))
                })
            }
            Value::Enum(_, _, values) => values.iter().fold(0usize, |n, (_, v)| {
                n.saturating_add(Self::container_logical_discount(v))
            }),
            Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                Self::container_logical_discount(v)
            }
            _ => 0,
        }
    }
    pub(crate) fn register_owned_value(
        value: &Arc<Value>,
        accounting: &shared_payload::Accounting,
    ) {
        if !accounting.includes_containers() {
            Self::register_shared_value(value, accounting);
            return;
        }
        if accounting.register_value(value, || {
            Self::allocation_bytes(value)
                .saturating_sub(Self::numeric_payload_bytes(value))
                .saturating_sub(Self::shared_payload_bytes(value))
                .saturating_sub(Self::byte_payload_info(value).0)
                .saturating_sub(Self::container_discount(value))
                .saturating_add(128)
        }) {
            Self::register_shared_value(value, accounting);
        }
    }
    pub(crate) fn register_shared_value(value: &Value, accounting: &shared_payload::Accounting) {
        match value {
            Value::Text(text) => text.register_shared(accounting),
            Value::Bytes(bytes) => accounting.register_bytes(bytes),
            Value::TypedList(_, values) => values.register_shared_payloads(accounting),
            Value::Map(values) | Value::TypedMap(_, _, values) => {
                values.register_shared_payloads(accounting)
            }
            Value::List(values) => {
                for value in values {
                    Self::register_shared_value(value, accounting);
                }
            }
            Value::OrderedMap(_, _, values) => {
                for (key, value) in values {
                    Self::register_shared_value(key, accounting);
                    Self::register_shared_value(value, accounting);
                }
            }
            Value::Struct(_, values) | Value::Closure(_, _, values) => {
                for value in values.values() {
                    Self::register_shared_value(value, accounting);
                }
            }
            Value::Enum(_, _, values) => {
                for (_, value) in values {
                    Self::register_shared_value(value, accounting);
                }
            }
            Value::Option(Some(value)) | Value::Result(Ok(value)) | Value::Result(Err(value)) => {
                Self::register_shared_value(value, accounting)
            }
            _ => {}
        }
    }
    pub fn value_bytes(value: &Value) -> usize {
        match value {
            Value::Text(text) => text.len(),
            Value::Bytes(bytes) => bytes.len(),
            Value::NumericArray(array) => array.retained_bytes(),
            Value::Regex(v) => v.retained_bytes(),
            Value::CsvStream(v) => v.retained_bytes(),
            Value::JsonStream(v) => v.retained_bytes(),
            Value::Instant(_) | Value::Duration(_) => 16,
            Value::BigInt(integer) => integer.retained_bytes(),
            Value::Decimal(value) => value.retained_bytes(),
            Value::FileError(error) => {
                error.code.len()
                    + error.path.len()
                    + error.causes.iter().map(String::len).sum::<usize>()
            }
            Value::TypedList(_, items) => items.logical_bytes(),
            Value::List(items) => items.iter().map(Self::value_bytes).sum(),
            Value::Map(items) | Value::TypedMap(_, _, items) => items.logical_bytes(),
            Value::OrderedMap(_, _, items) => items
                .iter()
                .map(|(key, value)| Self::value_bytes(key) + Self::value_bytes(value))
                .sum(),
            Value::Struct(_, items) => items
                .iter()
                .map(|(key, value)| key.len() + Self::value_bytes(value))
                .sum(),
            Value::Enum(_, _, fields) => fields
                .iter()
                .map(|(name, value)| name.len() + Self::value_bytes(value))
                .sum(),
            Value::Function(name, ty) => name.len() + ty.len(),
            Value::Closure(name, ty, captures) => {
                name.len() + ty.len() + captures.values().map(Self::value_bytes).sum::<usize>()
            }
            Value::Option(Some(value)) => Self::value_bytes(value),
            Value::Option(None) => 0,
            Value::Result(Ok(value)) | Value::Result(Err(value)) => Self::value_bytes(value),
            Value::Bool(_) => 1,
            Value::Int(_)
            | Value::Float(_)
            | Value::HeapRef(_)
            | Value::Handle(_)
            | Value::CellRef(_) => 8,
            Value::Null => 0,
        }
    }
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(Error::InvalidPath(root.display().to_string()));
        }
        Ok(Self {
            root,
            state: State::default(),
            checkpoints: BTreeMap::new(),
            current_parent: None,
            observations: BTreeMap::new(),
            directory_observations: BTreeMap::new(),
            input: Vec::new(),
            gui_host: None,
            gui_dialogs: BTreeMap::new(),
            gui_ime_enabled: false,
            gui_accessibility_enabled: false,
            gui_clipboard_enabled: false,
            gui_command_keys_enabled: false,
            gui_displayed: None,
            gui_observations: Vec::new(),
            gui_high_water: 0,
            gui_window_hosts: BTreeMap::new(),
            gui_window_frames: BTreeMap::new(),
            gui_window_observations: Vec::new(),
            gui_window_observation_ends: Vec::new(),
            gui_window_observation_bytes: 0,
            gui_window_scripted_bytes: 0,
            gui_window_high_water: 0,
            gui_window_scripted: None,
            gui_window_last_polled: None,
            external_entries: Vec::new(),
            external_high_water: 0,
            external_memory_bytes: 0,
            external_pending: 0,
            external_poll_high_water: 0,
            external_polls: Vec::new(),
            external_buffers: BTreeMap::new(),
            network_host: None,
            tcp_host: None,
            http_server_host: None,
            http_server_tls_credentials: BTreeMap::new(),
            http_server_bearer_credentials: BTreeMap::new(),
            database_host: None,
            network_credentials: BTreeMap::new(),
            sensitive_bytes: BTreeSet::new(),
            external_depth: 0,
            external_owner: 0,
            external_live: false,
            external_live_allowed: false,
            external_live_used: false,
            external_live_entries: BTreeMap::new(),
            external_live_next: 1_000_000,
            external_live_unattached: BTreeMap::new(),
            external_live_releases: Arc::new(std::sync::Mutex::new(
                std::collections::VecDeque::new(),
            )),
            next_native_lease: 1,
            gui_scripted: None,
            byte_input: Vec::new(),
            branch_roots: RefCell::new(Vec::new()),
            byte_eof: false,
            times: Vec::new(),
            arguments: Vec::new(),
            env_allowed: BTreeSet::new(),
            env_secrets: BTreeSet::new(),
            sensitive_values: BTreeSet::new(),
            sensitive_text_bytes: 0,
            sensitive_accounting: false,
            secret_input_indices: BTreeSet::new(),
            supplied_secret_input: None,
            history_budget_kind: std::cell::Cell::new("HistoryMemory"),
            env_observations: Vec::new(),
            entry_observations: Vec::new(),
            locale: "en-US".into(),
            published_stdout: Journal::default(),
            published_stderr: Journal::default(),
            budget: ResourceBudget::default(),
            publish_failure: None,
            publish_failure_detail: None,
            replaying: false,
            input_eof: false,
            virtual_publish: false,
            incremental_publish: false,
            next_operation: 1,
            published_operations: BTreeSet::new(),
            published_epoch: 0,
            virtual_files: BTreeMap::new(),
            virtual_directories: BTreeMap::new(),
            storage_start: (map_storage::map_nodes_created(), storage::storage_work()),
            gc_metrics: None,
            allocations_since_gc: 0,
            allocation_bytes_since_gc: 0,
            native_remaining: None,
            allocation_accounting: false,
            cached_heap_traversal: false,
            cached_heap_entries: false,
            numeric_accounting: None,
            numeric_checked_generation: std::cell::Cell::new(0),
            numeric_gc_start: 0,
            shared_accounting: None,
            shared_checked_generation: std::cell::Cell::new(0),
            shared_gc_start: 0,
            execution_remaining: None,
        })
    }
    /// Enable opt-in timing of borrowed-root heap collection, including failed attempts.
    pub fn enable_gc_profiling(&mut self) {
        self.gc_metrics.get_or_insert_with(GcMetrics::default);
    }
    pub fn gc_metrics(&self) -> Option<&GcMetrics> {
        self.gc_metrics.as_ref()
    }
    /// Runtime-scoped live storage vs cumulative allocations; neither is VM state.
    pub fn numeric_metrics(&self) -> Option<serde_json::Value> {
        self.numeric_accounting.as_ref().map(|a|{serde_json::json!({"live_bytes":a.bytes(),"allocated_bytes":a.allocated_bytes(),"registration_visits":a.visits()})})
    }
    pub fn storage_metrics(&self) -> (usize, usize, usize) {
        let (nodes, slots) = storage::storage_work();
        (
            map_storage::map_nodes_created().saturating_sub(self.storage_start.0),
            nodes.saturating_sub(self.storage_start.1 .0),
            slots.saturating_sub(self.storage_start.1 .1),
        )
    }
    pub fn enable_incremental_publish(&mut self) {
        self.incremental_publish = true;
    }
    fn operation(&mut self) -> Result<u64> {
        let id = self.next_operation;
        self.next_operation = id
            .checked_add(1)
            .ok_or_else(|| Error::InvalidOperation("operation ID exhausted".into()))?;
        Ok(id)
    }
    fn pending_transaction<T>(&mut self, action: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let previous = self.state.clone();
        let next_operation = self.next_operation;
        let result = action(self).and_then(|value| {
            if self.incremental_publish {
                self.enforce_budget()?;
            }
            Ok(value)
        });
        if result.is_err() {
            self.state = previous;
            self.next_operation = next_operation;
        }
        result
    }
    fn file_operation(&mut self, path: &str) -> Result<()> {
        if self.incremental_publish {
            let id = self.operation()?;
            Arc::make_mut(&mut self.state.file_operations).insert(path.into(), id);
        }
        Ok(())
    }
    fn directory_operation(&mut self, path: &str) -> Result<()> {
        if self.incremental_publish {
            let id = self.operation()?;
            Arc::make_mut(&mut self.state.directory_operations).insert(path.into(), id);
        }
        Ok(())
    }
    fn restore_pending(&mut self) {
        if !self.incremental_publish {
            return;
        }
        if self
            .state
            .gui_pending
            .as_ref()
            .is_some_and(|r| self.published_operations.contains(&r.id))
        {
            self.state.gui_pending = None;
        }
        Arc::make_mut(&mut self.state.gui_windows_pending)
            .retain(|_, r| !self.published_operations.contains(&r.id));
        self.state.stdout = self.state.stdout.unpublished(&self.published_operations);
        self.state.stderr = self.state.stderr.unpublished(&self.published_operations);
        let files = Arc::make_mut(&mut self.state.files);
        for (path, id) in self.state.file_operations.iter() {
            if self.published_operations.contains(id) {
                files.remove(path);
            }
        }
        let dirs = Arc::make_mut(&mut self.state.directories);
        for (path, id) in self.state.directory_operations.iter() {
            if self.published_operations.contains(id) {
                dirs.remove(path);
            }
        }
        Arc::make_mut(&mut self.state.file_operations)
            .retain(|_, id| !self.published_operations.contains(id));
        Arc::make_mut(&mut self.state.directory_operations)
            .retain(|_, id| !self.published_operations.contains(id));
        if self.state.files.is_empty() && self.state.directories.is_empty() {
            self.state.file_epoch = self.published_epoch;
        }
    }
    fn finish_publish(&mut self, next_epoch: u64) {
        self.state.gui_windows_pending = Arc::new(BTreeMap::new());
        if let Some(request) = self.state.gui_pending.take() {
            self.published_operations.insert(request.id);
        }
        if self.incremental_publish {
            if self.virtual_publish {
                self.virtual_files
                    .extend(self.state.files.iter().map(|(p, v)| (p.clone(), v.clone())));
                self.virtual_directories
                    .extend(self.state.directories.iter().map(|(p, v)| (p.clone(), *v)));
            }
            self.published_operations
                .extend(self.state.stdout.operations());
            self.published_operations
                .extend(self.state.stderr.operations());
            self.published_operations
                .extend(self.state.file_operations.values());
            self.published_operations
                .extend(self.state.directory_operations.values());
            self.state.stdout = Journal::default();
            self.state.stderr = Journal::default();
            self.state.file_operations = Arc::new(BTreeMap::new());
            self.state.directory_operations = Arc::new(BTreeMap::new());
            self.published_epoch = next_epoch;
        } else {
            self.published_stdout = self.state.stdout.clone();
            self.published_stderr = self.state.stderr.clone();
        }
        if !self.virtual_publish || self.incremental_publish {
            self.state.files = Arc::new(BTreeMap::new());
            self.state.directories = Arc::new(BTreeMap::new());
        }
        self.state.file_epoch = next_epoch;
        self.reclaim_operation_ledger();
    }

    fn reclaim_operation_ledger(&mut self) {
        if !self.incremental_publish {
            return;
        }
        let roots = self
            .branch_roots
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        let mut referenced = BTreeSet::new();
        for state in std::iter::once(&self.state)
            .chain(self.checkpoints.values().map(|c| &c.state))
            .chain(roots.iter().map(AsRef::as_ref))
        {
            referenced.extend(state.stdout.operations());
            referenced.extend(state.stderr.operations());
            referenced.extend(state.file_operations.values());
            referenced.extend(state.gui_pending.iter().map(|r| r.id));
            referenced.extend(state.gui_windows_pending.values().map(|r| r.id));
            referenced.extend(state.directory_operations.values());
        }
        self.published_operations
            .retain(|id| referenced.contains(id));
    }
    pub fn set_budget(&mut self, budget: ResourceBudget) -> Result<()> {
        let previous = self.budget;
        self.budget = budget;
        if let Err(error) = self.enforce_budget() {
            self.budget = previous;
            return Err(error);
        }
        Ok(())
    }
    fn enforce_budget(&self) -> Result<()> {
        let mut segments = Vec::new();
        let mut seen_segments = HashSet::new();
        let mut seen_compute = HashSet::new();
        let mut compute_memory = self.byte_input.len().saturating_mul(32);
        if self.allocation_accounting {
            compute_memory =
                compute_memory.saturating_add(self.checkpoints.len().saturating_mul(1024));
            compute_memory = compute_memory.saturating_add(
                self.input
                    .iter()
                    .map(|s| s.len().saturating_add(32))
                    .sum::<usize>(),
            );
            compute_memory = compute_memory.saturating_add(self.times.len().saturating_mul(32));
            compute_memory = compute_memory.saturating_add(
                self.observations
                    .keys()
                    .map(|(_, p)| p.len().saturating_add(128))
                    .sum::<usize>(),
            );
            compute_memory = compute_memory.saturating_add(
                self.directory_observations
                    .iter()
                    .map(|((_, p), entries)| {
                        p.len().saturating_add(128).saturating_add(
                            entries.as_ref().map_or(0, |v| {
                                v.iter().map(|s| s.len().saturating_add(96)).sum::<usize>()
                            }),
                        )
                    })
                    .sum::<usize>(),
            );
            compute_memory = compute_memory.saturating_add(
                self.entry_observations
                    .iter()
                    .map(|(p, entries)| {
                        p.len().saturating_add(64).saturating_add(
                            entries
                                .iter()
                                .map(|s| s.len().saturating_add(32))
                                .sum::<usize>(),
                        )
                    })
                    .sum::<usize>(),
            );
        }
        for (_, bytes) in &self.byte_input {
            if seen_segments.insert(Arc::as_ptr(bytes) as usize) {
                segments.push(bytes.clone());
            }
        }
        let branch_roots = self
            .branch_roots
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        if self.incremental_publish {
            compute_memory =
                compute_memory.saturating_add(self.published_operations.len().saturating_mul(32));
            compute_memory = compute_memory.saturating_add(
                (self.state.stdout.operations().len()
                    + self.state.stderr.operations().len()
                    + self.state.file_operations.len()
                    + self.state.directory_operations.len())
                .saturating_mul(32),
            );
        }
        for state in std::iter::once(&self.state)
            .chain(self.checkpoints.values().map(|c| &c.state))
            .chain(branch_roots.iter().map(AsRef::as_ref))
        {
            if seen_compute.insert(Arc::as_ptr(&state.native_owners) as usize) {
                compute_memory =
                    compute_memory.saturating_add(state.native_owners.len().saturating_mul(64));
            }
            for journal in [&state.stdout, &state.stderr] {
                for segment in journal.segments() {
                    let ptr = Arc::as_ptr(&segment) as usize;
                    if seen_segments.insert(ptr) {
                        segments.push(segment);
                    }
                }
            }
            for content in state.files.values().flatten() {
                for page in content.pages.values() {
                    if seen_segments.insert(Arc::as_ptr(page) as usize) {
                        segments.push(page.clone());
                    }
                }
            }
            for handle in state.handles.values() {
                if let Some(snapshot) = &handle.snapshot {
                    for page in snapshot.pages.values() {
                        if seen_segments.insert(Arc::as_ptr(page) as usize) {
                            segments.push(page.clone());
                        }
                    }
                }
            }
            if seen_compute.insert(Arc::as_ptr(&state.globals) as usize) {
                compute_memory = compute_memory.saturating_add(
                    state
                        .globals
                        .iter()
                        .map(|(k, v)| {
                            k.len()
                                + self.retained_value_bytes(v)
                                + if self.allocation_accounting { 96 } else { 0 }
                        })
                        .sum(),
                );
            }
            if seen_compute.insert(Arc::as_ptr(&state.heap) as usize) {
                let mut bytes = if self.allocation_accounting {
                    state.heap.allocation_bytes()
                } else {
                    state.heap.logical_bytes()
                };
                if let Some(a) = &self.numeric_accounting {
                    state.heap.register_numerics(a);
                    bytes = bytes.saturating_sub(state.heap.numeric_bytes());
                }
                if let Some(a) = &self.shared_accounting {
                    state.heap.register_shared_payloads(a);
                    bytes = bytes.saturating_sub(state.heap.shared_bytes());
                    if a.includes_bytes() {
                        bytes = bytes.saturating_sub(state.heap.byte_payload_info().0);
                    }
                    if a.includes_containers() {
                        let discount = state
                            .heap
                            .allocation_bytes()
                            .saturating_sub(state.heap.numeric_bytes())
                            .saturating_sub(state.heap.shared_bytes())
                            .saturating_sub(state.heap.byte_payload_info().0);
                        bytes = bytes.saturating_sub(discount);
                    }
                }
                compute_memory = compute_memory.saturating_add(bytes);
            }
            if seen_compute.insert(Arc::as_ptr(&state.stack) as usize) {
                compute_memory = compute_memory.saturating_add(
                    state
                        .stack
                        .iter()
                        .map(|v| self.retained_value_bytes(v))
                        .sum(),
                );
            }
            if seen_compute.insert(Arc::as_ptr(&state.call_frames) as usize) {
                compute_memory = compute_memory.saturating_add(
                    state
                        .call_frames
                        .iter()
                        .flat_map(|f| f.locals.iter())
                        .map(|(k, v)| {
                            k.len()
                                + self.retained_value_bytes(v)
                                + if self.allocation_accounting { 96 } else { 0 }
                        })
                        .sum(),
                );
            }
        }
        for journal in [&self.published_stdout, &self.published_stderr] {
            for segment in journal.segments() {
                let ptr = Arc::as_ptr(&segment) as usize;
                if seen_segments.insert(ptr) {
                    segments.push(segment);
                }
            }
        }
        for content in self.virtual_files.values().flatten() {
            for page in content.pages.values() {
                if seen_segments.insert(Arc::as_ptr(page) as usize) {
                    segments.push(page.clone());
                }
            }
        }
        for observation in self.observations.values() {
            for block in observation.blocks.values() {
                if seen_segments.insert(Arc::as_ptr(block) as usize) {
                    segments.push(block.clone());
                }
            }
        }
        for outcome in self
            .external_entries
            .iter()
            .chain(self.external_live_entries.values().map(|live| &live.entry))
            .filter_map(|e| e.outcome.as_ref())
        {
            if seen_segments.insert(Arc::as_ptr(&outcome.segment) as usize) {
                segments.push(outcome.segment.clone());
            }
        }
        compute_memory = compute_memory.saturating_add(
            self.gui_observations
                .iter()
                .map(|e| e.key.len() + 160)
                .sum::<usize>(),
        );
        compute_memory = compute_memory.saturating_add(
            self.gui_scripted
                .as_ref()
                .map_or(0, |v| v.iter().map(|e| e.key.len() + 160).sum::<usize>()),
        );
        compute_memory = compute_memory
            .saturating_add(
                self.network_credentials
                    .iter()
                    .map(|(a, v)| a.len() + v.len() + 128)
                    .sum::<usize>(),
            )
            .saturating_add(if self.sensitive_accounting {
                self.sensitive_text_bytes
                    .saturating_add(self.sensitive_bytes.len().saturating_mul(96))
            } else {
                self.sensitive_bytes
                    .iter()
                    .map(|v| v.len() + 64)
                    .sum::<usize>()
            })
            .saturating_add(self.external_memory_bytes)
            .saturating_add(self.retained_numeric_bytes())
            .saturating_add(
                self.shared_accounting
                    .as_ref()
                    .map_or(0, shared_payload::Accounting::bytes),
            )
            .saturating_add(
                self.database_host
                    .as_ref()
                    .map_or(0, database::Host::reserved_bytes),
            )
            .saturating_add(self.tcp_host.as_ref().map_or(0, tcp::Host::reserved_bytes))
            .saturating_add(
                self.http_server_host
                    .as_ref()
                    .map_or(0, http_server::Host::reserved_bytes),
            )
            .saturating_add(
                self.http_server_tls_credentials
                    .len()
                    .saturating_mul(http_server::TLS_CONFIG_MEMORY),
            )
            .saturating_add(
                self.http_server_bearer_credentials
                    .len()
                    .saturating_mul(http_server::AUTH_CONFIG_MEMORY),
            )
            .saturating_add(self.external_polls.len().saturating_mul(32))
            .saturating_add(
                self.network_host
                    .as_ref()
                    .map_or(0, network::Host::reserved_bytes),
            );
        compute_memory = compute_memory
            .saturating_add(self.gui_window_observation_bytes)
            .saturating_add(self.gui_window_scripted_bytes);
        let mut gui_roots = HashSet::new();
        for state in std::iter::once(&self.state)
            .chain(self.checkpoints.values().map(|c| &c.state))
            .chain(branch_roots.iter().map(AsRef::as_ref))
        {
            compute_memory = compute_memory.saturating_add(
                state
                    .gui_windows_pending
                    .keys()
                    .map(|k| k.len() + 64)
                    .sum::<usize>(),
            );
            for frame in state
                .gui_windows_pending
                .values()
                .filter_map(|r| r.frame.as_ref())
            {
                if gui_roots.insert(Arc::as_ptr(frame) as usize) {
                    compute_memory = compute_memory.saturating_add(frame.bytes());
                }
            }
            if let Some(frame) = state.gui_pending.as_ref().and_then(|r| r.frame.as_ref()) {
                if gui_roots.insert(Arc::as_ptr(frame) as usize) {
                    compute_memory = compute_memory.saturating_add(frame.bytes());
                }
            }
        }
        if let Some(frame) = &self.gui_displayed {
            compute_memory = compute_memory.saturating_add(frame.bytes().saturating_mul(2));
        }
        compute_memory = compute_memory.saturating_add(
            self.gui_dialogs
                .len()
                .saturating_mul(gui::dialog::RESERVATION),
        );
        if self.gui_clipboard_enabled || self.gui_ime_enabled || self.gui_accessibility_enabled {
            compute_memory = compute_memory.saturating_add(
                (self.gui_window_hosts.len() + usize::from(self.gui_host.is_some()))
                    .saturating_mul(self.gui_host_reservation()),
            );
        }
        for (id, frame) in &self.gui_window_frames {
            compute_memory = compute_memory
                .saturating_add(id.len() + 128 + frame.bytes().saturating_mul(2))
                .saturating_add(
                    (frame.width as usize)
                        .saturating_mul(frame.height as usize)
                        .saturating_mul(4),
                );
        }
        if compute_memory > self.budget.history_memory {
            if let Some(a) = &self.shared_accounting {
                compute_memory = compute_memory.saturating_sub(a.prune_bytes());
            }
        }
        if compute_memory > self.budget.history_memory {
            self.history_budget_kind.set("HistoryMemory");
            return Err(Error::HistoryBudgetExceeded);
        }
        segments.sort_by_key(|segment| segment.id);
        let mut journal_memory = 0usize;
        let mut storage = 0usize;
        for segment in &segments {
            let (mem, disk) = segment.usage();
            journal_memory = journal_memory.saturating_add(mem);
            storage = storage.saturating_add(disk);
        }
        if storage > self.budget.history_storage {
            self.history_budget_kind.set("HistoryStorage");
            return Err(Error::HistoryBudgetExceeded);
        }
        let target = self
            .budget
            .spill_threshold
            .min(self.budget.history_memory - compute_memory);
        for segment in segments {
            if journal_memory <= target {
                break;
            }
            let (mem, _) = segment.usage();
            if mem == 0 {
                continue;
            }
            if storage.saturating_add(mem) > self.budget.history_storage {
                self.history_budget_kind.set("HistoryStorage");
                return Err(Error::HistoryBudgetExceeded);
            }
            segment.spill()?;
            journal_memory -= mem;
            storage += mem;
        }
        if let Some(accounting) = &self.numeric_accounting {
            self.numeric_checked_generation.set(accounting.generation());
        }
        if let Some(a) = &self.shared_accounting {
            self.shared_checked_generation.set(a.generation());
        }
        Ok(())
    }

    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn set_program_counter(&mut self, pc: usize) {
        self.state.program_counter = pc;
    }
    /// Replace only the active task's compute roots; virtual I/O and heap stay shared.
    pub fn set_task_compute(
        &mut self,
        stack: Arc<Vec<Value>>,
        frames: Arc<Vec<CallFrame>>,
        globals: Arc<BTreeMap<String, Value>>,
    ) -> Result<()> {
        let previous = (
            self.state.stack.clone(),
            self.state.call_frames.clone(),
            self.state.globals.clone(),
        );
        self.state.stack = stack;
        self.state.call_frames = frames;
        self.state.globals = globals;
        if let Err(error) = self.enforce_budget() {
            (self.state.stack, self.state.call_frames, self.state.globals) = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn push_stack(&mut self, value: Value) -> Result<()> {
        let previous = self.state.stack.clone();
        Arc::make_mut(&mut self.state.stack).push(value);
        if let Err(error) = self.enforce_budget() {
            self.state.stack = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn pop_stack(&mut self) -> Result<Value> {
        Arc::make_mut(&mut self.state.stack)
            .pop()
            .ok_or_else(|| Error::InvalidOperation("stack underflow".into()))
    }
    pub fn push_frame(&mut self, return_pc: usize) -> Result<()> {
        let previous = self.state.call_frames.clone();
        Arc::make_mut(&mut self.state.call_frames).push(CallFrame {
            return_pc,
            locals: BTreeMap::new(),
        });
        if let Err(error) = self.enforce_budget() {
            self.state.call_frames = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn pop_frame(&mut self) -> Result<CallFrame> {
        Arc::make_mut(&mut self.state.call_frames)
            .pop()
            .ok_or_else(|| Error::InvalidOperation("call stack underflow".into()))
    }
    pub fn set_local(&mut self, name: impl Into<String>, value: Value) -> Result<()> {
        let previous = self.state.call_frames.clone();
        let frame = Arc::make_mut(&mut self.state.call_frames)
            .last_mut()
            .ok_or_else(|| Error::InvalidOperation("no active call frame".into()))?;
        frame.locals.insert(name.into(), value);
        if let Err(error) = self.enforce_budget() {
            self.state.call_frames = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn local(&self, name: &str) -> Option<&Value> {
        self.state.call_frames.last()?.locals.get(name)
    }
    pub fn remove_local(&mut self, name: &str) -> Result<()> {
        let frame = Arc::make_mut(&mut self.state.call_frames)
            .last_mut()
            .ok_or_else(|| Error::InvalidOperation("no active call frame".into()))?;
        frame.locals.remove(name);
        Ok(())
    }
    pub fn set_global(&mut self, name: impl Into<String>, value: Value) -> Result<()> {
        let previous = self.state.globals.clone();
        Arc::make_mut(&mut self.state.globals).insert(name.into(), value);
        if let Err(error) = self.enforce_budget() {
            self.state.globals = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn global(&self, name: &str) -> Option<&Value> {
        self.state.globals.get(name)
    }
    pub fn remove_global(&mut self, name: &str) {
        Arc::make_mut(&mut self.state.globals).remove(name);
    }
    pub fn configure_execution_work(&mut self, limit: usize) {
        self.execution_remaining = Some(limit);
    }
    pub fn execution_work_remaining(&self) -> Option<usize> {
        self.execution_remaining
    }
    pub fn charge_execution_work(&mut self, work: usize) -> Result<()> {
        if let Some(remaining) = &mut self.execution_remaining {
            *remaining = remaining.checked_sub(work).ok_or_else(|| {
                Error::InvalidOperation("ExecutionBudgetExceeded: host ceiling".into())
            })?;
        }
        Ok(())
    }
    pub fn native_work_remaining(&self) -> Option<usize> {
        self.native_remaining
    }
    pub fn configure_native_work(&mut self, limit: usize) {
        self.native_remaining = Some(limit);
    }
    pub fn charge_native_work(&mut self, amount: usize) -> Result<()> {
        if let Some(remaining) = &mut self.native_remaining {
            *remaining = remaining
                .checked_sub(amount)
                .ok_or_else(|| Error::InvalidOperation("NativeWorkBudgetExceeded".into()))?;
        }
        Ok(())
    }
    pub fn collection_due(&self) -> bool {
        let numeric = self.numeric_accounting.as_ref().map_or(0, |a| {
            a.allocated_bytes().saturating_sub(self.numeric_gc_start)
        });
        let shared = self.shared_accounting.as_ref().map_or(0, |a| {
            a.allocated_bytes().saturating_sub(self.shared_gc_start)
        });
        self.allocations_since_gc >= 256
            || self
                .allocation_bytes_since_gc
                .saturating_add(numeric)
                .saturating_add(shared)
                >= 4 * 1024 * 1024
    }
    /// Embedders must supply every external live Value at a safe point.
    /// Checkpoints keep independent roots; collection never mutates those roots.
    pub fn collect_heap(
        &mut self,
        external_roots: &[Value],
        work_limit: usize,
    ) -> Result<(usize, usize)> {
        if external_roots.len() > work_limit {
            return Err(Error::InvalidOperation(
                "NativeWorkBudgetExceeded: heap roots".into(),
            ));
        }
        self.collect_heap_refs(&external_roots.iter().collect::<Vec<_>>(), work_limit)
    }
    /// Borrowed equivalent of collect_heap; supply all external roots at a safe point.
    pub fn collect_heap_refs(
        &mut self,
        external_roots: &[&Value],
        work_limit: usize,
    ) -> Result<(usize, usize)> {
        if self.gc_metrics.is_none() {
            return self.collect_heap_refs_inner(external_roots, work_limit);
        }
        let started = std::time::Instant::now();
        let result = self.collect_heap_refs_inner(external_roots, work_limit);
        let duration = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        let metrics = self.gc_metrics.as_mut().expect("enabled profiling");
        metrics.attempts = metrics.attempts.saturating_add(1);
        metrics.duration_nanos = metrics.duration_nanos.saturating_add(duration);
        match &result {
            Ok((reclaimed, work)) => {
                metrics.completed = metrics.completed.saturating_add(1);
                metrics.reclaimed_objects = metrics.reclaimed_objects.saturating_add(*reclaimed);
                metrics.work = metrics.work.saturating_add(*work);
            }
            Err(_) => {
                metrics.failed = metrics.failed.saturating_add(1);
            }
        }
        result
    }
    fn collect_heap_refs_inner(
        &mut self,
        external_roots: &[&Value],
        work_limit: usize,
    ) -> Result<(usize, usize)> {
        let work_limit = self
            .native_remaining
            .map_or(work_limit, |n| n.min(work_limit));
        if self.cached_heap_traversal {
            let (live, work) = self.trace_heap_cached(external_roots, work_limit)?;
            return self.sweep_heap(live, work);
        }
        let mut live = BTreeSet::new();
        let mut pending = self
            .state
            .globals
            .values()
            .chain(self.state.stack.iter())
            .chain(
                self.state
                    .call_frames
                    .iter()
                    .flat_map(|f| f.locals.values()),
            )
            .chain(external_roots.iter().copied())
            .take(work_limit.saturating_add(1))
            .collect::<Vec<_>>();
        if pending.len() > work_limit {
            return Err(Error::InvalidOperation(
                "NativeWorkBudgetExceeded: heap roots".into(),
            ));
        }
        let mut work = 0usize;
        while let Some(value) = pending.pop() {
            work += 1;
            if work > work_limit {
                return Err(Error::InvalidOperation(
                    "NativeWorkBudgetExceeded: heap collection".into(),
                ));
            }
            let children = match value {
                Value::List(v) => v.len(),
                Value::TypedList(_, v) => v.len(),
                Value::Map(v) | Value::TypedMap(_, _, v) => v.len(),
                Value::OrderedMap(_, _, v) => v.len().saturating_mul(2),
                Value::Struct(_, v) | Value::Closure(_, _, v) => v.len(),
                Value::Enum(_, _, v) => v.len(),
                Value::Option(Some(_)) | Value::Result(_) => 1,
                Value::HeapRef(id) | Value::CellRef(id)
                    if !live.contains(id) && self.state.heap.get(id).is_some() =>
                {
                    1
                }
                _ => 0,
            };
            if children
                > work_limit
                    .saturating_sub(work)
                    .saturating_sub(pending.len())
            {
                return Err(Error::InvalidOperation(
                    "NativeWorkBudgetExceeded: heap children".into(),
                ));
            }
            match value {
                Value::HeapRef(id) | Value::CellRef(id) => {
                    if live.insert(*id) {
                        if let Some(value) = self.state.heap.get(id) {
                            pending.push(value);
                        }
                    }
                }
                Value::List(values) => pending.extend(values),
                Value::TypedList(_, values) => pending.extend(values.iter()),
                Value::Map(values) | Value::TypedMap(_, _, values) => {
                    pending.extend(values.values())
                }
                Value::OrderedMap(_, _, values) => {
                    for (k, v) in values {
                        pending.push(k);
                        pending.push(v);
                    }
                }
                Value::Struct(_, values) | Value::Closure(_, _, values) => {
                    pending.extend(values.values())
                }
                Value::Enum(_, _, values) => pending.extend(values.iter().map(|(_, v)| v)),
                Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                    pending.push(v)
                }
                _ => {}
            }
        }
        work = work.saturating_add(self.state.heap.len());
        if work > work_limit {
            return Err(Error::InvalidOperation(
                "NativeWorkBudgetExceeded: heap collection".into(),
            ));
        }
        self.sweep_heap(live, work)
    }
    fn trace_heap_cached(
        &self,
        external_roots: &[&Value],
        limit: usize,
    ) -> Result<(BTreeSet<u64>, usize)> {
        let mut gc = HeapTraversal {
            pending: Vec::new(),
            seen: HashSet::new(),
            work: 0,
            limit,
        };
        for value in self
            .state
            .globals
            .values()
            .chain(self.state.stack.iter())
            .chain(
                self.state
                    .call_frames
                    .iter()
                    .flat_map(|f| f.locals.values()),
            )
            .chain(external_roots.iter().copied())
        {
            gc.push(value)?;
        }
        let mut live = BTreeSet::new();
        while let Some(value) = gc.pending.pop() {
            gc.charge(1)?;
            match value {
                Value::HeapRef(id) | Value::CellRef(id) => {
                    if live.insert(*id) {
                        let edge = if self.cached_heap_entries {
                            self.state.heap.edge_value(*id)
                        } else {
                            self.state.heap.get(id)
                        };
                        if let Some(v) = edge {
                            gc.push(v)?;
                        }
                    }
                }
                Value::TypedList(_, v) => v.trace_heap_refs(&mut gc)?,
                Value::Map(v) | Value::TypedMap(_, _, v) => v.trace_heap_refs(&mut gc)?,
                Value::List(v) => {
                    for v in v {
                        gc.push(v)?;
                    }
                }
                Value::OrderedMap(_, _, v) => {
                    for (k, v) in v {
                        gc.push(k)?;
                        gc.push(v)?;
                    }
                }
                Value::Struct(_, v) | Value::Closure(_, _, v) => {
                    for v in v.values() {
                        gc.push(v)?;
                    }
                }
                Value::Enum(_, _, v) => {
                    for (_, v) in v {
                        gc.push(v)?;
                    }
                }
                Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                    gc.push(v)?
                }
                _ => {}
            }
        }
        gc.charge(self.state.heap.len())?;
        Ok((live, gc.work))
    }
    fn sweep_heap(&mut self, live: BTreeSet<u64>, work: usize) -> Result<(usize, usize)> {
        self.charge_native_work(work)?;
        let dead = self
            .state
            .heap
            .iter()
            .filter(|(id, _)| !live.contains(id))
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        let previous = self.state.heap.clone();
        for id in &dead {
            Arc::make_mut(&mut self.state.heap).remove(*id);
        }
        if let Err(error) = self.enforce_budget() {
            self.state.heap = previous;
            return Err(error);
        }
        drop(previous);
        if let Some(a) = &self.shared_accounting {
            a.prune_bytes();
        }
        self.allocations_since_gc = 0;
        self.allocation_bytes_since_gc = 0;
        self.numeric_gc_start = self
            .numeric_accounting
            .as_ref()
            .map_or(0, numeric::Accounting::allocated_bytes);
        self.shared_gc_start = self
            .shared_accounting
            .as_ref()
            .map_or(0, shared_payload::Accounting::allocated_bytes);
        Ok((dead.len(), work))
    }
    pub fn alloc(&mut self, value: Value) -> Result<u64> {
        let mut bytes = if self.numeric_accounting.is_some() {
            Self::value_bytes(&value).saturating_sub(Self::numeric_payload_bytes(&value))
        } else {
            Self::value_bytes(&value)
        };
        if let Some(a) = &self.shared_accounting {
            bytes = bytes.saturating_sub(Self::shared_payload_bytes(&value));
            if a.includes_bytes() {
                bytes = bytes.saturating_sub(Self::byte_payload_info(&value).0);
            }
            if a.includes_containers() {
                bytes = bytes.saturating_sub(Self::container_logical_discount(&value));
            }
        }
        let id = self.state.next_heap_id;
        let previous = self.state.heap.clone();
        self.state.next_heap_id = id
            .checked_add(1)
            .ok_or_else(|| Error::InvalidOperation("heap ID exhausted".into()))?;
        Arc::make_mut(&mut self.state.heap).insert(id, value);
        if let Err(error) = self.enforce_budget() {
            self.state.heap = previous;
            self.state.next_heap_id = id;
            return Err(error);
        }
        self.allocations_since_gc = self.allocations_since_gc.saturating_add(1);
        self.allocation_bytes_since_gc = self.allocation_bytes_since_gc.saturating_add(bytes);
        Ok(id)
    }
    pub fn heap_set(&mut self, id: u64, value: Value) -> Result<()> {
        let bytes = |value: &Value| {
            let mut total = Self::value_bytes(value);
            if self.numeric_accounting.is_some() {
                total = total.saturating_sub(Self::numeric_payload_bytes(value));
            }
            if let Some(a) = &self.shared_accounting {
                total = total.saturating_sub(Self::shared_payload_bytes(value));
                if a.includes_bytes() {
                    total = total.saturating_sub(Self::byte_payload_info(value).0);
                }
                if a.includes_containers() {
                    total = total.saturating_sub(Self::container_logical_discount(value));
                }
            }
            total
        };
        let growth = bytes(&value).saturating_sub(self.state.heap.get(&id).map_or(0, bytes));
        let previous = self.state.heap.clone();
        let heap = Arc::make_mut(&mut self.state.heap);
        if !heap.contains_key(&id) {
            return Err(Error::InvalidOperation(format!(
                "unknown heap object: {id}"
            )));
        }
        heap.insert(id, value);
        if let Err(error) = self.enforce_budget() {
            self.state.heap = previous;
            return Err(error);
        }
        self.allocation_bytes_since_gc = self.allocation_bytes_since_gc.saturating_add(growth);
        Ok(())
    }
    pub fn heap_get(&self, id: u64) -> Option<&Value> {
        self.state.heap.get(&id)
    }
    pub fn display_value(&self, value: &Value) -> String {
        fn secret(rt: &Runtime, v: &Value, seen: &mut BTreeSet<u64>, depth: usize) -> bool {
            if depth > 64 {
                return true;
            }
            match v {
                Value::Struct(t, _) if t.starts_with("Secret<") => true,
                Value::HeapRef(id) | Value::CellRef(id) => {
                    seen.insert(*id)
                        && rt
                            .heap_get(*id)
                            .is_some_and(|v| secret(rt, v, seen, depth + 1))
                }
                Value::List(v) => v.iter().any(|v| secret(rt, v, seen, depth + 1)),
                Value::TypedList(_, v) => v.iter().any(|v| secret(rt, v, seen, depth + 1)),
                Value::Struct(_, v) | Value::Closure(_, _, v) => {
                    v.values().any(|v| secret(rt, v, seen, depth + 1))
                }
                Value::Enum(_, _, v) => v.iter().any(|(_, v)| secret(rt, v, seen, depth + 1)),
                Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                    secret(rt, v, seen, depth + 1)
                }
                Value::Map(v) | Value::TypedMap(_, _, v) => {
                    v.values().any(|v| secret(rt, v, seen, depth + 1))
                }
                Value::OrderedMap(_, _, v) => v
                    .iter()
                    .any(|(a, b)| secret(rt, a, seen, depth + 1) || secret(rt, b, seen, depth + 1)),
                _ => false,
            }
        }
        if secret(self, value, &mut BTreeSet::new(), 0) {
            return "<redacted: Secret>".into();
        }
        match value {
            Value::HeapRef(id) => self
                .heap_get(*id)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("<missing object:{id}>")),
            other => other.to_string(),
        }
    }

    /// Install the immutable language-level initial checkpoint, before user code.
    pub fn install_begin(&mut self) -> Result<()> {
        self.commit("begin")
    }
    /// Reset transactional state and discard every user checkpoint. Observed input
    /// and published effects remain external facts, just as for ordinary revert.
    pub fn reset_begin(&mut self) -> Result<()> {
        self.require_internal()?;
        self.state = self
            .checkpoints
            .get("begin")
            .ok_or_else(|| Error::MissingCheckpoint("begin".into()))?
            .state
            .clone();
        self.restore_pending();
        self.current_parent = Some("begin".into());
        self.checkpoints.retain(|name, _| name == "begin");
        self.reclaim_operation_ledger();
        Ok(())
    }
    pub fn commit(&mut self, name: impl Into<String>) -> Result<()> {
        self.require_internal()?;
        let name = name.into();
        if self.checkpoints.contains_key(&name) {
            return Err(Error::InvalidOperation(format!(
                "checkpoint already exists: {name}"
            )));
        }
        self.checkpoints.insert(
            name.clone(),
            Checkpoint {
                state: self.state.clone(),
                parent: self.current_parent.clone(),
                tainted: false,
            },
        );
        if let Err(error) = self.enforce_budget() {
            self.checkpoints.remove(&name);
            return Err(error);
        }
        self.current_parent = Some(name);
        Ok(())
    }
    pub fn revert(&mut self, name: &str) -> Result<()> {
        self.require_internal()?;
        let checkpoint = self
            .checkpoints
            .get(name)
            .ok_or_else(|| Error::MissingCheckpoint(name.into()))?;
        if checkpoint.tainted {
            return Err(Error::TaintedCheckpoint(name.into()));
        }
        self.state = checkpoint.state.clone();
        self.restore_pending();
        self.current_parent = Some(name.into());
        Ok(())
    }
    pub fn drop_checkpoint(&mut self, name: &str) -> Result<()> {
        self.require_internal()?;
        let removed = self
            .checkpoints
            .remove(name)
            .ok_or_else(|| Error::MissingCheckpoint(name.into()))?;
        for checkpoint in self.checkpoints.values_mut() {
            if checkpoint.parent.as_deref() == Some(name) {
                checkpoint.parent = removed.parent.clone();
            }
        }
        if self.current_parent.as_deref() == Some(name) {
            self.current_parent = removed.parent;
        }
        self.reclaim_operation_ledger();
        Ok(())
    }
    pub fn checkpoint_parent(&self, name: &str) -> Result<Option<&str>> {
        Ok(self
            .checkpoints
            .get(name)
            .ok_or_else(|| Error::MissingCheckpoint(name.into()))?
            .parent
            .as_deref())
    }
    pub fn trace_checkpoint(&self, name: &str) -> Result<String> {
        let checkpoint = self
            .checkpoints
            .get(name)
            .ok_or_else(|| Error::MissingCheckpoint(name.into()))?;
        let state = &checkpoint.state;
        let files = state
            .files
            .iter()
            .map(|(path, value)| {
                format!(
                    "{}:{path}",
                    if value.is_some() { "write" } else { "delete" }
                )
            })
            .collect::<Vec<_>>();
        let directories = state
            .directories
            .iter()
            .map(|(path, exists)| format!("{}:{path}", if *exists { "create" } else { "delete" }))
            .collect::<Vec<_>>();
        Ok(format!(
            "checkpoint={name} parent={} pc={} files=[{}] directories=[{}] stdout={} stderr={} stdin_cursor={} time_cursor={}",
            checkpoint.parent.as_deref().unwrap_or("<root>"), state.program_counter,
            files.join(","), directories.join(","), state.stdout.len(), state.stderr.len(),
            state.stdin_cursor, state.time_cursor
        ))
    }
    pub fn trace_checkpoint_json(&self, name: &str) -> Result<String> {
        fn quote(input: &str) -> String {
            let mut out = String::from("\"");
            for c in input.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if c <= '\u{001f}' => out.push_str(&format!("\\u{:04x}", c as u32)),
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        }
        let checkpoint = self
            .checkpoints
            .get(name)
            .ok_or_else(|| Error::MissingCheckpoint(name.into()))?;
        let state = &checkpoint.state;
        let files = state
            .files
            .iter()
            .map(|(path, v)| {
                format!(
                    "{{\"path\":{},\"operation\":{}}}",
                    quote(path),
                    quote(if v.is_some() { "write" } else { "delete" })
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let dirs = state
            .directories
            .iter()
            .map(|(path, v)| {
                format!(
                    "{{\"path\":{},\"operation\":{}}}",
                    quote(path),
                    quote(if *v { "create" } else { "delete" })
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let parent = checkpoint
            .parent
            .as_ref()
            .map_or("null".into(), |p| quote(p));
        Ok(format!("{{\"checkpoint\":{},\"parent\":{},\"pc\":{},\"frames\":{},\"files\":[{}],\"directories\":[{}],\"stdout_bytes\":{},\"stderr_bytes\":{},\"cursors\":{{\"stdin\":{},\"time\":{},\"args\":{},\"env\":{},\"directory\":{}}},\"secret_env_count\":{}}}",quote(name),parent,state.program_counter,state.call_frames.len(),files,dirs,state.stdout.len(),state.stderr.len(),state.stdin_cursor,state.time_cursor,state.args_cursor,state.env_cursor,state.directory_cursor,self.env_secrets.len()))
    }
    pub fn begin_branch(&self) -> BranchAnchor {
        let state = Arc::new(self.state.clone());
        let mut roots = self.branch_roots.borrow_mut();
        roots.retain(|root| root.strong_count() > 0);
        roots.push(Arc::downgrade(&state));
        BranchAnchor {
            state,
            parent: self.current_parent.clone(),
        }
    }
    pub fn end_branch(&mut self, name: impl Into<String>, anchor: BranchAnchor) -> Result<()> {
        self.commit(name)?;
        self.state = (*anchor.state).clone();
        self.restore_pending();
        self.current_parent = anchor.parent;
        Ok(())
    }
    pub fn taint_checkpoints(&mut self) {
        for checkpoint in self.checkpoints.values_mut() {
            checkpoint.tainted = true;
        }
    }

    pub fn print_out(&mut self, text: &str) -> Result<()> {
        self.write_output(text.as_bytes())
    }
    pub fn write_output(&mut self, bytes: &[u8]) -> Result<()> {
        let next_operation = self.next_operation;
        let previous = self.state.stdout.clone();
        let operation = if self.incremental_publish {
            Some(self.operation()?)
        } else {
            None
        };
        self.state.stdout.append_operation(bytes, operation);
        if let Err(error) = self.enforce_budget() {
            self.state.stdout = previous;
            self.next_operation = next_operation;
            return Err(error);
        }
        Ok(())
    }
    pub fn print_err(&mut self, text: &str) -> Result<()> {
        let next_operation = self.next_operation;
        let previous = self.state.stderr.clone();
        let operation = if self.incremental_publish {
            Some(self.operation()?)
        } else {
            None
        };
        self.state
            .stderr
            .append_operation(text.as_bytes(), operation);
        if let Err(error) = self.enforce_budget() {
            self.state.stderr = previous;
            self.next_operation = next_operation;
            return Err(error);
        }
        Ok(())
    }
    pub fn input_chunk(&mut self, source: &mut impl Read, limit: usize) -> Result<Option<Vec<u8>>> {
        if !(1..=65536).contains(&limit) {
            return Err(Error::InvalidOperation(
                "readChunk requires 1..65536 bytes".into(),
            ));
        }
        let cursor = self.state.byte_cursor;
        if cursor == self.byte_input.len() {
            if self.byte_eof {
                return Ok(None);
            }
            if self.replaying {
                return Err(Error::InvalidOperation("ReplayMismatch: byte input".into()));
            }
            let mut buffer = vec![0; limit];
            let count = loop {
                match source.read(&mut buffer) {
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    result => break result?,
                }
            };
            if count == 0 {
                self.byte_eof = true;
                return Ok(None);
            }
            buffer.truncate(count);
            self.byte_input
                .push((limit, Arc::new(Segment::new(buffer))));
            // The observation stays recorded even on failure: the Host was consumed.
        }
        // A recorded observation must also stay within budget when retried/restored.
        self.enforce_budget()?;
        let (recorded_limit, bytes) = &self.byte_input[cursor];
        if *recorded_limit != limit {
            return Err(Error::InvalidOperation(
                "ReplayMismatch: readChunk size differs at restored input cursor".into(),
            ));
        }
        self.state.byte_cursor += 1;
        Ok(Some(bytes.bytes()?))
    }
    pub fn input_line(&mut self, source: &mut impl io::BufRead) -> Result<Option<String>> {
        let cursor = self.state.stdin_cursor;
        if cursor == self.input.len() {
            if self.replaying {
                return if self.input_eof {
                    Ok(None)
                } else {
                    Err(Error::InvalidOperation(format!(
                        "ReplayMismatch: stdin at {cursor}"
                    )))
                };
            }
            let mut line = String::new();
            if source.read_line(&mut line)? == 0 {
                self.input_eof = true;
                return Ok(None);
            }
            self.input.push(line);
        }
        self.state.stdin_cursor += 1;
        Ok(Some(
            self.input[cursor]
                .trim_end_matches(['\r', '\n'])
                .to_string(),
        ))
    }
    pub fn now_millis(&mut self) -> Result<u128> {
        let cursor = self.state.time_cursor;
        if cursor == self.times.len() {
            if self.replaying {
                return Err(Error::InvalidOperation(format!(
                    "ReplayMismatch: clock at {cursor}"
                )));
            }
            self.times.push(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| Error::InvalidOperation(e.to_string()))?
                    .as_millis(),
            );
        }
        self.state.time_cursor += 1;
        Ok(self.times[cursor])
    }
    pub fn random_u64(&mut self) -> u64 {
        let mut x = self.state.random_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state.random_state = x;
        x
    }

    fn checked_path(&self, path: &str) -> Result<PathBuf> {
        let relative = Path::new(path);
        if path.contains('\0')
            || path.contains('\\')
            || path.contains(':')
            || relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(Error::InvalidPath(path.into()));
        }
        #[cfg(windows)]
        for part in path.split('/') {
            let base = part
                .split('.')
                .next()
                .unwrap_or("")
                .trim_end()
                .to_uppercase();
            let device = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || ["COM", "LPT"].iter().any(|prefix| {
                    base.strip_prefix(prefix).is_some_and(|suffix| {
                        matches!(
                            suffix,
                            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                        )
                    })
                });
            if device
                || part.ends_with(['.', ' '])
                || part
                    .chars()
                    .any(|c| c.is_control() || "<>\"|?*".contains(c))
            {
                return Err(Error::InvalidPath(path.into()));
            }
        }
        let full = self.root.join(relative);
        if self
            .database_host
            .as_ref()
            .is_some_and(|host| host.blocks_path(&full))
        {
            return Err(Error::InvalidOperation(
                "DbFileBusy: close the database before virtual file access".into(),
            ));
        }
        // Existing symlinks (including parent directories) may not escape the root.
        let mut ancestor = full.as_path();
        while !ancestor.exists() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| Error::InvalidPath(path.into()))?;
        }
        if !fs::canonicalize(ancestor)?.starts_with(&self.root) {
            return Err(Error::InvalidPath(path.into()));
        }
        Ok(full)
    }
    fn host_directory_entries(full: &Path) -> Result<Option<BTreeSet<String>>> {
        match fs::read_dir(full) {
            Ok(entries) => {
                let mut names = BTreeSet::new();
                for entry in entries {
                    names.insert(entry?.file_name().to_string_lossy().into_owned());
                }
                Ok(Some(names))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    fn observe_directory(&mut self, path: &str) -> Result<Option<BTreeSet<String>>> {
        let full = self.checked_path(path)?;
        let key = (self.state.file_epoch, path.to_string());
        if let Some(entries) = self.directory_observations.get(&key) {
            return Ok(entries.clone());
        }
        if self.replaying {
            return Err(Error::InvalidOperation(format!(
                "ReplayMismatch: directory observation {path}"
            )));
        }
        let entries = Self::host_directory_entries(&full)?;
        self.directory_observations.insert(key, entries.clone());
        Ok(entries)
    }
    fn host_version(full: &Path) -> Result<Option<HostVersion>> {
        match fs::metadata(full) {
            Ok(metadata) if metadata.is_file() => {
                let mut file = fs::File::open(full)?;
                let mut hash = 0x6c62272e07bb014262b821756295c58du128;
                let mut buffer = [0u8; 64 * 1024];
                loop {
                    let count = file.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    for byte in &buffer[..count] {
                        hash ^= u128::from(*byte);
                        hash = hash.wrapping_mul(0x0000000001000000000000000000013bu128);
                    }
                }
                let after = file.metadata()?;
                if metadata.len() != after.len()
                    || metadata.modified().ok() != after.modified().ok()
                {
                    return Err(Error::ExternalStateConflict(full.display().to_string()));
                }
                Ok(Some(HostVersion {
                    len: metadata.len(),
                    modified: metadata.modified().ok(),
                    content_hash: hash,
                }))
            }
            Ok(_) => Err(Error::InvalidPath(full.display().to_string())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    fn observe_metadata(&mut self, path: &str) -> Result<Option<HostVersion>> {
        let full = self.checked_path(path)?;
        let key = (self.state.file_epoch, path.to_string());
        if let Some(o) = self.observations.get(&key) {
            return Ok(o.version.clone());
        }
        if self.replaying {
            return Err(Error::InvalidOperation(format!(
                "ReplayMismatch: file metadata {path}"
            )));
        }
        let version = Self::host_version(&full)?;
        self.observations.insert(
            key,
            Observation {
                version: version.clone(),
                blocks: BTreeMap::new(),
            },
        );
        Ok(version)
    }
    fn read_observed_range(&mut self, path: &str, offset: usize, count: usize) -> Result<Vec<u8>> {
        let version = self
            .observe_metadata(path)?
            .ok_or_else(|| Error::MissingFile(path.into()))?;
        let key = (self.state.file_epoch, path.to_string());
        let total = usize::try_from(version.len)
            .map_err(|_| Error::InvalidOperation("file too large".into()))?;
        if offset >= total || count == 0 {
            return Ok(Vec::new());
        }
        let end = offset.saturating_add(count).min(total);
        let first = offset / FILE_PAGE_SIZE;
        let last = (end - 1) / FILE_PAGE_SIZE;
        let needs_host =
            (first..=last).any(|index| !self.observations[&key].blocks.contains_key(&index));
        if needs_host {
            if self.replaying {
                return Err(Error::InvalidOperation(format!(
                    "ReplayMismatch: file blocks {path} offset {offset}"
                )));
            }
            let full = self.checked_path(path)?;
            if Self::host_version(&full)? != Some(version.clone()) {
                return Err(Error::ExternalStateConflict(path.into()));
            }
            let mut file = fs::File::open(&full)?;
            let mut new_blocks = Vec::new();
            for index in first..=last {
                if self.observations[&key].blocks.contains_key(&index) {
                    continue;
                }
                let start = index * FILE_PAGE_SIZE;
                let len = (total - start).min(FILE_PAGE_SIZE);
                let mut block = vec![0; len];
                file.seek(SeekFrom::Start(start as u64))?;
                file.read_exact(&mut block)?;
                new_blocks.push((index, Arc::new(Segment::new(block))));
            }
            if Self::host_version(&full)? != Some(version) {
                return Err(Error::ExternalStateConflict(path.into()));
            }
            let observation = self.observations.get_mut(&key).expect("observation exists");
            for (index, block) in &new_blocks {
                observation.blocks.insert(*index, block.clone());
            }
            if let Err(error) = self.enforce_budget() {
                let observation = self.observations.get_mut(&key).expect("observation exists");
                for (index, _) in new_blocks {
                    observation.blocks.remove(&index);
                }
                return Err(error);
            }
        }
        let mut result = Vec::with_capacity(end - offset);
        for index in first..=last {
            let block = self.observations[&key].blocks[&index].bytes()?;
            let start = if index == first {
                offset % FILE_PAGE_SIZE
            } else {
                0
            };
            let stop = if index == last {
                (end - 1) % FILE_PAGE_SIZE + 1
            } else {
                block.len()
            };
            result.extend_from_slice(&block[start..stop]);
        }
        Ok(result)
    }
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        self.checked_path(path)?;
        if let Some(entry) = self
            .state
            .files
            .get(path)
            .or_else(|| self.virtual_files.get(path))
        {
            return entry
                .as_ref()
                .ok_or_else(|| Error::MissingFile(path.into()))?
                .to_vec();
        }
        let version = self
            .observe_metadata(path)?
            .ok_or_else(|| Error::MissingFile(path.into()))?;
        let len = usize::try_from(version.len)
            .map_err(|_| Error::InvalidOperation("file too large".into()))?;
        self.read_observed_range(path, 0, len)
    }
    pub fn write_file(&mut self, path: &str, data: impl AsRef<[u8]>) -> Result<()> {
        let data = data.as_ref();
        self.pending_transaction(|rt| rt.write_file_inner(path, data))
    }
    fn write_file_inner(&mut self, path: &str, data: impl AsRef<[u8]>) -> Result<()> {
        self.observe_metadata(path)?;
        let previous = self.state.files.clone();
        Arc::make_mut(&mut self.state.files).insert(
            path.into(),
            Some(Arc::new(PagedFile::from_bytes(data.as_ref()))),
        );
        if let Err(error) = self.enforce_budget() {
            self.state.files = previous;
            return Err(error);
        }
        self.file_operation(path)?;
        Ok(())
    }
    pub fn append_file(&mut self, path: &str, data: impl AsRef<[u8]>) -> Result<()> {
        let data = data.as_ref();
        self.pending_transaction(|rt| rt.append_file_inner(path, data))
    }
    fn append_file_inner(&mut self, path: &str, data: impl AsRef<[u8]>) -> Result<()> {
        let previous = self.state.files.clone();
        self.checked_path(path)?;
        if !self.state.files.contains_key(path) {
            let content = match self.read_file(path) {
                Ok(bytes) => PagedFile::from_bytes(&bytes),
                Err(Error::MissingFile(_)) => PagedFile::from_bytes(&[]),
                Err(error) => return Err(error),
            };
            Arc::make_mut(&mut self.state.files).insert(path.into(), Some(Arc::new(content)));
        }
        let file = Arc::make_mut(&mut self.state.files)
            .get_mut(path)
            .and_then(Option::as_mut)
            .ok_or_else(|| Error::MissingFile(path.into()))?;
        if let Err(error) = Arc::make_mut(file).append(data.as_ref()) {
            self.state.files = previous;
            return Err(error);
        }
        if let Err(error) = self.enforce_budget() {
            self.state.files = previous;
            return Err(error);
        }
        self.file_operation(path)?;
        Ok(())
    }
    pub fn truncate_file(&mut self, path: &str, len: usize) -> Result<()> {
        self.pending_transaction(|rt| rt.truncate_file_inner(path, len))
    }
    fn truncate_file_inner(&mut self, path: &str, len: usize) -> Result<()> {
        let previous = self.state.files.clone();
        self.checked_path(path)?;
        if !self.state.files.contains_key(path) {
            let content = self.read_file(path)?;
            Arc::make_mut(&mut self.state.files)
                .insert(path.into(), Some(Arc::new(PagedFile::from_bytes(&content))));
        }
        let file = Arc::make_mut(&mut self.state.files)
            .get_mut(path)
            .and_then(Option::as_mut)
            .ok_or_else(|| Error::MissingFile(path.into()))?;
        if let Err(error) = Arc::make_mut(file).truncate(len) {
            self.state.files = previous;
            return Err(error);
        }
        if let Err(error) = self.enforce_budget() {
            self.state.files = previous;
            return Err(error);
        }
        self.file_operation(path)?;
        Ok(())
    }
    pub fn delete_file(&mut self, path: &str) -> Result<()> {
        self.pending_transaction(|rt| rt.delete_file_inner(path))
    }
    fn delete_file_inner(&mut self, path: &str) -> Result<()> {
        self.read_file(path)?;
        Arc::make_mut(&mut self.state.files).insert(path.into(), None);
        self.file_operation(path)?;
        Ok(())
    }
    pub fn copy_file(&mut self, from: &str, to: &str) -> Result<()> {
        self.pending_transaction(|rt| rt.copy_file_inner(from, to))
    }
    fn copy_file_inner(&mut self, from: &str, to: &str) -> Result<()> {
        let previous = self.state.files.clone();
        self.checked_path(from)?;
        self.observe_metadata(to)?;
        if let Some(entry) = self.state.files.get(from) {
            let content = entry
                .clone()
                .ok_or_else(|| Error::MissingFile(from.into()))?;
            Arc::make_mut(&mut self.state.files).insert(to.into(), Some(content));
            if let Err(error) = self.enforce_budget() {
                self.state.files = previous;
                return Err(error);
            }
            self.file_operation(to)?;
            Ok(())
        } else {
            let content = self.read_file(from)?;
            Arc::make_mut(&mut self.state.files)
                .insert(to.into(), Some(Arc::new(PagedFile::from_bytes(&content))));
            if let Err(error) = self.enforce_budget() {
                self.state.files = previous;
                return Err(error);
            }
            self.file_operation(to)?;
            Ok(())
        }
    }
    pub fn move_file(&mut self, from: &str, to: &str) -> Result<()> {
        self.pending_transaction(|rt| rt.move_file_inner(from, to))
    }
    fn move_file_inner(&mut self, from: &str, to: &str) -> Result<()> {
        self.copy_file(from, to)?;
        self.delete_file(from)
    }
    pub fn create_directory(&mut self, path: &str) -> Result<()> {
        self.pending_transaction(|rt| rt.create_directory_inner(path))
    }
    fn create_directory_inner(&mut self, path: &str) -> Result<()> {
        self.observe_directory(path)?;
        Arc::make_mut(&mut self.state.directories).insert(path.into(), true);
        self.directory_operation(path)?;
        Ok(())
    }
    pub fn delete_directory(&mut self, path: &str) -> Result<()> {
        self.pending_transaction(|rt| rt.delete_directory_inner(path))
    }
    fn delete_directory_inner(&mut self, path: &str) -> Result<()> {
        if !self.directory_entries(path)?.is_empty() {
            return Err(Error::InvalidOperation(format!(
                "directory is not empty: {path}"
            )));
        }
        Arc::make_mut(&mut self.state.directories).insert(path.into(), false);
        self.directory_operation(path)?;
        Ok(())
    }
    pub fn move_directory(&mut self, from: &str, to: &str) -> Result<()> {
        self.pending_transaction(|rt| rt.move_directory_inner(from, to))
    }
    fn move_directory_inner(&mut self, from: &str, to: &str) -> Result<()> {
        if to == from || to.starts_with(&format!("{from}/")) {
            return Err(Error::InvalidOperation(
                "cannot move a directory into itself".into(),
            ));
        }
        self.checked_path(from)?;
        self.checked_path(to)?;
        if self
            .state
            .directories
            .get(to)
            .or_else(|| self.virtual_directories.get(to))
            == Some(&true)
            || self.observe_directory(to)?.is_some()
        {
            return Err(Error::InvalidOperation(format!(
                "destination already exists: {to}"
            )));
        }
        let mut files = BTreeSet::new();
        let mut dirs = BTreeSet::new();
        self.collect_directory(from, &mut dirs, &mut files)?;
        for (path, content) in self.state.files.iter() {
            if path.starts_with(&format!("{from}/")) && content.is_some() {
                files.insert(path.clone());
            }
        }
        for (path, exists) in self.state.directories.iter() {
            if path.starts_with(&format!("{from}/")) && *exists {
                dirs.insert(path.clone());
            }
        }
        for path in &dirs {
            let dest = format!("{to}{}", &path[from.len()..]);
            self.create_directory(&dest)?;
        }
        for path in &files {
            if matches!(self.state.files.get(path), Some(None)) {
                continue;
            }
            let dest = format!("{to}{}", &path[from.len()..]);
            self.move_file(path, &dest)?;
        }
        for path in dirs.iter().rev() {
            self.delete_directory(path)?;
        }
        Ok(())
    }
    fn collect_directory(
        &mut self,
        path: &str,
        dirs: &mut BTreeSet<String>,
        files: &mut BTreeSet<String>,
    ) -> Result<()> {
        let entries = self.directory_entries(path)?;
        dirs.insert(path.into());
        for name in entries {
            let child = format!("{path}/{name}");
            let full = self.checked_path(&child)?;
            let is_dir = self
                .state
                .directories
                .get(&child)
                .or_else(|| self.virtual_directories.get(&child))
                .copied();
            let is_file = self
                .state
                .files
                .get(&child)
                .or_else(|| self.virtual_files.get(&child));
            if is_dir == Some(true) {
                self.collect_directory(&child, dirs, files)?;
            } else if is_file.is_some_and(|v| v.is_some()) {
                files.insert(child);
            } else {
                let metadata = fs::symlink_metadata(&full)?;
                if metadata.file_type().is_symlink() {
                    return Err(Error::InvalidPath(child));
                }
                if metadata.is_dir() {
                    self.collect_directory(&child, dirs, files)?;
                } else if metadata.is_file() {
                    files.insert(child);
                }
            }
        }
        Ok(())
    }
    pub fn open_file(&mut self, path: &str) -> Result<u64> {
        self.checked_path(path)?;
        if let Some(content) = self
            .state
            .files
            .get(path)
            .or_else(|| self.virtual_files.get(path))
        {
            if content.is_none() {
                return Err(Error::MissingFile(path.into()));
            }
        } else {
            self.observe_metadata(path)?
                .ok_or_else(|| Error::MissingFile(path.into()))?;
        }
        if self.state.files.get(path).is_some_and(Option::is_none) {
            return Err(Error::MissingFile(path.into()));
        }
        let id = self.state.next_handle_id;
        self.state.next_handle_id += 1;
        Arc::make_mut(&mut self.state.handles).insert(
            id,
            FileHandle {
                path: path.into(),
                mode: FileMode::ReadWrite,
                position: 0,
                buffer: Vec::new(),
                virtual_file_version: self.state.file_epoch,
                snapshot: None,
            },
        );
        Ok(id)
    }
    pub fn seek(&mut self, id: u64, position: usize) -> Result<()> {
        Arc::make_mut(&mut self.state.handles)
            .get_mut(&id)
            .ok_or(Error::MissingHandle(id))?
            .position = position;
        Ok(())
    }
    pub fn handle(&self, id: u64) -> Result<&FileHandle> {
        self.state.handles.get(&id).ok_or(Error::MissingHandle(id))
    }
    pub fn close_handle(&mut self, id: u64) -> Result<()> {
        if Arc::make_mut(&mut self.state.handles).remove(&id).is_some() {
            Ok(())
        } else {
            Err(Error::MissingHandle(id))
        }
    }
    pub fn read_handle(&mut self, id: u64, count: usize) -> Result<Vec<u8>> {
        let handle = self.handle(id)?.clone();
        if handle.mode == FileMode::Write {
            return Err(Error::InvalidOperation("handle is write-only".into()));
        }
        let bytes = if let Some(snapshot) = &handle.snapshot {
            snapshot.read_range(handle.position, count)?
        } else if let Some(file) = self
            .state
            .files
            .get(&handle.path)
            .or_else(|| self.virtual_files.get(&handle.path))
        {
            file.as_ref()
                .ok_or_else(|| Error::MissingFile(handle.path.clone()))?
                .read_range(handle.position, count)?
        } else {
            self.read_observed_range(&handle.path, handle.position, count)?
        };
        Arc::make_mut(&mut self.state.handles)
            .get_mut(&id)
            .expect("handle exists")
            .position += bytes.len();
        Ok(bytes)
    }
    pub fn open_snapshot(&mut self, path: &str) -> Result<u64> {
        self.checked_path(path)?;
        let snapshot = if let Some(content) = self
            .state
            .files
            .get(path)
            .or_else(|| self.virtual_files.get(path))
        {
            content
                .as_ref()
                .ok_or_else(|| Error::MissingFile(path.into()))?
                .clone()
        } else {
            Arc::new(PagedFile::from_bytes(&self.read_file(path)?))
        };
        let previous = self.state.handles.clone();
        let previous_id = self.state.next_handle_id;
        let id = self.open_file(path)?;
        let handle = Arc::make_mut(&mut self.state.handles)
            .get_mut(&id)
            .expect("handle exists");
        handle.mode = FileMode::Read;
        handle.snapshot = Some(snapshot);
        if let Err(error) = self.enforce_budget() {
            self.state.handles = previous;
            self.state.next_handle_id = previous_id;
            return Err(error);
        }
        Ok(id)
    }
    pub fn write_handle(&mut self, id: u64, bytes: &[u8]) -> Result<()> {
        self.pending_transaction(|rt| rt.write_handle_inner(id, bytes))
    }
    fn write_handle_inner(&mut self, id: u64, bytes: &[u8]) -> Result<()> {
        let handle = self.handle(id)?.clone();
        if handle.mode == FileMode::Read {
            return Err(Error::InvalidOperation("handle is read-only".into()));
        }
        let previous = self.state.files.clone();
        if !self.state.files.contains_key(&handle.path) {
            let content = self.read_file(&handle.path)?;
            Arc::make_mut(&mut self.state.files).insert(
                handle.path.clone(),
                Some(Arc::new(PagedFile::from_bytes(&content))),
            );
        }
        let file = Arc::make_mut(&mut self.state.files)
            .get_mut(&handle.path)
            .and_then(Option::as_mut)
            .ok_or_else(|| Error::MissingFile(handle.path.clone()))?;
        if let Err(error) = Arc::make_mut(file).write_at(handle.position, bytes) {
            self.state.files = previous;
            return Err(error);
        }
        if let Err(error) = self.enforce_budget() {
            self.state.files = previous;
            return Err(error);
        }
        Arc::make_mut(&mut self.state.handles)
            .get_mut(&id)
            .expect("handle exists")
            .position += bytes.len();
        self.file_operation(&handle.path)?;
        Ok(())
    }

    /// Structured terminal publish failure, retained outside checkpoints.
    pub fn publish_failure(&self) -> Option<&PublishFailure> {
        self.publish_failure_detail.as_ref()
    }
    fn fail_publish(
        &mut self,
        phase: &str,
        path: Option<&Path>,
        applied: &[String],
        error: impl fmt::Display,
    ) -> Error {
        let failure = PublishFailure {
            phase: phase.into(),
            path: path.map(|p| p.to_string_lossy().into_owned()),
            applied: applied.to_vec(),
            cause: error.to_string(),
            retryable: false,
        };
        let detail = serde_json::to_string(&failure).expect("publish failure is serializable");
        self.publish_failure = Some(detail.clone());
        self.publish_failure_detail = Some(failure);
        Error::PublishPartiallyApplied(detail)
    }
    /// Validate the complete write set before touching the host. File replacement
    /// is per-file atomic where the host supports rename; multi-file publication is
    /// not globally atomic, matching the specification's level 1 limitation.
    pub fn publish(
        &mut self,
        force: bool,
        stdout: &mut impl Write,
        stderr: &mut impl Write,
    ) -> Result<()> {
        self.require_internal()?;
        if let Some(detail) = &self.publish_failure {
            return Err(Error::PublishPartiallyApplied(detail.clone()));
        }
        if self.incremental_publish {
            self.enforce_budget()?;
        }
        let next_epoch = (if self.incremental_publish {
            self.published_epoch
        } else {
            self.state.file_epoch
        })
        .checked_add(1)
        .ok_or_else(|| Error::InvalidOperation("file observation epoch exhausted".into()))?;
        if !self.state.stdout.has_prefix(&self.published_stdout)
            || !self.state.stderr.has_prefix(&self.published_stderr)
        {
            return Err(Error::InvalidOperation(
                "cannot publish an output history that predates an earlier publish".into(),
            ));
        }
        self.gui_prepare()?;
        self.gui_windows_prepare()?;
        if self.replaying || self.virtual_publish {
            if !self.virtual_publish {
                if let Err(error) = self
                    .state
                    .stdout
                    .write_since(&self.published_stdout, stdout)
                {
                    return Err(self.fail_publish("stdout", None, &[], error));
                }
                if let Err(error) = self
                    .state
                    .stderr
                    .write_since(&self.published_stderr, stderr)
                {
                    return Err(self.fail_publish("stderr", None, &["stdout".into()], error));
                }
                if let Err(error) = stdout.flush().and_then(|_| stderr.flush()) {
                    return Err(self.fail_publish(
                        "flush",
                        None,
                        &["stdout".into(), "stderr".into()],
                        error,
                    ));
                }
            }
            self.gui_apply()?;
            self.gui_windows_apply(&mut Vec::new())?;
            self.finish_publish(next_epoch);
            return Ok(());
        }
        let changed: BTreeSet<_> = self
            .state
            .files
            .keys()
            .chain(self.state.directories.keys())
            .cloned()
            .collect();
        for path in &changed {
            let full = self.checked_path(path)?;
            if !force {
                let expected = self
                    .observations
                    .get(&(self.state.file_epoch, path.clone()))
                    .map(|o| o.version.clone());
                let actual = if self.state.files.contains_key(path) {
                    Self::host_version(&full)?
                } else {
                    None
                };
                if expected != Some(actual) && self.state.files.contains_key(path) {
                    return Err(Error::ExternalStateConflict(path.clone()));
                }
            }
        }
        if !force {
            for path in self.state.directories.keys() {
                let expected = self
                    .directory_observations
                    .get(&(self.state.file_epoch, path.clone()));
                let actual = Self::host_directory_entries(&self.checked_path(path)?)?;
                if expected != Some(&actual) {
                    return Err(Error::ExternalStateConflict(path.clone()));
                }
            }
            for ((epoch, path), expected) in &self.directory_observations {
                if *epoch != self.state.file_epoch {
                    continue;
                }
                let full = if path == "." {
                    self.root.clone()
                } else {
                    self.checked_path(path)?
                };
                if Self::host_directory_entries(&full)? != *expected {
                    return Err(Error::ExternalStateConflict(path.clone()));
                }
            }
        }
        let resolved = changed
            .iter()
            .map(|path| Ok((path.clone(), self.checked_path(path)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut applied = Vec::new();
        for (path, exists) in self.state.directories.iter() {
            let full = resolved[path].clone();
            if *exists && !full.is_dir() {
                if let Err(error) = fs::create_dir_all(&full) {
                    return Err(self.fail_publish(
                        "directory-create",
                        Some(&full),
                        &applied,
                        error,
                    ));
                }
                applied.push(format!("directory {}", full.display()));
            }
        }
        // Stage all replacement files before applying any changes.
        let mut staged = StagedFiles(Vec::new());
        for (path, content) in self.state.files.iter() {
            if let Some(content) = content {
                let full = resolved[path].clone();
                if let Some(parent) = full.parent() {
                    if !parent.is_dir() {
                        if let Err(error) = fs::create_dir_all(parent) {
                            return Err(self.fail_publish(
                                "parent-create",
                                Some(parent),
                                &applied,
                                error,
                            ));
                        }
                        applied.push(format!("directory {}", parent.display()));
                    }
                }
                let (temp, mut file) = loop {
                    let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
                    let temp =
                        full.with_extension(format!("rewind-{}-{id}.tmp", std::process::id()));
                    match fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&temp)
                    {
                        Ok(file) => break (temp, file),
                        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                        Err(e) => {
                            return Err(self.fail_publish("stage-open", Some(&temp), &applied, e))
                        }
                    }
                };
                staged.0.push((temp, full));
                for page in content.pages.values() {
                    if let Err(error) = page.write_to(&mut file) {
                        return Err(self.fail_publish(
                            "stage-write",
                            Some(staged.0.last().unwrap().0.as_path()),
                            &applied,
                            error,
                        ));
                    }
                }
                if let Err(error) = file.sync_all() {
                    return Err(self.fail_publish(
                        "stage-sync",
                        Some(staged.0.last().unwrap().0.as_path()),
                        &applied,
                        error,
                    ));
                }
            }
        }
        for (temp, full) in &staged.0 {
            if let Err(error) = replace_file(temp, full) {
                return Err(self.fail_publish("rename", Some(full), &applied, error));
            }
            applied.push(format!("file {}", full.display()));
        }
        for (path, content) in self.state.files.iter() {
            if content.is_none() {
                let full = resolved[path].clone();
                match fs::remove_file(&full) {
                    Ok(()) => applied.push(format!("deleted file {}", full.display())),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(self.fail_publish("file-delete", Some(&full), &applied, error))
                    }
                }
            }
        }
        for (path, exists) in self.state.directories.iter().rev() {
            if !exists {
                let full = resolved[path].clone();
                if let Err(error) = fs::remove_dir(&full) {
                    return Err(self.fail_publish(
                        "directory-delete",
                        Some(&full),
                        &applied,
                        error,
                    ));
                }
                applied.push(format!("deleted directory {}", full.display()));
            }
        }
        if let Err(error) = self.gui_apply() {
            return Err(self.fail_publish("gui", None, &applied, error));
        }
        if self.state.gui_pending.is_some() {
            applied.push("gui".into());
        }
        if let Err(error) = self.gui_windows_apply(&mut applied) {
            return Err(self.fail_publish("gui-window", None, &applied, error));
        }
        if let Err(error) = self
            .state
            .stdout
            .write_since(&self.published_stdout, stdout)
        {
            return Err(self.fail_publish("stdout", None, &applied, error));
        }
        applied.push("stdout".into());
        if let Err(error) = self
            .state
            .stderr
            .write_since(&self.published_stderr, stderr)
        {
            return Err(self.fail_publish("stderr", None, &applied, error));
        }
        applied.push("stderr".into());
        if let Err(error) = stdout.flush().and_then(|_| stderr.flush()) {
            return Err(self.fail_publish("flush", None, &applied, error));
        }
        self.finish_publish(next_epoch);
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    const REPLACE_EXISTING: u32 = 0x1;
    const WRITE_THROUGH: u32 = 0x8;
    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe {
        MoveFileExW(
            from_wide.as_ptr(),
            to_wide.as_ptr(),
            REPLACE_EXISTING | WRITE_THROUGH,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
