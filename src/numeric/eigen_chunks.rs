//! Pure cyclic Jacobi eigen decomposition with bounded steps and stable sorting.
use super::*;
pub const EIGEN_CHUNK: usize = 4096;
#[derive(Clone)]
pub struct EigenWork {
    pub input: Array,
    pub matrix: Array,
    // Private eigenvectors are stored by column for contiguous rotation updates.
    pub vectors: Array,
    pub order: Array,
    pub order_scratch: Array,
    pub sorted: Array,
    pub values: Array,
    pub tolerance: f64,
    pub scale: f64,
    pub largest: f64,
    pub cosine: f64,
    pub sine: f64,
    pub max_sweeps: usize,
    pub sweeps: usize,
    pub phase: u8,
    pub cursor: usize,
    pub p: usize,
    pub q: usize,
    pub width: usize,
    pub base: usize,
    pub left: usize,
    pub mid: usize,
    pub right: usize,
    pub end: usize,
    pub out: usize,
}
impl EigenWork {
    pub fn new(input: &Array, tolerance: f64, max_sweeps: usize) -> Result<Self> {
        if input.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let [n, m] = input.shape.as_slice() else {
            return Err(Error::Shape);
        };
        if n != m {
            return Err(Error::Shape);
        }
        if !tolerance.is_finite() || tolerance < 0.0 || max_sweeps > 10000 {
            return Err(Error::Domain);
        }
        Ok(Self {
            input: input.clone(),
            matrix: Array::zeros(DType::Float64, vec![*n, *n])?,
            vectors: Array::zeros(DType::Float64, vec![*n, *n])?,
            order: Array::zeros(DType::Int64, vec![*n])?,
            order_scratch: Array::zeros(DType::Int64, vec![*n])?,
            sorted: Array::zeros(DType::Float64, vec![*n, *n])?,
            values: Array::zeros(DType::Float64, vec![*n])?,
            tolerance,
            scale: 0.0,
            largest: 0.0,
            cosine: 1.0,
            sine: 0.0,
            max_sweeps,
            sweeps: 0,
            phase: 0,
            cursor: 0,
            p: 0,
            q: 1,
            width: 1,
            base: 0,
            left: 0,
            mid: 0,
            right: 0,
            end: 0,
            out: 0,
        })
    }
    pub fn validate(&self) -> Result<()> {
        if self.input.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let [n, m] = self.input.shape.as_slice() else {
            return Err(Error::Shape);
        };
        if n != m {
            return Err(Error::Shape);
        }
        for (a, shape, dtype) in [
            (&self.matrix, vec![*n, *n], DType::Float64),
            (&self.vectors, vec![*n, *n], DType::Float64),
            (&self.sorted, vec![*n, *n], DType::Float64),
            (&self.order, vec![*n], DType::Int64),
            (&self.order_scratch, vec![*n], DType::Int64),
            (&self.values, vec![*n], DType::Float64),
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
            || self.max_sweeps > 10000
            || self.sweeps > self.max_sweeps
            || [self.scale, self.largest]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
            || !self.cosine.is_finite()
            || !self.sine.is_finite()
            || self.cosine.abs() > 1.0
            || self.sine.abs() > 1.0
            || self.p > *n
            || self.q > n + 1
            || self.width == 0
            || self.width > n.saturating_mul(2).max(1)
            || [
                self.base, self.left, self.mid, self.right, self.end, self.out,
            ]
            .iter()
            .any(|i| *i > *n)
        {
            return Err(Error::Domain);
        }
        let pair = self.p == *n || self.p < *n && self.q > self.p && self.q <= *n;
        let valid = match self.phase {
            0 | 1 => self.cursor <= n * n,
            2 | 4 | 5 => pair,
            3 | 7 | 10 => self.cursor <= *n,
            6 => self.p < *n && self.q > self.p && self.q < *n && self.cursor <= *n,
            8 => self.base <= *n,
            9 => {
                self.base <= self.left
                    && self.left <= self.mid
                    && self.mid <= self.right
                    && self.right <= self.end
                    && self.end <= *n
                    && self.out >= self.base
                    && self.out <= self.end
                    && self.out == self.left + self.right - self.mid
            }
            11 => self.cursor <= n * n,
            12 => true,
            _ => false,
        };
        if !valid {
            return Err(Error::Domain);
        }
        Ok(())
    }
    pub fn done(&self) -> bool {
        self.phase == 12
    }
    pub fn result(&self) -> Result<Eigen> {
        self.validate()?;
        if !self.done() {
            return Err(Error::Domain);
        }
        Ok(Eigen {
            values: self.values.clone(),
            vectors: self.sorted.clone(),
            sweeps: self.sweeps,
        })
    }
    fn get(a: &Array, i: usize) -> f64 {
        f64::from_bits(a.buffer.get(i))
    }
    fn finite(v: f64) -> Result<f64> {
        if v.is_finite() {
            Ok(v)
        } else {
            Err(Error::Overflow)
        }
    }
    fn put(a: &mut Array, i: usize, v: f64) {
        Node::write(&mut a.buffer.root, a.buffer.height, i, v.to_bits());
    }
    fn order_at(&self, i: usize) -> Result<usize> {
        if i >= self.values.len() {
            return Err(Error::Domain);
        }
        let index = self.order.buffer.get(i) as usize;
        if index < self.values.len() {
            Ok(index)
        } else {
            Err(Error::Domain)
        }
    }
    fn next_pair(&mut self, n: usize) {
        self.q += 1;
        if self.q >= n {
            self.p += 1;
            self.q = self.p + 1;
        }
    }
    pub fn step(&self) -> Result<Self> {
        self.validate()?;
        let n = self.values.len();
        let mut w = self.clone();
        let mut used = 0;
        while used < EIGEN_CHUNK && !w.done() {
            used += 1;
            match w.phase {
                0 => {
                    if w.cursor == n * n {
                        w.phase = 1;
                        w.cursor = 0;
                        continue;
                    }
                    let end = (w.cursor + EIGEN_CHUNK - used + 1).min(n * n);
                    let mut bits = Vec::with_capacity(end - w.cursor);
                    for b in w.input.window_bits(w.cursor, end)? {
                        let v = f64::from_bits(b);
                        if !v.is_finite() {
                            return Err(Error::NonFinite);
                        }
                        w.scale = w.scale.max(v.abs());
                        bits.push(b);
                    }
                    let height = w.matrix.buffer.height;
                    Node::write_range(&mut w.matrix.buffer.root, height, w.cursor, &bits);
                    used += bits.len() - 1;
                    w.cursor = end;
                }
                1 => {
                    if w.cursor == n * n || w.scale == 0.0 {
                        w.phase = 2;
                        w.p = 0;
                        w.q = 1;
                        continue;
                    }
                    let end = (w.cursor + EIGEN_CHUNK - used + 1).min(n * n);
                    let bits = w
                        .matrix
                        .window_bits(w.cursor, end)?
                        .map(|v| (f64::from_bits(v) / w.scale).to_bits())
                        .collect::<Vec<_>>();
                    let height = w.matrix.buffer.height;
                    Node::write_range(&mut w.matrix.buffer.root, height, w.cursor, &bits);
                    used += bits.len() - 1;
                    w.cursor = end;
                }
                2 => {
                    if w.p >= n {
                        w.phase = 3;
                        w.cursor = 0;
                        continue;
                    }
                    if w.q >= n {
                        w.next_pair(n);
                        continue;
                    }
                    let a = Self::get(&w.matrix, w.p * n + w.q);
                    let b = Self::get(&w.matrix, w.q * n + w.p);
                    if (a - b).abs() > w.tolerance {
                        return Err(Error::Domain);
                    }
                    let value = 0.5 * a + 0.5 * b;
                    // Preserve averaging and signed-zero bits, but do not copy
                    // already identical symmetric storage just to rewrite it.
                    if value.to_bits() != a.to_bits() {
                        Self::put(&mut w.matrix, w.p * n + w.q, value);
                    }
                    if value.to_bits() != b.to_bits() {
                        Self::put(&mut w.matrix, w.q * n + w.p, value);
                    }
                    w.next_pair(n);
                }
                3 => {
                    if w.cursor == n {
                        w.phase = 4;
                        w.p = 0;
                        w.q = 1;
                        w.largest = 0.0;
                        continue;
                    }
                    Self::put(&mut w.vectors, w.cursor * n + w.cursor, 1.0);
                    w.cursor += 1;
                }
                4 => {
                    if w.p >= n {
                        if w.largest <= w.tolerance {
                            w.phase = 7;
                            w.cursor = 0;
                            continue;
                        }
                        if w.sweeps == w.max_sweeps {
                            return Err(Error::Convergence);
                        }
                        w.phase = 5;
                        w.p = 0;
                        w.q = 1;
                        continue;
                    }
                    if w.q >= n {
                        w.next_pair(n);
                        continue;
                    }
                    w.largest = w.largest.max(Self::get(&w.matrix, w.p * n + w.q).abs());
                    w.next_pair(n);
                }
                5 => {
                    if w.p >= n {
                        w.sweeps += 1;
                        w.phase = 4;
                        w.p = 0;
                        w.q = 1;
                        w.largest = 0.0;
                        continue;
                    }
                    if w.q >= n {
                        w.next_pair(n);
                        continue;
                    }
                    let cross = Self::get(&w.matrix, w.p * n + w.q);
                    if cross.abs() <= w.tolerance {
                        w.next_pair(n);
                        continue;
                    }
                    let difference =
                        Self::get(&w.matrix, w.q * n + w.q) - Self::get(&w.matrix, w.p * n + w.p);
                    let twice = 2.0 * cross;
                    let denominator = difference + difference.hypot(twice).copysign(difference);
                    let t = twice / denominator;
                    w.cosine = 1.0 / (1.0 + t * t).sqrt();
                    w.sine = t * w.cosine;
                    let a = Self::finite(Self::get(&w.matrix, w.p * n + w.p) - t * cross)?;
                    let b = Self::finite(Self::get(&w.matrix, w.q * n + w.q) + t * cross)?;
                    Self::put(&mut w.matrix, w.p * n + w.p, a);
                    Self::put(&mut w.matrix, w.q * n + w.q, b);
                    Self::put(&mut w.matrix, w.p * n + w.q, 0.0);
                    Self::put(&mut w.matrix, w.q * n + w.p, 0.0);
                    w.phase = 6;
                    w.cursor = 0;
                }
                6 => {
                    if w.cursor == n {
                        w.next_pair(n);
                        w.phase = 5;
                        continue;
                    }
                    let start = w.cursor;
                    let end = (start + EIGEN_CHUNK - used + 1).min(n);
                    let count = end - start;
                    let mut matrix_p = Vec::with_capacity(count);
                    let mut matrix_q = Vec::with_capacity(count);
                    let mut vector_p = Vec::with_capacity(count);
                    let mut vector_q = Vec::with_capacity(count);
                    // Symmetry makes row p/q reads identical to column p/q.
                    // Each row's arithmetic is independent; stage bounded values
                    // before bulk row writes and mirrored column writes.
                    let matrix = w
                        .matrix
                        .window_bits(w.p * n + start, w.p * n + end)?
                        .zip(w.matrix.window_bits(w.q * n + start, w.q * n + end)?);
                    let vectors = w
                        .vectors
                        .window_bits(w.p * n + start, w.p * n + end)?
                        .zip(w.vectors.window_bits(w.q * n + start, w.q * n + end)?);
                    for (offset, ((x, y), (vx, vy))) in matrix.zip(vectors).enumerate() {
                        let row = start + offset;
                        let (x, y) = (f64::from_bits(x), f64::from_bits(y));
                        if row == w.p || row == w.q {
                            matrix_p.push(x.to_bits());
                            matrix_q.push(y.to_bits());
                        } else {
                            matrix_p.push((w.cosine * x - w.sine * y).to_bits());
                            matrix_q.push((w.sine * x + w.cosine * y).to_bits());
                        }
                        let (vx, vy) = (f64::from_bits(vx), f64::from_bits(vy));
                        vector_p.push((w.cosine * vx - w.sine * vy).to_bits());
                        vector_q.push((w.sine * vx + w.cosine * vy).to_bits());
                    }
                    for offset in 0..count {
                        let row = start + offset;
                        if row != w.p && row != w.q {
                            Self::put(
                                &mut w.matrix,
                                row * n + w.p,
                                f64::from_bits(matrix_p[offset]),
                            );
                            Self::put(
                                &mut w.matrix,
                                row * n + w.q,
                                f64::from_bits(matrix_q[offset]),
                            );
                        }
                    }
                    let height = w.matrix.buffer.height;
                    Node::write_range(
                        &mut w.matrix.buffer.root,
                        height,
                        w.p * n + start,
                        &matrix_p,
                    );
                    Node::write_range(
                        &mut w.matrix.buffer.root,
                        height,
                        w.q * n + start,
                        &matrix_q,
                    );
                    let height = w.vectors.buffer.height;
                    Node::write_range(
                        &mut w.vectors.buffer.root,
                        height,
                        w.p * n + start,
                        &vector_p,
                    );
                    Node::write_range(
                        &mut w.vectors.buffer.root,
                        height,
                        w.q * n + start,
                        &vector_q,
                    );
                    used += count - 1;
                    w.cursor = end;
                }
                7 => {
                    if w.cursor == n {
                        w.phase = 8;
                        w.width = 1;
                        w.base = 0;
                        continue;
                    }
                    Node::write(
                        &mut w.order.buffer.root,
                        w.order.buffer.height,
                        w.cursor,
                        w.cursor as u64,
                    );
                    w.cursor += 1;
                }
                8 => {
                    if w.width >= n {
                        w.phase = 10;
                        w.cursor = 0;
                        continue;
                    }
                    if w.base == n {
                        std::mem::swap(&mut w.order, &mut w.order_scratch);
                        w.width *= 2;
                        w.base = 0;
                        continue;
                    }
                    w.left = w.base;
                    w.mid = (w.base + w.width).min(n);
                    w.right = w.mid;
                    w.end = (w.mid + w.width).min(n);
                    w.out = w.base;
                    w.phase = 9;
                }
                9 => {
                    if w.out == w.end {
                        w.base = w.end;
                        w.phase = 8;
                        continue;
                    }
                    let left = w.left < w.mid
                        && (w.right == w.end || {
                            let l = w.order_at(w.left)?;
                            let r = w.order_at(w.right)?;
                            Self::get(&w.matrix, l * n + l)
                                .total_cmp(&Self::get(&w.matrix, r * n + r))
                                != std::cmp::Ordering::Greater
                        });
                    let index = if left {
                        let v = w.order_at(w.left)?;
                        w.left += 1;
                        v
                    } else {
                        let v = w.order_at(w.right)?;
                        w.right += 1;
                        v
                    };
                    Node::write(
                        &mut w.order_scratch.buffer.root,
                        w.order_scratch.buffer.height,
                        w.out,
                        index as u64,
                    );
                    w.out += 1;
                }
                10 => {
                    if w.cursor == n {
                        w.phase = 11;
                        w.cursor = 0;
                        continue;
                    }
                    let index = w.order_at(w.cursor)?;
                    let v = Self::finite(Self::get(&w.matrix, index * n + index) * w.scale)?;
                    Self::put(&mut w.values, w.cursor, v);
                    w.cursor += 1;
                }
                11 => {
                    if w.cursor == n * n {
                        w.phase = 12;
                        continue;
                    }
                    let row = w.cursor / n;
                    let col = w.cursor % n;
                    let old = w.order_at(col)?;
                    let v = Self::get(&w.vectors, old * n + row);
                    Self::put(&mut w.sorted, w.cursor, v);
                    w.cursor += 1;
                }
                _ => return Err(Error::Domain),
            }
        }
        Ok(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn finish(mut work: EigenWork) -> Result<EigenWork> {
        let mut steps = 0;
        while !work.done() {
            work = work.step()?;
            steps += 1;
            assert!(steps < 1_000_000);
        }
        Ok(work)
    }
    fn compare(input: &Array, tolerance: f64, sweeps: usize) {
        let expected = input.eigen_symmetric(tolerance, sweeps).unwrap();
        let actual = finish(EigenWork::new(input, tolerance, sweeps).unwrap())
            .unwrap()
            .result()
            .unwrap();
        assert_eq!(actual.values, expected.values);
        assert_eq!(actual.vectors, expected.vectors);
        assert_eq!(actual.sweeps, expected.sweeps);
    }
    #[test]
    fn synchronous_bits_match_rotations_stable_order_views_and_scaled_inputs() {
        for n in 0..12 {
            let data = (0..n * n)
                .map(|i| {
                    let (r, c) = (i / n, i % n);
                    if r == c {
                        (r + 2) as f64
                    } else {
                        ((r + c) * 17 % 31) as f64 / 100.0
                    }
                })
                .collect::<Vec<_>>();
            let a = Array::floats(vec![n, n], &data).unwrap();
            compare(&a, 1e-13, 80);
            compare(&a.transpose(&[1, 0]).unwrap(), 1e-13, 80);
        }
        let a = Array::floats(
            vec![4, 4],
            &[
                -0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, -1.0,
            ],
        )
        .unwrap();
        compare(&a, 0.0, 0);
        for scale in [1e-150, 1e150] {
            compare(
                &Array::floats(vec![2, 2], &[2.0 * scale, scale, scale, 2.0 * scale]).unwrap(),
                1e-13,
                20,
            );
        }
    }
    #[test]
    fn checkpoint_inside_rotation_restores_private_arrays_and_independent_residual() {
        let n = 65;
        let data = (0..n * n)
            .map(|i| {
                let (r, c) = (i / n, i % n);
                if r == c {
                    2.0 + (r as f64) / 10.0
                } else {
                    ((r + c) * 17 % 31) as f64 / 1000.0
                }
            })
            .collect::<Vec<_>>();
        let a = Array::floats(vec![n, n], &data).unwrap();
        let mut w = EigenWork::new(&a, 1e-13, 60).unwrap();
        while w.phase != 6 {
            w = w.step().unwrap();
            assert!(!w.done());
        }
        let old = w.matrix.clone();
        let first = finish(w.clone()).unwrap().result().unwrap();
        let second = finish(w.clone()).unwrap().result().unwrap();
        assert_eq!(first.values, second.values);
        assert_eq!(first.vectors, second.vectors);
        assert_eq!(w.matrix, old);
        let expected = a.eigen_symmetric(1e-13, 60).unwrap();
        assert_eq!(first.values, expected.values);
        assert_eq!(first.vectors, expected.vectors);
        let product = a.matmul(&first.vectors).unwrap();
        let identity = first
            .vectors
            .transpose(&[1, 0])
            .unwrap()
            .matmul(&first.vectors)
            .unwrap();
        for row in 0..n {
            for col in 0..n {
                assert!(
                    (product.float(&[row, col]).unwrap()
                        - first.vectors.float(&[row, col]).unwrap()
                            * first.values.float(&[col]).unwrap())
                    .abs()
                        < 1e-10
                );
                assert!(
                    (identity.float(&[row, col]).unwrap() - if row == col { 1.0 } else { 0.0 })
                        .abs()
                        < 1e-10
                );
            }
        }
    }
    #[test]
    fn partial_column_rotation_crosses_pages_without_changing_old_versions() {
        let n = 257;
        let input = Array::zeros(DType::Float64, vec![n, n]).unwrap();
        let mut work = EigenWork::new(&input, 0.0, 1).unwrap();
        for i in 0..n {
            work.vectors.set_float(&[i, i], 1.0).unwrap();
        }
        work.phase = 6;
        work.p = 0;
        work.q = n - 1;
        work.cursor = 250;
        work.cosine = std::f64::consts::FRAC_1_SQRT_2;
        work.sine = std::f64::consts::FRAC_1_SQRT_2;
        let old = work.vectors.clone();
        let first = work.step().unwrap();
        let repeated = work.step().unwrap();
        assert_eq!(first.vectors, repeated.vectors);
        assert_eq!(work.vectors, old);
        assert_eq!(old.buffer.get(n - 1), 0);
        assert_eq!(first.vectors.buffer.get(n - 1), (-work.sine).to_bits());
        assert_eq!(first.vectors.buffer.get(n * n - 1), work.cosine.to_bits());
        assert_eq!(first.vectors.buffer.get(0), 1.0f64.to_bits());
        assert_eq!(first.matrix, work.matrix);
    }

    #[test]
    fn large_zero_copy_is_bounded_and_late_failures_preserve_versions_and_ledger() {
        let input = Array::zeros(DType::Float64, vec![4096, 4096]).unwrap();
        let first = EigenWork::new(&input, 0.0, 0).unwrap().step().unwrap();
        assert_eq!(first.cursor, EIGEN_CHUNK);
        let accounting = Accounting::default();
        for a in [
            &first.input,
            &first.matrix,
            &first.vectors,
            &first.order,
            &first.order_scratch,
            &first.sorted,
            &first.values,
        ] {
            accounting.register(a);
        }
        assert!(accounting.bytes() < 1024 * 1024);
        drop(first);
        drop(input);
        assert_eq!(accounting.bytes(), 0);
        let mut bad = Array::zeros(DType::Float64, vec![65, 65]).unwrap();
        bad.set_float(&[64, 64], f64::NAN).unwrap();
        let first = EigenWork::new(&bad, 0.0, 0).unwrap().step().unwrap();
        let old = first.matrix.clone();
        assert!(matches!(first.step(), Err(Error::NonFinite)));
        assert_eq!(first.matrix, old);
        let input = Array::floats(vec![2, 2], &[1.4e308, 1.4e308, 1.4e308, 1.4e308]).unwrap();
        let w = EigenWork::new(&input, 1e-13, 20).unwrap();
        let ledger = Accounting::default();
        for a in [
            &w.input,
            &w.matrix,
            &w.vectors,
            &w.order,
            &w.order_scratch,
            &w.sorted,
            &w.values,
        ] {
            ledger.register(a);
        }
        let before = ledger.bytes();
        assert!(matches!(finish(w.clone()), Err(Error::Overflow)));
        assert_eq!(ledger.bytes(), before);
        drop(w);
        drop(input);
        assert_eq!(ledger.bytes(), 0);
    }
    #[test]
    fn symmetry_convergence_and_invalid_private_state_are_rejected() {
        let a = Array::floats(vec![2, 2], &[1.0, 1.0, 0.0, 1.0]).unwrap();
        assert!(matches!(
            finish(EigenWork::new(&a, 0.0, 10).unwrap()),
            Err(Error::Domain)
        ));
        let a = Array::floats(vec![2, 2], &[2.0, 1.0, 1.0, 2.0]).unwrap();
        assert!(matches!(
            finish(EigenWork::new(&a, 1e-13, 0).unwrap()),
            Err(Error::Convergence)
        ));
        let mut w = EigenWork::new(&a, 0.0, 10).unwrap();
        w.phase = 9;
        w.mid = 1;
        w.left = 1;
        w.right = 2;
        w.end = 2;
        w.out = 0;
        assert!(matches!(w.step(), Err(Error::Domain)));
        assert!(matches!(EigenWork::new(&a, -1.0, 10), Err(Error::Domain)));
        assert!(matches!(EigenWork::new(&a, 0.0, 10001), Err(Error::Domain)));
    }
}
