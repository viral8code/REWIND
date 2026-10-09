//! Typed numeric storage. Values occupy native pages, without per-element VM objects.
//! A view owns a storage version: later writes copy only the touched data page.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex, OnceLock, Weak,
};
mod chunks;
#[cfg(test)]
mod digest_tests;
mod sparse_chunks;
pub use sparse_chunks::{SparseWork, SPARSE_CHUNK};
mod solve_chunks;
pub use solve_chunks::{SolveWork, SOLVE_CHUNK};
mod eigen_chunks;
pub use eigen_chunks::{EigenWork, EIGEN_CHUNK};
mod qr_chunks;
pub use qr_chunks::{QrWork, QR_CHUNK};
mod fft_chunks;
pub use fft_chunks::{FftWork, FFT_CHUNK};
mod least_squares_chunks;
pub use least_squares_chunks::{LeastSquaresWork, LEAST_SQUARES_CHUNK};
mod graph;
mod linalg;
mod model;
mod optimizer;
mod stats;
pub use graph::{graph_adjacency, graph_bfs, MAX_GRAPH_ITEMS};
#[cfg(test)]
mod page_builder_tests;
#[cfg(test)]
mod reduction_tests;
mod tensor;
mod unary;
pub use chunks::{
    MomentsProgress, NormProgress, Progress as KernelProgress, VectorOperation, COOPERATIVE_MACS,
};
pub use linalg::{Eigen, Qr};
pub use model::{decode_model, encode_model, model_size, MAX_MODEL_BYTES, MAX_MODEL_PARAMETERS};
pub use stats::Histogram;
pub use unary::UnaryOperation;
const PAGE: usize = 256;
pub const MAX_ELEMENTS: usize = 16 * 1024 * 1024;
pub const MAX_RANK: usize = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DType {
    Float64,
    Int64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Shape,
    Index,
    Size,
    Type,
    Overflow,
    NonFinite,
    Domain,
    Singular,
    Empty,
    ReadOnly,
    Convergence,
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone)]
enum NodeKind {
    Leaf(Vec<u64>),
    Branch(Arc<Node>, Arc<Node>),
}
struct Node {
    kind: NodeKind,
    hash: OnceLock<[u8; 32]>,
    bytes: usize,
    ledger_cost: usize,
    ledgers: Mutex<Vec<Weak<Ledger>>>,
    first_ledger: AtomicUsize,
}
// An address cache could accept a recycled Ledger allocation after its last Weak
// is pruned. Monotonic IDs keep the fast path independent of allocator reuse.
static NEXT_LEDGER_ID: AtomicUsize = AtomicUsize::new(1);
struct Ledger {
    id: usize,
    bytes: AtomicUsize,
    visits: AtomicUsize,
    generation: AtomicUsize,
    allocated: AtomicUsize,
}
impl Ledger {
    fn charge(&self, bytes: usize) {
        self.bytes.fetch_add(bytes, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.allocated
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_add(bytes))
            })
            .unwrap();
    }
}
impl Default for Ledger {
    fn default() -> Self {
        Self {
            id: NEXT_LEDGER_ID
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .expect("numeric ledger ID exhausted"),
            bytes: AtomicUsize::new(0),
            visits: AtomicUsize::new(0),
            generation: AtomicUsize::new(0),
            allocated: AtomicUsize::new(0),
        }
    }
}
/// Runtime-scoped accounting. Weak registrations never keep a numeric page alive.
/// A Runtime registers values on its VM thread; independent runtimes use distinct IDs.
#[derive(Clone, Default)]
pub(crate) struct Accounting(Arc<Ledger>);
#[derive(Default)]
pub(crate) struct RegistrationMemo {
    first_ledger: AtomicUsize,
    ledgers: Mutex<Vec<Weak<Ledger>>>,
}
impl RegistrationMemo {
    pub(crate) fn mark(&self, accounting: &Accounting) -> bool {
        if self.first_ledger.load(Ordering::Acquire) == accounting.0.id {
            return false;
        }
        let mut ledgers = self.ledgers.lock().unwrap();
        ledgers.retain(|ledger| ledger.strong_count() > 0);
        let candidate = Arc::downgrade(&accounting.0);
        if ledgers
            .iter()
            .any(|ledger| Weak::ptr_eq(ledger, &candidate))
        {
            return false;
        }
        ledgers.push(candidate);
        self.first_ledger.store(
            ledgers.iter().find_map(Weak::upgrade).unwrap().id,
            Ordering::Release,
        );
        true
    }
}
impl Accounting {
    pub fn bytes(&self) -> usize {
        self.0.bytes.load(Ordering::Relaxed)
    }
    pub fn generation(&self) -> usize {
        self.0.generation.load(Ordering::Relaxed)
    }
    pub fn allocated_bytes(&self) -> usize {
        self.0.allocated.load(Ordering::Relaxed)
    }
    pub fn visits(&self) -> usize {
        self.0.visits.load(Ordering::Relaxed)
    }
    pub fn register(&self, array: &Array) {
        array.buffer.root.register(&self.0);
    }
}
impl Clone for Node {
    fn clone(&self) -> Self {
        let kind = self.kind.clone();
        let own_cost = std::mem::size_of::<Self>()
            + 64
            + match &kind {
                NodeKind::Leaf(bits) => bits.capacity() * 8,
                NodeKind::Branch(_, _) => 0,
            };
        // Cloning does not change content. Preserve its digest instead of hashing
        // every old page again; an uncached clone stays uncached.
        let mut n = Self {
            kind,
            hash: self.hash.clone(),
            bytes: self.bytes - self.ledger_cost + own_cost,
            ledger_cost: own_cost,
            ledgers: Mutex::new(Vec::new()),
            first_ledger: AtomicUsize::new(0),
        };
        let registrations = self.ledgers.lock().unwrap();
        for ledger in registrations.iter().filter_map(Weak::upgrade) {
            ledger.charge(n.ledger_cost);
            n.ledgers.get_mut().unwrap().push(Arc::downgrade(&ledger));
        }
        if let Some(first) = n.ledgers.get_mut().unwrap().iter().find_map(Weak::upgrade) {
            n.first_ledger.store(first.id, Ordering::Release);
        }
        n
    }
}
impl Drop for Node {
    fn drop(&mut self) {
        for ledger in self
            .ledgers
            .get_mut()
            .unwrap()
            .iter()
            .filter_map(Weak::upgrade)
        {
            let old = ledger.bytes.fetch_sub(self.ledger_cost, Ordering::Relaxed);
            debug_assert!(old >= self.ledger_cost);
        }
    }
}
impl Node {
    fn register(&self, ledger: &Arc<Ledger>) {
        if self.first_ledger.load(Ordering::Acquire) == ledger.id {
            return;
        }
        ledger.visits.fetch_add(1, Ordering::Relaxed);
        let mut registrations = self.ledgers.lock().unwrap();
        registrations.retain(|l| l.strong_count() > 0);
        if registrations
            .iter()
            .any(|l| Weak::ptr_eq(l, &Arc::downgrade(ledger)))
        {
            return;
        }
        registrations.push(Arc::downgrade(ledger));
        ledger.charge(self.ledger_cost);
        self.first_ledger.store(
            registrations.iter().find_map(Weak::upgrade).unwrap().id,
            Ordering::Release,
        );
        drop(registrations);
        if let NodeKind::Branch(a, b) = &self.kind {
            a.register(ledger);
            b.register(ledger);
        }
    }

