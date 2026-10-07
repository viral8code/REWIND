//! Partial-pivot LU in bounded pure steps, including input copy and row swaps.
use super::*;
pub const SOLVE_CHUNK: usize = 4096;

#[derive(Clone)]
pub struct SolveWork {
    pub input: Array,
    pub right: Array,
    pub matrix: Array,
    pub rhs: Array,
    pub output: Array,
    pub tolerance: f64,
    pub scale: f64,
    pub phase: u8,
    pub cursor: usize,
    pub col: usize,
    pub row: usize,
    pub pivot: usize,
    pub factor: f64,
    pub value: f64,
}
impl SolveWork {
    pub fn new(input: &Array, right: &Array, tolerance: f64) -> Result<Self> {
        if input.dtype != DType::Float64 || right.dtype != input.dtype {
            return Err(Error::Type);
        }
        let [n, m] = input.shape.as_slice() else {
            return Err(Error::Shape);
        };
        if n != m || right.shape != [*n] || !tolerance.is_finite() || tolerance < 0.0 {
            return Err(Error::Shape);
        }
        Ok(Self {
            input: input.clone(),
            right: right.clone(),
            matrix: Array::zeros(DType::Float64, vec![*n, *n])?,
            rhs: Array::zeros(DType::Float64, vec![*n])?,
            output: Array::zeros(DType::Float64, vec![*n])?,
            tolerance,
            scale: 0.0,
            phase: if *n == 0 { 8 } else { 0 },
            cursor: 0,
            col: 0,
            row: 0,
            pivot: 0,
            factor: 0.0,
            value: 0.0,
        })
    }
    pub fn validate(&self) -> Result<()> {
        if self.input.dtype != DType::Float64 || self.right.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let [n, m] = self.input.shape.as_slice() else {
            return Err(Error::Shape);
        };
        if n != m || self.right.shape != [*n] || !self.tolerance.is_finite() || self.tolerance < 0.0
        {
            return Err(Error::Shape);
        }
        count(&[*n, *n])?;
        for (array, shape) in [
            (&self.matrix, vec![*n, *n]),
            (&self.rhs, vec![*n]),
            (&self.output, vec![*n]),
        ] {
            if array.dtype != DType::Float64 {
                return Err(Error::Type);
            }
            if array.shape != shape {
                return Err(Error::Shape);
            }
            if !array.contiguous() || !array.writable || array.offset != 0 {
                return Err(Error::ReadOnly);
            }
        }
        if !self.scale.is_finite() || self.scale < 0.0 {
            return Err(Error::NonFinite);
        }
        if self.phase > 8 || self.col > *n || self.row > *n || self.pivot > *n {
            return Err(Error::Domain);
        }
        let valid = match self.phase {
            0 => *n > 0 && self.cursor <= n * n && self.col == 0,
            1 => *n > 0 && self.cursor <= *n && self.col == 0,
            2 => {
                self.col < *n
                    && self.row > self.col
                    && self.row <= *n
                    && self.pivot >= self.col
                    && self.pivot < self.row
            }
            3 => self.col < *n && self.cursor <= *n && self.pivot >= self.col && self.pivot < *n,
            4 => self.col < *n && self.row > self.col && self.row <= *n,
            5 => {
                self.col < *n
                    && self.row > self.col
                    && self.row < *n
                    && self.cursor > self.col
                    && self.cursor <= *n
            }
            6 => self.col == *n && self.row <= *n,
            7 => self.col == *n && self.row < *n && self.cursor > self.row && self.cursor <= *n,
            8 => self.col == *n && self.row == 0,
            _ => false,
        };
        if !valid {
            return Err(Error::Domain);
        }
        // Elimination intermediates may overflow. Preserve synchronous solve's
        // arithmetic/error order; it checks representable back-substitution results.
        Ok(())
    }
    pub fn done(&self) -> bool {
        self.phase == 8
    }
    pub fn result(&self) -> Result<Array> {
        self.validate()?;
        if !self.done() {
            return Err(Error::Domain);
        }
        Ok(self.output.clone())
    }
    fn get(array: &Array, index: usize) -> f64 {
        f64::from_bits(array.buffer.get(index))
    }
    fn write(array: &mut Array, index: usize, bits: &[u64]) {
        let height = array.buffer.height;
        Node::write_range(&mut array.buffer.root, height, index, bits);
    }
    pub fn step(&self) -> Result<Self> {
        self.validate()?;
        let mut next = self.clone();
        let n = self.right.len();
        let mut used = 0;
        while used < SOLVE_CHUNK && !next.done() {
            used += 1;
            let remaining = SOLVE_CHUNK - used + 1;
            match next.phase {
                0 => {
                    if next.cursor == n * n {
                        next.phase = 1;
                        next.cursor = 0;
                        continue;
                    }
                    let end = (next.cursor + remaining).min(n * n);
                    let mut copied = Vec::with_capacity(end - next.cursor);
                    for bits in next.input.window_bits(next.cursor, end)? {
                        let value = f64::from_bits(bits);
                        if !value.is_finite() {
                            return Err(Error::NonFinite);
                        }
                        next.scale = next.scale.max(value.abs());
                        copied.push(bits);
                    }
                    Self::write(&mut next.matrix, next.cursor, &copied);
                    used += copied.len() - 1;
                    next.cursor = end;
                }
                1 => {
                    if next.cursor == n {
                        next.phase = 2;
                        next.cursor = 0;
                        next.row = 1;
                        continue;
                    }
                    let end = (next.cursor + remaining).min(n);
                    let copied: Vec<_> = next.right.window_bits(next.cursor, end)?.collect();
                    if copied.iter().any(|b| !f64::from_bits(*b).is_finite()) {
                        return Err(Error::NonFinite);
                    }
                    Self::write(&mut next.rhs, next.cursor, &copied);
                    used += copied.len() - 1;
                    next.cursor = end;
                }
                2 => {
                    if next.row < n {
                        if Self::get(&next.matrix, next.row * n + next.col).abs()
                            > Self::get(&next.matrix, next.pivot * n + next.col).abs()
                        {
                            next.pivot = next.row;
                        }
                        next.row += 1;
                    } else {
                        if Self::get(&next.matrix, next.pivot * n + next.col).abs()
                            <= next.tolerance * next.scale
                        {
                            return Err(Error::Singular);
                        }
                        next.phase = 3;
                        next.cursor = 0;
                    }
                }
                3 => {
                    if next.pivot != next.col && next.cursor < n {
                        let end = (next.cursor + remaining).min(n);
                        let a: Vec<_> = next
                            .matrix
                            .window_bits(next.col * n + next.cursor, next.col * n + end)?
                            .collect();
                        let b: Vec<_> = next
                            .matrix
                            .window_bits(next.pivot * n + next.cursor, next.pivot * n + end)?
                            .collect();
                        Self::write(&mut next.matrix, next.col * n + next.cursor, &b);
                        Self::write(&mut next.matrix, next.pivot * n + next.cursor, &a);
                        used += end - next.cursor - 1;
                        next.cursor = end;
                    } else {
                        if next.pivot != next.col {
                            let a = next.rhs.buffer.get(next.col);
                            let b = next.rhs.buffer.get(next.pivot);
                            Self::write(&mut next.rhs, next.col, &[b]);
                            Self::write(&mut next.rhs, next.pivot, &[a]);
                        }
                        next.phase = 4;
                        next.row = next.col + 1;
                        next.cursor = 0;
                    }
                }
                4 => {
                    if next.row == n {
                        next.col += 1;
                        next.cursor = 0;
                        if next.col == n {
                            next.phase = 6;
                            next.row = n;
                        } else {
                            next.phase = 2;
                            next.pivot = next.col;
                            next.row = next.col + 1;
                        }
                    } else {
                        next.factor = Self::get(&next.matrix, next.row * n + next.col)
                            / Self::get(&next.matrix, next.col * n + next.col);
                        Self::write(
                            &mut next.matrix,
                            next.row * n + next.col,
                            &[0.0f64.to_bits()],
                        );
                        next.cursor = next.col + 1;
                        next.phase = 5;
                    }
                }
                5 => {
                    if next.cursor < n {
                        let end = (next.cursor + remaining).min(n);
                        let mut updated = Vec::with_capacity(end - next.cursor);
                        let row = next
                            .matrix
                            .window_bits(next.row * n + next.cursor, next.row * n + end)?;
                        let pivot = next
                            .matrix
                            .window_bits(next.col * n + next.cursor, next.col * n + end)?;
                        for (left, right) in row.zip(pivot) {
                            let value = f64::from_bits(left) - next.factor * f64::from_bits(right);
                            updated.push(value.to_bits());
                        }
                        Self::write(&mut next.matrix, next.row * n + next.cursor, &updated);
                        used += updated.len() - 1;
                        next.cursor = end;
                    } else {
                        let value = Self::get(&next.rhs, next.row)
                            - next.factor * Self::get(&next.rhs, next.col);
                        Self::write(&mut next.rhs, next.row, &[value.to_bits()]);
                        next.row += 1;
                        next.phase = 4;
                    }
                }
                6 => {
                    if next.row == 0 {
                        next.phase = 8;
                        next.cursor = 0;
                    } else {
                        next.row -= 1;
                        next.value = Self::get(&next.rhs, next.row);
                        next.cursor = next.row + 1;
                        next.phase = 7;
                    }
                }
                7 => {
                    if next.cursor < n {
                        let end = (next.cursor + remaining).min(n);
                        let row = next
                            .matrix
                            .window_bits(next.row * n + next.cursor, next.row * n + end)?;
                        let solution = next.output.window_bits(next.cursor, end)?;
                        for (left, right) in row.zip(solution) {
                            next.value -= f64::from_bits(left) * f64::from_bits(right);
                        }
                        used += end - next.cursor - 1;
                        next.cursor = end;
                    } else {
                        let value = next.value / Self::get(&next.matrix, next.row * n + next.row);
                        if !value.is_finite() {
                            return Err(Error::NonFinite);
                        }
                        Self::write(&mut next.output, next.row, &[value.to_bits()]);
                        next.phase = 6;
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn finish(mut work: SolveWork) -> Result<Array> {
        let mut steps = 0;
        while !work.done() {
            work = work.step()?;
            steps += 1;
            assert!(steps < 100000);
        }
        work.result()
    }
    #[test]
    fn pivot_order_views_empty_and_arithmetic_match_the_synchronous_solver() {
        for n in [0, 1, 2, 17, 65] {
            let mut values = vec![0.0; n * n];
            for i in 0..n {
                for j in 0..n {
                    values[i * n + j] = if i == j {
                        100.0
                    } else {
                        (i as f64 - j as f64) * 0.01
                    };
                }
            }
            if n > 1 {
                for j in 0..n {
                    values.swap(j, (n - 1) * n + j);
                }
            }
            let a = Array::floats(vec![n, n], &values).unwrap();
            let b = Array::floats(vec![n], &(0..n).map(|i| i as f64 + 1.0).collect::<Vec<_>>())
                .unwrap();
            for (left, right) in [
                (a.clone(), b.clone()),
                (
                    a.slice(0, n.saturating_sub(1), n, -1).unwrap(),
                    b.slice(0, n.saturating_sub(1), n, -1).unwrap(),
                ),
            ] {
                let expected = left.solve(&right, 1e-13).unwrap();
                let actual = finish(SolveWork::new(&left, &right, 1e-13).unwrap()).unwrap();
                assert_eq!(
                    actual
                        .float_values()
                        .unwrap()
                        .iter()
                        .map(|f| f.to_bits())
                        .collect::<Vec<_>>(),
                    expected
                        .float_values()
                        .unwrap()
                        .iter()
                        .map(|f| f.to_bits())
                        .collect::<Vec<_>>()
                );
            }
        }
        let a = Array::floats(vec![2, 2], &[0.0, 1.0, 1.0, 1.0]).unwrap();
        let b = Array::floats(vec![2], &[2.0, 3.0]).unwrap();
        assert_eq!(
            finish(SolveWork::new(&a, &b, 0.0).unwrap())
                .unwrap()
                .float_values()
                .unwrap(),
            vec![1.0, 2.0]
        );
    }
    #[test]
    fn checkpointed_private_copy_and_elimination_restore_and_residual_is_small() {
        let n = 97;
        let mut values = vec![0.0; n * n];
        let known: Vec<_> = (0..n).map(|i| (i % 7) as f64 - 3.0).collect();
        let mut b = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                let v = if i == j {
                    200.0
                } else {
                    ((i * 31 + j * 7) % 17) as f64 * 0.01 - 0.08
                };
                values[i * n + j] = v;
                b[i] += v * known[j];
            }
        }
        let a = Array::floats(vec![n, n], &values).unwrap();
        let right = Array::floats(vec![n], &b).unwrap();
        let initial = SolveWork::new(&a, &right, 1e-13).unwrap();
        let halfway = initial.step().unwrap();
        assert_eq!(halfway.cursor, SOLVE_CHUNK);
        assert!(!halfway.done());
        assert!(initial
            .matrix
            .float_values()
            .unwrap()
            .iter()
            .all(|f| *f == 0.0));
        let expected = finish(halfway.clone()).unwrap();
        assert_eq!(expected, finish(halfway).unwrap());
        let actual = expected.float_values().unwrap();
        for i in 0..n {
            assert!((actual[i] - known[i]).abs() < 1e-12);
            let residual = (0..n).map(|j| values[i * n + j] * actual[j]).sum::<f64>() - b[i];
            assert!(residual.abs() < 1e-9);
        }
        let mut work = initial;
        while work.phase < 4 {
            work = work.step().unwrap();
        }
        let retained = work.clone();
        let old = retained.matrix.clone();
        assert_eq!(finish(work).unwrap(), expected);
        assert_eq!(retained.matrix, old);
        assert_eq!(finish(retained).unwrap(), expected);
    }
    #[test]
    fn late_input_failure_singularity_and_invalid_metadata_leave_old_state_unchanged() {
        let n = 65;
        let mut values = vec![0.0; n * n];
        for i in 0..n {
            values[i * n + i] = 1.0;
        }
        values[n * n - 1] = f64::NAN;
        let a = Array::floats(vec![n, n], &values).unwrap();
        let b = Array::zeros(DType::Float64, vec![n]).unwrap();
        let first = SolveWork::new(&a, &b, 0.0).unwrap().step().unwrap();
        let retained = first.matrix.clone();
        assert!(matches!(first.step(), Err(Error::NonFinite)));
        assert_eq!(first.matrix, retained);
        let zero = Array::zeros(DType::Float64, vec![2, 2]).unwrap();
        let rhs = Array::zeros(DType::Float64, vec![2]).unwrap();
        assert!(matches!(
            finish(SolveWork::new(&zero, &rhs, 0.0).unwrap()),
            Err(Error::Singular)
        ));
        for phase in [2, 3, 5, 7, 9] {
            let mut bad = SolveWork::new(&zero, &rhs, 0.0).unwrap();
            bad.phase = phase;
            assert!(bad.step().is_err());
        }
        let mut bad = SolveWork::new(&zero, &rhs, 0.0).unwrap();
        bad.cursor = 5;
        assert!(matches!(bad.step(), Err(Error::Domain)));
        assert!(matches!(
            SolveWork::new(&zero, &rhs, -1.0),
            Err(Error::Shape)
        ));
    }
}
