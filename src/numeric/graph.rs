//! Bounded adjacency construction and BFS over typed integer pages.
use super::{Array, DType, Error, Result};
pub const MAX_GRAPH_ITEMS: usize = 1_048_576;

pub(super) fn vector(a: &Array) -> Result<usize> {
    if a.dtype() != DType::Int64 {
        return Err(Error::Type);
    }
    if a.shape().len() != 1 {
        return Err(Error::Shape);
    }
    if a.len() > MAX_GRAPH_ITEMS {
        return Err(Error::Size);
    }
    Ok(a.len())
}
pub(super) fn vertex(value: i64, vertices: usize) -> Result<usize> {
    usize::try_from(value)
        .ok()
        .filter(|&v| v < vertices)
        .ok_or(Error::Index)
}
/// One-based edge links keep untouched heads virtually zero. Input pages are borrowed.
pub fn graph_adjacency(vertices: usize, froms: &Array, tos: &Array) -> Result<(Array, Array)> {
    if vertices > MAX_GRAPH_ITEMS {
        return Err(Error::Size);
    }
    let edges = vector(froms)?;
    if vector(tos)? != edges {
        return Err(Error::Shape);
    }
    // Validate the complete input before allocating or writing output storage.
    for (from, to) in froms.bits().zip(tos.bits()) {
        vertex(from as i64, vertices)?;
        vertex(to as i64, vertices)?;
    }
    let mut heads = Array::zeros(DType::Int64, vec![vertices])?;
    let mut links = Array::zeros(DType::Int64, vec![edges])?;
    for (edge, from) in froms.bits().enumerate() {
        let from = from as usize;
        let previous = heads.integer_flat(from)?;
        links.set_integer_flat(edge, previous)?;
        heads.set_integer_flat(from, (edge + 1) as i64)?;
    }
    Ok((heads, links))
}
/// Validate the partition before traversal; raw language primitives can supply
/// malformed arrays even though the standard library's Graph fields are private.
pub fn graph_bfs(
    heads: &Array,
    tos: &Array,
    links: &Array,
    edges: usize,
    source: usize,
) -> Result<Array> {
    let vertices = vector(heads)?;
    let capacity = vector(tos)?;
    if vector(links)? != capacity {
        return Err(Error::Shape);
    }
    if edges > capacity {
        return Err(Error::Index);
    }
    if source >= vertices {
        return Err(Error::Index);
    }
    let mut seen = vec![false; edges];
    let mut visited = 0usize;
    for v in 0..vertices {
        let mut edge = heads.integer_flat(v)?;
        while edge != 0 {
            let index = usize::try_from(edge)
                .ok()
                .and_then(|n| n.checked_sub(1))
                .filter(|&i| i < edges)
                .ok_or(Error::Index)?;
            if seen[index] {
                return Err(Error::Domain);
            }
            seen[index] = true;
            visited += 1;
            vertex(tos.integer_flat(index)?, vertices)?;
            let next = links.integer_flat(index)?;
            if next < 0 || next as usize > index {
                return Err(Error::Domain);
            }
            edge = next;
        }
    }
    if visited != edges {
        return Err(Error::Domain);
    }
    let mut distance = vec![-1_i64; vertices];
    let mut queue = Vec::with_capacity(vertices);
    distance[source] = 0;
    queue.push(source);
    let mut cursor = 0;
    while cursor < queue.len() {
        let from = queue[cursor];
        cursor += 1;
        let mut edge = heads.integer_flat(from)?;
        while edge != 0 {
            let index = edge as usize - 1;
            let to = tos.integer_flat(index)? as usize;
            if distance[to] == -1 {
                distance[to] = distance[from] + 1;
                queue.push(to);
            }
            edge = links.integer_flat(index)?;
        }
    }
    Array::integers(vec![vertices], &distance)
}