    fn new(kind: NodeKind) -> Self {
        let mut n = Self {
            kind,
            hash: OnceLock::new(),
            bytes: 0,
            ledger_cost: 0,
            ledgers: Mutex::new(Vec::new()),
            first_ledger: AtomicUsize::new(0),
        };
        n.refresh();
        n
    }
    fn refresh(&mut self) {
        // Only an exclusively owned node can change. Invalidate its cached
        // digest while updating byte metadata; immutable old versions retain
        // their own cache. Hashing is deferred until content identity is needed.
        self.hash.take();
        self.bytes = std::mem::size_of::<Self>() + 64;
        match &self.kind {
            NodeKind::Leaf(bits) => self.bytes += bits.capacity() * 8,
            NodeKind::Branch(a, b) => self.bytes += a.bytes + b.bytes,
        }
        let cost = std::mem::size_of::<Self>()
            + 64
            + match &self.kind {
                NodeKind::Leaf(bits) => bits.capacity() * 8,
                NodeKind::Branch(_, _) => 0,
            };
        if cost != self.ledger_cost {
            for ledger in self
                .ledgers
                .get_mut()
                .unwrap()
                .iter()
                .filter_map(Weak::upgrade)
            {
                if cost > self.ledger_cost {
                    ledger.charge(cost - self.ledger_cost);
                } else {
                    ledger
                        .bytes
                        .fetch_sub(self.ledger_cost - cost, Ordering::Relaxed);
                }
            }
        }
        self.ledger_cost = cost;
    }
    fn digest(&self) -> [u8; 32] {
        *self.hash.get_or_init(|| {
            let mut hash = Sha256::new();
            match &self.kind {
                NodeKind::Leaf(bits) => {
                    hash.update([0]);
                    hash.update((bits.len() as u64).to_le_bytes());
                    for bit in bits {
                        hash.update(bit.to_le_bytes());
                    }
                }
                NodeKind::Branch(a, b) => {
                    hash.update([1]);
                    hash.update(a.digest());
                    hash.update(b.digest());
                }
            }
            hash.finalize().into()
        })
    }
    fn get(&self, height: usize, index: usize) -> u64 {
        match &self.kind {
            NodeKind::Leaf(v) => v[index],
            NodeKind::Branch(a, b) => {
                let half = PAGE << (height - 1);
                if index < half {
                    a.get(height - 1, index)
                } else {
                    b.get(height - 1, index - half)
                }
            }
        }
    }
    fn write(node: &mut Arc<Self>, height: usize, index: usize, value: u64) {
        let n = Arc::make_mut(node);
        match &mut n.kind {
            NodeKind::Leaf(v) => v[index] = value,
            NodeKind::Branch(a, b) => {
                let half = PAGE << (height - 1);
                if index < half {
                    Self::write(a, height - 1, index, value)
                } else {
                    Self::write(b, height - 1, index - half, value)
                }
            }
        }
        n.refresh();
    }
}
#[derive(Clone)]
struct Buffer {
    len: usize,
    root: Arc<Node>,
    height: usize,
}
impl Buffer {
    fn from_bits(mut bits: impl ExactSizeIterator<Item = u64>) -> Result<Self> {
        let len = bits.len();
        if len > MAX_ELEMENTS {
            return Err(Error::Size);
        }
        let mut pages = Vec::with_capacity(len.div_ceil(PAGE).max(1));
        while bits.len() > 0 {
            pages.push(Arc::new(Node::new(NodeKind::Leaf(
                bits.by_ref().take(PAGE).collect(),
            ))));
        }
        Self::from_pages(len, pages)
    }
    fn from_fallible_bits(mut bits: impl ExactSizeIterator<Item = Result<u64>>) -> Result<Self> {
        let len = bits.len();
        if len > MAX_ELEMENTS {
            return Err(Error::Size);
        }
        let mut pages = Vec::with_capacity(len.div_ceil(PAGE).max(1));
        for start in (0..len).step_by(PAGE) {
            let count = (len - start).min(PAGE);
            let mut page = Vec::with_capacity(count);
            for _ in 0..count {
                page.push(bits.next().ok_or(Error::Shape)??);
            }
            pages.push(Arc::new(Node::new(NodeKind::Leaf(page))));
        }
        if bits.next().is_some() {
            return Err(Error::Shape);
        }
        Self::from_pages(len, pages)
    }
    fn from_pages(len: usize, mut pages: Vec<Arc<Node>>) -> Result<Self> {
        if pages.is_empty() {
            pages.push(Arc::new(Node::new(NodeKind::Leaf(Vec::new()))));
        }
        let mut height = 0;
        while pages.len() > 1 {
            // Empty right branches complete the balanced tree without duplicating payloads.
            let empty = Arc::new(Node::new(NodeKind::Leaf(Vec::new())));
            let mut parents = Vec::with_capacity(pages.len().div_ceil(2));
            for pair in pages.chunks(2) {
                parents.push(Arc::new(Node::new(NodeKind::Branch(
                    pair[0].clone(),
                    pair.get(1).cloned().unwrap_or_else(|| empty.clone()),
                ))));
            }
            pages = parents;
            height += 1;
        }
        Ok(Self {
            len,
            root: pages.pop().unwrap(),
            height,
        })
    }
    // Identical zero subtrees share immutable pages. The tree has the same
    // shape/hash as materialized storage; the first write copies its path.
    fn zeros(len: usize) -> Result<Self> {
        if len <= PAGE {
            return Self::from_bits(std::iter::repeat_n(0, len));
        }
        if len > MAX_ELEMENTS {
            return Err(Error::Size);
        }
        let pages = len.div_ceil(PAGE).max(1);
        let height = pages.next_power_of_two().trailing_zeros() as usize;
        let mut full = vec![Arc::new(Node::new(NodeKind::Leaf(vec![0; PAGE])))];
        for h in 1..=height {
            let child = full[h - 1].clone();
            full.push(Arc::new(Node::new(NodeKind::Branch(child.clone(), child))));
        }
        fn build(len: usize, height: usize, full: &[Arc<Node>]) -> Arc<Node> {
            if len == PAGE << height {
                return full[height].clone();
            }
            if height == 0 {
                return Arc::new(Node::new(NodeKind::Leaf(vec![0; len])));
            }
            let half = PAGE << (height - 1);
            let left = build(len.min(half), height - 1, full);
            let right = if len > half {
                build(len - half, height - 1, full)
            } else {
                Arc::new(Node::new(NodeKind::Leaf(Vec::new())))
            };
            Arc::new(Node::new(NodeKind::Branch(left, right)))
        }
        Ok(Self {
            len,
            root: build(len, height, &full),
            height,
        })
    }
    fn get(&self, index: usize) -> u64 {
        self.root.get(self.height, index)
    }
    fn set(&mut self, index: usize, value: u64) {
        Node::write(&mut self.root, self.height, index, value);
    }
    fn leaf(&self, index: usize) -> &Arc<Node> {
        let mut node = &self.root;
        let mut h = self.height;
        let mut i = index;
        while let NodeKind::Branch(a, b) = &node.kind {
            let half = PAGE << (h - 1);
            node = if i < half {
                a
            } else {
                i -= half;
                b
            };
            h -= 1;
        }
        node
    }
}
// Sequential storage reads descend once per page and never allocate a temporary buffer.
struct BufferRange<'a> {
    buffer: &'a Buffer,
    next: usize,
    end: usize,
    pending: &'a [u64],
}
impl Iterator for BufferRange<'_> {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        if self.next == self.end {
            return None;
        }
        if self.pending.is_empty() {
            let NodeKind::Leaf(bits) = &self.buffer.leaf(self.next).kind else {
                unreachable!()
            };
            let offset = self.next % PAGE;
            let count = (self.end - self.next).min(bits.len() - offset);
            self.pending = &bits[offset..offset + count];
        }
        let value = self.pending[0];
        self.pending = &self.pending[1..];
        self.next += 1;
        Some(value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.end - self.next;
        (n, Some(n))
    }
}
impl ExactSizeIterator for BufferRange<'_> {}
enum ArrayBits<'a> {
    Contiguous(BufferRange<'a>),
    Strided {
        array: &'a Array,
        next: usize,
        end: usize,
    },
}
impl Iterator for ArrayBits<'_> {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        match self {
            Self::Contiguous(range) => range.next(),
            Self::Strided { array, next, end } => {
                if *next == *end {
                    return None;
                }
                let value = array.buffer.get(array.flat_index(*next));
                *next += 1;
                Some(value)
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = match self {
            Self::Contiguous(range) => range.len(),
            Self::Strided { next, end, .. } => end - next,
        };
        (n, Some(n))
    }
}
impl ExactSizeIterator for ArrayBits<'_> {}
impl PartialEq for Buffer {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
            && (Arc::ptr_eq(&self.root, &other.root)
                || (self.root.digest() == other.root.digest()
                    && (0..self.len).all(|i| self.get(i) == other.get(i))))
    }
}
impl Eq for Buffer {}
#[derive(Clone, PartialEq, Eq)]
pub struct Array {
    dtype: DType,
    shape: Vec<usize>,
    strides: Vec<isize>,
    offset: isize,
    writable: bool,
    buffer: Buffer,
}
impl std::fmt::Debug for Array {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Replay digests must observe content changes without formatting every element.
        f.debug_struct("NumericArray")
            .field("dtype", &self.dtype)
            .field("shape", &self.shape)
            .field("strides", &self.strides)
            .field("offset", &self.offset)
            .field("writable", &self.writable)
            .field("storageHash", &self.buffer.root.digest())
            .finish()
    }
}
impl Array {
    fn from_fallible_bits(
        dtype: DType,
        shape: Vec<usize>,
        bits: impl ExactSizeIterator<Item = Result<u64>>,
    ) -> Result<Self> {
        let len = count(&shape)?;
        if bits.len() != len {
            return Err(Error::Shape);
        }
        let strides = strides(&shape)?;
        Ok(Self {
            dtype,
            shape,
            strides,
            offset: 0,
            writable: true,
            buffer: Buffer::from_fallible_bits(bits)?,
        })
    }
    /// Deterministic work bound for content identity, independent of cache
    /// warmth and view size. Views retain the entire backing storage version.
    pub fn storage_digest_work(&self) -> usize {
        self.buffer.len.saturating_mul(16).saturating_add(1024)
    }
    pub fn storage_bytes(&self) -> usize {
        self.buffer.root.bytes
    }
    pub fn retained_bytes(&self) -> usize {
        self.buffer.root.bytes
            + self.shape.capacity() * 8
            + self.strides.capacity() * std::mem::size_of::<isize>()
            + std::mem::size_of::<Self>()
    }
    pub fn storage_estimate(elements: usize) -> usize {
        // Bound canonical materialized storage, including the empty right
        // leaf used at each odd tree level. Native node layout varies by OS;
        // a fixed per-page constant can underprice the digest cache on Windows.
        let mut width = elements.div_ceil(PAGE).max(1);
        let mut nodes = width;
        while width > 1 {
            nodes = nodes
                .saturating_add(width.div_ceil(2))
                .saturating_add(width % 2);
            width = width.div_ceil(2);
        }
        elements
            .saturating_mul(8)
            .saturating_add(nodes.saturating_mul(std::mem::size_of::<Node>() + 64))
            .saturating_add(2048)
    }
    pub fn update_estimate(&self) -> usize {
        PAGE * 8 + (self.buffer.height + 1) * 256 + 2048
    }
    pub fn bits(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        let len = self.len();
        if self.contiguous() {
            // Empty views can have an offset outside storage; no page is accessed.
            let start = if len == 0 { 0 } else { self.offset as usize };
            ArrayBits::Contiguous(BufferRange {
                buffer: &self.buffer,
                next: start,
                end: start + len,
                pending: &[],
            })
        } else {
            ArrayBits::Strided {
                array: self,
                next: 0,
                end: len,
            }
        }
    }
}
// Wire descriptors preserve IEEE bits, shape, negative strides and read-only broadcasting.
// Each variable-length component is bounded during deserialization, before building storage.
struct BoundedVec<T, const N: usize>(Vec<T>);
impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for BoundedVec<T, N> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Visitor<T, const N: usize>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Visitor<T, N> {
            type Value = BoundedVec<T, N>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "at most {N} numeric descriptor entries")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                if a.size_hint().is_some_and(|n| n > N) {
                    return Err(serde::de::Error::custom("numeric descriptor limit"));
                }
                let mut v = Vec::new();
                while let Some(x) = a.next_element()? {
                    if v.len() == N {
                        return Err(serde::de::Error::custom("numeric descriptor limit"));
                    }
                    v.push(x);
                }
                Ok(BoundedVec(v))
            }
        }
        d.deserialize_seq(Visitor::<T, N>(std::marker::PhantomData))
    }
}
impl Serialize for Array {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        struct Bits<'a>(&'a Buffer);
        impl Serialize for Bits<'_> {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                s.collect_seq((0..self.0.len).map(|i| self.0.get(i)))
            }
        }
        let mut t = s.serialize_tuple(6)?;
        t.serialize_element(&self.dtype)?;
        t.serialize_element(&self.shape)?;
        t.serialize_element(&self.strides)?;
        t.serialize_element(&self.offset)?;
        t.serialize_element(&self.writable)?;
        t.serialize_element(&Bits(&self.buffer))?;
        t.end()
    }
}
impl<'de> Deserialize<'de> for Array {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let (dtype, shape, strides, offset, writable, bits): (
            DType,
            BoundedVec<usize, MAX_RANK>,
            BoundedVec<isize, MAX_RANK>,
            isize,
            bool,
            BoundedVec<u64, MAX_ELEMENTS>,
        ) = Deserialize::deserialize(d)?;
        let shape = shape.0;
        let strides = strides.0;
        let invalid = || serde::de::Error::custom("invalid numeric array descriptor");
        let len = count(&shape).map_err(|_| invalid())?;
        if strides.len() != shape.len() || offset < 0 || offset as usize > bits.0.len() {
            return Err(invalid());
        }
        if len > 0 {
            let mut low = offset;
            let mut high = offset;
            for (&dim, &stride) in shape.iter().zip(&strides) {
                let span = ((dim - 1) as isize)
                    .checked_mul(stride)
                    .ok_or_else(invalid)?;
                if span < 0 {
                    low = low.checked_add(span).ok_or_else(invalid)?;
                } else {
                    high = high.checked_add(span).ok_or_else(invalid)?;
                }
            }
            if low < 0 || high as usize >= bits.0.len() {
                return Err(invalid());
            }
            if writable {
                let mut axes = shape
                    .iter()
                    .zip(&strides)
                    .filter(|(&d, _)| d > 1)
                    .map(|(&d, &s)| s.checked_abs().map(|s| (s as usize, d)).ok_or_else(invalid))
                    .collect::<std::result::Result<Vec<_>, D::Error>>()?;
                axes.sort_unstable();
                let mut span = 1usize;
                for (stride, dim) in axes {
                    if stride < span {
                        return Err(invalid());
                    }
                    span = span
                        .checked_add((dim - 1).checked_mul(stride).ok_or_else(invalid)?)
                        .ok_or_else(invalid)?;
                }
            }
        }
        let buffer = Buffer::from_bits(bits.0.into_iter()).map_err(|_| invalid())?;
        Ok(Self {
            dtype,
            shape,
            strides,
            offset,
            writable,
            buffer,
        })
    }
}
fn count(shape: &[usize]) -> Result<usize> {
    if shape.iter().any(|&n| n > MAX_ELEMENTS) {
        return Err(Error::Size);
    }
    if shape.len() > MAX_RANK {
        return Err(Error::Shape);
    }
    shape.iter().try_fold(1usize, |n, &d| {
        n.checked_mul(d)
            .filter(|&n| n <= MAX_ELEMENTS)
            .ok_or(Error::Size)
    })
}
fn strides(shape: &[usize]) -> Result<Vec<isize>> {
    let mut result = vec![0; shape.len()];
    let mut step = 1isize;
    for i in (0..shape.len()).rev() {
        result[i] = step;
        step = step
            .checked_mul(isize::try_from(shape[i]).map_err(|_| Error::Size)?)
            .ok_or(Error::Size)?;
    }
    Ok(result)
}
impl Array {
    pub fn from_bits(
        dtype: DType,
        shape: Vec<usize>,
        bits: impl ExactSizeIterator<Item = u64>,
    ) -> Result<Self> {
        let len = count(&shape)?;
        if bits.len() != len {
            return Err(Error::Shape);
        }
        let strides = strides(&shape)?;
        Ok(Self {
            dtype,
            shape,
            strides,
            offset: 0,
            writable: true,
            buffer: Buffer::from_bits(bits)?,
        })
    }
    pub fn zeros(dtype: DType, shape: Vec<usize>) -> Result<Self> {
        let n = count(&shape)?;
        let strides = strides(&shape)?;
        Ok(Self {
            dtype,
            shape,
            strides,
            offset: 0,
            writable: true,
            buffer: Buffer::zeros(n)?,
        })
    }
    pub fn floats(shape: Vec<usize>, values: &[f64]) -> Result<Self> {
        Self::from_bits(DType::Float64, shape, values.iter().map(|v| v.to_bits()))
    }
    /// Checked arithmetic progression in typed pages, without a VM List.
    pub fn integer_range(start: i64, step: i64, length: usize) -> Result<Self> {
        if length > MAX_ELEMENTS {
            return Err(Error::Size);
        }
        if length > 0 {
            let last = start as i128 + step as i128 * (length - 1) as i128;
            i64::try_from(last).map_err(|_| Error::Overflow)?;
        }
        if start == 0 && step == 0 {
            return Self::zeros(DType::Int64, vec![length]);
        }
        Self::from_bits(
            DType::Int64,
            vec![length],
            (0..length).map(|i| (start as i128 + step as i128 * i as i128) as i64 as u64),
        )
    }
    pub fn integers(shape: Vec<usize>, values: &[i64]) -> Result<Self> {
        Self::from_bits(DType::Int64, shape, values.iter().map(|&v| v as u64))
    }
    pub fn dtype(&self) -> DType {
        self.dtype
    }
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }
    pub fn strides(&self) -> &[isize] {
        &self.strides
    }
    pub fn len(&self) -> usize {
        self.shape.iter().product()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn index(&self, indices: &[usize]) -> Result<usize> {
        if indices.len() != self.shape.len() {
            return Err(Error::Index);
        }
        let mut offset = self.offset;
        for ((&i, &dim), &stride) in indices.iter().zip(&self.shape).zip(&self.strides) {
            if i >= dim {
                return Err(Error::Index);
            }
            offset = offset
                .checked_add((i as isize).checked_mul(stride).ok_or(Error::Index)?)
                .ok_or(Error::Index)?;
        }
        usize::try_from(offset)
            .ok()
            .filter(|&i| i < self.buffer.len)
            .ok_or(Error::Index)
    }
    fn flat_index(&self, mut index: usize) -> usize {
        let mut offset = self.offset;
        for i in (0..self.shape.len()).rev() {
            offset += (index % self.shape[i]) as isize * self.strides[i];
            index /= self.shape[i];
        }
        offset as usize
    }
    /// Address the view in logical row-major order, without a coordinate buffer.
    fn checked_flat_index(&self, index: usize) -> Result<usize> {
        if index >= self.len() {
            return Err(Error::Index);
        }
        // Array construction and wire decoding validate every reachable offset.
        Ok(self.flat_index(index))
    }
    pub fn float_flat(&self, index: usize) -> Result<f64> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        Ok(f64::from_bits(
            self.buffer.get(self.checked_flat_index(index)?),
        ))
    }
    pub fn integer_flat(&self, index: usize) -> Result<i64> {
        if self.dtype != DType::Int64 {
            return Err(Error::Type);
        }
        Ok(self.buffer.get(self.checked_flat_index(index)?) as i64)
    }
    pub fn set_float_flat(&mut self, index: usize, value: f64) -> Result<()> {
        if !self.writable {
            return Err(Error::ReadOnly);
        }
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let index = self.checked_flat_index(index)?;
        self.buffer.set(index, value.to_bits());
        Ok(())
    }
    pub fn set_integer_flat(&mut self, index: usize, value: i64) -> Result<()> {
        if !self.writable {
            return Err(Error::ReadOnly);
        }
        if self.dtype != DType::Int64 {
            return Err(Error::Type);
        }
        let index = self.checked_flat_index(index)?;
        self.buffer.set(index, value as u64);
        Ok(())
    }
    pub fn float(&self, indices: &[usize]) -> Result<f64> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        Ok(f64::from_bits(self.buffer.get(self.index(indices)?)))
    }
    pub fn integer(&self, indices: &[usize]) -> Result<i64> {
        if self.dtype != DType::Int64 {
            return Err(Error::Type);
        }
        Ok(self.buffer.get(self.index(indices)?) as i64)
    }
    pub fn set_float(&mut self, indices: &[usize], value: f64) -> Result<()> {
        if !self.writable {
            return Err(Error::ReadOnly);
        }
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let index = self.index(indices)?;
        self.buffer.set(index, value.to_bits());
        Ok(())
    }
    pub fn set_integer(&mut self, indices: &[usize], value: i64) -> Result<()> {
        if !self.writable {
            return Err(Error::ReadOnly);
        }
        if self.dtype != DType::Int64 {
            return Err(Error::Type);
        }
        let index = self.index(indices)?;
        self.buffer.set(index, value as u64);
        Ok(())
    }
    pub fn contiguous(&self) -> bool {
        strides(&self.shape).is_ok_and(|s| s == self.strides)
    }
    pub fn reshape(&self, shape: Vec<usize>) -> Result<Self> {
        if count(&shape)? != self.len() || !self.contiguous() {
            return Err(Error::Shape);
        }
        Ok(Self {
            strides: strides(&shape)?,
            shape,
            ..self.clone()
        })
    }
    pub fn transpose(&self, axes: &[usize]) -> Result<Self> {
        if axes.len() != self.shape.len() {
            return Err(Error::Shape);
        }
        let mut seen = vec![false; axes.len()];
        for &axis in axes {
            if axis >= axes.len() || seen[axis] {
                return Err(Error::Shape);
            }
            seen[axis] = true;
        }
        Ok(Self {
            shape: axes.iter().map(|&i| self.shape[i]).collect(),
            strides: axes.iter().map(|&i| self.strides[i]).collect(),
            ..self.clone()
        })
    }
    /// Explicit slice length avoids implicit negative-index conventions. Empty views have no readable element.
    pub fn slice(&self, axis: usize, start: usize, length: usize, step: isize) -> Result<Self> {
        if axis >= self.shape.len() || step == 0 {
            return Err(Error::Index);
        }
        let mut result = self.clone();
        if length == 0 {
            if start > self.shape[axis] {
                return Err(Error::Index);
            }
            result.shape[axis] = 0;
            result.offset = 0;
            return Ok(result);
        }
        if length > 0 {
            if start >= self.shape[axis] {
                return Err(Error::Index);
            }
            let last = (length - 1) as isize;
            let end = (start as isize)
                .checked_add(last.checked_mul(step).ok_or(Error::Index)?)
                .ok_or(Error::Index)?;
            if end < 0 || end as usize >= self.shape[axis] {
                return Err(Error::Index);
            }
        }
        result.shape[axis] = length;
        count(&result.shape)?;
        result.offset = result
            .offset
            .checked_add(
                (start as isize)
                    .checked_mul(result.strides[axis])
                    .ok_or(Error::Index)?,
            )
            .ok_or(Error::Index)?;
        result.strides[axis] = result.strides[axis].checked_mul(step).ok_or(Error::Index)?;
        Ok(result)
    }
    pub fn materialize(&self) -> Result<Self> {
        Self::from_bits(self.dtype, self.shape.clone(), self.bits())
    }
    pub fn float_values(&self) -> Result<Vec<f64>> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        Ok(self.bits().map(f64::from_bits).collect())
    }
    pub fn integer_values(&self) -> Result<Vec<i64>> {
        if self.dtype != DType::Int64 {
            return Err(Error::Type);
        }
        Ok(self.bits().map(|bit| bit as i64).collect())
    }
    /// Broadcast is explicit and read-only in the native descriptor. Materialize before writing a repeated view.
    pub fn broadcast(&self, shape: Vec<usize>) -> Result<Self> {
        count(&shape)?;
        if shape.len() < self.shape.len() {
            return Err(Error::Shape);
        }
        let extra = shape.len() - self.shape.len();
        let mut strides = vec![0; shape.len()];
        for i in extra..shape.len() {
            let old = self.shape[i - extra];
            if old == shape[i] {
                strides[i] = self.strides[i - extra];
            } else if old != 1 {
                return Err(Error::Shape);
            }
        }
        Ok(Self {
            shape,
            strides,
            writable: false,
            ..self.clone()
        })
    }
    pub fn map_float(&self, mut f: impl FnMut(f64) -> Result<f64>) -> Result<Self> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        Self::from_fallible_bits(
            DType::Float64,
            self.shape.clone(),
            self.bits().map(|bit| {
                let v = f(f64::from_bits(bit))?;
                if !v.is_finite() {
                    return Err(Error::NonFinite);
                }
                Ok(v.to_bits())
            }),
        )
    }
    pub fn zip_float(
        &self,
        other: &Self,
        mut f: impl FnMut(f64, f64) -> Result<f64>,
    ) -> Result<Self> {
        if self.dtype != DType::Float64 || other.dtype != self.dtype {
            return Err(Error::Type);
        }
        if self.shape != other.shape {
            return Err(Error::Shape);
        }
        Self::from_fallible_bits(
            DType::Float64,
            self.shape.clone(),
            self.bits().zip(other.bits()).map(|(a, b)| {
                let a = f64::from_bits(a);
                let b = f64::from_bits(b);
                let v = f(a, b)?;
                if !v.is_finite() {
                    return Err(Error::NonFinite);
                }
                Ok(v.to_bits())
            }),
        )
    }
    pub fn zip_integer(
        &self,
        other: &Self,
        mut f: impl FnMut(i64, i64) -> Option<i64>,
    ) -> Result<Self> {
        if self.dtype != DType::Int64 || other.dtype != self.dtype {
            return Err(Error::Type);
        }
        if self.shape != other.shape {
            return Err(Error::Shape);
        }
        Self::from_fallible_bits(
            DType::Int64,
            self.shape.clone(),
            self.bits().zip(other.bits()).map(|(a, b)| {
                f(a as i64, b as i64)
                    .map(|value| value as u64)
                    .ok_or(Error::Overflow)
            }),
        )
    }
    pub fn sum(&self) -> Result<f64> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let mut sum = 0.0f64;
        let mut correction = 0.0;
        for bit in self.bits() {
            let v = f64::from_bits(bit);
            if !v.is_finite() {
                return Err(Error::NonFinite);
            }
            let next = sum + v;
            correction += if sum.abs() >= v.abs() {
                (sum - next) + v
            } else {
                (v - next) + sum
            };
            sum = next;
        }
        let result = sum + correction;
        if !result.is_finite() {
            return Err(Error::Overflow);
        }
        Ok(result)
    }
    pub fn mean_variance(&self, ddof: usize) -> Result<(f64, f64)> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        if self.len() <= ddof {
            return Err(Error::Empty);
        }
        let mut mean = 0.0;
        let mut m2 = 0.0;
        for (i, bit) in self.bits().enumerate() {
            let v = f64::from_bits(bit);
            if !v.is_finite() {
                return Err(Error::NonFinite);
            }
            let delta = v - mean;
            mean += delta / (i + 1) as f64;
            m2 += delta * (v - mean);
        }
        let variance = m2 / (self.len() - ddof) as f64;
        if !mean.is_finite() || !variance.is_finite() {
            return Err(Error::Overflow);
        }
        Ok((mean, variance))
    }
    pub fn dot(&self, other: &Self) -> Result<f64> {
        if self.dtype != DType::Float64 || other.dtype != self.dtype {
            return Err(Error::Type);
        }
        if self.shape.len() != 1 || other.shape != self.shape {
            return Err(Error::Shape);
        }
        let mut sum = 0.0f64;
        let mut correction = 0.0;
        for (a, b) in self.bits().zip(other.bits()) {
            let product = f64::from_bits(a) * f64::from_bits(b);
            if !product.is_finite() {
                return Err(Error::NonFinite);
            }
            let next = sum + product;
            correction += if sum.abs() >= product.abs() {
                (sum - next) + product
            } else {
                (product - next) + sum
            };
            sum = next;
        }
        let result = sum + correction;
        if !result.is_finite() {
            return Err(Error::Overflow);
        }
        Ok(result)
    }
    pub fn matmul(&self, other: &Self) -> Result<Self> {
        if self.dtype != DType::Float64 || other.dtype != self.dtype {
            return Err(Error::Type);
        }
        let ([m, k], [other_k, n]) = (self.shape.as_slice(), other.shape.as_slice()) else {
            return Err(Error::Shape);
        };
        if k != other_k {
            return Err(Error::Shape);
        }
        count(&[*m, *n])?;
        if *m == 0 || *n == 0 || *k == 0 {
            return Self::zeros(DType::Float64, vec![*m, *n]);
        }
        let mut output = vec![0.0; m * n];
        for i in 0..*m {
            for j in 0..*n {
                let mut sum = 0.0;
                let mut correction = 0.0;
                for p in 0..*k {
                    // Rank, shape and all descriptor bounds were validated before the kernel.
                    // Avoid repeating fallible multidimensional indexing for each multiply.
                    let ai = (self.offset
                        + i as isize * self.strides[0]
                        + p as isize * self.strides[1]) as usize;
                    let bi = (other.offset
                        + p as isize * other.strides[0]
                        + j as isize * other.strides[1]) as usize;
                    let value =
                        f64::from_bits(self.buffer.get(ai)) * f64::from_bits(other.buffer.get(bi));
                    let next = sum + value;
                    correction += if sum.abs() >= value.abs() {
                        (sum - next) + value
                    } else {
                        (value - next) + sum
                    };
                    sum = next;
                }
                let value = sum + correction;
                if !value.is_finite() {
                    return Err(Error::NonFinite);
                }
                output[i * n + j] = value;
            }
        }
        Self::floats(vec![*m, *n], &output)
    }
    /// Partial-pivot LU solve. No inverse is formed. The caller reserves O(n^3) work and O(n^2) temporary bytes.
    pub fn solve(&self, right: &Self, tolerance: f64) -> Result<Self> {
        if self.dtype != DType::Float64 || right.dtype != self.dtype {
            return Err(Error::Type);
        }
        let [n, m] = self.shape.as_slice() else {
            return Err(Error::Shape);
        };
        if n != m || right.shape != [*n] || !tolerance.is_finite() || tolerance < 0.0 {
            return Err(Error::Shape);
        }
        if *n == 0 {
            return Self::floats(vec![0], &[]);
        }
        let mut a = self.float_values()?;
        let mut b = right.float_values()?;
        if a.iter().chain(&b).any(|v| !v.is_finite()) {
            return Err(Error::NonFinite);
        }
        let scale = a.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        for col in 0..*n {
            let mut pivot = col;
            for row in col + 1..*n {
                if a[row * n + col].abs() > a[pivot * n + col].abs() {
                    pivot = row;
                }
            }
            if a[pivot * n + col].abs() <= tolerance * scale {
                return Err(Error::Singular);
            }
            if pivot != col {
                for j in 0..*n {
                    a.swap(col * n + j, pivot * n + j);
                }
                b.swap(col, pivot);
            }
            for row in col + 1..*n {
                let factor = a[row * n + col] / a[col * n + col];
                a[row * n + col] = 0.0;
                for j in col + 1..*n {
                    a[row * n + j] -= factor * a[col * n + j];
                }
                b[row] -= factor * b[col];
            }
        }
        let mut x = vec![0.0; *n];
        for i in (0..*n).rev() {
            let mut value = b[i];
            for j in i + 1..*n {
                value -= a[i * n + j] * x[j];
            }
            x[i] = value / a[i * n + i];
            if !x[i].is_finite() {
                return Err(Error::NonFinite);
            }
        }
        Self::floats(vec![*n], &x)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-10 * (1.0 + b.abs()), "{a} != {b}");
    }
    #[test]
    fn snapshot_and_view_keep_old_storage_and_a_write_copies_one_page() {
        let mut a = Array::zeros(DType::Int64, vec![PAGE * 3]).unwrap();
        let snapshot = a.clone();
        let view = a.slice(0, PAGE, 4, 1).unwrap();
        a.set_integer(&[PAGE], 99).unwrap();
        assert_eq!(view.integer(&[0]).unwrap(), 0);
        assert_eq!(snapshot.integer(&[PAGE]).unwrap(), 0);
        assert_eq!(a.integer(&[PAGE]).unwrap(), 99);
        assert!(Arc::ptr_eq(a.buffer.leaf(0), snapshot.buffer.leaf(0)));
        assert!(!Arc::ptr_eq(
            a.buffer.leaf(PAGE),
            snapshot.buffer.leaf(PAGE)
        ));
        assert!(Arc::ptr_eq(
            a.buffer.leaf(PAGE * 2),
            snapshot.buffer.leaf(PAGE * 2)
        ));
    }
    #[test]
    fn strides_transpose_reshape_and_negative_slices_are_checked() {
        let a = Array::integers(vec![2, 3], &[1, 2, 3, 4, 5, 6]).unwrap();
        let b = a.transpose(&[1, 0]).unwrap();
        assert_eq!(b.integer_values().unwrap(), vec![1, 4, 2, 5, 3, 6]);
        assert!(b.reshape(vec![6]).is_err());
        let c = a.slice(1, 2, 3, -1).unwrap();
        assert_eq!(c.integer_values().unwrap(), vec![3, 2, 1, 6, 5, 4]);
        assert!(c.integer(&[2, 0]).is_err());
        assert!(a.slice(0, 1, 3, 1).is_err());
        assert_eq!(
            c.materialize().unwrap().reshape(vec![6]).unwrap().shape(),
            &[6]
        );
    }
    #[test]
    fn explicit_broadcast_and_checked_integer_arithmetic() {
        let mut a = Array::integers(vec![1, 2], &[3, 4])
            .unwrap()
            .broadcast(vec![3, 2])
            .unwrap();
        assert_eq!(a.set_integer(&[0, 0], 5), Err(Error::ReadOnly));
        assert_eq!(a.integer_values().unwrap(), vec![3, 4, 3, 4, 3, 4]);
        assert!(a.broadcast(vec![3, 3]).is_err());
        let x = Array::integers(vec![1], &[i64::MAX]).unwrap();
        let y = Array::integers(vec![1], &[1]).unwrap();
        assert_eq!(x.zip_integer(&y, i64::checked_add), Err(Error::Overflow));
    }
    #[test]
    fn cancellation_resistant_sum_and_online_variance() {
        let a = Array::floats(vec![3], &[1e16, 1.0, -1e16]).unwrap();
        assert_eq!(a.sum().unwrap(), 1.0);
        let b = Array::floats(vec![4], &[1.0, 2.0, 3.0, 4.0]).unwrap();
        let (mean, variance) = b.mean_variance(1).unwrap();
        close(mean, 2.5);
        close(variance, 5.0 / 3.0);
    }
    #[test]
    fn matrix_product_pivoted_solve_and_residual() {
        let a = Array::floats(vec![2, 2], &[0.0, 2.0, 1.0, 3.0]).unwrap();
        let b = Array::floats(vec![2], &[4.0, 7.0]).unwrap();
        let x = a.solve(&b, 1e-14).unwrap();
        close(x.float(&[0]).unwrap(), 1.0);
        close(x.float(&[1]).unwrap(), 2.0);
        let residual = a.matmul(&x.reshape(vec![2, 1]).unwrap()).unwrap();
        for i in 0..2 {
            close(residual.float(&[i, 0]).unwrap(), b.float(&[i]).unwrap());
        }
        let singular = Array::floats(vec![2, 2], &[1.0, 2.0, 2.0, 4.0]).unwrap();
        assert_eq!(singular.solve(&b, 1e-14), Err(Error::Singular));
    }
    #[test]
    fn matrix_kernel_handles_transposed_reversed_and_broadcast_inputs() {
        let a = Array::floats(vec![2, 3], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let reverse = a.slice(1, 2, 3, -1).unwrap();
        let product = reverse
            .matmul(&reverse.transpose(&[1, 0]).unwrap())
            .unwrap();
        assert_eq!(
            product.float_values().unwrap(),
            vec![14.0, 32.0, 32.0, 77.0]
        );
        let b = Array::floats(vec![1, 2], &[1.0, 2.0])
            .unwrap()
            .broadcast(vec![3, 2])
            .unwrap();
        assert_eq!(
            a.matmul(&b).unwrap().float_values().unwrap(),
            vec![6.0, 12.0, 15.0, 30.0]
        );
        assert_eq!(
            Array::zeros(DType::Float64, vec![2, 0])
                .unwrap()
                .matmul(&Array::zeros(DType::Float64, vec![0, 3]).unwrap())
                .unwrap()
                .float_values()
                .unwrap(),
            vec![0.0; 6]
        );
    }
    #[test]
    fn bounded_wire_roundtrip_preserves_views_and_rejects_forged_bounds() {
        let a = Array::integers(vec![2, 3], &[1, 2, 3, 4, 5, 6])
            .unwrap()
            .slice(1, 2, 3, -1)
            .unwrap()
            .transpose(&[1, 0])
            .unwrap();
        let wire = serde_json::to_value(&a).unwrap();
        let restored: Array = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(a, restored);
        assert_eq!(a.integer_values(), restored.integer_values());
        let mut invalid = wire.clone();
        invalid[3] = serde_json::json!(-1);
        assert!(serde_json::from_value::<Array>(invalid).is_err());
        let mut invalid = wire.clone();
        invalid[2] = serde_json::json!([100, 1]);
        assert!(serde_json::from_value::<Array>(invalid).is_err());
        let mut invalid = wire;
        invalid[1] = serde_json::json!([1, 1, 1, 1, 1, 1, 1, 1, 1]);
        assert!(serde_json::from_value::<Array>(invalid).is_err());
        let broadcast = Array::integers(vec![1], &[1])
            .unwrap()
            .broadcast(vec![5])
            .unwrap();
        let mut wire = serde_json::to_value(&broadcast).unwrap();
        wire[4] = serde_json::json!(true);
        assert!(serde_json::from_value::<Array>(wire).is_err());
        let wire = serde_json::to_string(&broadcast).unwrap();
        assert_eq!(serde_json::from_str::<Array>(&wire).unwrap(), broadcast);
    }
    #[test]
    fn cached_digest_changes_on_write_and_debug_size_is_bounded() {
        let mut a = Array::zeros(DType::Int64, vec![65536]).unwrap();
        let previous = format!("{a:?}");
        assert!(previous.len() < 512);
        a.set_integer(&[12345], 99).unwrap();
        assert_ne!(previous, format!("{a:?}"));
        assert_eq!(
            format!("{a:?}"),
            format!(
                "{:?}",
                serde_json::from_str::<Array>(&serde_json::to_string(&a).unwrap()).unwrap()
            )
        );
    }
    #[test]
    fn copied_paths_are_logarithmic_and_untouched_pages_keep_identity() {
        let mut a = Array::zeros(DType::Int64, vec![PAGE * 1024]).unwrap();
        let saved = a.clone();
        let old_root = a.buffer.root.clone();
        a.set_integer(&[PAGE * 501], 17).unwrap();
        assert!(!Arc::ptr_eq(&old_root, &a.buffer.root));
        for page in [0, 500, 502, 1023] {
            assert!(Arc::ptr_eq(
                a.buffer.leaf(PAGE * page),
                saved.buffer.leaf(PAGE * page)
            ));
        }
        assert!(a.update_estimate() < 8192);
        assert_eq!(saved.integer(&[PAGE * 501]).unwrap(), 0);
        assert_eq!(a.slice(0, 0, 0, -1).unwrap().len(), 0);
        assert_eq!(a.slice(0, usize::MAX, 0, 1), Err(Error::Index));
    }
    #[test]
    fn native_pages_release_after_the_last_snapshot_reference() {
        let array = Array::zeros(DType::Int64, vec![PAGE * 8]).unwrap();
        let weak = Arc::downgrade(&array.buffer.root);
        let snapshot = array.clone();
        let view = array.slice(0, PAGE, 2, 1).unwrap();
        drop(array);
        drop(snapshot);
        assert!(weak.upgrade().is_some());
        drop(view);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn limits_empty_arrays_and_ieee_bits() {
        assert!(Array::zeros(DType::Float64, vec![MAX_ELEMENTS, 2]).is_err());
        assert!(Array::zeros(DType::Float64, vec![1; MAX_RANK + 1]).is_err());
        let empty = Array::zeros(DType::Float64, vec![0, 3]).unwrap();
        assert!(empty.float(&[0, 0]).is_err());
        assert_eq!(empty.sum().unwrap(), 0.0);
        let a = Array::floats(vec![2], &[-0.0, f64::from_bits(0x7ff8000000000001)]).unwrap();
        assert_eq!(a.float(&[0]).unwrap().to_bits(), (-0.0f64).to_bits());
        assert_eq!(a.float(&[1]).unwrap().to_bits(), 0x7ff8000000000001);
        assert_eq!(a.sum(), Err(Error::NonFinite));
    }
    #[test]
    fn page_iterator_preserves_offsets_views_and_exact_remaining_length() {
        let values = (0..PAGE * 5 + 17).map(|i| i as f64).collect::<Vec<_>>();
        let original = Array::floats(vec![values.len()], &values).unwrap();
        let slice = original.slice(0, PAGE - 3, PAGE * 3 + 9, 1).unwrap();
        let mut iter = slice.bits();
        for expected in &values[PAGE - 3..PAGE * 4 + 6] {
            assert_eq!(iter.size_hint(), (iter.len(), Some(iter.len())));
            assert_eq!(iter.next(), Some(expected.to_bits()));
        }
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        let matrix = original
            .slice(0, 0, PAGE * 4, 1)
            .unwrap()
            .reshape(vec![32, 32])
            .unwrap();
        let transpose = matrix.transpose(&[1, 0]).unwrap();
        let reversed = original
            .slice(0, values.len() - 1, values.len(), -1)
            .unwrap();
        let broadcast = original
            .slice(0, 3, 1, 1)
            .unwrap()
            .broadcast(vec![PAGE * 3])
            .unwrap();
        for view in [&slice, &transpose, &reversed, &broadcast] {
            let expected = (0..view.len())
                .map(|i| view.buffer.get(view.flat_index(i)))
                .collect::<Vec<_>>();
            assert_eq!(view.bits().collect::<Vec<_>>(), expected);
            assert_eq!(
                view.materialize().unwrap().bits().collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                view.map_float(|v| Ok(v + 1.0))
                    .unwrap()
                    .float_values()
                    .unwrap(),
                expected
                    .iter()
                    .map(|b| f64::from_bits(*b) + 1.0)
                    .collect::<Vec<_>>()
            );
        }
        let empty = original.slice(0, values.len(), 0, 1).unwrap();
        assert_eq!(empty.bits().len(), 0);
        assert_eq!(empty.bits().next(), None);
    }
}

#[cfg(test)]
mod accounting_tests {
    use super::*;
    #[test]
    fn accounts_shared_pages_and_cow_without_holding_storage() {
        let ledger = Accounting::default();
        let a = Array::from_bits(
            DType::Float64,
            vec![1_000_000],
            std::iter::repeat_n(0, 1_000_000),
        )
        .unwrap();
        ledger.register(&a);
        let initial = ledger.bytes();
        assert!(initial >= 8_000_000);
        let visits = ledger.visits();
        for _ in 0..1000 {
            ledger.register(&a);
        }
        assert_eq!(ledger.visits() - visits, 0);
        let mut b = a.clone();
        ledger.register(&b);
        assert_eq!(ledger.bytes(), initial);
        b.set_float(&[1], 2.0).unwrap();
        ledger.register(&b);
        assert!(ledger.bytes() > initial);
        assert!(ledger.bytes() - initial < 16384);
        assert_eq!(f64::from_bits(a.buffer.get(1)), 0.0);
        drop(a);
        let retained = ledger.bytes();
        assert!(retained >= 8_000_000);
        assert!(retained < initial + 16384);
        drop(b);
        assert_eq!(ledger.bytes(), 0);
    }
    #[test]
    fn independent_ledgers_and_unique_mutation() {
        let one = Accounting::default();
        let two = Accounting::default();
        let mut a = Array::zeros(DType::Float64, vec![300]).unwrap();
        one.register(&a);
        two.register(&a);
        assert_eq!(one.bytes(), two.bytes());
        let before = one.bytes();
        a.set_float(&[270], 7.0).unwrap();
        assert_eq!(before, one.bytes());
        let mut b = a.clone();
        b.set_float(&[2], 3.0).unwrap();
        assert_eq!(one.bytes(), two.bytes());
        drop(one);
        b.set_float(&[299], 4.0).unwrap();
        two.register(&b);
        drop(a);
        drop(b);
        assert_eq!(two.bytes(), 0);
    }
}
