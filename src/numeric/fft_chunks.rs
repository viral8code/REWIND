//! Cooperative radix-2 FFT: contiguous COW writes and cached twiddles.
use super::*;
pub const FFT_CHUNK: usize = 4096;
#[derive(Clone)]
pub struct FftWork {
    pub input_real: Array,
    pub input_imag: Array,
    pub real: Array,
    pub imag: Array,
    pub twiddle_real: Array,
    pub twiddle_imag: Array,
    pub inverse: bool,
    pub phase: usize,
    pub cursor: usize,
    pub width: usize,
}
impl FftWork {
    pub fn new(real: &Array, imag: &Array, inverse: bool) -> Result<Self> {
        if real.dtype() != DType::Float64 || imag.dtype() != DType::Float64 {
            return Err(Error::Type);
        }
        if real.shape().len() != 1
            || real.shape() != imag.shape()
            || real.len() == 0
            || !real.len().is_power_of_two()
        {
            return Err(Error::Shape);
        }
        let n = real.len();
        if n > crate::transforms::MAX_FFT {
            return Err(Error::Size);
        }
        Ok(Self {
            input_real: real.clone(),
            input_imag: imag.clone(),
            real: Array::zeros(DType::Float64, vec![n])?,
            imag: Array::zeros(DType::Float64, vec![n])?,
            twiddle_real: Array::zeros(DType::Float64, vec![n / 2])?,
            twiddle_imag: Array::zeros(DType::Float64, vec![n / 2])?,
            inverse,
            phase: 0,
            cursor: 0,
            width: 2,
        })
    }
    pub fn done(&self) -> bool {
        self.phase == 4
    }
    pub fn result(&self) -> Result<(Array, Array)> {
        self.validate()?;
        if !self.done() {
            return Err(Error::Domain);
        }
        Ok((self.real.clone(), self.imag.clone()))
    }
    pub fn validate(&self) -> Result<()> {
        let n = self.real.len();
        if n == 0
            || n > crate::transforms::MAX_FFT
            || !n.is_power_of_two()
            || self.phase > 4
            || self.width < 2
            || self.width > n * 2
            || !self.width.is_power_of_two()
        {
            return Err(Error::Domain);
        }
        for array in [&self.input_real, &self.input_imag, &self.real, &self.imag] {
            if array.dtype() != DType::Float64 || array.shape() != [n] {
                return Err(Error::Shape);
            }
        }
        for array in [&self.twiddle_real, &self.twiddle_imag] {
            if array.dtype() != DType::Float64 || array.shape() != [n / 2] {
                return Err(Error::Shape);
            }
        }
        for array in [
            &self.real,
            &self.imag,
            &self.twiddle_real,
            &self.twiddle_imag,
        ] {
            if !array.contiguous() || !array.writable || array.offset != 0 {
                return Err(Error::ReadOnly);
            }
        }
        if self.phase == 2 && self.cursor % FFT_CHUNK != 0 {
            return Err(Error::Domain);
        }
        let limit = if self.phase == 1 || self.phase == 2 {
            n / 2
        } else {
            n
        };
        if self.cursor > limit {
            return Err(Error::Index);
        }
        Ok(())
    }
    pub fn step(&self) -> Result<Self> {
        self.validate()?;
        let mut next = self.clone();
        let n = self.real.len();
        if self.phase == 0 {
            let end = (self.cursor + FFT_CHUNK).min(n);
            let bits = n.trailing_zeros();
            let mut real = Vec::with_capacity(end - self.cursor);
            let mut imag = Vec::with_capacity(end - self.cursor);
            for i in self.cursor..end {
                let from = if bits == 0 {
                    0
                } else {
                    i.reverse_bits() >> (usize::BITS - bits)
                };
                let re = self.input_real.float(&[from])?;
                let im = self.input_imag.float(&[from])?;
                if !re.is_finite() || !im.is_finite() {
                    return Err(Error::NonFinite);
                }
                real.push(re.to_bits());
                imag.push(im.to_bits());
            }
            next.write(false, self.cursor, &real, &imag);
            next.cursor = end;
            if end == n {
                next.phase = 1;
                next.cursor = 0;
            }
        } else if self.phase == 1 {
            let end = (self.cursor + FFT_CHUNK).min(n / 2);
            let mut real = Vec::with_capacity(end - self.cursor);
            let mut imag = Vec::with_capacity(end - self.cursor);
            let sign = if self.inverse { 1.0 } else { -1.0 };
            for k in self.cursor..end {
                let (s, c) = (sign * std::f64::consts::TAU * k as f64 / n as f64).sin_cos();
                real.push(c.to_bits());
                imag.push(s.to_bits());
            }
            next.write(true, self.cursor, &real, &imag);
            next.cursor = end;
            if end == n / 2 {
                next.phase = 2;
                next.cursor = 0;
            }
        } else if self.phase == 2 {
            if self.width > n {
                next.phase = if self.inverse { 3 } else { 4 };
                next.cursor = 0;
                return Ok(next);
            }
            let half = self.width / 2;
            let end = (self.cursor + FFT_CHUNK).min(n / 2);
            // Small stages cover whole adjacent blocks. Collect them into two
            // contiguous writes instead of rehashing a page per butterfly.
            if half <= FFT_CHUNK {
                let start = (self.cursor / half) * self.width;
                let mut real = vec![0; 2 * (end - self.cursor)];
                let mut imag = vec![0; 2 * (end - self.cursor)];
                for cursor in self.cursor..end {
                    let base = (cursor / half) * self.width;
                    let k = cursor % half;
                    let values = self.butterfly(base + k, base + k + half, k, n)?;
                    real[base + k - start] = values[0];
                    imag[base + k - start] = values[1];
                    real[base + k + half - start] = values[2];
                    imag[base + k + half - start] = values[3];
                }
                next.write(false, start, &real, &imag);
            } else {
                let base = (self.cursor / half) * self.width;
                let k = self.cursor % half;
                let stop = end - self.cursor;
                let mut ar = Vec::with_capacity(stop);
                let mut ai = Vec::with_capacity(stop);
                let mut br = Vec::with_capacity(stop);
                let mut bi = Vec::with_capacity(stop);
                for j in k..k + stop {
                    let values = self.butterfly(base + j, base + j + half, j, n)?;
                    ar.push(values[0]);
                    ai.push(values[1]);
                    br.push(values[2]);
                    bi.push(values[3]);
                }
                next.write(false, base + k, &ar, &ai);
                next.write(false, base + k + half, &br, &bi);
            }
            next.cursor = end;
            if end == n / 2 {
                next.width *= 2;
                next.cursor = 0;
                if next.width > n {
                    next.phase = if self.inverse { 3 } else { 4 };
                }
            }
        } else if self.phase == 3 {
            let end = (self.cursor + FFT_CHUNK).min(n);
            let real = (self.cursor..end)
                .map(|i| (f64::from_bits(self.real.buffer.get(i)) / n as f64).to_bits())
                .collect::<Vec<_>>();
            let imag = (self.cursor..end)
                .map(|i| (f64::from_bits(self.imag.buffer.get(i)) / n as f64).to_bits())
                .collect::<Vec<_>>();
            next.write(false, self.cursor, &real, &imag);
            next.cursor = end;
            if end == n {
                next.phase = 4;
            }
        } else if self.phase != 4 {
            return Err(Error::Domain);
        }
        Ok(next)
    }
    fn butterfly(&self, a: usize, b: usize, k: usize, n: usize) -> Result<[u64; 4]> {
        let twiddle = k * (n / self.width);
        let c = f64::from_bits(self.twiddle_real.buffer.get(twiddle));
        let s = f64::from_bits(self.twiddle_imag.buffer.get(twiddle));
        let re = f64::from_bits(self.real.buffer.get(b));
        let im = f64::from_bits(self.imag.buffer.get(b));
        let tr = c * re - s * im;
        let ti = s * re + c * im;
        let re = f64::from_bits(self.real.buffer.get(a));
        let im = f64::from_bits(self.imag.buffer.get(a));
        let values = [re + tr, im + ti, re - tr, im - ti];
        if values.iter().any(|v| !v.is_finite()) {
            return Err(Error::NonFinite);
        }
        Ok(values.map(f64::to_bits))
    }
    fn write(&mut self, twiddle: bool, start: usize, real: &[u64], imag: &[u64]) {
        let (re, im) = if twiddle {
            (&mut self.twiddle_real, &mut self.twiddle_imag)
        } else {
            (&mut self.real, &mut self.imag)
        };
        Node::write_range(&mut re.buffer.root, re.buffer.height, start, real);
        Node::write_range(&mut im.buffer.root, im.buffer.height, start, imag);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_nonfinite_and_invalid_state_are_typed_without_modifying_a_checkpoint() {
        let n = FFT_CHUNK * 4;
        let mut input = vec![1.0; n];
        input[1] = f64::NAN;
        let real = Array::floats(vec![n], &input).unwrap();
        let imag = Array::zeros(DType::Float64, vec![n]).unwrap();
        let initial = FftWork::new(&real, &imag, false).unwrap();
        let saved = initial.step().unwrap().step().unwrap();
        let hash = saved.real.buffer.root.digest();
        assert!(matches!(saved.step(), Err(Error::NonFinite)));
        assert_eq!(saved.real.buffer.root.digest(), hash);
        let mut broken = initial.clone();
        broken.width = 0;
        assert!(matches!(broken.step(), Err(Error::Domain)));
        broken = initial.clone();
        broken.cursor = usize::MAX;
        assert!(matches!(broken.step(), Err(Error::Index)));
        broken = initial;
        broken.phase = 2;
        broken.cursor = 1;
        assert!(matches!(broken.step(), Err(Error::Domain)));
        let large = Array::zeros(DType::Float64, vec![crate::transforms::MAX_FFT * 2]).unwrap();
        assert!(matches!(
            FftWork::new(&large, &large, false),
            Err(Error::Size)
        ));
    }

    #[test]
    fn shared_zeros_keep_canonical_hashes_and_copy_only_changed_values() {
        for n in [0, 1, 255, 256, 257, 511, 512, 513, 768, 1023, 1024, 65537] {
            let zero = Array::zeros(DType::Float64, vec![n]).unwrap();
            let materialized = Array::floats(vec![n], &vec![0.0; n]).unwrap();
            assert_eq!(zero.buffer.root.digest(), materialized.buffer.root.digest());
            let accounting = Accounting::default();
            accounting.register(&zero);
            assert!(accounting.bytes() < 32768);
            if n > 0 {
                let mut changed = zero.clone();
                changed.set_float(&[n - 1], 3.0).unwrap();
                assert_eq!(zero.float(&[n - 1]), Ok(0.0));
                assert_eq!(changed.float(&[n - 1]), Ok(3.0));
            }
        }
    }
    #[test]
    fn cooperative_fft_matches_independent_dft_and_inverse() {
        for n in [1, 2, 4, 8, 16, 32] {
            let real = (0..n)
                .map(|i| ((i * 17 + 3) % 13) as f64 - 6.0)
                .collect::<Vec<_>>();
            let imag = (0..n)
                .map(|i| ((i * 7 + 1) % 11) as f64 - 5.0)
                .collect::<Vec<_>>();
            let re = Array::floats(vec![n], &real).unwrap();
            let im = Array::floats(vec![n], &imag).unwrap();
            let mut work = FftWork::new(&re, &im, false).unwrap();
            while !work.done() {
                work = work.step().unwrap();
            }
            let (r, z) = work.result().unwrap();
            for k in 0..n {
                let (mut dr, mut di) = (0.0, 0.0);
                for j in 0..n {
                    let (s, c) = (-std::f64::consts::TAU * (k * j) as f64 / n as f64).sin_cos();
                    dr += real[j] * c - imag[j] * s;
                    di += real[j] * s + imag[j] * c;
                }
                assert!((r.float(&[k]).unwrap() - dr).abs() < 1e-10);
                assert!((z.float(&[k]).unwrap() - di).abs() < 1e-10);
            }
            let mut inverse = FftWork::new(&r, &z, true).unwrap();
            while !inverse.done() {
                inverse = inverse.step().unwrap();
            }
            let (r, z) = inverse.result().unwrap();
            for i in 0..n {
                assert!((r.float(&[i]).unwrap() - real[i]).abs() < 1e-12);
                assert!((z.float(&[i]).unwrap() - imag[i]).abs() < 1e-12);
            }
        }
    }
    #[test]
    fn a_checkpoint_of_mid_kernel_work_repeats_identical_chunks_and_preserves_input() {
        let n = FFT_CHUNK * 4;
        let re =
            Array::floats(vec![n], &(0..n).map(|i| (i % 7) as f64).collect::<Vec<_>>()).unwrap();
        let im = Array::zeros(DType::Float64, vec![n]).unwrap();
        let begin = FftWork::new(&re, &im, false).unwrap();
        let halfway = begin.step().unwrap();
        assert_eq!(halfway.cursor, FFT_CHUNK);
        assert_eq!(begin.cursor, 0);
        assert_eq!(begin.real.float(&[1]), Ok(0.0));
        let mut a = halfway.clone();
        let mut b = halfway;
        while !a.done() {
            a = a.step().unwrap();
        }
        while !b.done() {
            b = b.step().unwrap();
        }
        assert_eq!(a.real.buffer.root.digest(), b.real.buffer.root.digest());
        assert_eq!(a.imag.buffer.root.digest(), b.imag.buffer.root.digest());
        let (r, z) = crate::transforms::fft(&re, &im, false).unwrap();
        for i in 0..n {
            assert!((a.real.float(&[i]).unwrap() - r.float(&[i]).unwrap()).abs() < 1e-8);
            assert!((a.imag.float(&[i]).unwrap() - z.float(&[i]).unwrap()).abs() < 1e-8);
        }
    }
}
