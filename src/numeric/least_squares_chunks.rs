//! Full-column-rank least squares with bounded validation, QR and substitution.
use super::*;
pub const LEAST_SQUARES_CHUNK: usize = 4096;

#[derive(Clone)]
pub struct LeastSquaresWork {
    pub qr: QrWork,
    pub right: Array,
    pub projected: Array,
    pub output: Array,
    // Bit encoding permits immutable, serializable progress even when an
    // intermediate compensated sum overflows. The synchronous routine checks
    // finite products first and checks the completed sum at the column boundary.
    pub tolerance_bits: u64,
    pub phase: u8,
    pub cursor: usize,
    pub column: usize,
    pub row: usize,
    pub sum: f64,
    pub correction: f64,
}
impl LeastSquaresWork {
    pub fn new(input: &Array, right: &Array, tolerance: f64) -> Result<Self> {
        if input.dtype != DType::Float64 || right.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let [m, n] = input.shape.as_slice() else {
            return Err(Error::Shape);
        };
        if m < n || right.shape != [*m] {
            return Err(Error::Shape);
        }
        Ok(Self {
            // Validate the complete rhs before applying the requested QR
            // tolerance, preserving least_squares' error ordering.
            qr: QrWork::new(input, 0.0)?,
            right: right.clone(),
            projected: Array::zeros(DType::Float64, vec![*n])?,
            output: Array::zeros(DType::Float64, vec![*n])?,
            tolerance_bits: tolerance.to_bits(),
            phase: 0,
            cursor: 0,
            column: 0,
            row: 0,
            sum: 0.0,
            correction: 0.0,
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.qr.validate()?;
        let [m, n] = self.qr.input.shape.as_slice() else {
            return Err(Error::Shape);
        };
        if m < n || self.right.dtype != DType::Float64 || self.right.shape != [*m] {
            return Err(Error::Shape);
        }
        for a in [&self.projected, &self.output] {
            if a.dtype != DType::Float64 {
                return Err(Error::Type);
            }
            if a.shape != [*n] {
                return Err(Error::Shape);
            }
            if !a.contiguous() || !a.writable || a.offset != 0 {
                return Err(Error::ReadOnly);
            }
        }
        if self.phase >= 2 {
            if !self.qr.done() || self.qr.rank != *n {
                return Err(Error::Domain);
            }
            if self.qr.tolerance.to_bits() != self.tolerance_bits {
                return Err(Error::Domain);
            }
        }
        let valid = match self.phase {
            0 => self.cursor <= *m && self.column == 0 && self.row == 0,
            1 => self.cursor == 0 && self.column == 0 && self.row == 0,
            2 => self.column <= *n && self.cursor <= *m && self.row == 0,
            3 => self.column == *n && self.row <= *n && self.cursor <= *n,
            4 => self.column == *n && self.row < *n && self.cursor > self.row && self.cursor <= *n,
            5 => self.column == *n && self.row == 0 && self.cursor <= *n,
            6 => self.column == *n && self.row == 0 && self.cursor == *n,
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Domain)
        }
    }
    pub fn done(&self) -> bool {
        self.phase == 6
    }
    pub fn result(&self) -> Result<Array> {
        self.validate()?;
        if !self.done() {
            return Err(Error::Domain);
        }
        Ok(self.output.clone())
    }
    fn get(a: &Array, index: usize) -> f64 {
        f64::from_bits(a.buffer.get(a.flat_index(index)))
    }
    fn put(a: &mut Array, index: usize, value: f64) {
        Node::write_range(
            &mut a.buffer.root,
            a.buffer.height,
            index,
            &[value.to_bits()],
        );
    }
    fn finite(value: f64) -> Result<f64> {
        if value.is_finite() {
            Ok(value)
        } else {
            Err(Error::Overflow)
        }
    }
    fn accumulate(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Err(Error::NonFinite);
        }
        let next = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        self.sum = next;
        Ok(())
    }
    pub fn step(&self) -> Result<Self> {
        self.validate()?;
        let m = self.right.len();
        let n = self.output.len();
        let mut w = self.clone();
        if w.phase == 0 {
            let end = (w.cursor + LEAST_SQUARES_CHUNK).min(m);
            for bits in w.right.window_bits(w.cursor, end)? {
                if !f64::from_bits(bits).is_finite() {
                    return Err(Error::NonFinite);
                }
            }
            w.cursor = end;
            if end == m {
                w.phase = 1;
                w.cursor = 0;
            }
            return Ok(w);
        }
        if w.phase == 1 {
            let tolerance = f64::from_bits(w.tolerance_bits);
            if !tolerance.is_finite() || tolerance < 0.0 {
                return Err(Error::Domain);
            }
            w.qr.tolerance = tolerance;
            if !w.qr.done() {
                // A QR step already consumes its entire independent chunk.
                w.qr = w.qr.step()?;
            } else {
                if w.qr.rank < n {
                    return Err(Error::Singular);
                }
                w.phase = 2;
            }
            return Ok(w);
        }
        let mut used = 0;
        while used < LEAST_SQUARES_CHUNK && !w.done() {
            used += 1;
            match w.phase {
                2 => {
                    if w.column == n {
                        w.phase = 3;
                        w.row = n;
                        w.cursor = 0;
                    } else if w.cursor < m {
                        let value = Self::get(&w.qr.q, w.cursor * n + w.column)
                            * Self::get(&w.right, w.cursor);
                        w.accumulate(value)?;
                        w.cursor += 1;
                    } else {
                        let value = Self::finite(w.sum + w.correction)?;
                        Self::put(&mut w.projected, w.column, value);
                        w.column += 1;
                        w.cursor = 0;
                        w.sum = 0.0;
                        w.correction = 0.0;
                    }
                }
                3 => {
                    if w.row == 0 {
                        w.phase = 5;
                        w.cursor = 0;
                    } else {
                        w.row -= 1;
                        w.cursor = w.row + 1;
                        w.sum = 0.0;
                        w.correction = 0.0;
                        w.phase = 4;
                    }
                }
                4 => {
                    if w.cursor < n {
                        let value = Self::get(&w.qr.r, w.row * n + w.cursor)
                            * Self::get(&w.projected, w.cursor);
                        w.accumulate(value)?;
                        w.cursor += 1;
                    } else {
                        let tail = Self::finite(w.sum + w.correction)?;
                        let value = Self::finite(
                            (Self::get(&w.projected, w.row) - tail)
                                / Self::get(&w.qr.r, w.row * n + w.row),
                        )?;
                        Self::put(&mut w.projected, w.row, value);
                        w.phase = 3;
                    }
                }
                5 => {
                    if w.cursor == n {
                        w.phase = 6;
                    } else {
                        let original = w.qr.permutation.buffer.get(w.cursor) as usize;
                        if original >= n {
                            return Err(Error::Domain);
                        }
                        let value = Self::get(&w.projected, w.cursor);
                        Self::put(&mut w.output, original, value);
                        w.cursor += 1;
                    }
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
    fn finish(mut w: LeastSquaresWork) -> Result<Array> {
        let mut steps = 0;
        while !w.done() {
            w = w.step()?;
            steps += 1;
            assert!(steps < 100000);
        }
        w.result()
    }
    fn bits(a: &Array) -> Vec<u64> {
        a.float_values()
            .unwrap()
            .iter()
            .map(|v| v.to_bits())
            .collect()
    }
    #[test]
    fn rectangular_pivots_empty_shapes_and_views_match_synchronous_bits() {
        for (m, n) in [(0, 0), (5, 0), (7, 1), (19, 7), (71, 17)] {
            let values = (0..m * n)
                .map(|i| {
                    let (row, col) = (i / n, i % n);
                    if row == col {
                        20.0 * (col + 1) as f64
                    } else {
                        ((row * 7 + col * 11) % 19) as f64 * 0.03 - 0.2
                    }
                })
                .collect::<Vec<_>>();
            let a = Array::floats(vec![m, n], &values).unwrap();
            let b = Array::floats(
                vec![m],
                &(0..m).map(|i| i as f64 * 0.1 - 0.3).collect::<Vec<_>>(),
            )
            .unwrap();
            for (a, b) in [
                (a.clone(), b.clone()),
                (
                    a.slice(0, m.saturating_sub(1), m, -1).unwrap(),
                    b.slice(0, m.saturating_sub(1), m, -1).unwrap(),
                ),
            ] {
                assert_eq!(
                    bits(&finish(LeastSquaresWork::new(&a, &b, 1e-13).unwrap()).unwrap()),
                    bits(&a.least_squares(&b, 1e-13).unwrap())
                );
            }
        }
    }
    #[test]
    fn independent_solution_and_residual_are_preserved_across_projection_checkpoints() {
        let (m, n) = (257, 17);
        let known = (0..n).map(|i| i as f64 * 0.25 - 2.0).collect::<Vec<_>>();
        let values = (0..m * n)
            .map(|i| {
                let (row, col) = (i / n, i % n);
                if row == col {
                    50.0
                } else {
                    ((row * 13 + col * 7) % 31) as f64 * 0.02 - 0.3
                }
            })
            .collect::<Vec<_>>();
        let rhs = (0..m)
            .map(|row| (0..n).map(|col| values[row * n + col] * known[col]).sum())
            .collect::<Vec<_>>();
        let a = Array::floats(vec![m, n], &values).unwrap();
        let b = Array::floats(vec![m], &rhs).unwrap();
        let mut w = LeastSquaresWork::new(&a, &b, 1e-13).unwrap();
        while w.phase != 2 {
            w = w.step().unwrap();
        }
        let checkpoint = w.step().unwrap();
        assert_eq!(checkpoint.phase, 2);
        assert!(checkpoint.cursor > 0);
        let saved = bits(&checkpoint.projected);
        let first = finish(checkpoint.clone()).unwrap();
        let second = finish(checkpoint.clone()).unwrap();
        assert_eq!(bits(&first), bits(&second));
        assert_eq!(bits(&checkpoint.projected), saved);
        let x = first.float_values().unwrap();
        for i in 0..n {
            assert!((x[i] - known[i]).abs() < 1e-12);
        }
        for row in 0..m {
            let predicted = (0..n)
                .map(|col| values[row * n + col] * x[col])
                .sum::<f64>();
            assert!((predicted - rhs[row]).abs() < 1e-10);
        }
    }
    #[test]
    fn validation_errors_follow_rhs_then_qr_order_and_rank_failure_is_typed() {
        let a = Array::floats(vec![1, 1], &[1.0]).unwrap();
        let nan = Array::floats(vec![1], &[f64::NAN]).unwrap();
        assert!(matches!(
            finish(LeastSquaresWork::new(&a, &nan, -1.0).unwrap()),
            Err(Error::NonFinite)
        ));
        let b = Array::floats(vec![1], &[1.0]).unwrap();
        assert!(matches!(
            finish(LeastSquaresWork::new(&a, &b, -1.0).unwrap()),
            Err(Error::Domain)
        ));
        let singular = Array::zeros(DType::Float64, vec![3, 2]).unwrap();
        let right = Array::zeros(DType::Float64, vec![3]).unwrap();
        assert!(matches!(
            finish(LeastSquaresWork::new(&singular, &right, 0.0).unwrap()),
            Err(Error::Singular)
        ));
        let wide = Array::zeros(DType::Float64, vec![2, 3]).unwrap();
        assert!(matches!(
            LeastSquaresWork::new(&wide, &right, 0.0),
            Err(Error::Shape)
        ));
    }
    #[test]
    fn large_shared_inputs_validate_without_materializing_rhs_or_changing_old_work() {
        let a = Array::zeros(DType::Float64, vec![1_048_576, 1]).unwrap();
        let b = Array::zeros(DType::Float64, vec![1_048_576]).unwrap();
        let w = LeastSquaresWork::new(&a, &b, 1e-13).unwrap();
        let before = w.right.buffer.root.digest();
        let next = w.step().unwrap();
        assert_eq!(next.phase, 0);
        assert_eq!(next.cursor, LEAST_SQUARES_CHUNK);
        assert_eq!(w.cursor, 0);
        assert!(Arc::ptr_eq(&next.right.buffer.root, &w.right.buffer.root));
        assert_eq!(before, w.right.buffer.root.digest());
        assert!(Arc::ptr_eq(
            &next.qr.matrix.buffer.root,
            &w.qr.matrix.buffer.root
        ));
    }
    #[test]
    fn compensated_projection_overflow_can_span_an_immutable_step() {
        let a = Array::floats(vec![5000, 1], &vec![1.0; 5000]).unwrap();
        let b = Array::floats(vec![5000], &vec![1e308; 5000]).unwrap();
        assert!(matches!(a.least_squares(&b, 0.0), Err(Error::Overflow)));
        let mut w = LeastSquaresWork::new(&a, &b, 0.0).unwrap();
        while w.phase < 2 {
            w = w.step().unwrap();
        }
        let old = w.clone();
        w = w.step().unwrap();
        assert_eq!(w.phase, 2);
        assert!(w.cursor > 0);
        assert!(!w.sum.is_finite() || !w.correction.is_finite());
        w.validate().unwrap();
        assert_eq!(old.cursor, 0);
        assert_eq!(old.sum, 0.0);
        assert!(matches!(finish(w.clone()), Err(Error::Overflow)));
        assert!(matches!(finish(w), Err(Error::Overflow)));
    }
}
