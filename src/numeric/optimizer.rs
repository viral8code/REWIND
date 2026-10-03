//! One pass per named parameter: borrow four page streams and build only the
//! actual weights/moments, without intermediate array trees for each formula.
use super::*;
impl Array {
    pub fn sgd_step(&self, gradient: &Self, rate: f64) -> Result<Self> {
        if self.dtype != DType::Float64 || gradient.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        if self.shape != gradient.shape {
            return Err(Error::Shape);
        }
        if !rate.is_finite() || rate <= 0.0 {
            return Err(Error::Domain);
        }
        let mut values = Vec::with_capacity(self.len());
        for (w, g) in self.bits().zip(gradient.bits()) {
            let w = f64::from_bits(w);
            let g = f64::from_bits(g);
            if !w.is_finite() || !g.is_finite() {
                return Err(Error::NonFinite);
            }
            let change = rate * g;
            let updated = w - change;
            if !change.is_finite() || !updated.is_finite() {
                return Err(Error::Overflow);
            }
            values.push(updated);
        }
        Self::floats(self.shape.clone(), &values)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn adam_step(
        &self,
        gradient: &Self,
        first: &Self,
        second: &Self,
        beta1: f64,
        beta2: f64,
        rate: f64,
        epsilon: f64,
        first_correction: f64,
        second_correction: f64,
    ) -> Result<(Self, Self, Self)> {
        for a in [self, gradient, first, second] {
            if a.dtype != DType::Float64 {
                return Err(Error::Type);
            }
            if a.shape != self.shape {
                return Err(Error::Shape);
            }
        }
        if ![
            beta1,
            beta2,
            rate,
            epsilon,
            first_correction,
            second_correction,
        ]
        .iter()
        .all(|v| v.is_finite())
        {
            return Err(Error::NonFinite);
        }
        if !(0.0..1.0).contains(&beta1)
            || !(0.0..1.0).contains(&beta2)
            || rate <= 0.0
            || epsilon <= 0.0
            || first_correction <= 0.0
            || first_correction > 1.0
            || second_correction <= 0.0
            || second_correction > 1.0
        {
            return Err(Error::Domain);
        }
        let mut weights = Vec::with_capacity(self.len());
        let mut moments = Vec::with_capacity(self.len());
        let mut variances = Vec::with_capacity(self.len());
        let gradient_scale = (1.0 - beta2).sqrt();
        let variance_scale = second_correction.sqrt();
        for (((w, g), m), v) in self
            .bits()
            .zip(gradient.bits())
            .zip(first.bits())
            .zip(second.bits())
        {
            let w = f64::from_bits(w);
            let g = f64::from_bits(g);
            let m = f64::from_bits(m);
            let v = f64::from_bits(v);
            if ![w, g, m, v].iter().all(|v| v.is_finite()) {
                return Err(Error::NonFinite);
            }
            if v < 0.0 {
                return Err(Error::Domain);
            }
            let next_m = beta1 * m + (1.0 - beta1) * g;
            // Scale before squaring and sqrt before bias correction. A finite weighted
            // second moment need not require a representable unweighted g*g or v/c.
            let scaled_gradient = g * gradient_scale;
            let next_v = beta2 * v + scaled_gradient * scaled_gradient;
            let denominator = next_v.sqrt() / variance_scale + epsilon;
            let corrected_m = next_m / first_correction;
            let step = (corrected_m / denominator) * rate;
            let updated = w - step;
            if ![next_m, next_v, denominator, corrected_m, step, updated]
                .iter()
                .all(|v| v.is_finite())
            {
                return Err(Error::Overflow);
            }
            weights.push(updated);
            moments.push(next_m);
            variances.push(next_v);
        }
        Ok((
            Self::floats(self.shape.clone(), &weights)?,
            Self::floats(self.shape.clone(), &moments)?,
            Self::floats(self.shape.clone(), &variances)?,
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_constant_gradient_steps_match_normalized_adam_reference() {
        let weight = Array::floats(vec![3], &[3., 3., 3.]).unwrap();
        let grad = Array::floats(vec![3], &[2., -2., 0.]).unwrap();
        let zero = Array::zeros(DType::Float64, vec![3]).unwrap();
        let (a, m, v) = weight
            .adam_step(&grad, &zero, &zero, 0.9, 0.999, 0.1, 1e-12, 0.1, 0.001)
            .unwrap();
        let (b, _, _) = a
            .adam_step(&grad, &m, &v, 0.9, 0.999, 0.1, 1e-12, 0.19, 0.001999)
            .unwrap();
        for (actual, expected) in b.bits().map(f64::from_bits).zip([2.8, 3.2, 3.]) {
            assert!((actual - expected).abs() < 1e-12);
        }
        let sgd = weight.sgd_step(&grad, 0.25).unwrap();
        assert_eq!(
            sgd.bits().map(f64::from_bits).collect::<Vec<_>>(),
            vec![2.5, 3.5, 3.]
        );
    }
    #[test]
    fn weighted_second_moment_can_be_finite_when_raw_square_overflows() {
        let weight = Array::floats(vec![1], &[2.]).unwrap();
        let grad = Array::floats(vec![1], &[1e155]).unwrap();
        let zero = Array::zeros(DType::Float64, vec![1]).unwrap();
        let (a, _, _) = weight
            .adam_step(&grad, &zero, &zero, 0.9, 0.999, 0.1, 1e-8, 0.1, 0.001)
            .unwrap();
        assert!((a.float(&[0]).unwrap() - 1.9).abs() < 1e-12);
    }
    #[test]
    fn invalid_shapes_moments_and_overflow_preserve_all_inputs() {
        let weight = Array::floats(vec![1], &[2.]).unwrap();
        let grad = Array::floats(vec![1], &[f64::MAX]).unwrap();
        let zero = Array::zeros(DType::Float64, vec![1]).unwrap();
        assert!(matches!(
            weight.adam_step(&grad, &zero, &zero, 0.9, 0.999, 0.1, 1e-8, 0.1, 0.001),
            Err(Error::Overflow)
        ));
        let negative = Array::floats(vec![1], &[-1.]).unwrap();
        assert!(matches!(
            weight.adam_step(&weight, &zero, &negative, 0.9, 0.999, 0.1, 1e-8, 0.1, 0.001),
            Err(Error::Domain)
        ));
        assert_eq!(weight.float(&[0]).unwrap(), 2.);
        assert_eq!(zero.float(&[0]).unwrap(), 0.);
    }
}
