use super::*;
pub const COOPERATIVE_MACS: usize = 4096;
#[derive(Clone, Copy, Debug)]
pub enum VectorOperation {
    Scale(f64),
    Add,
    Sub,
    Mul,
    Div,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormProgress {
    pub cursor: usize,
    pub scale: f64,
    pub sum: f64,
}
impl NormProgress {
    pub fn initial() -> Self {
        Self {
            cursor: 0,
            scale: 0.0,
            sum: 1.0,
        }
    }
    pub fn value(self) -> Result<f64> {
        if !self.scale.is_finite() || self.scale < 0.0 || !self.sum.is_finite() || self.sum < 1.0 {
            return Err(Error::NonFinite);
        }
        let value = self.scale * self.sum.sqrt();
        if value.is_finite() {
            Ok(value)
        } else {
            Err(Error::Overflow)
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub cursor: usize,
    pub sum: f64,
    pub correction: f64,
}
impl Progress {
    pub fn initial() -> Self {
        Self {
            cursor: 0,
            sum: 0.0,
            correction: 0.0,
        }
    }
    fn validate(self, total: usize) -> Result<()> {
        if self.cursor > total {
            return Err(Error::Index);
        }
        if !self.sum.is_finite() || !self.correction.is_finite() {
            return Err(Error::NonFinite);
        }
        Ok(())
    }
    fn accumulate(&mut self, value: f64) {
        let next = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        self.sum = next;
        self.cursor += 1;
    }
}
impl Node {
    pub(super) fn write_range(node: &mut Arc<Self>, height: usize, index: usize, values: &[u64]) {
        if values.is_empty() {
            return;
        }
        let n = Arc::make_mut(node);
        match &mut n.kind {
            NodeKind::Leaf(v) => v[index..index + values.len()].copy_from_slice(values),
            NodeKind::Branch(a, b) => {
                let half = PAGE << (height - 1);
                if index < half {
                    let left = values.len().min(half - index);
                    Self::write_range(a, height - 1, index, &values[..left]);
                    if left < values.len() {
                        Self::write_range(b, height - 1, 0, &values[left..]);
                    }
                } else {
                    Self::write_range(b, height - 1, index - half, values);
                }
            }
        }
        n.refresh();
    }
}
impl Array {
    fn vector_require(&self, other: &Self, operation: VectorOperation) -> Result<()> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        if let VectorOperation::Scale(factor) = operation {
            if !factor.is_finite() {
                return Err(Error::NonFinite);
            }
        } else {
            if other.dtype != self.dtype {
                return Err(Error::Type);
            }
            if other.shape != self.shape {
                return Err(Error::Shape);
            }
        }
        Ok(())
    }
    pub fn vector_init(&self, other: &Self, operation: VectorOperation) -> Result<Self> {
        self.vector_require(other, operation)?;
        Self::zeros(DType::Float64, self.shape.clone())
    }
    pub fn vector_step(
        &self,
        other: &Self,
        output: &Self,
        operation: VectorOperation,
        cursor: usize,
    ) -> Result<(Self, usize)> {
        self.vector_require(other, operation)?;
        if cursor > self.len() {
            return Err(Error::Index);
        }
        if output.dtype != self.dtype {
            return Err(Error::Type);
        }
        if output.shape != self.shape || !output.contiguous() {
            return Err(Error::Shape);
        }
        if !output.writable {
            return Err(Error::ReadOnly);
        }
        let end = cursor.saturating_add(COOPERATIVE_MACS).min(self.len());
        let mut completed = Vec::with_capacity(end - cursor);
        let mut right = if matches!(operation, VectorOperation::Scale(_)) {
            None
        } else {
            Some(other.window_bits(cursor, end)?)
        };
        for bit in self.window_bits(cursor, end)? {
            let left = f64::from_bits(bit);
            if !left.is_finite() {
                return Err(Error::NonFinite);
            }
            let value = if let VectorOperation::Scale(factor) = operation {
                left * factor
            } else {
                let right = f64::from_bits(right.as_mut().unwrap().next().unwrap());
                if !right.is_finite() {
                    return Err(Error::NonFinite);
                }
                match operation {
                    VectorOperation::Add => left + right,
                    VectorOperation::Sub => left - right,
                    VectorOperation::Mul => left * right,
                    VectorOperation::Div => {
                        if right == 0.0 {
                            return Err(Error::Domain);
                        }
                        left / right
                    }
                    VectorOperation::Scale(_) => unreachable!(),
                }
            };
            if !value.is_finite() {
                return Err(if matches!(operation, VectorOperation::Scale(_)) {
                    Error::Overflow
                } else {
                    Error::NonFinite
                });
            }
            completed.push(value.to_bits());
        }
        let mut result = output.clone();
        Node::write_range(
            &mut result.buffer.root,
            result.buffer.height,
            result.offset as usize + cursor,
            &completed,
        );
        Ok((result, end))
    }
    pub fn norm_step(&self, mut state: NormProgress) -> Result<NormProgress> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        if state.cursor > self.len() {
            return Err(Error::Index);
        }
        if !state.scale.is_finite()
            || state.scale < 0.0
            || !state.sum.is_finite()
            || state.sum < 1.0
        {
            return Err(Error::NonFinite);
        }
        let end = state
            .cursor
            .saturating_add(COOPERATIVE_MACS)
            .min(self.len());
        for bit in self.window_bits(state.cursor, end)? {
            let value = f64::from_bits(bit);
            if !value.is_finite() {
                return Err(Error::NonFinite);
            }
            let x = value.abs();
            if x != 0.0 {
                if state.scale < x {
                    let ratio = state.scale / x;
                    state.sum = 1.0 + state.sum * ratio * ratio;
                    state.scale = x;
                } else {
                    let ratio = x / state.scale;
                    state.sum += ratio * ratio;
                }
            }
        }
        state.cursor = end;
        if end == self.len() {
            state.value()?;
        }
        Ok(state)
    }
    pub(super) fn window_bits(&self, start: usize, end: usize) -> Result<ArrayBits<'_>> {
        if start > end || end > self.len() {
            return Err(Error::Index);
        }
        Ok(if self.contiguous() {
            let offset = if start == end {
                0
            } else {
                self.offset as usize
            };
            ArrayBits::Contiguous(BufferRange {
                buffer: &self.buffer,
                next: offset + start,
                end: offset + end,
                pending: &[],
            })
        } else {
            ArrayBits::Strided {
                array: self,
                next: start,
                end,
            }
        })
    }
    pub fn dot_step(&self, other: &Self, mut state: Progress) -> Result<Progress> {
        if self.dtype != DType::Float64 || other.dtype != self.dtype {
            return Err(Error::Type);
        }
        if self.shape.len() != 1 || self.shape != other.shape {
            return Err(Error::Shape);
        }
        state.validate(self.len())?;
        let end = state
            .cursor
            .saturating_add(COOPERATIVE_MACS)
            .min(self.len());
        for (a, b) in self
            .window_bits(state.cursor, end)?
            .zip(other.window_bits(state.cursor, end)?)
        {
            let product = f64::from_bits(a) * f64::from_bits(b);
            if !product.is_finite() {
                return Err(Error::NonFinite);
            }
            state.accumulate(product);
            if !state.sum.is_finite() || !state.correction.is_finite() {
                return Err(Error::Overflow);
            }
        }
        if state.cursor == self.len() && !(state.sum + state.correction).is_finite() {
            return Err(Error::Overflow);
        }
        Ok(state)
    }
    pub fn matmul_work(&self, other: &Self) -> Result<(usize, usize, usize, usize)> {
        if self.dtype != DType::Float64 || other.dtype != self.dtype {
            return Err(Error::Type);
        }
        let ([m, k], [l, n]) = (self.shape.as_slice(), other.shape.as_slice()) else {
            return Err(Error::Shape);
        };
        if k != l {
            return Err(Error::Shape);
        }
        let cells = count(&[*m, *n])?;
        let total = cells.checked_mul(*k).ok_or(Error::Size)?;
        Ok((*m, *k, *n, total))
    }
    pub fn matmul_step(
        &self,
        other: &Self,
        output: &Self,
        mut state: Progress,
    ) -> Result<(Self, Progress)> {
        let (m, k, n, total) = self.matmul_work(other)?;
        if output.dtype != self.dtype {
            return Err(Error::Type);
        }
        if output.shape != [m, n] || !output.contiguous() {
            return Err(Error::Shape);
        }
        if !output.writable {
            return Err(Error::ReadOnly);
        }
        state.validate(total)?;
        if k == 0 || state.cursor == total {
            return Ok((output.clone(), state));
        }
        let first_cell = state.cursor / k;
        let end = state.cursor.saturating_add(COOPERATIVE_MACS).min(total);
        let mut completed = Vec::with_capacity((end - state.cursor).div_ceil(k));
        while state.cursor < end {
            let cell = state.cursor / k;
            let from = state.cursor % k;
            let stop = (from + end - state.cursor).min(k);
            let i = cell / n;
            let j = cell % n;
            for p in from..stop {
                let ai = (self.offset + i as isize * self.strides[0] + p as isize * self.strides[1])
                    as usize;
                let bi = (other.offset
                    + p as isize * other.strides[0]
                    + j as isize * other.strides[1]) as usize;
                state.accumulate(
                    f64::from_bits(self.buffer.get(ai)) * f64::from_bits(other.buffer.get(bi)),
                );
            }
            if !state.sum.is_finite() || !state.correction.is_finite() {
                return Err(Error::NonFinite);
            }
            if stop == k {
                let value = state.sum + state.correction;
                if !value.is_finite() {
                    return Err(Error::NonFinite);
                }
                completed.push(value.to_bits());
                state.sum = 0.0;
                state.correction = 0.0;
            }
        }
        let mut result = output.clone();
        Node::write_range(
            &mut result.buffer.root,
            result.buffer.height,
            result.offset as usize + first_cell,
            &completed,
        );
        Ok((result, state))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vector_steps_match_strided_references_and_keep_partial_snapshots() {
        let values = (0..9001)
            .map(|i| (i % 29) as f64 / 3.0 - 4.0)
            .collect::<Vec<_>>();
        let base = Array::floats(vec![values.len()], &values).unwrap();
        let right = Array::floats(vec![1], &[2.0])
            .unwrap()
            .broadcast(vec![values.len()])
            .unwrap();
        for left in [
            base.clone(),
            base.slice(0, values.len() - 1, values.len(), -1).unwrap(),
        ] {
            for operation in [
                VectorOperation::Scale(1.75),
                VectorOperation::Add,
                VectorOperation::Sub,
                VectorOperation::Mul,
                VectorOperation::Div,
            ] {
                let mut output = left.vector_init(&right, operation).unwrap();
                let (first, cursor) = left.vector_step(&right, &output, operation, 0).unwrap();
                assert_eq!(cursor, COOPERATIVE_MACS);
                let saved = first.clone();
                output = first;
                let mut position = cursor;
                while position < left.len() {
                    let next = left
                        .vector_step(&right, &output, operation, position)
                        .unwrap();
                    assert!(next.1 - position <= COOPERATIVE_MACS);
                    output = next.0;
                    position = next.1;
                }
                let reference = left
                    .float_values()
                    .unwrap()
                    .iter()
                    .map(|&a| match operation {
                        VectorOperation::Scale(f) => a * f,
                        VectorOperation::Add => a + 2.0,
                        VectorOperation::Sub => a - 2.0,
                        VectorOperation::Mul => a * 2.0,
                        VectorOperation::Div => a / 2.0,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    output.bits().collect::<Vec<_>>(),
                    reference.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                );
                assert!(saved.bits().skip(cursor).all(|bit| bit == 0));
                assert_eq!(base.float_values().unwrap(), values);
            }
        }
    }
    #[test]
    fn norm_steps_preserve_scaled_order_and_extreme_magnitudes() {
        for values in [
            vec![],
            vec![0.0; 9001],
            vec![1e-200; 9001],
            vec![1e200; 9001],
            (0..9001).map(|i| (i % 37) as f64 - 18.0).collect(),
        ] {
            let input = Array::floats(vec![values.len()], &values).unwrap();
            let mut state = NormProgress::initial();
            loop {
                let next = input.norm_step(state).unwrap();
                assert!(next.cursor - state.cursor <= COOPERATIVE_MACS);
                state = next;
                if state.cursor == input.len() {
                    break;
                }
            }
            assert_eq!(
                state.value().unwrap().to_bits(),
                crate::transforms::norm2(&input).unwrap().to_bits()
            );
        }
    }
    #[test]
    fn vector_and_norm_late_failures_expose_no_mutated_output() {
        let good = Array::floats(vec![5000], &vec![1.0; 5000]).unwrap();
        let mut values = vec![1.0; 5000];
        values[4999] = f64::NAN;
        let bad = Array::floats(vec![5000], &values).unwrap();
        let output = good.vector_init(&bad, VectorOperation::Add).unwrap();
        let (partial, cursor) = good
            .vector_step(&bad, &output, VectorOperation::Add, 0)
            .unwrap();
        assert!(matches!(
            good.vector_step(&bad, &partial, VectorOperation::Add, cursor),
            Err(Error::NonFinite)
        ));
        assert!(partial.bits().skip(cursor).all(|bit| bit == 0));
        let state = bad.norm_step(NormProgress::initial()).unwrap();
        assert_eq!(bad.norm_step(state), Err(Error::NonFinite));
        let readonly = output.broadcast(vec![5000]).unwrap();
        assert!(matches!(
            good.vector_step(&good, &readonly, VectorOperation::Add, 0),
            Err(Error::ReadOnly)
        ));
        assert!(matches!(
            good.vector_step(&good, &output, VectorOperation::Div, 5001),
            Err(Error::Index)
        ));
        assert!(matches!(
            good.vector_init(&good, VectorOperation::Scale(f64::INFINITY)),
            Err(Error::NonFinite)
        ));
    }
    #[test]
    fn dot_steps_preserve_order_strides_and_compensation() {
        let values: Vec<_> = (0..10003)
            .map(|i| match i % 3 {
                0 => 1e16,
                1 => 1.0,
                _ => -1e16,
            })
            .collect();
        let a = Array::floats(vec![values.len()], &values).unwrap();
        let b = Array::floats(vec![values.len()], &vec![1.0; values.len()]).unwrap();
        for left in [
            a.clone(),
            a.slice(0, values.len() - 1, values.len(), -1).unwrap(),
        ] {
            let mut state = Progress::initial();
            let mut chunks = 0;
            while state.cursor < left.len() {
                let old = state.cursor;
                state = left.dot_step(&b, state).unwrap();
                assert!(state.cursor - old <= COOPERATIVE_MACS);
                chunks += 1;
            }
            assert_eq!(
                (state.sum + state.correction).to_bits(),
                left.dot(&b).unwrap().to_bits()
            );
            assert_eq!(chunks, 3);
        }
    }
    #[test]
    fn matmul_steps_cross_cells_pages_and_keep_snapshots() {
        for (m, k, n) in [(17, 3, 263), (3, 5001, 5), (4, 0, 9), (0, 3, 5)] {
            let a = Array::floats(
                vec![m, k],
                &(0..m * k).map(|i| (i % 7) as f64 - 3.0).collect::<Vec<_>>(),
            )
            .unwrap();
            let b = Array::floats(
                vec![k, n],
                &(0..k * n).map(|i| (i % 9) as f64 - 4.0).collect::<Vec<_>>(),
            )
            .unwrap();
            let original = Array::zeros(DType::Float64, vec![m, n]).unwrap();
            let mut out = original.clone();
            let mut state = Progress::initial();
            while state.cursor < m * k * n {
                let old = out.clone();
                let old_values = old.float_values().unwrap();
                let next = a.matmul_step(&b, &out, state).unwrap();
                out = next.0;
                state = next.1;
                assert_eq!(old.float_values().unwrap(), old_values);
            }
            assert_eq!(out, a.matmul(&b).unwrap());
            assert!(original.float_values().unwrap().iter().all(|v| *v == 0.0));
        }
    }
    #[test]
    fn matmul_steps_support_transposed_reversed_and_broadcast_views() {
        let base = Array::floats(
            vec![5001, 3],
            &(0..15003)
                .map(|i| (i % 11) as f64 - 5.0)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let left = base
            .transpose(&[1, 0])
            .unwrap()
            .slice(1, 5000, 5001, -1)
            .unwrap();
        let row = Array::floats(vec![1, 5], &[1e12, -1e12, 1.0, 0.25, -0.5]).unwrap();
        let right = row.broadcast(vec![5001, 5]).unwrap();
        let mut out = Array::zeros(DType::Float64, vec![3, 5]).unwrap();
        let mut progress = Progress::initial();
        while progress.cursor < 3 * 5001 * 5 {
            let next = left.matmul_step(&right, &out, progress).unwrap();
            out = next.0;
            progress = next.1;
        }
        assert_eq!(out, left.matmul(&right).unwrap());
        assert_eq!(left.float(&[0, 0]), base.float(&[5000, 0]));
    }
    #[test]
    fn invalid_progress_and_nonfinite_leave_output_unchanged() {
        let a = Array::floats(vec![1, 1], &[f64::MAX]).unwrap();
        let b = Array::floats(vec![1, 1], &[2.0]).unwrap();
        let out = Array::zeros(DType::Float64, vec![1, 1]).unwrap();
        assert_eq!(
            a.matmul_step(&b, &out, Progress::initial()).err(),
            Some(Error::NonFinite)
        );
        assert_eq!(out.float(&[0, 0]), Ok(0.0));
        assert_eq!(
            a.matmul_step(
                &b,
                &out,
                Progress {
                    cursor: 2,
                    ..Progress::initial()
                }
            )
            .err(),
            Some(Error::Index)
        );
    }
}
