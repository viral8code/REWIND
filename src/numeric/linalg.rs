//! Bounded dense decompositions. The VM caller reserves output, scratch and work.
use super::{Array, DType, Error, Result};
#[derive(Clone, Debug)]
pub struct Qr {
    pub q: Array,
    pub r: Array,
    pub permutation: Vec<i64>,
    pub rank: usize,
}
#[derive(Clone, Debug)]
pub struct Eigen {
    pub values: Array,
    pub vectors: Array,
    pub sweeps: usize,
}
fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::Overflow)
    }
}
fn compensated(values: impl Iterator<Item = f64>) -> Result<f64> {
    let mut sum = 0.0f64;
    let mut correction = 0.0;
    for value in values {
        if !value.is_finite() {
            return Err(Error::NonFinite);
        }
        let next = sum + value;
        correction += if sum.abs() >= value.abs() {
            (sum - next) + value
        } else {
            (value - next) + sum
        };
        sum = next;
    }
    finite(sum + correction)
}
impl Array {
    /// Column-pivoted Householder QR: A[:, permutation] = Q R, with an economy Q.
    pub fn qr(&self, tolerance: f64) -> Result<Qr> {
        if self.dtype() != DType::Float64 {
            return Err(Error::Type);
        }
        let [m, n] = self.shape() else {
            return Err(Error::Shape);
        };
        let (m, n) = (*m, *n);
        let p = m.min(n);
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(Error::Domain);
        }
        let mut a = self.float_values()?;
        if a.iter().any(|v| !v.is_finite()) {
            return Err(Error::NonFinite);
        }
        let scale = a.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
        if scale > 0.0 {
            for value in &mut a {
                *value /= scale;
            }
        }
        let norm = |a: &[f64], column: usize, start: usize| {
            (start..m).fold(0.0f64, |sum, row| sum.hypot(a[row * n + column]))
        };
        let original_norm = (0..n)
            .map(|column| norm(&a, column, 0))
            .fold(0.0f64, f64::max);
        let mut permutation = (0..n).map(|i| i as i64).collect::<Vec<_>>();
        let mut rank = 0usize;
        let mut reflectors = Vec::with_capacity(p);
        for k in 0..p {
            let mut pivot = k;
            let mut pivot_norm = norm(&a, k, k);
            for column in k + 1..n {
                let candidate = norm(&a, column, k);
                if candidate > pivot_norm {
                    pivot = column;
                    pivot_norm = candidate;
                }
            }
            if pivot != k {
                for row in 0..m {
                    a.swap(row * n + k, row * n + pivot);
                }
                permutation.swap(k, pivot);
            }
            if pivot_norm > tolerance * original_norm {
                rank += 1;
            }
            if pivot_norm == 0.0 {
                reflectors.push(Vec::new());
                continue;
            }
            let sign = if a[k * n + k] < 0.0 { -1.0 } else { 1.0 };
            let mut v = (k..m)
                .map(|row| a[row * n + k] / pivot_norm)
                .collect::<Vec<_>>();
            v[0] += sign;
            let v_norm = v.iter().fold(0.0f64, |s, &v| s.hypot(v));
            for value in &mut v {
                *value /= v_norm;
            }
            for column in k..n {
                let dot = compensated((k..m).map(|row| v[row - k] * a[row * n + column]))?;
                for row in k..m {
                    a[row * n + column] = finite(a[row * n + column] - (2.0 * v[row - k]) * dot)?;
                }
            }
            a[k * n + k] = -sign * pivot_norm;
            for row in k + 1..m {
                a[row * n + k] = 0.0;
            }
            reflectors.push(v);
        }
        let mut q = vec![0.0; m * p];
        for column in 0..p {
            q[column * p + column] = 1.0;
        }
        for k in (0..p).rev() {
            let v = &reflectors[k];
            if v.is_empty() {
                continue;
            }
            for column in 0..p {
                let dot = compensated((k..m).map(|row| v[row - k] * q[row * p + column]))?;
                for row in k..m {
                    q[row * p + column] = finite(q[row * p + column] - (2.0 * v[row - k]) * dot)?;
                }
            }
        }
        let mut r = vec![0.0; p * n];
        for row in 0..p {
            for column in row..n {
                r[row * n + column] = finite(a[row * n + column] * scale)?;
            }
        }
        Ok(Qr {
            q: Self::floats(vec![m, p], &q)?,
            r: Self::floats(vec![p, n], &r)?,
            permutation,
            rank,
        })
    }
    /// Overdetermined full-column-rank least squares. Rank deficiency is explicit.
    pub fn least_squares(&self, right: &Self, tolerance: f64) -> Result<Self> {
        if self.dtype() != DType::Float64 || right.dtype() != DType::Float64 {
            return Err(Error::Type);
        }
        let [m, n] = self.shape() else {
            return Err(Error::Shape);
        };
        let (m, n) = (*m, *n);
        if m < n || right.shape() != [m] {
            return Err(Error::Shape);
        }
        let b = right.float_values()?;
        if b.iter().any(|v| !v.is_finite()) {
            return Err(Error::NonFinite);
        }
        let decomposition = self.qr(tolerance)?;
        if decomposition.rank < n {
            return Err(Error::Singular);
        }
        let q = decomposition.q.float_values()?;
        let r = decomposition.r.float_values()?;
        let mut y = vec![0.0; n];
        for column in 0..n {
            y[column] = compensated((0..m).map(|row| q[row * n + column] * b[row]))?;
        }
        for row in (0..n).rev() {
            let tail = compensated((row + 1..n).map(|column| r[row * n + column] * y[column]))?;
            y[row] = finite((y[row] - tail) / r[row * n + row])?;
        }
        let mut result = vec![0.0; n];
        for (column, &original) in decomposition.permutation.iter().enumerate() {
            result[original as usize] = y[column];
        }
        Self::floats(vec![n], &result)
    }
    /// Cyclic Jacobi decomposition of a finite symmetric matrix. Eigenvectors are columns.
    pub fn eigen_symmetric(&self, tolerance: f64, max_sweeps: usize) -> Result<Eigen> {
        if self.dtype() != DType::Float64 {
            return Err(Error::Type);
        }
        let [n, m] = self.shape() else {
            return Err(Error::Shape);
        };
        let n = *n;
        if n != *m {
            return Err(Error::Shape);
        }
        if !tolerance.is_finite() || tolerance < 0.0 || max_sweeps > 10_000 {
            return Err(Error::Domain);
        }
        let mut a = self.float_values()?;
        if a.iter().any(|v| !v.is_finite()) {
            return Err(Error::NonFinite);
        }
        let scale = a.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
        if scale > 0.0 {
            for value in &mut a {
                *value /= scale;
            }
        }
        for i in 0..n {
            for j in i + 1..n {
                if (a[i * n + j] - a[j * n + i]).abs() > tolerance {
                    return Err(Error::Domain);
                }
                let value = 0.5 * a[i * n + j] + 0.5 * a[j * n + i];
                a[i * n + j] = value;
                a[j * n + i] = value;
            }
        }
        let mut vectors = vec![0.0; n * n];
        for i in 0..n {
            vectors[i * n + i] = 1.0;
        }
        let mut sweeps = 0;
        loop {
            let mut largest = 0.0f64;
            for i in 0..n {
                for j in i + 1..n {
                    largest = largest.max(a[i * n + j].abs());
                }
            }
            if largest <= tolerance {
                break;
            }
            if sweeps == max_sweeps {
                return Err(Error::Convergence);
            }
            for p in 0..n {
                for q in p + 1..n {
                    let cross = a[p * n + q];
                    if cross.abs() <= tolerance {
                        continue;
                    }
                    let difference = a[q * n + q] - a[p * n + p];
                    let twice = 2.0 * cross;
                    let denominator = difference + difference.hypot(twice).copysign(difference);
                    let t = twice / denominator;
                    let c = 1.0 / (1.0 + t * t).sqrt();
                    let s = t * c;
                    a[p * n + p] = finite(a[p * n + p] - t * cross)?;
                    a[q * n + q] = finite(a[q * n + q] + t * cross)?;
                    a[p * n + q] = 0.0;
                    a[q * n + p] = 0.0;
                    for row in 0..n {
                        if row != p && row != q {
                            let x = a[row * n + p];
                            let y = a[row * n + q];
                            let xp = c * x - s * y;
                            let yq = s * x + c * y;
                            a[row * n + p] = xp;
                            a[p * n + row] = xp;
                            a[row * n + q] = yq;
                            a[q * n + row] = yq;
                        }
                        let x = vectors[row * n + p];
                        let y = vectors[row * n + q];
                        vectors[row * n + p] = c * x - s * y;
                        vectors[row * n + q] = s * x + c * y;
                    }
                }
            }
            sweeps += 1;
        }
        let mut order = (0..n).collect::<Vec<_>>();
        order.sort_by(|&i, &j| a[i * n + i].total_cmp(&a[j * n + j]));
        let values = order
            .iter()
            .map(|&i| finite(a[i * n + i] * scale))
            .collect::<Result<Vec<_>>>()?;
        let mut sorted = vec![0.0; n * n];
        for row in 0..n {
            for (column, &old) in order.iter().enumerate() {
                sorted[row * n + column] = vectors[row * n + old];
            }
        }
        Ok(Eigen {
            values: Self::floats(vec![n], &values)?,
            vectors: Self::floats(vec![n, n], &sorted)?,
            sweeps,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-10 * (1.0 + b.abs()), "{a} != {b}");
    }
    #[test]
    fn pivoted_qr_reconstructs_rectangular_inputs_and_has_orthonormal_columns() {
        for (shape, values) in [
            (vec![3, 2], vec![1.0, 10.0, 2.0, 1.0, 3.0, 5.0]),
            (vec![2, 3], vec![1.0, 10.0, 2.0, 1.0, 3.0, 5.0]),
        ] {
            let a = Array::floats(shape.clone(), &values).unwrap();
            let qr = a.qr(1e-12).unwrap();
            let reconstructed = qr.q.matmul(&qr.r).unwrap();
            for row in 0..shape[0] {
                for column in 0..shape[1] {
                    close(
                        reconstructed.float(&[row, column]).unwrap(),
                        a.float(&[row, qr.permutation[column] as usize]).unwrap(),
                    );
                }
            }
            let identity = qr.q.transpose(&[1, 0]).unwrap().matmul(&qr.q).unwrap();
            for i in 0..identity.shape()[0] {
                for j in 0..identity.shape()[1] {
                    close(
                        identity.float(&[i, j]).unwrap(),
                        if i == j { 1.0 } else { 0.0 },
                    );
                }
            }
        }
    }
    #[test]
    fn least_squares_preserves_pivot_permutation_and_reports_rank_deficiency() {
        let a = Array::floats(vec![4, 2], &[1.0, 0.0, 1.0, 1.0, 1.0, 2.0, 1.0, 3.0]).unwrap();
        let b = Array::floats(vec![4], &[1.0, 3.0, 5.0, 7.0]).unwrap();
        let x = a.least_squares(&b, 1e-12).unwrap();
        close(x.float(&[0]).unwrap(), 1.0);
        close(x.float(&[1]).unwrap(), 2.0);
        let a = Array::floats(vec![4, 2], &[1.0, 2.0, 2.0, 4.0, 3.0, 6.0, 4.0, 8.0]).unwrap();
        assert_eq!(a.qr(1e-12).unwrap().rank, 1);
        assert_eq!(a.least_squares(&b, 1e-12), Err(Error::Singular));
    }
    #[test]
    fn eigen_residual_orthogonality_ordering_and_convergence_errors() {
        let a = Array::floats(vec![3, 3], &[4.0, 1.0, 2.0, 1.0, 3.0, 0.0, 2.0, 0.0, 2.0]).unwrap();
        let eigen = a.eigen_symmetric(1e-13, 40).unwrap();
        let product = a.matmul(&eigen.vectors).unwrap();
        let identity = eigen
            .vectors
            .transpose(&[1, 0])
            .unwrap()
            .matmul(&eigen.vectors)
            .unwrap();
        for row in 0..3 {
            for column in 0..3 {
                close(
                    product.float(&[row, column]).unwrap(),
                    eigen.vectors.float(&[row, column]).unwrap()
                        * eigen.values.float(&[column]).unwrap(),
                );
                close(
                    identity.float(&[row, column]).unwrap(),
                    if row == column { 1.0 } else { 0.0 },
                );
            }
        }
        assert!(eigen
            .values
            .float_values()
            .unwrap()
            .windows(2)
            .all(|v| v[0] <= v[1]));
        assert!(matches!(
            a.eigen_symmetric(1e-13, 0),
            Err(Error::Convergence)
        ));
        assert!(matches!(
            Array::floats(vec![2, 2], &[1.0, 2.0, 0.0, 1.0])
                .unwrap()
                .eigen_symmetric(1e-13, 20),
            Err(Error::Domain)
        ));
    }
    #[test]
    fn decompositions_handle_empty_zero_and_scaled_inputs() {
        let a = Array::zeros(DType::Float64, vec![0, 0]).unwrap();
        assert_eq!(a.qr(1e-12).unwrap().rank, 0);
        assert_eq!(a.eigen_symmetric(1e-12, 0).unwrap().values.len(), 0);
        for scale in [1e-150, 1e150] {
            let a = Array::floats(vec![2, 2], &[2.0 * scale, scale, scale, 2.0 * scale]).unwrap();
            let e = a.eigen_symmetric(1e-13, 20).unwrap();
            close(e.values.float(&[0]).unwrap() / scale, 1.0);
            close(e.values.float(&[1]).unwrap() / scale, 3.0);
            let q = a.qr(1e-12).unwrap();
            assert_eq!(q.rank, 2);
        }
    }
}
