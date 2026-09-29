use crate::{Error, Result};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_SPILL: AtomicU64 = AtomicU64::new(0);

enum Storage {
    Memory(Vec<u8>),
    Spill(PathBuf, usize),
}

pub(crate) struct Segment {
    pub(crate) id: u64,
    storage: Mutex<Storage>,
}
impl Segment {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            id: NEXT_SPILL.fetch_add(1, Ordering::Relaxed),
            storage: Mutex::new(Storage::Memory(bytes)),
        }
    }
    pub(crate) fn usage(&self) -> (usize, usize) {
        match &*self.storage.lock().expect("journal lock") {
            Storage::Memory(bytes) => (bytes.len(), 0),
            Storage::Spill(_, len) => (0, *len),
        }
    }
    pub(crate) fn spill(&self) -> Result<()> {
        let mut storage = self.storage.lock().expect("journal lock");
        let Storage::Memory(bytes) = &*storage else {
            return Ok(());
        };
        let path = std::env::temp_dir().join(format!(
            "rewind-journal-{}-{}.tmp",
            std::process::id(),
            self.id
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&path);
            return Err(e.into());
        }
        *storage = Storage::Spill(path, bytes.len());
        Ok(())
    }
    fn write_to(&self, writer: &mut impl Write) -> io::Result<()> {
        match &*self.storage.lock().expect("journal lock") {
            Storage::Memory(bytes) => writer.write_all(bytes),
            Storage::Spill(path, _) => {
                let mut file = fs::File::open(path)?;
                io::copy(&mut file, writer)?;
                Ok(())
            }
        }
    }
}
impl Drop for Segment {
    fn drop(&mut self) {
        if let Ok(Storage::Spill(path, _)) = self.storage.get_mut() {
            let _ = fs::remove_file(path);
        }
    }
}

struct Node {
    prev: Option<Arc<Node>>,
    segment: Arc<Segment>,
}

#[derive(Clone, Default)]
pub struct Journal {
    tail: Option<Arc<Node>>,
    len: usize,
}
impl std::fmt::Debug for Journal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Journal").field("len", &self.len).finish()
    }
}
impl Journal {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn append(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let segment = Arc::new(Segment::new(bytes.to_vec()));
        self.tail = Some(Arc::new(Node {
            prev: self.tail.take(),
            segment,
        }));
        self.len += bytes.len();
    }
    pub(crate) fn segments(&self) -> Vec<Arc<Segment>> {
        let mut segments = Vec::new();
        let mut node = self.tail.as_ref();
        while let Some(current) = node {
            segments.push(current.segment.clone());
            node = current.prev.as_ref();
        }
        segments.reverse();
        segments
    }
    pub fn has_prefix(&self, prefix: &Journal) -> bool {
        if prefix.tail.is_none() {
            return true;
        }
        let mut node = self.tail.as_ref();
        while let Some(current) = node {
            if prefix
                .tail
                .as_ref()
                .is_some_and(|p| Arc::ptr_eq(current, p))
            {
                return true;
            }
            node = current.prev.as_ref();
        }
        false
    }
    pub fn write_since(&self, prefix: &Journal, writer: &mut impl Write) -> Result<()> {
        if !self.has_prefix(prefix) {
            return Err(Error::InvalidOperation(
                "cannot publish an output history that predates an earlier publish".into(),
            ));
        }
        let mut segments = Vec::new();
        let mut node = self.tail.as_ref();
        while let Some(current) = node {
            if prefix
                .tail
                .as_ref()
                .is_some_and(|p| Arc::ptr_eq(current, p))
            {
                break;
            }
            segments.push(current.segment.clone());
            node = current.prev.as_ref();
        }
        for segment in segments.iter().rev() {
            segment.write_to(writer)?;
        }
        Ok(())
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(self.len);
        self.write_since(&Journal::default(), &mut bytes)?;
        Ok(bytes)
    }
    pub fn storage_bytes(&self) -> usize {
        self.segments()
            .iter()
            .map(|segment| segment.usage().1)
            .sum()
    }
}
