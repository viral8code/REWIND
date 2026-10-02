//! Deterministic finite-data statistics. Missing and non-finite values are rejected.
use super::{Array, DType, Error, Result};
#[derive(Clone, Debug)]
pub struct Histogram {
    pub counts: Array,
    pub underflow: usize,
    pub overflow: usize,
}
impl Array {
    fn paired_moments(&self, other: &Self, ddof: usize) -> Result<(f64, f64, f64)> {
        if self.dtype() != DType::Float64 || other.dtype() != DType::Float64 {
            return Err(Error::Type);
        }
        if self.shape().len() != 1 || other.shape() != self.shape() {
            return Err(Error::Shape);
        }
        if self.len() <= ddof {
            return Err(Error::Empty);
        }
        let (mut mx, mut my, mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (index, (xb, yb)) in self.bits().zip(other.bits()).enumerate() {
            let x = f64::from_bits(xb);
            let y = f64::from_bits(yb);
            if !x.is_finite() || !y.is_finite() {
                return Err(Error::NonFinite);
            }
            let dx = x - mx;
            let dy = y - my;
            let n = (index + 1) as f64;
            mx += dx / n;
            my += dy / n;
            xx += dx * (x - mx);
            yy += dy * (y - my);
            xy += dx * (y - my);
        }
        let divisor = (self.len() - ddof) as f64;
        let result = (xy / divisor, xx / divisor, yy / divisor);
        if !mx.is_finite()
            || !my.is_finite()
            || !result.0.is_finite()
            || !result.1.is_finite()
            || !result.2.is_finite()
        {
            return Err(Error::Overflow);
        }
        Ok(result)
    }
    pub fn covariance(&self, other: &Self, ddof: usize) -> Result<f64> {
        Ok(self.paired_moments(other, ddof)?.0)
    }
    pub fn correlation(&self, other: &Self) -> Result<f64> {
        let (covariance, x, y) = self.paired_moments(other, 0)?;
        if x <= 0.0 || y <= 0.0 {
            return Err(Error::Domain);
        }
        let result = (covariance / x.sqrt()) / y.sqrt();
        if !result.is_finite() {
            return Err(Error::Overflow);
        }
        Ok(result.clamp(-1.0, 1.0))
    }
    /// Linear interpolation at probability*(len-1), after ordering all logical elements.
    pub fn quantile(&self, probability: f64) -> Result<f64> {
        if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
            return Err(Error::Domain);
        }
        let mut values = self.float_values()?;
        if values.is_empty() {
            return Err(Error::Empty);
        }
        if values.iter().any(|v| !v.is_finite()) {
            return Err(Error::NonFinite);
        }
        values.sort_unstable_by(f64::total_cmp);
        let index = probability * (values.len() - 1) as f64;
        let low = index.floor() as usize;
        let high = (low + 1).min(values.len() - 1);
        let weight = index - low as f64;
        let result = (1.0 - weight) * values[low] + weight * values[high];
        if !result.is_finite() {
            return Err(Error::Overflow);
        }
        Ok(result)
    }
    /// [edge[i],edge[i+1]), with the final right boundary included in the final bin.
    pub fn histogram(&self, edges: &Self) -> Result<Histogram> {
        if self.dtype() != DType::Float64 || edges.dtype() != DType::Float64 {
            return Err(Error::Type);
        }
        if edges.shape().len() != 1 || edges.len() < 2 {
            return Err(Error::Shape);
        }
        let edges = edges.float_values()?;
        if edges.iter().any(|v| !v.is_finite()) {
            return Err(Error::NonFinite);
        }
        if edges.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Error::Domain);
        }
        let mut counts = vec![0i64; edges.len() - 1];
        let (mut underflow, mut overflow) = (0, 0);
        for bit in self.bits() {
            let value = f64::from_bits(bit);
            if !value.is_finite() {
                return Err(Error::NonFinite);
            }
            if value < edges[0] {
                underflow += 1;
                continue;
            }
            if value > *edges.last().unwrap() {
                overflow += 1;
                continue;
            }
            let index = if value == *edges.last().unwrap() {
                counts.len() - 1
            } else {
                edges.partition_point(|edge| *edge <= value) - 1
            };
            counts[index] += 1;
        }
        Ok(Histogram {
            counts: Self::integers(vec![counts.len()], &counts)?,
            underflow,
            overflow,
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
    fn covariance_correlation_and_linear_quantiles_match_reference() {
        let x = Array::floats(vec![4], &[1.0, 2.0, 3.0, 4.0]).unwrap();
        let y = Array::floats(vec![4], &[2.0, 4.0, 6.0, 8.0]).unwrap();
        close(x.covariance(&y, 1).unwrap(), 10.0 / 3.0);
        close(x.correlation(&y).unwrap(), 1.0);
        close(x.quantile(0.5).unwrap(), 2.5);
        close(x.quantile(0.25).unwrap(), 1.75);
        assert_eq!(x.quantile(0.0).unwrap(), 1.0);
        assert_eq!(x.quantile(1.0).unwrap(), 4.0);
        let constant = Array::floats(vec![4], &[2.0; 4]).unwrap();
        assert_eq!(constant.correlation(&x), Err(Error::Domain));
    }
    #[test]
    fn histogram_reports_outliers_and_includes_only_the_last_right_boundary() {
        let data = Array::floats(vec![7], &[-1.0, 0.0, 0.5, 1.0, 1.5, 2.0, 3.0]).unwrap();
        let edges = Array::floats(vec![3], &[0.0, 1.0, 2.0]).unwrap();
        let h = data.histogram(&edges).unwrap();
        assert_eq!(h.counts.integer_values().unwrap(), vec![2, 3]);
        assert_eq!((h.underflow, h.overflow), (1, 1));
        let bad = Array::floats(vec![3], &[0.0, 1.0, 1.0]).unwrap();
        assert!(matches!(data.histogram(&bad), Err(Error::Domain)));
    }
    #[test]
    fn statistics_reject_nonfinite_empty_and_invalid_inputs() {
        let a = Array::floats(vec![0], &[]).unwrap();
        assert_eq!(a.quantile(0.5), Err(Error::Empty));
        let a = Array::floats(vec![2], &[1.0, f64::NAN]).unwrap();
        assert_eq!(a.quantile(0.5), Err(Error::NonFinite));
        let a = Array::floats(vec![2], &[-f64::MAX, f64::MAX]).unwrap();
        assert_eq!(a.quantile(0.5).unwrap(), 0.0);
        assert_eq!(a.quantile(1.1), Err(Error::Domain));
        assert_eq!(a.covariance(&a, 2), Err(Error::Empty));
    }
}
