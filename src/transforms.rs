//! Bounded pure transforms on native numeric buffers.
use crate::numeric::{Array, DType, Error, Result};
pub const MAX_FFT: usize = 1 << 20;
pub const MAX_SPARSE: usize = 1 << 20;
fn finite(n: f64) -> Result<f64> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(Error::NonFinite)
    }
}
fn vector(a: &Array, dtype: DType) -> Result<()> {
    if a.dtype() != dtype {
        return Err(Error::Type);
    }
    if a.shape().len() != 1 {
        return Err(Error::Shape);
    }
    Ok(())
}
/// Scaled Euclidean norm avoids overflow/underflow in squaring individual entries.
pub fn norm2(array: &Array) -> Result<f64> {
    if array.dtype() != DType::Float64 {
        return Err(Error::Type);
    }
    let mut scale = 0.0f64;
    let mut sum = 1.0f64;
    for bit in array.bits() {
        let value = f64::from_bits(bit);
        finite(value)?;
        let x = value.abs();
        if x != 0.0 {
            if scale < x {
                let r = scale / x;
                sum = 1.0 + sum * r * r;
                scale = x;
            } else {
                let r = x / scale;
                sum += r * r;
            }
        }
    }
    let result = scale * sum.sqrt();
    if result.is_finite() {
        Ok(result)
    } else {
        Err(Error::Overflow)
    }
}
fn fft_raw(re: &mut [f64], im: &mut [f64], inverse: bool) -> Result<()> {
    let n = re.len();
    if n != im.len() || n == 0 || !n.is_power_of_two() {
        return Err(Error::Shape);
    }
    if n > MAX_FFT {
        return Err(Error::Size);
    }
    for &v in re.iter().chain(im.iter()) {
        finite(v)?;
    }
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut width = 2;
    while width <= n {
        let half = width / 2;
        // Recalculate each twiddle rather than repeatedly multiplying a rounded rotation.
        // Reuse it over all blocks; O(n log n), with O(n) storage and O(n) trig calls.
        for k in 0..half {
            let (s, c) = (sign * std::f64::consts::TAU * k as f64 / width as f64).sin_cos();
            for base in (0..n).step_by(width) {
                let a = base + k;
                let b = a + half;
                let tr = finite(c * re[b] - s * im[b])?;
                let ti = finite(s * re[b] + c * im[b])?;
                let ar = re[a];
                let ai = im[a];
                re[a] = finite(ar + tr)?;
                im[a] = finite(ai + ti)?;
                re[b] = finite(ar - tr)?;
                im[b] = finite(ai - ti)?;
            }
        }
        width *= 2;
    }
    if inverse {
        let scale = n as f64;
        for v in re.iter_mut().chain(im.iter_mut()) {
            *v /= scale;
        }
    }
    Ok(())
}
pub fn fft(real: &Array, imag: &Array, inverse: bool) -> Result<(Array, Array)> {
    vector(real, DType::Float64)?;
    vector(imag, DType::Float64)?;
    let n = real.len();
    if n != imag.len() || n == 0 || !n.is_power_of_two() {
        return Err(Error::Shape);
    }
    if n > MAX_FFT {
        return Err(Error::Size);
    }
    let mut re = real.float_values()?;
    let mut im = imag.float_values()?;
    fft_raw(&mut re, &mut im, inverse)?;
    Ok((Array::floats(vec![n], &re)?, Array::floats(vec![n], &im)?))
}
pub fn convolve(left: &Array, right: &Array) -> Result<Array> {
    vector(left, DType::Float64)?;
    vector(right, DType::Float64)?;
    if left.len() == 0 || right.len() == 0 {
        return Array::floats(vec![0], &[]);
    }
    let output = left
        .len()
        .checked_add(right.len())
        .and_then(|n| n.checked_sub(1))
        .ok_or(Error::Size)?;
    let n = output
        .checked_next_power_of_two()
        .filter(|n| *n <= MAX_FFT)
        .ok_or(Error::Size)?;
    let mut ar = left.float_values()?;
    ar.resize(n, 0.0);
    let mut ai = vec![0.0; n];
    let mut br = right.float_values()?;
    br.resize(n, 0.0);
    let mut bi = vec![0.0; n];
    fft_raw(&mut ar, &mut ai, false)?;
    fft_raw(&mut br, &mut bi, false)?;
    for i in 0..n {
        let re = finite(ar[i] * br[i] - ai[i] * bi[i])?;
        ai[i] = finite(ar[i] * bi[i] + ai[i] * br[i])?;
        ar[i] = re;
    }
    drop(br);
    drop(bi);
    fft_raw(&mut ar, &mut ai, true)?;
    Array::floats(vec![output], &ar[..output])
}
/// Canonical COO -> CSR. Stable duplicate ordering and compensated accumulation.
pub fn csr(
    rows: usize,
    cols: usize,
    row: &Array,
    col: &Array,
    values: &Array,
) -> Result<(Array, Array, Array)> {
    if rows > MAX_SPARSE || cols > MAX_SPARSE {
        return Err(Error::Size);
    }
    vector(row, DType::Int64)?;
    vector(col, DType::Int64)?;
    vector(values, DType::Float64)?;
    let n = values.len();
    if row.len() != n || col.len() != n {
        return Err(Error::Shape);
    }
    if n > MAX_SPARSE {
        return Err(Error::Size);
    }
    let mut entries = Vec::with_capacity(n);
    for ((r, c), v) in row.bits().zip(col.bits()).zip(values.bits()) {
        let r = usize::try_from(r as i64).map_err(|_| Error::Index)?;
        let c = usize::try_from(c as i64).map_err(|_| Error::Index)?;
        if r >= rows || c >= cols {
            return Err(Error::Index);
        }
        let v = finite(f64::from_bits(v))?;
        entries.push((r, c, v));
    }
    entries.sort_by_key(|&(r, c, _)| (r, c));
    let mut offsets = vec![0i64; rows + 1];
    let mut indices = Vec::with_capacity(n);
    let mut data = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        let (r, c, _) = entries[i];
        let mut sum = 0.0;
        let mut correction = 0.0;
        while i < n && entries[i].0 == r && entries[i].1 == c {
            let x = entries[i].2;
            let t = finite(sum + x)?;
            correction = finite(
                correction
                    + if sum.abs() >= x.abs() {
                        (sum - t) + x
                    } else {
                        (x - t) + sum
                    },
            )?;
            sum = t;
            i += 1;
        }
        let value = finite(sum + correction)?;
        if value != 0.0 {
            offsets[r + 1] += 1;
            indices.push(c as i64);
            data.push(value);
        }
    }
    for r in 0..rows {
        offsets[r + 1] += offsets[r];
    }
    Ok((
        Array::integers(vec![rows + 1], &offsets)?,
        Array::integers(vec![data.len()], &indices)?,
        Array::floats(vec![data.len()], &data)?,
    ))
}
pub fn matvec(
    rows: usize,
    cols: usize,
    offsets: &Array,
    indices: &Array,
    values: &Array,
    right: &Array,
) -> Result<Array> {
    if rows > MAX_SPARSE || cols > MAX_SPARSE || values.len() > MAX_SPARSE {
        return Err(Error::Size);
    }
    vector(offsets, DType::Int64)?;
    vector(indices, DType::Int64)?;
    vector(values, DType::Float64)?;
    vector(right, DType::Float64)?;
    if offsets.len() != rows + 1 || indices.len() != values.len() || right.len() != cols {
        return Err(Error::Shape);
    }
    // Scan CSR pages without cloning nnz-sized column/value buffers on each iteration.
    // Materialize only the right vector for O(1) indexed access and the output vector.
    let xs = right.float_values()?;
    for &x in &xs {
        finite(x)?;
    }
    let mut os = offsets.bits();
    if os.next() != Some(0) {
        return Err(Error::Shape);
    }
    let mut entries = indices.bits().zip(values.bits());
    let mut start = 0usize;
    let mut out = Vec::with_capacity(rows);
    for _ in 0..rows {
        let end =
            usize::try_from(os.next().ok_or(Error::Shape)? as i64).map_err(|_| Error::Index)?;
        if end < start || end > values.len() {
            return Err(Error::Shape);
        }
        let mut previous = None;
        let mut sum = 0.0f64;
        let mut correction = 0.0f64;
        for _ in start..end {
            let (c, value) = entries.next().ok_or(Error::Shape)?;
            let c = usize::try_from(c as i64).map_err(|_| Error::Index)?;
            if c >= cols || previous.is_some_and(|p| p >= c) {
                return Err(Error::Index);
            }
            previous = Some(c);
            let value = finite(f64::from_bits(value))?;
            let x = finite(value * xs[c])?;
            let t = finite(sum + x)?;
            correction = finite(
                correction
                    + if sum.abs() >= x.abs() {
                        (sum - t) + x
                    } else {
                        (x - t) + sum
                    },
            )?;
            sum = t;
        }
        out.push(finite(sum + correction)?);
        start = end;
    }
    if start != values.len() {
        return Err(Error::Shape);
    }
    Array::floats(vec![rows], &out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn f(v: &[f64]) -> Array {
        Array::floats(vec![v.len()], v).unwrap()
    }
    fn ints(v: &[i64]) -> Array {
        Array::integers(vec![v.len()], v).unwrap()
    }
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() <= 2e-10 * (1.0 + b.abs()), "{a} != {b}");
    }
    #[test]
    fn scaled_norm_preserves_tiny_and_large_finite_values() {
        near(norm2(&f(&[3.0, 4.0])).unwrap(), 5.0);
        assert!((norm2(&f(&[3e-200, 4e-200])).unwrap() / 5e-200 - 1.0).abs() < 1e-15);
        assert!((norm2(&f(&[3e200, 4e200])).unwrap() / 5e200 - 1.0).abs() < 1e-15);
        assert_eq!(norm2(&f(&[])).unwrap(), 0.0);
        assert!(matches!(
            norm2(&f(&[f64::MAX, f64::MAX])),
            Err(Error::Overflow)
        ));
        assert!(matches!(norm2(&f(&[f64::NAN])), Err(Error::NonFinite)));
    }
    #[test]
    fn fft_matches_independent_dft_and_roundtrips() {
        for n in [1, 2, 4, 8, 16, 64] {
            let re = (0..n)
                .map(|i| (i * i % 17) as f64 - 4.0)
                .collect::<Vec<_>>();
            let im = (0..n)
                .map(|i| (i * 7 % 11) as f64 - 3.0)
                .collect::<Vec<_>>();
            let (r, j) = fft(&f(&re), &f(&im), false).unwrap();
            let rv = r.float_values().unwrap();
            let jv = j.float_values().unwrap();
            for k in 0..n {
                let (mut a, mut b) = (0.0, 0.0);
                for t in 0..n {
                    let (s, c) =
                        (-std::f64::consts::TAU * k as f64 * t as f64 / n as f64).sin_cos();
                    a += re[t] * c - im[t] * s;
                    b += re[t] * s + im[t] * c;
                }
                near(rv[k], a);
                near(jv[k], b);
            }
            let (a, b) = fft(&r, &j, true).unwrap();
            for i in 0..n {
                near(a.float_values().unwrap()[i], re[i]);
                near(b.float_values().unwrap()[i], im[i]);
            }
        }
    }
    #[test]
    fn convolution_matches_direct_sum_and_empty() {
        for (a, b) in [
            (vec![1.0, 2.0, 3.0], vec![4.0, -5.0]),
            (vec![0.25; 31], vec![-0.5; 17]),
        ] {
            let result = convolve(&f(&a), &f(&b)).unwrap().float_values().unwrap();
            for k in 0..result.len() {
                let expect = (0..a.len())
                    .filter_map(|i| {
                        k.checked_sub(i)
                            .filter(|&j| j < b.len())
                            .map(|j| a[i] * b[j])
                    })
                    .sum();
                near(result[k], expect);
            }
        }
        assert_eq!(convolve(&f(&[]), &f(&[1.0])).unwrap().len(), 0);
    }
    #[test]
    fn csr_aggregates_unsorted_duplicates_and_drops_zero() {
        let (o, i, v) = csr(
            3,
            3,
            &ints(&[1, 0, 1, 0, 1, 2]),
            &ints(&[2, 1, 2, 1, 0, 2]),
            &f(&[1.0, 5.0, -1.0, -3.0, 4.0, 7.0]),
        )
        .unwrap();
        assert_eq!(o.integer_values().unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(i.integer_values().unwrap(), vec![1, 0, 2]);
        assert_eq!(v.float_values().unwrap(), vec![2.0, 4.0, 7.0]);
        assert_eq!(
            matvec(3, 3, &o, &i, &v, &f(&[10.0, 20.0, 30.0]))
                .unwrap()
                .float_values()
                .unwrap(),
            vec![40.0, 40.0, 210.0]
        );
    }
    #[test]
    fn malformed_sparse_and_transform_inputs_are_errors() {
        assert!(matches!(
            fft(&f(&[1.0; 3]), &f(&[0.0; 3]), false),
            Err(Error::Shape)
        ));
        assert!(matches!(
            fft(&f(&[f64::NAN]), &f(&[0.0]), false),
            Err(Error::NonFinite)
        ));
        assert!(matches!(
            csr(1, 1, &ints(&[-1]), &ints(&[0]), &f(&[2.0])),
            Err(Error::Index)
        ));
        assert!(matches!(
            matvec(1, 1, &ints(&[0, 2]), &ints(&[0]), &f(&[1.0]), &f(&[1.0])),
            Err(Error::Shape)
        ));
    }
}
