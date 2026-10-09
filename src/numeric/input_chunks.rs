//! Bounded raw input copies. An unfinished result stays local to its caller.
use super::*;
pub const INPUT_CHUNK: usize = 4096;
impl Array {
    /// Copy at most one chunk of raw scalar bits into an owned, contiguous result.
    /// Shared previous versions remain unchanged; no finite/arithmetic conversion occurs.
    pub fn input_bits_step(&self, cursor: usize, bits: &[u64]) -> Result<(Self, usize, bool)> {
        if cursor > self.len() || bits.len() > INPUT_CHUNK || bits.len() > self.len() - cursor {
            return Err(Error::Index);
        }
        if !self.writable || !self.contiguous() || self.offset != 0 || self.buffer.len != self.len()
        {
            return Err(Error::ReadOnly);
        }
        if bits.is_empty() && cursor < self.len() {
            return Err(Error::Domain);
        }
        let mut next = self.clone();
        for (index, &bit) in bits.iter().enumerate() {
            let flat = cursor + index;
            if next.buffer.get(flat) != bit {
                next.buffer.set(flat, bit);
            }
        }
        let end = cursor + bits.len();
        Ok((next, end, end == self.len()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_empty_and_noncanonical_outputs_have_checked_input_boundaries() {
        let empty = Array::zeros(DType::Int64, vec![0]).unwrap();
        let (copied, end, done) = empty.input_bits_step(0, &[]).unwrap();
        assert_eq!(copied, empty);
        assert_eq!(end, 0);
        assert!(done);
        let original = Array::zeros(DType::Int64, vec![3]).unwrap();
        let values = [i64::MIN as u64, (-1i64) as u64, i64::MAX as u64];
        let step = original.input_bits_step(0, &values).unwrap();
        assert!(step.2);
        assert_eq!(
            step.0,
            Array::from_bits(DType::Int64, vec![3], values.into_iter()).unwrap()
        );
        let large = Array::zeros(DType::Float64, vec![65536]).unwrap();
        let prefix = large.slice(0, 0, 3, 1).unwrap();
        assert!(matches!(
            prefix.input_bits_step(0, &[1, 2, 3]),
            Err(Error::ReadOnly)
        ));
    }
    #[test]
    fn input_chunks_preserve_raw_float_bits_and_retained_versions() {
        let length = INPUT_CHUNK * 2 + 1;
        let bits = (0..length)
            .map(|i| match i % 5 {
                0 => f64::NAN.to_bits() + i as u64,
                1 => (-0.0f64).to_bits(),
                _ => ((i as f64) * 0.25).to_bits(),
            })
            .collect::<Vec<_>>();
        let original = Array::zeros(DType::Float64, vec![length]).unwrap();
        let mut output = original.clone();
        let mut cursor = 0;
        let mut retained = None;
        while cursor < length {
            let end = (cursor + INPUT_CHUNK).min(length);
            let step = output.input_bits_step(cursor, &bits[cursor..end]).unwrap();
            output = step.0;
            cursor = step.1;
            assert_eq!(step.2, cursor == length);
            if retained.is_none() {
                retained = Some(output.clone());
            }
        }
        let expected =
            Array::from_bits(DType::Float64, vec![length], bits.iter().copied()).unwrap();
        assert_eq!(output, expected);
        assert_eq!(original.float_flat(0).unwrap().to_bits(), 0);
        assert_eq!(
            retained.unwrap().float_flat(INPUT_CHUNK).unwrap().to_bits(),
            0
        );
        assert!(matches!(
            output.input_bits_step(length + 1, &[]),
            Err(Error::Index)
        ));
        assert!(matches!(
            original.input_bits_step(0, &[]),
            Err(Error::Domain)
        ));
        assert!(matches!(
            original.input_bits_step(0, &vec![1; INPUT_CHUNK + 1]),
            Err(Error::Index)
        ));
        assert_eq!(output.input_bits_step(length, &[]).unwrap().1, length);
    }
}
