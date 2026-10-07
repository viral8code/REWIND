//! Column-pivoted Householder QR with persistent, bounded private scratch.
use super::*;
use std::collections::HashSet;
pub const QR_CHUNK: usize = 4096;

// Writes keep arithmetic order, but refresh each touched page/path once when
// the private step finishes. Pointer keys are identities only: never dereferenced.
// No edited array is cloned or exposed between the first edit and finish.
fn edit(node: &mut Arc<Node>, height: usize, index: usize, value: u64, dirty: &mut HashSet<usize>) {
    let node = Arc::make_mut(node);
    dirty.insert(node as *const Node as usize);
    match &mut node.kind {
        NodeKind::Leaf(bits) => bits[index] = value,
        NodeKind::Branch(left, right) => {
            let half = PAGE << (height - 1);
            if index < half {
                edit(left, height - 1, index, value, dirty);
            } else {
                edit(right, height - 1, index - half, value, dirty);
            }
        }
    }
}
fn finish_edits(node: &mut Arc<Node>, dirty: &mut HashSet<usize>) {
    if !dirty.remove(&(Arc::as_ptr(node) as usize)) {
        return;
    }
    let node = Arc::get_mut(node).expect("QR private edited page unexpectedly shared");
    if let NodeKind::Branch(left, right) = &mut node.kind {
        finish_edits(left, dirty);
        finish_edits(right, dirty);
    }
    node.refresh();
}

