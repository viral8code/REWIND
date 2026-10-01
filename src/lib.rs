//! REWIND's transactional state and I/O core.
//!
//! Checkpoints share immutable roots. A mutation clones only the map it changes;
//! file contents are reference counted. The external input and time observations
//! live outside checkpoints, while their cursors live inside them.

pub mod journal;
pub mod map_storage;
pub mod storage;
use map_storage::PersistentMap;
use storage::{HeapStore, PagedValues};
mod replay;
use journal::{Journal, Segment};

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MapKey {
    Bool(bool),
    Int(i64),
    Float(u64),
    Text(String),
    Bytes(Vec<u8>),
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
            }
        }
        rank(self)
            .cmp(&rank(other))
            .then_with(|| match (self, other) {
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
            Value::Bool(v) => Self::Bool(*v),
            Value::Int(v) => Self::Int(*v),
            Value::Float(v) => Self::Float(*v),
            Value::Text(v) => Self::Text(v.clone()),
            Value::Bytes(v) => Self::Bytes(v.as_ref().clone()),
            _ => return None,
        })
    }
    pub fn value(&self) -> Value {
        match self {
            Self::Bool(v) => Value::Bool(*v),
            Self::Int(v) => Value::Int(*v),
            Self::Float(v) => Value::Float(*v),
            Self::Text(v) => Value::Text(v.clone()),
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
    Text(String),
    Bytes(Arc<Vec<u8>>),
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
    pub causes: Vec<DiagnosticRecord>,
    pub wait_edges: Vec<WaitEdge>,
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

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
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
    state: State,
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

pub struct Runtime {
    root: PathBuf,
    state: State,
    checkpoints: BTreeMap<String, Checkpoint>,
    current_parent: Option<String>,
    observations: BTreeMap<(u64, String), Observation>,
    directory_observations: BTreeMap<(u64, String), Option<BTreeSet<String>>>,
    input: Vec<String>,
    byte_input: Vec<(usize, Vec<u8>)>,
    byte_eof: bool,
    times: Vec<u128>,
    arguments: Vec<String>,
    env_allowed: BTreeSet<String>,
    env_secrets: BTreeSet<String>,
    sensitive_values: BTreeSet<String>,
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
}

impl Runtime {
    pub fn register_secret_value(&mut self, value: &Value) {
        fn visit(
            rt: &Runtime,
            v: &Value,
            out: &mut BTreeSet<String>,
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
                            visit(rt, v, out, seen, depth + 1);
                        }
                    }
                }
                Value::Struct(_, fields) | Value::Closure(_, _, fields) => {
                    for v in fields.values() {
                        visit(rt, v, out, seen, depth + 1);
                    }
                }
                Value::List(items) => {
                    for v in items.iter() {
                        visit(rt, v, out, seen, depth + 1);
                    }
                }
                Value::TypedList(_, items) => {
                    for v in items.iter() {
                        visit(rt, v, out, seen, depth + 1);
                    }
                }
                Value::Map(items) | Value::TypedMap(_, _, items) => {
                    for (k, v) in items {
                        visit(rt, &k.value(), out, seen, depth + 1);
                        visit(rt, v, out, seen, depth + 1);
                    }
                }
                Value::OrderedMap(_, _, items) => {
                    for (k, v) in items {
                        visit(rt, k, out, seen, depth + 1);
                        visit(rt, v, out, seen, depth + 1);
                    }
                }
                Value::Enum(_, _, items) => {
                    for (_, v) in items {
                        visit(rt, v, out, seen, depth + 1);
                    }
                }
                Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                    visit(rt, v, out, seen, depth + 1)
                }
                Value::Null | Value::Option(None) | Value::Handle(_) | Value::Function(_, _) => {}
                _ => {
                    out.insert(v.to_string());
                    if let Value::Bytes(bytes) = v {
                        if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                            out.insert(text);
                        }
                    }
                }
            }
        }
        let mut values = BTreeSet::new();
        visit(self, value, &mut values, &mut BTreeSet::new(), 0);
        self.sensitive_values
            .extend(values.into_iter().filter(|s| !s.is_empty()));
    }
    pub fn mask_debug_json(&self, value: &serde_json::Value) -> serde_json::Value {
        use serde_json::Value as Json;
        match value {
            Json::String(s) => Json::String(self.masked_value(&Value::Text(s.clone()))),
            Json::Array(a) => Json::Array(a.iter().map(|v| self.mask_debug_json(v)).collect()),
            Json::Object(o) => Json::Object(
                o.iter()
                    .map(|(k, v)| {
                        (
                            self.masked_value(&Value::Text(k.clone())),
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
            self.register_secret_value(&Value::Text(text.clone()));
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
            self.register_secret_value(&Value::Text(text.clone()));
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
    pub fn value_bytes(value: &Value) -> usize {
        match value {
            Value::Text(text) => text.len(),
            Value::Bytes(bytes) => bytes.len(),
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
            byte_input: Vec::new(),
            byte_eof: false,
            times: Vec::new(),
            arguments: Vec::new(),
            env_allowed: BTreeSet::new(),
            env_secrets: BTreeSet::new(),
            sensitive_values: BTreeSet::new(),
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
        })
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
        let mut compute_memory = self
            .byte_input
            .iter()
            .map(|(_, bytes)| bytes.len() + 16)
            .sum::<usize>();
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
        for state in std::iter::once(&self.state).chain(self.checkpoints.values().map(|c| &c.state))
        {
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
                        .map(|(k, v)| k.len() + Self::value_bytes(v))
                        .sum(),
                );
            }
            if seen_compute.insert(Arc::as_ptr(&state.heap) as usize) {
                compute_memory = compute_memory.saturating_add(state.heap.logical_bytes());
            }
            if seen_compute.insert(Arc::as_ptr(&state.stack) as usize) {
                compute_memory =
                    compute_memory.saturating_add(state.stack.iter().map(Self::value_bytes).sum());
            }
            if seen_compute.insert(Arc::as_ptr(&state.call_frames) as usize) {
                compute_memory = compute_memory.saturating_add(
                    state
                        .call_frames
                        .iter()
                        .flat_map(|f| f.locals.iter())
                        .map(|(k, v)| k.len() + Self::value_bytes(v))
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
    pub fn alloc(&mut self, value: Value) -> Result<u64> {
        let id = self.state.next_heap_id;
        let previous = self.state.heap.clone();
        self.state.next_heap_id += 1;
        Arc::make_mut(&mut self.state.heap).insert(id, value);
        if let Err(error) = self.enforce_budget() {
            self.state.heap = previous;
            self.state.next_heap_id = id;
            return Err(error);
        }
        Ok(id)
    }
    pub fn heap_set(&mut self, id: u64, value: Value) -> Result<()> {
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

    pub fn commit(&mut self, name: impl Into<String>) -> Result<()> {
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
        self.current_parent = Some(name);
        Ok(())
    }
    pub fn revert(&mut self, name: &str) -> Result<()> {
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
        BranchAnchor {
            state: self.state.clone(),
            parent: self.current_parent.clone(),
        }
    }
    pub fn end_branch(&mut self, name: impl Into<String>, anchor: BranchAnchor) -> Result<()> {
        self.commit(name)?;
        self.state = anchor.state;
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
        let previous = self.state.stdout.clone();
        let operation = if self.incremental_publish {
            Some(self.operation()?)
        } else {
            None
        };
        self.state.stdout.append_operation(bytes, operation);
        if let Err(error) = self.enforce_budget() {
            self.state.stdout = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn print_err(&mut self, text: &str) -> Result<()> {
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
            if self.replaying {
                return if self.byte_eof {
                    Ok(None)
                } else {
                    Err(Error::InvalidOperation("ReplayMismatch: byte input".into()))
                };
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
            self.byte_input.push((limit, buffer));
            // The observation stays recorded even on failure: the Host was consumed.
            self.enforce_budget()?;
        }
        let (recorded_limit, bytes) = &self.byte_input[cursor];
        if *recorded_limit != limit {
            return Err(Error::InvalidOperation(
                "ReplayMismatch: readChunk size differs at restored input cursor".into(),
            ));
        }
        self.state.byte_cursor += 1;
        Ok(Some(bytes.clone()))
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
        if path.contains('\\')
            || path.contains(':')
            || relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(Error::InvalidPath(path.into()));
        }
        let full = self.root.join(relative);
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
        self.read_file(path)?;
        Arc::make_mut(&mut self.state.files).insert(path.into(), None);
        self.file_operation(path)?;
        Ok(())
    }
    pub fn copy_file(&mut self, from: &str, to: &str) -> Result<()> {
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
        self.copy_file(from, to)?;
        self.delete_file(from)
    }
    pub fn create_directory(&mut self, path: &str) -> Result<()> {
        self.observe_directory(path)?;
        Arc::make_mut(&mut self.state.directories).insert(path.into(), true);
        self.directory_operation(path)?;
        Ok(())
    }
    pub fn delete_directory(&mut self, path: &str) -> Result<()> {
        let host_entries = self.observe_directory(path)?;
        if host_entries.is_none() && self.state.directories.get(path) != Some(&true) {
            return Err(Error::InvalidPath(path.into()));
        }
        let prefix = format!("{path}/");
        let virtual_file = self
            .state
            .files
            .iter()
            .any(|(p, content)| p.starts_with(&prefix) && content.is_some());
        let virtual_dir = self
            .state
            .directories
            .iter()
            .any(|(p, exists)| p.starts_with(&prefix) && *exists);
        let host_remaining = host_entries.unwrap_or_default().into_iter().any(|name| {
            let child = format!("{path}/{name}");
            !matches!(self.state.files.get(&child), Some(None))
                && self.state.directories.get(&child) != Some(&false)
        });
        if virtual_file || virtual_dir || host_remaining {
            return Err(Error::InvalidOperation(format!(
                "directory is not empty: {path}"
            )));
        }
        Arc::make_mut(&mut self.state.directories).insert(path.into(), false);
        self.directory_operation(path)?;
        Ok(())
    }
    pub fn move_directory(&mut self, from: &str, to: &str) -> Result<()> {
        let previous = self.state.clone();
        if let Err(error) = self.move_directory_inner(from, to) {
            self.state = previous;
            return Err(error);
        }
        Ok(())
    }
    fn move_directory_inner(&mut self, from: &str, to: &str) -> Result<()> {
        if to == from || to.starts_with(&format!("{from}/")) {
            return Err(Error::InvalidOperation(
                "cannot move a directory into itself".into(),
            ));
        }
        self.checked_path(from)?;
        self.checked_path(to)?;
        if self.state.directories.get(to) == Some(&true) || self.observe_directory(to)?.is_some() {
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
        let entries = self.observe_directory(path)?;
        if entries.is_none() && self.state.directories.get(path) != Some(&true) {
            return Err(Error::InvalidPath(path.into()));
        }
        dirs.insert(path.into());
        for name in entries.unwrap_or_default() {
            let child = format!("{path}/{name}");
            let full = self.checked_path(&child)?;
            if fs::symlink_metadata(&full)?.file_type().is_symlink() {
                return Err(Error::InvalidPath(child));
            }
            if full.is_dir() {
                self.collect_directory(&child, dirs, files)?;
            } else if full.is_file() {
                files.insert(child);
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
            snapshot
                .to_vec()?
                .into_iter()
                .skip(handle.position)
                .take(count)
                .collect()
        } else if self.state.files.contains_key(&handle.path)
            || self.virtual_files.contains_key(&handle.path)
        {
            self.read_file(&handle.path)?
                .into_iter()
                .skip(handle.position)
                .take(count)
                .collect()
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
        let content = self.read_file(path)?;
        let previous = self.state.handles.clone();
        let previous_id = self.state.next_handle_id;
        let id = self.open_file(path)?;
        let handle = Arc::make_mut(&mut self.state.handles)
            .get_mut(&id)
            .expect("handle exists");
        handle.mode = FileMode::Read;
        handle.snapshot = Some(Arc::new(PagedFile::from_bytes(&content)));
        if let Err(error) = self.enforce_budget() {
            self.state.handles = previous;
            self.state.next_handle_id = previous_id;
            return Err(error);
        }
        Ok(id)
    }
    pub fn write_handle(&mut self, id: u64, bytes: &[u8]) -> Result<()> {
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

    /// Validate the complete write set before touching the host. File replacement
    /// is per-file atomic where the host supports rename; multi-file publication is
    /// not globally atomic, matching the specification's level 1 limitation.
    pub fn publish(
        &mut self,
        force: bool,
        stdout: &mut impl Write,
        stderr: &mut impl Write,
    ) -> Result<()> {
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
        if self.replaying || self.virtual_publish {
            if !self.virtual_publish {
                self.state
                    .stdout
                    .write_since(&self.published_stdout, stdout)?;
                self.state
                    .stderr
                    .write_since(&self.published_stderr, stderr)?;
            }
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
        for (path, exists) in self.state.directories.iter() {
            let full = self.checked_path(path)?;
            if *exists {
                fs::create_dir_all(full)?;
            }
        }
        // Stage all replacement files before applying any changes.
        let mut staged = StagedFiles(Vec::new());
        for (path, content) in self.state.files.iter() {
            if let Some(content) = content {
                let full = self.checked_path(path)?;
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent)?;
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
                        Err(e) => return Err(e.into()),
                    }
                };
                staged.0.push((temp, full));
                for page in content.pages.values() {
                    page.write_to(&mut file)?;
                }
                file.sync_all()?;
            }
        }
        let mut applied = Vec::new();
        for (temp, full) in &staged.0 {
            if let Err(error) = replace_file(temp, full) {
                let detail = format!(
                    "applied {applied:?}; file replacement {} failed: {error}",
                    full.display()
                );
                self.publish_failure = Some(detail.clone());
                return Err(Error::PublishPartiallyApplied(detail));
            }
            applied.push(format!("file {}", full.display()));
        }
        for (path, content) in self.state.files.iter() {
            if content.is_none() {
                let full = self.checked_path(path)?;
                if full.exists() {
                    if let Err(error) = fs::remove_file(&full) {
                        let detail = format!(
                            "applied {applied:?}; file deletion {} failed: {error}",
                            full.display()
                        );
                        self.publish_failure = Some(detail.clone());
                        return Err(Error::PublishPartiallyApplied(detail));
                    }
                    applied.push(format!("deleted file {}", full.display()));
                }
            }
        }
        for (path, exists) in self.state.directories.iter().rev() {
            if !exists {
                let full = self.checked_path(path)?;
                if let Err(error) = fs::remove_dir(&full) {
                    let detail = format!(
                        "applied {applied:?}; directory deletion {} failed: {error}",
                        full.display()
                    );
                    self.publish_failure = Some(detail.clone());
                    return Err(Error::PublishPartiallyApplied(detail));
                }
                applied.push(format!("deleted directory {}", full.display()));
            }
        }
        if let Err(error) = self
            .state
            .stdout
            .write_since(&self.published_stdout, stdout)
        {
            let detail = format!("applied {applied:?}; stdout failed: {error}");
            self.publish_failure = Some(detail.clone());
            return Err(Error::PublishPartiallyApplied(detail));
        }
        if let Err(error) = self
            .state
            .stderr
            .write_since(&self.published_stderr, stderr)
        {
            let detail = format!("applied {applied:?}; stderr failed after stdout: {error}");
            self.publish_failure = Some(detail.clone());
            return Err(Error::PublishPartiallyApplied(detail));
        }
        if let Err(error) = stdout.flush().and_then(|_| stderr.flush()) {
            let detail = format!("applied {applied:?}; terminal flush failed: {error}");
            self.publish_failure = Some(detail.clone());
            return Err(Error::PublishPartiallyApplied(detail));
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
