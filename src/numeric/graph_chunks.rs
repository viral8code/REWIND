//! Cooperative bulk adjacency construction and BFS from immutable edge arrays.
use super::graph::{vector, vertex};
use super::*;
pub const GRAPH_CHUNK: usize = 4096;
#[derive(Clone)]
pub struct GraphBfsWork {
    pub froms: Array,
    pub tos: Array,
    pub heads: Array,
    pub links: Array,
    pub distance: Array,
    pub queue: Array,
    pub source: usize,
    pub phase: u8,
    pub cursor: usize,
    pub read: usize,
    pub write: usize,
    pub current: usize,
    pub edge: usize,
}
impl GraphBfsWork {
    pub fn new(vertices: usize, froms: &Array, tos: &Array, source: usize) -> Result<Self> {
        if vertices > MAX_GRAPH_ITEMS {
            return Err(Error::Size);
        }
        let edges = vector(froms)?;
        if vector(tos)? != edges {
            return Err(Error::Shape);
        }
        if source >= vertices {
            return Err(Error::Index);
        }
        Ok(Self {
            froms: froms.clone(),
            tos: tos.clone(),
            heads: Array::zeros(DType::Int64, vec![vertices])?,
            links: Array::zeros(DType::Int64, vec![edges])?,
            distance: Array::zeros(DType::Int64, vec![vertices])?,
            queue: Array::zeros(DType::Int64, vec![vertices])?,
            source,
            phase: 0,
            cursor: 0,
            read: 0,
            write: 0,
            current: source,
            edge: 0,
        })
    }
    pub fn validate(&self) -> Result<()> {
        let edges = vector(&self.froms)?;
        if vector(&self.tos)? != edges {
            return Err(Error::Shape);
        }
        let vertices = vector(&self.heads)?;
        for (array, length) in [
            (&self.heads, vertices),
            (&self.links, edges),
            (&self.distance, vertices),
            (&self.queue, vertices),
        ] {
            if vector(array)? != length {
                return Err(Error::Shape);
            }
            if !array.contiguous() || !array.writable || array.offset != 0 {
                return Err(Error::ReadOnly);
            }
        }
        if self.source >= vertices || self.current >= vertices {
            return Err(Error::Index);
        }
        if self.read > self.write || self.write > vertices || self.edge > edges {
            return Err(Error::Domain);
        }
        let valid = match self.phase {
            0 | 1 => self.cursor <= edges && self.read == 0 && self.write == 0 && self.edge == 0,
            2 | 3 => self.cursor == 0 && self.write >= 1 && (self.phase != 3 || self.read >= 1),
            4 => self.cursor <= vertices && self.read == self.write && self.edge == 0,
            5 => self.cursor == vertices && self.read == self.write && self.edge == 0,
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Domain)
        }
    }
    pub fn done(&self) -> bool {
        self.phase == 5
    }
    pub fn result(&self) -> Result<Array> {
        self.validate()?;
        if !self.done() {
            return Err(Error::Domain);
        }
        Ok(self.distance.clone())
    }
    pub fn units_bound(&self) -> usize {
        let n = self.heads.len();
        let e = self.froms.len();
        let remaining = match self.phase {
            0 => e
                .saturating_sub(self.cursor)
                .saturating_add(e.saturating_mul(2))
                .saturating_add(n.saturating_mul(3))
                .saturating_add(6),
            1 => e
                .saturating_sub(self.cursor)
                .saturating_add(e)
                .saturating_add(n.saturating_mul(3))
                .saturating_add(5),
            2 | 3 => e.saturating_add(n.saturating_mul(3)).saturating_add(4),
            4 => n.saturating_sub(self.cursor).saturating_add(1),
            _ => 1,
        };
        remaining.min(GRAPH_CHUNK).max(1)
    }
    /// Conservative scatter-page COW admission, including transitions within a step.
    pub fn scratch_estimate(&self) -> usize {
        let units = self.units_bound();
        if self.phase == 5
            || (self.phase == 0 && self.froms.len().saturating_sub(self.cursor) >= units)
        {
            return 8192;
        }
        let sequential = units.div_ceil(256) + 2;
        let copy = |a: &Array, pages: usize| {
            Array::storage_estimate(a.len()).min(a.update_estimate().saturating_mul(pages))
        };
        if self.froms.len() == 0 && self.phase <= 3 {
            // The only queued vertex is the source; no scattered edge writes.
            // Include its initial page and the bounded distance-conversion run.
            return copy(&self.distance, sequential + 1)
                .saturating_add(copy(&self.queue, 1))
                .saturating_add(8192);
        }
        if self.phase == 4 {
            return copy(&self.distance, sequential).saturating_add(8192);
        }
        let mut bytes = 0usize;
        if self.phase <= 1 {
            let writes = if self.phase == 1 {
                self.froms.len().saturating_sub(self.cursor).min(units)
            } else {
                self.froms.len().min(units)
            };
            bytes = bytes
                .saturating_add(copy(&self.heads, writes))
                .saturating_add(copy(&self.links, sequential));
            if self.phase == 1 && self.froms.len().saturating_sub(self.cursor) >= units {
                return bytes.saturating_add(8192);
            }
        }
        bytes
            .saturating_add(copy(&self.distance, units))
            .saturating_add(copy(&self.queue, sequential))
            .saturating_add(8192)
    }
    pub fn step(&self) -> Result<Self> {
        self.validate()?;
        let mut w = self.clone();
        let n = w.heads.len();
        let e = w.froms.len();
        for _ in 0..GRAPH_CHUNK {
            match w.phase {
                0 => {
                    if w.cursor == e {
                        w.phase = 1;
                        w.cursor = 0;
                        continue;
                    }
                    vertex(w.froms.integer_flat(w.cursor)?, n)?;
                    vertex(w.tos.integer_flat(w.cursor)?, n)?;
                    w.cursor += 1;
                }
                1 => {
                    if w.cursor == e {
                        w.distance.set_integer_flat(w.source, 1)?;
                        w.queue.set_integer_flat(0, w.source as i64)?;
                        w.write = 1;
                        w.cursor = 0;
                        w.phase = 2;
                        continue;
                    }
                    let from = vertex(w.froms.integer_flat(w.cursor)?, n)?;
                    let previous = w.heads.integer_flat(from)?;
                    if previous < 0 || previous as usize > w.cursor {
                        return Err(Error::Domain);
                    }
                    w.links.set_integer_flat(w.cursor, previous)?;
                    w.heads.set_integer_flat(from, (w.cursor + 1) as i64)?;
                    w.cursor += 1;
                }
                2 => {
                    if w.read == w.write {
                        w.phase = 4;
                        w.cursor = 0;
                        w.edge = 0;
                        continue;
                    }
                    w.current = vertex(w.queue.integer_flat(w.read)?, n)?;
                    w.read += 1;
                    w.edge = usize::try_from(w.heads.integer_flat(w.current)?)
                        .map_err(|_| Error::Index)?;
                    if w.edge > e {
                        return Err(Error::Index);
                    }
                    w.phase = 3;
                }
                3 => {
                    if w.edge == 0 {
                        w.phase = 2;
                        continue;
                    }
                    let index = w.edge - 1;
                    let to = vertex(w.tos.integer_flat(index)?, n)?;
                    let next =
                        usize::try_from(w.links.integer_flat(index)?).map_err(|_| Error::Domain)?;
                    if next > index {
                        return Err(Error::Domain);
                    }
                    if w.distance.integer_flat(to)? == 0 {
                        let distance = w.distance.integer_flat(w.current)?;
                        if distance < 1 || distance as usize >= n || w.write >= n {
                            return Err(Error::Domain);
                        }
                        w.distance.set_integer_flat(to, distance + 1)?;
                        w.queue.set_integer_flat(w.write, to as i64)?;
                        w.write += 1;
                    }
                    w.edge = next;
                }
                4 => {
                    if w.cursor == n {
                        w.phase = 5;
                        continue;
                    }
                    let encoded = w.distance.integer_flat(w.cursor)?;
                    if encoded < 0 || encoded as usize > n {
                        return Err(Error::Domain);
                    }
                    w.distance.set_integer_flat(w.cursor, encoded - 1)?;
                    w.cursor += 1;
                }
                5 => break,
                _ => return Err(Error::Domain),
            }
        }
        w.validate()?;
        Ok(w)
    }
}