#[derive(Clone)]
pub struct QrWork {
    pub input: Array,
    pub matrix: Array,
    pub reflectors: Array,
    pub q: Array,
    pub r: Array,
    pub permutation: Array,
    pub tolerance: f64,
    pub scale: f64,
    pub original_norm: f64,
    pub pivot_norm: f64,
    pub sign: f64,
    pub v_norm: f64,
    pub sum: f64,
    pub correction: f64,
    pub phase: u8,
    pub cursor: usize,
    pub k: usize,
    pub col: usize,
    pub pivot: usize,
    pub rank: usize,
}
impl QrWork {
    pub fn new(input: &Array, tolerance: f64) -> Result<Self> {
        if input.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let [m, n] = input.shape.as_slice() else {
            return Err(Error::Shape);
        };
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(Error::Domain);
        }
        let p = (*m).min(*n);
        Ok(Self {
            input: input.clone(),
            matrix: Array::zeros(DType::Float64, vec![*m, *n])?,
            reflectors: Array::zeros(DType::Float64, vec![p, *m])?,
            q: Array::zeros(DType::Float64, vec![*m, p])?,
            r: Array::zeros(DType::Float64, vec![p, *n])?,
            permutation: Array::zeros(DType::Int64, vec![*n])?,
            tolerance,
            scale: 0.0,
            original_norm: 0.0,
            pivot_norm: 0.0,
            sign: 1.0,
            v_norm: 0.0,
            sum: 0.0,
            correction: 0.0,
            phase: 0,
            cursor: 0,
            k: 0,
            col: 0,
            pivot: 0,
            rank: 0,
        })
    }
    pub fn validate(&self) -> Result<()> {
        if self.input.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let [m, n] = self.input.shape.as_slice() else {
            return Err(Error::Shape);
        };
        let p = (*m).min(*n);
        for (a, shape, dtype) in [
            (&self.matrix, vec![*m, *n], DType::Float64),
            (&self.reflectors, vec![p, *m], DType::Float64),
            (&self.q, vec![*m, p], DType::Float64),
            (&self.r, vec![p, *n], DType::Float64),
            (&self.permutation, vec![*n], DType::Int64),
        ] {
            if a.dtype != dtype {
                return Err(Error::Type);
            }
            if a.shape != shape {
                return Err(Error::Shape);
            }
            if !a.contiguous() || !a.writable || a.offset != 0 {
                return Err(Error::ReadOnly);
            }
        }
        if !self.tolerance.is_finite()
            || self.tolerance < 0.0
            || [self.scale, self.original_norm, self.pivot_norm, self.v_norm]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
            || !self.sum.is_finite()
            || !self.correction.is_finite()
            || !matches!(self.sign, 1.0 | -1.0)
        {
            return Err(Error::Domain);
        }
        if self.k > p || self.col > *n || self.pivot > *n || self.rank > p {
            return Err(Error::Domain);
        }
        let valid = match self.phase {
            0 | 1 => self.cursor <= m * n && self.k == 0,
            2 => self.col <= *n && self.cursor <= *m && self.k == 0,
            3 => {
                self.k < p
                    && self.col >= self.k
                    && self.col <= *n
                    && self.cursor >= self.k
                    && self.cursor <= *m
                    && self.pivot >= self.k
                    && self.pivot < *n
            }
            4 => self.k < p && self.cursor <= *m && self.pivot < *n,
            5 | 6 | 9 => self.k < p && self.cursor >= self.k && self.cursor <= *m,
            7 | 8 => {
                self.k < p
                    && self.col >= self.k
                    && self.col <= *n
                    && self.cursor >= self.k
                    && self.cursor <= *m
            }
            10 => self.k == p && self.cursor <= p,
            11 | 12 => self.k < p && self.col <= p && self.cursor >= self.k && self.cursor <= *m,
            13 => self.cursor <= p * n,
            14 => self.cursor <= *n,
            15 => true,
            _ => false,
        };
        if !valid {
            return Err(Error::Domain);
        }
        Ok(())
    }
    pub fn done(&self) -> bool {
        self.phase == 15
    }
    pub fn result(&self) -> Result<(Array, Array, Array, usize)> {
        self.validate()?;
        if !self.done() {
            return Err(Error::Domain);
        }
        Ok((
            self.q.clone(),
            self.r.clone(),
            self.permutation.clone(),
            self.rank,
        ))
    }
    fn get(a: &Array, i: usize) -> f64 {
        f64::from_bits(a.buffer.get(i))
    }
    fn write(a: &mut Array, i: usize, v: f64, dirty: &mut HashSet<usize>) {
        edit(&mut a.buffer.root, a.buffer.height, i, v.to_bits(), dirty);
    }
    fn bits(a: &mut Array, i: usize, v: &[u64]) {
        let height = a.buffer.height;
        Node::write_range(&mut a.buffer.root, height, i, v);
    }
    fn finite(v: f64) -> Result<f64> {
        if v.is_finite() {
            Ok(v)
        } else {
            Err(Error::Overflow)
        }
    }
    fn add(&mut self, v: f64) -> Result<()> {
        if !v.is_finite() {
            return Err(Error::NonFinite);
        }
        let next = self.sum + v;
        self.correction += if self.sum.abs() >= v.abs() {
            (self.sum - next) + v
        } else {
            (v - next) + self.sum
        };
        self.sum = next;
        Ok(())
    }
    fn next_column(&mut self, phase: u8) {
        self.col += 1;
        self.cursor = self.k;
        self.sum = 0.0;
        self.correction = 0.0;
        self.phase = phase;
    }
    fn next_pivot(&mut self, p: usize) {
        self.k += 1;
        self.sum = 0.0;
        self.correction = 0.0;
        if self.k == p {
            self.phase = 10;
            self.cursor = 0;
        } else {
            self.phase = 3;
            self.col = self.k;
            self.pivot = self.k;
            self.pivot_norm = 0.0;
            self.cursor = self.k;
        }
    }
    pub fn step(&self) -> Result<Self> {
        self.validate()?;
        let mut w = self.clone();
        let m = self.input.shape[0];
        let n = self.input.shape[1];
        let p = m.min(n);
        let mut used = 0;
        let mut dirty = HashSet::new();
        while used < QR_CHUNK && !w.done() {
            used += 1;
            match w.phase {
                0 => {
                    if w.cursor == m * n {
                        w.phase = 14;
                        w.cursor = 0;
                        continue;
                    }
                    let end = (w.cursor + QR_CHUNK - used + 1).min(m * n);
                    let mut bits = Vec::with_capacity(end - w.cursor);
                    for b in w.input.window_bits(w.cursor, end)? {
                        let v = f64::from_bits(b);
                        if !v.is_finite() {
                            return Err(Error::NonFinite);
                        }
                        w.scale = w.scale.max(v.abs());
                        bits.push(b);
                    }
                    Self::bits(&mut w.matrix, w.cursor, &bits);
                    used += bits.len() - 1;
                    w.cursor = end;
                }
                1 => {
                    if w.cursor == m * n || w.scale == 0.0 {
                        w.phase = 2;
                        w.cursor = 0;
                        w.col = 0;
                        continue;
                    }
                    let end = (w.cursor + QR_CHUNK - used + 1).min(m * n);
                    let bits = w
                        .matrix
                        .window_bits(w.cursor, end)?
                        .map(|b| (f64::from_bits(b) / w.scale).to_bits())
                        .collect::<Vec<_>>();
                    Self::bits(&mut w.matrix, w.cursor, &bits);
                    used += bits.len() - 1;
                    w.cursor = end;
                }
                2 => {
                    if w.col == n {
                        if p == 0 {
                            w.phase = 15;
                            continue;
                        }
                        w.phase = 3;
                        w.col = 0;
                        w.cursor = 0;
                        w.sum = 0.0;
                        continue;
                    }
                    if w.cursor == m {
                        w.original_norm = w.original_norm.max(w.sum);
                        w.col += 1;
                        w.cursor = 0;
                        w.sum = 0.0;
                        continue;
                    }
                    w.sum = w.sum.hypot(Self::get(&w.matrix, w.cursor * n + w.col));
                    w.cursor += 1;
                }
                3 => {
                    if w.col == n {
                        if w.pivot_norm > w.tolerance * w.original_norm {
                            w.rank += 1;
                        }
                        w.phase = 4;
                        w.cursor = 0;
                        continue;
                    }
                    if w.cursor == m {
                        if w.col == w.k || w.sum > w.pivot_norm {
                            w.pivot_norm = w.sum;
                            w.pivot = w.col;
                        }
                        w.col += 1;
                        w.cursor = w.k;
                        w.sum = 0.0;
                        continue;
                    }
                    w.sum = w.sum.hypot(Self::get(&w.matrix, w.cursor * n + w.col));
                    w.cursor += 1;
                }
                4 => {
                    if w.cursor == m || w.pivot == w.k {
                        let a = w.permutation.buffer.get(w.k);
                        let b = w.permutation.buffer.get(w.pivot);
                        edit(
                            &mut w.permutation.buffer.root,
                            w.permutation.buffer.height,
                            w.k,
                            b,
                            &mut dirty,
                        );
                        edit(
                            &mut w.permutation.buffer.root,
                            w.permutation.buffer.height,
                            w.pivot,
                            a,
                            &mut dirty,
                        );
                        w.phase = 5;
                        w.cursor = w.k;
                        w.v_norm = 0.0;
                        w.sign = if Self::get(&w.matrix, w.k * n + w.k) < 0.0 {
                            -1.0
                        } else {
                            1.0
                        };
                        continue;
                    }
                    let a = Self::get(&w.matrix, w.cursor * n + w.k);
                    let b = Self::get(&w.matrix, w.cursor * n + w.pivot);
                    Self::write(&mut w.matrix, w.cursor * n + w.k, b, &mut dirty);
                    Self::write(&mut w.matrix, w.cursor * n + w.pivot, a, &mut dirty);
                    w.cursor += 1;
                }
                5 => {
                    if w.pivot_norm == 0.0 {
                        w.next_pivot(p);
                        continue;
                    }
                    if w.cursor == m {
                        w.phase = 6;
                        w.cursor = w.k;
                        continue;
                    }
                    let mut v = Self::get(&w.matrix, w.cursor * n + w.k) / w.pivot_norm;
                    if w.cursor == w.k {
                        v += w.sign;
                    }
                    w.v_norm = w.v_norm.hypot(v);
                    Self::write(&mut w.reflectors, w.k * m + w.cursor, v, &mut dirty);
                    w.cursor += 1;
                }
                6 => {
                    if w.cursor == m {
                        w.phase = 7;
                        w.col = w.k;
                        w.cursor = w.k;
                        w.sum = 0.0;
                        w.correction = 0.0;
                        continue;
                    }
                    let v = Self::get(&w.reflectors, w.k * m + w.cursor) / w.v_norm;
                    Self::write(&mut w.reflectors, w.k * m + w.cursor, v, &mut dirty);
                    w.cursor += 1;
                }
                7 | 11 => {
                    let a_phase = w.phase == 7;
                    let columns = if a_phase { n } else { p };
                    if w.col == columns {
                        if a_phase {
                            w.phase = 9;
                            w.cursor = w.k;
                        } else if w.k == 0 {
                            w.phase = 13;
                            w.cursor = 0;
                        } else {
                            w.k -= 1;
                            w.col = 0;
                            w.cursor = w.k;
                            w.sum = 0.0;
                            w.correction = 0.0;
                        }
                        continue;
                    }
                    if !a_phase && Self::get(&w.reflectors, w.k * m + w.k) == 0.0 {
                        w.col = columns;
                        continue;
                    }
                    if w.cursor == m {
                        w.sum = Self::finite(w.sum + w.correction)?;
                        w.phase = if a_phase { 8 } else { 12 };
                        w.cursor = w.k;
                        continue;
                    }
                    let v = Self::get(&w.reflectors, w.k * m + w.cursor);
                    let a = if a_phase { &w.matrix } else { &w.q };
                    let value = v * Self::get(a, w.cursor * columns + w.col);
                    w.add(value)?;
                    w.cursor += 1;
                }
                8 | 12 => {
                    let a_phase = w.phase == 8;
                    let columns = if a_phase { n } else { p };
                    if w.cursor == m {
                        w.next_column(if a_phase { 7 } else { 11 });
                        continue;
                    }
                    let v = Self::get(&w.reflectors, w.k * m + w.cursor);
                    let a = if a_phase { &mut w.matrix } else { &mut w.q };
                    let i = w.cursor * columns + w.col;
                    let value = Self::finite(Self::get(a, i) - (2.0 * v) * w.sum)?;
                    Self::write(a, i, value, &mut dirty);
                    w.cursor += 1;
                }
                9 => {
                    if w.cursor == m {
                        w.next_pivot(p);
                        continue;
                    }
                    let v = if w.cursor == w.k {
                        -w.sign * w.pivot_norm
                    } else {
                        0.0
                    };
                    Self::write(&mut w.matrix, w.cursor * n + w.k, v, &mut dirty);
                    w.cursor += 1;
                }
                10 => {
                    if w.cursor == p {
                        w.k = p - 1;
                        w.phase = 11;
                        w.col = 0;
                        w.cursor = w.k;
                        w.sum = 0.0;
                        w.correction = 0.0;
                        continue;
                    }
                    Self::write(&mut w.q, w.cursor * p + w.cursor, 1.0, &mut dirty);
                    w.cursor += 1;
                }
                13 => {
                    if w.cursor == p * n {
                        w.phase = 15;
                        continue;
                    }
                    let i = w.cursor;
                    let row = i / n;
                    let col = i % n;
                    if col >= row {
                        let v = Self::finite(Self::get(&w.matrix, i) * w.scale)?;
                        Self::write(&mut w.r, i, v, &mut dirty);
                    }
                    w.cursor += 1;
                }
                14 => {
                    if w.cursor == n {
                        w.phase = 1;
                        w.cursor = 0;
                        continue;
                    }
                    edit(
                        &mut w.permutation.buffer.root,
                        w.permutation.buffer.height,
                        w.cursor,
                        w.cursor as u64,
                        &mut dirty,
                    );
                    w.cursor += 1;
                }
                _ => return Err(Error::Domain),
            }
        }
        for array in [
            &mut w.matrix,
            &mut w.reflectors,
            &mut w.q,
            &mut w.r,
            &mut w.permutation,
        ] {
            finish_edits(&mut array.buffer.root, &mut dirty);
        }
        debug_assert!(dirty.is_empty());
        Ok(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn finish(mut w: QrWork) -> QrWork {
        let mut count = 0;
        while !w.done() {
            w = w.step().unwrap();
            count += 1;
            assert!(count < 100_000);
        }
        w
    }
    fn compare(input: &Array, tolerance: f64) {
        let expected = input.qr(tolerance).unwrap();
        let work = finish(QrWork::new(input, tolerance).unwrap());
        let (q, r, permutation, rank) = work.result().unwrap();
        assert_eq!(
            q, expected.q,
            "completed Q digest differs from fresh synchronous storage"
        );
        assert_eq!(
            r, expected.r,
            "completed R digest differs from fresh synchronous storage"
        );
        assert_eq!(
            q.float_values()
                .unwrap()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            expected
                .q
                .float_values()
                .unwrap()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            r.float_values()
                .unwrap()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            expected
                .r
                .float_values()
                .unwrap()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            permutation.bits().map(|v| v as i64).collect::<Vec<_>>(),
            expected.permutation
        );
        assert_eq!(rank, expected.rank);
    }
    #[test]
    fn matches_synchronous_bits_for_pivots_views_rank_and_scale() {
        for (shape, data) in [
            (vec![3, 2], vec![1.0, 10.0, 2.0, 1.0, 3.0, 5.0]),
            (vec![2, 3], vec![1.0, 10.0, 2.0, 1.0, 3.0, 5.0]),
            (vec![4, 2], vec![1.0, 2.0, 2.0, 4.0, 3.0, 6.0, 4.0, 8.0]),
            (vec![2, 2], vec![-0.0, 0.0, 0.0, -0.0]),
        ] {
            let a = Array::floats(shape, &data).unwrap();
            compare(&a, 1e-12);
            compare(&a.transpose(&[1, 0]).unwrap(), 1e-12);
        }
        for scale in [1e-150, 1e150] {
            compare(
                &Array::floats(vec![2, 2], &[2.0 * scale, scale, scale, 2.0 * scale]).unwrap(),
                1e-12,
            );
        }
        for shape in [vec![0, 0], vec![0, 7], vec![7, 0]] {
            compare(&Array::zeros(DType::Float64, shape).unwrap(), 1e-12);
        }
    }
    #[test]
    fn multi_step_snapshot_is_immutable_and_releases_private_versions() {
        let m = 129;
        let n = 7;
        let data = (0..m * n)
            .map(|i| ((i * 17 % 101) as f64) - 50.0)
            .collect::<Vec<_>>();
        let input = Array::floats(vec![m, n], &data).unwrap();
        let first = QrWork::new(&input, 1e-12).unwrap().step().unwrap();
        assert!(!first.done());
        let frozen = first.matrix.float_values().unwrap();
        let branch = finish(first.clone());
        assert_eq!(first.matrix.float_values().unwrap(), frozen);
        let again = finish(first.clone());
        assert_eq!(branch.q, again.q);
        assert_eq!(branch.r, again.r);
        let restored = first.step().unwrap();
        assert!(!restored.done());
        compare(&input, 1e-12);
    }
    #[test]
    fn bounded_copy_does_not_materialize_large_zero_input_or_hide_late_failure() {
        let input = Array::zeros(DType::Float64, vec![4096, 4096]).unwrap();
        let first = QrWork::new(&input, 0.0).unwrap().step().unwrap();
        assert_eq!(first.cursor, QR_CHUNK);
        let accounting = Accounting::default();
        for array in [
            &first.input,
            &first.matrix,
            &first.reflectors,
            &first.q,
            &first.r,
            &first.permutation,
        ] {
            accounting.register(array);
        }
        assert!(accounting.bytes() < 1024 * 1024);
        drop(first);
        drop(input);
        assert_eq!(accounting.bytes(), 0);
        let mut bad = Array::floats(vec![1, QR_CHUNK + 1], &vec![0.0; QR_CHUNK + 1]).unwrap();
        bad.set_float(&[0, QR_CHUNK], f64::NAN).unwrap();
        let initial = QrWork::new(&bad, 0.0).unwrap();
        let next = initial.step().unwrap();
        assert_eq!(next.cursor, QR_CHUNK);
        assert!(matches!(next.step(), Err(Error::NonFinite)));
        assert!(matches!(initial.result(), Err(Error::Domain)));
        let mut forged = next;
        forged.cursor = usize::MAX;
        assert!(matches!(forged.step(), Err(Error::Domain)));
    }
    #[test]
    fn late_overflow_discards_deferred_edits_and_restores_accounting() {
        let input = Array::floats(vec![2, 2], &[1.4e308, 1.4e308, 1.4e308, 1.4e308]).unwrap();
        assert!(matches!(input.qr(0.0), Err(Error::Overflow)));
        let work = QrWork::new(&input, 0.0).unwrap();
        let accounting = Accounting::default();
        for a in [
            &work.input,
            &work.matrix,
            &work.reflectors,
            &work.q,
            &work.r,
            &work.permutation,
        ] {
            accounting.register(a);
        }
        let before = accounting.bytes();
        let old = work.matrix.clone();
        assert!(matches!(work.step(), Err(Error::Overflow)));
        assert_eq!(work.matrix, old);
        assert_eq!(accounting.bytes(), before);
        drop(old);
        drop(work);
        drop(input);
        assert_eq!(accounting.bytes(), 0);
    }
}
