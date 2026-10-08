//! Native tensor operations used by reverse-mode differentiation and optimizers.
use super::*;

impl Array {
    pub fn reshape_logical(&self, shape: Vec<usize>) -> Result<Self> {
        if count(&shape)? != self.len() {
            return Err(Error::Shape);
        }
        if self.contiguous() {
            self.reshape(shape)
        } else {
            self.materialize()?.reshape(shape)
        }
    }
    pub fn reshape_logical_scratch(&self) -> usize {
        if self.contiguous() {
            2048
        } else {
            Self::storage_estimate(self.len())
        }
    }

    pub fn check_finite(&self) -> Result<()> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        if self.bits().any(|b| !f64::from_bits(b).is_finite()) {
            return Err(Error::NonFinite);
        }
        Ok(())
    }

    /// Structural identity of an immutable view and its storage version, not a
    /// flattened logical-content digest. Cold identity hashes backing storage;
    /// cached identity costs O(rank). No payload copies are made.
    pub fn tensor_key(&self, tag: &str, index: i64, left: &[u8], right: &[u8]) -> Result<[u8; 32]> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        if tag.len() > 256
            || index < 0
            || !matches!(left.len(), 0 | 32)
            || !matches!(right.len(), 0 | 32)
        {
            return Err(Error::Domain);
        }
        let mut h = Sha256::new();
        h.update(b"REWIND tensor node v1\0");
        h.update((tag.len() as u64).to_le_bytes());
        h.update(tag.as_bytes());
        h.update(index.to_le_bytes());
        h.update([left.len() as u8]);
        h.update(left);
        h.update([right.len() as u8]);
        h.update(right);
        h.update((self.shape.len() as u64).to_le_bytes());
        for &d in &self.shape {
            h.update((d as u64).to_le_bytes());
        }
        for &s in &self.strides {
            h.update((s as i64).to_le_bytes());
        }
        h.update((self.offset as i64).to_le_bytes());
        h.update([self.writable as u8]);
        h.update(self.buffer.root.digest());
        Ok(h.finalize().into())
    }

    pub fn affine(&self, scale: f64, offset: f64) -> Result<Self> {
        if !scale.is_finite() || !offset.is_finite() {
            return Err(Error::NonFinite);
        }
        self.map_float(|x| {
            if !x.is_finite() {
                return Err(Error::NonFinite);
            }
            let y = x * scale + offset;
            if y.is_finite() {
                Ok(y)
            } else {
                Err(Error::Overflow)
            }
        })
    }

    pub fn activation(&self, operation: &str) -> Result<Self> {
        if !matches!(
            operation,
            "relu" | "reluGrad" | "sigmoid" | "sigmoidGrad" | "tanhGrad" | "reciprocal"
        ) {
            return Err(Error::Domain);
        }
        self.map_float(|x| {
            if !x.is_finite() {
                return Err(Error::NonFinite);
            }
            let y = match operation {
                "relu" => x.max(0.0),
                "reluGrad" => {
                    if x > 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                "sigmoid" => {
                    if x >= 0.0 {
                        1.0 / (1.0 + (-x).exp())
                    } else {
                        let e = x.exp();
                        e / (1.0 + e)
                    }
                }
                // Gradient operations consume the corresponding forward output.
                "sigmoidGrad" => {
                    if !(0.0..=1.0).contains(&x) {
                        return Err(Error::Domain);
                    }
                    x * (1.0 - x)
                }
                "tanhGrad" => {
                    if !(-1.0..=1.0).contains(&x) {
                        return Err(Error::Domain);
                    }
                    1.0 - x * x
                }
                "reciprocal" => {
                    if x == 0.0 {
                        return Err(Error::Domain);
                    }
                    1.0 / x
                }
                _ => unreachable!(),
            };
            if y.is_finite() {
                Ok(y)
            } else {
                Err(Error::Overflow)
            }
        })
    }

    /// Sum over dimensions added or expanded by explicit broadcasting. Shape
    /// may be scalar; singleton axes are retained. Neumaier accumulation per
    /// output cell avoids cancellation loss, without a VM object per cell.
    pub fn sum_to_shape(&self, shape: Vec<usize>) -> Result<Self> {
        if self.dtype != DType::Float64 {
            return Err(Error::Type);
        }
        let n = count(&shape)?;
        if shape.len() > self.shape.len() {
            return Err(Error::Shape);
        }
        let lead = self.shape.len() - shape.len();
        if shape
            .iter()
            .zip(&self.shape[lead..])
            .any(|(&t, &s)| t != s && t != 1)
        {
            return Err(Error::Shape);
        }
        let mut sums = vec![0.0f64; n];
        let mut corrections = vec![0.0f64; n];
        for (flat, bits) in self.bits().enumerate() {
            let x = f64::from_bits(bits);
            if !x.is_finite() {
                return Err(Error::NonFinite);
            }
            let mut remaining = flat;
            let mut target = 0;
            let mut stride = 1;
            for axis in (0..self.shape.len()).rev() {
                let coordinate = remaining % self.shape[axis];
                remaining /= self.shape[axis];
                if axis >= lead {
                    let dim = shape[axis - lead];
                    if dim != 1 {
                        target += coordinate * stride;
                    }
                    stride *= dim;
                }
            }
            let old = sums[target];
            let next = old + x;
            corrections[target] += if old.abs() >= x.abs() {
                (old - next) + x
            } else {
                (x - next) + old
            };
            if !next.is_finite() || !corrections[target].is_finite() {
                return Err(Error::Overflow);
            }
            sums[target] = next;
        }
        for (s, c) in sums.iter_mut().zip(corrections) {
            *s += c;
            if !s.is_finite() {
                return Err(Error::Overflow);
            }
        }
        Self::floats(shape, &sums)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn broadcast_reduction_and_transposed_views_match_explicit_reference() {
        let a = Array::floats(vec![2, 3], &[1e16, 2., 3., -1e16, 5., 6.]).unwrap();
        let r = a.sum_to_shape(vec![1, 3]).unwrap();
        assert_eq!(
            r.bits().map(f64::from_bits).collect::<Vec<_>>(),
            vec![0., 7., 9.]
        );
        assert_eq!(a.sum_to_shape(vec![]).unwrap().float(&[]).unwrap(), 16.);
        let t = a.transpose(&[1, 0]).unwrap();
        let r = t.sum_to_shape(vec![3, 1]).unwrap();
        assert_eq!(
            r.bits().map(f64::from_bits).collect::<Vec<_>>(),
            vec![0., 7., 9.]
        );
        assert!(matches!(a.sum_to_shape(vec![2, 2]), Err(Error::Shape)));
        let empty = Array::zeros(DType::Float64, vec![0, 3]).unwrap();
        assert_eq!(
            empty
                .sum_to_shape(vec![1, 3])
                .unwrap()
                .bits()
                .collect::<Vec<_>>(),
            vec![0; 3]
        );
    }
    #[test]
    fn stable_activations_and_affine_fail_atomically() {
        let a = Array::floats(vec![3], &[-1000., 0., 1000.]).unwrap();
        assert_eq!(
            a.activation("sigmoid")
                .unwrap()
                .bits()
                .map(f64::from_bits)
                .collect::<Vec<_>>(),
            vec![0., 0.5, 1.]
        );
        assert_eq!(
            a.activation("reluGrad")
                .unwrap()
                .bits()
                .map(f64::from_bits)
                .collect::<Vec<_>>(),
            vec![0., 0., 1.]
        );
        let large = Array::floats(vec![1], &[f64::MAX]).unwrap();
        assert!(matches!(large.affine(2., 0.), Err(Error::Overflow)));
        assert_eq!(large.float(&[0]).unwrap(), f64::MAX);
        assert!(matches!(a.affine(f64::INFINITY, 0.), Err(Error::NonFinite)));
    }
    #[test]
    fn structural_keys_distinguish_parent_graphs_storage_and_operation() {
        let a = Array::floats(vec![2], &[1., 2.]).unwrap();
        let k = a.tensor_key("param", 0, &[], &[]).unwrap();
        assert_eq!(k, a.clone().tensor_key("param", 0, &[], &[]).unwrap());
        assert_ne!(k, a.tensor_key("constant", 0, &[], &[]).unwrap());
        assert_ne!(k, a.tensor_key("param", 1, &[], &[]).unwrap());
        let b = Array::floats(vec![2], &[2., 1.]).unwrap();
        let other = b.tensor_key("param", 0, &[], &[]).unwrap();
        assert_ne!(
            a.tensor_key("add", 2, &k, &k).unwrap(),
            a.tensor_key("add", 2, &other, &k).unwrap()
        );
    }
}
