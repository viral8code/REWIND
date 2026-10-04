//! Bounded CSR matrix-vector steps. State and scratch are ordinary COW values.
use super::*;
pub const SPARSE_CHUNK: usize = 4096;
#[derive(Clone)]
pub struct SparseWork {
    pub rows: usize,
    pub cols: usize,
    pub offsets: Array,
    pub indices: Array,
    pub values: Array,
    pub right: Array,
    pub output: Array,
    pub phase: usize,
    pub cursor: usize,
    pub entry: usize,
    pub previous: i64,
    pub sum: f64,
    pub correction: f64,
}
impl SparseWork {
    pub fn new(
        rows: usize,
        cols: usize,
        offsets: &Array,
        indices: &Array,
        values: &Array,
        right: &Array,
    ) -> Result<Self> {
        if rows > crate::transforms::MAX_SPARSE
            || cols > crate::transforms::MAX_SPARSE
            || values.len() > crate::transforms::MAX_SPARSE
        {
            return Err(Error::Size);
        }
        for (a, dtype) in [
            (offsets, DType::Int64),
            (indices, DType::Int64),
            (values, DType::Float64),
            (right, DType::Float64),
        ] {
            if a.dtype() != dtype {
                return Err(Error::Type);
            }
            if a.shape().len() != 1 {
                return Err(Error::Shape);
            }
        }
        if offsets.len() != rows + 1 || indices.len() != values.len() || right.len() != cols {
            return Err(Error::Shape);
        }
        if offsets.integer(&[0])? != 0 {
            return Err(Error::Shape);
        }
        Ok(Self {
            rows,
            cols,
            offsets: offsets.clone(),
            indices: indices.clone(),
            values: values.clone(),
            right: right.clone(),
            output: Array::zeros(DType::Float64, vec![rows])?,
            phase: 0,
            cursor: 0,
            entry: 0,
            previous: -1,
            sum: 0.,
            correction: 0.,
        })
    }
    pub fn validate(&self) -> Result<()> {
        if self.rows > crate::transforms::MAX_SPARSE
            || self.cols > crate::transforms::MAX_SPARSE
            || self.values.len() > crate::transforms::MAX_SPARSE
        {
            return Err(Error::Size);
        }
        for (a, dtype, len) in [
            (&self.offsets, DType::Int64, self.rows + 1),
            (&self.indices, DType::Int64, self.values.len()),
            (&self.values, DType::Float64, self.values.len()),
            (&self.right, DType::Float64, self.cols),
            (&self.output, DType::Float64, self.rows),
        ] {
            if a.dtype() != dtype {
                return Err(Error::Type);
            }
            if a.shape() != [len] {
                return Err(Error::Shape);
            }
        }
        if !self.output.contiguous() || !self.output.writable || self.output.offset != 0 {
            return Err(Error::ReadOnly);
        }
        let limit = if self.phase == 0 {
            self.cols
        } else {
            self.rows
        };
        if self.phase > 2
            || self.cursor > limit
            || self.entry > self.values.len()
            || self.previous < -1
            || (self.previous >= 0 && self.previous as usize >= self.cols)
        {
            return Err(Error::Domain);
        }
        if !self.sum.is_finite() || !self.correction.is_finite() {
            return Err(Error::NonFinite);
        }
        Ok(())
    }
    pub fn done(&self) -> bool {
        self.phase == 2
    }
    pub fn result(&self) -> Result<Array> {
        self.validate()?;
        if !self.done() {
            return Err(Error::Domain);
        }
        Ok(self.output.clone())
    }
    pub fn step(&self) -> Result<Self> {
        self.validate()?;
        let mut next = self.clone();
        if self.phase == 0 {
            let end = (self.cursor + SPARSE_CHUNK).min(self.cols);
            for i in self.cursor..end {
                finite(self.right.float(&[i])?)?;
            }
            next.cursor = end;
            if end == self.cols {
                next.phase = 1;
                next.cursor = 0;
            }
            return Ok(next);
        }
        if self.done() {
            return Ok(next);
        }
        let output_start = self.cursor;
        let mut out = Vec::with_capacity(SPARSE_CHUNK);
        let mut remaining = SPARSE_CHUNK;
        // A row completion also consumes a work item: millions of empty rows
        // cannot turn one native step into an unbounded scan.
        while remaining > 0 && next.cursor < self.rows {
            let start =
                usize::try_from(self.offsets.integer(&[next.cursor])?).map_err(|_| Error::Index)?;
            let end = usize::try_from(self.offsets.integer(&[next.cursor + 1])?)
                .map_err(|_| Error::Index)?;
            if start > end || end > self.values.len() || next.entry < start || next.entry > end {
                return Err(Error::Shape);
            }
            if next.entry == end {
                out.push(finite(next.sum + next.correction)?.to_bits());
                next.cursor += 1;
                next.previous = -1;
                next.sum = 0.;
                next.correction = 0.;
                remaining -= 1;
                continue;
            }
            let c = self.indices.integer(&[next.entry])?;
            if c < 0 || c as usize >= self.cols || c <= next.previous {
                return Err(Error::Index);
            }
            let value = finite(self.values.float(&[next.entry])?)?;
            let x = finite(value * self.right.float(&[c as usize])?)?;
            let t = finite(next.sum + x)?;
            next.correction = finite(
                next.correction
                    + if next.sum.abs() >= x.abs() {
                        (next.sum - t) + x
                    } else {
                        (x - t) + next.sum
                    },
            )?;
            next.sum = t;
            next.previous = c;
            next.entry += 1;
            remaining -= 1;
        }
        if next.cursor == self.rows {
            if next.entry != self.values.len() {
                return Err(Error::Shape);
            }
            next.phase = 2;
        }
        if !out.is_empty() {
            Node::write_range(
                &mut next.output.buffer.root,
                next.output.buffer.height,
                output_start,
                &out,
            );
        }
        Ok(next)
    }
}
fn finite(x: f64) -> Result<f64> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(Error::NonFinite)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn f(x: &[f64]) -> Array {
        Array::floats(vec![x.len()], x).unwrap()
    }
    fn i(x: &[i64]) -> Array {
        Array::integers(vec![x.len()], x).unwrap()
    }
    fn finish(mut w: SparseWork) -> Array {
        while !w.done() {
            w = w.step().unwrap()
        }
        w.result().unwrap()
    }
    #[test]
    fn matches_dense_reference_with_views_empty_rows_and_compensation() {
        let right = f(&[10., 99., 20., 99., 30.]).slice(0, 0, 3, 2).unwrap();
        let w = SparseWork::new(
            4,
            3,
            &i(&[0, 2, 2, 3, 6]),
            &i(&[0, 2, 1, 0, 1, 2]),
            &f(&[2., -1., 4., 1e16, 1., -1e16]),
            &right,
        )
        .unwrap();
        let result = finish(w.clone());
        let expected =
            crate::transforms::matvec(4, 3, &w.offsets, &w.indices, &w.values, &right).unwrap();
        assert_eq!(
            result.float_values().unwrap(),
            expected.float_values().unwrap()
        );
        assert_eq!(result.float(&[0]).unwrap(), -10.);
        assert_eq!(result.float(&[1]).unwrap(), 0.);
    }
    #[test]
    fn wide_row_and_empty_rows_are_bounded_checkpoint_stable() {
        let n = SPARSE_CHUNK * 3;
        let cols = i(&(0..n as i64).collect::<Vec<_>>());
        let w = SparseWork::new(
            1,
            n,
            &i(&[0, n as i64]),
            &cols,
            &f(&vec![1.; n]),
            &f(&vec![1.; n]),
        )
        .unwrap();
        let mut w = w;
        while w.phase == 0 {
            w = w.step().unwrap()
        }
        let checkpoint = w.step().unwrap();
        assert_eq!(checkpoint.entry, SPARSE_CHUNK);
        assert!(!checkpoint.done());
        let a = finish(checkpoint.clone());
        let b = finish(checkpoint.clone());
        assert_eq!(a, b);
        assert_eq!(a.float(&[0]).unwrap(), n as f64);
        assert_eq!(checkpoint.output.float(&[0]).unwrap(), 0.);
        let zeros = Array::zeros(DType::Int64, vec![n + 1]).unwrap();
        let empty = SparseWork::new(n, 0, &zeros, &i(&[]), &f(&[]), &f(&[]))
            .unwrap()
            .step()
            .unwrap()
            .step()
            .unwrap();
        assert_eq!(empty.cursor, SPARSE_CHUNK);
        assert!(!empty.done());
        assert_eq!(finish(empty).len(), n);
    }
    #[test]
    fn late_invalid_entries_and_unused_nonfinite_inputs_are_atomic() {
        let n = SPARSE_CHUNK + 1;
        let mut columns = (0..n as i64).collect::<Vec<_>>();
        columns[n - 1] = columns[n - 2];
        let mut w = SparseWork::new(
            1,
            n,
            &i(&[0, n as i64]),
            &i(&columns),
            &f(&vec![1.; n]),
            &f(&vec![1.; n]),
        )
        .unwrap();
        while w.phase == 0 {
            w = w.step().unwrap()
        }
        let old = w.step().unwrap();
        assert!(matches!(old.step(), Err(Error::Index)));
        assert_eq!(old.entry, SPARSE_CHUNK);
        assert_eq!(old.output.float(&[0]).unwrap(), 0.);
        let unused = SparseWork::new(0, 1, &i(&[0]), &i(&[]), &f(&[]), &f(&[f64::NAN])).unwrap();
        assert!(matches!(unused.step(), Err(Error::NonFinite)));
        let trailing = SparseWork::new(0, 1, &i(&[0]), &i(&[0]), &f(&[1.]), &f(&[1.]))
            .unwrap()
            .step()
            .unwrap();
        assert!(matches!(trailing.step(), Err(Error::Shape)));
        let mut invalid = old;
        invalid.cursor = usize::MAX;
        assert!(matches!(invalid.step(), Err(Error::Domain)));
    }
}
