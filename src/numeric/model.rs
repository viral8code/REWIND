//! Bounded deterministic model container. No host I/O or VM cells per weight.
use super::*;
pub const MAX_MODEL_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_MODEL_PARAMETERS: usize = 128;
const MAGIC: &[u8; 8] = b"RWMDL\0\x01\0";
pub fn model_size(parameters: &[(&str, &Array)]) -> Result<usize> {
    if parameters.len() > MAX_MODEL_PARAMETERS {
        return Err(Error::Size);
    }
    let mut size = 8usize + 4 + 32;
    let mut previous = None;
    for &(name, array) in parameters {
        if name.is_empty()
            || name.len() > 128
            || name.contains('\0')
            || previous.is_some_and(|p| p >= name)
        {
            return Err(Error::Domain);
        }
        previous = Some(name);
        if array.dtype() != DType::Float64 {
            return Err(Error::Type);
        }
        size = size
            .checked_add(2 + name.len() + 1 + 8 * array.shape().len())
            .and_then(|n| n.checked_add(8 * array.len()))
            .filter(|&n| n <= MAX_MODEL_BYTES)
            .ok_or(Error::Size)?;
    }
    Ok(size)
}
pub fn encode_model(parameters: &[(&str, &Array)]) -> Result<Vec<u8>> {
    let size = model_size(parameters)?;
    // Validate all values before allocating the output.
    for &(_, array) in parameters {
        array.check_finite()?;
    }
    let mut output = Vec::with_capacity(size);
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&(parameters.len() as u32).to_le_bytes());
    for &(name, array) in parameters {
        output.extend_from_slice(&(name.len() as u16).to_le_bytes());
        output.extend_from_slice(name.as_bytes());
        output.push(array.shape().len() as u8);
        for &d in array.shape() {
            output.extend_from_slice(&(d as u64).to_le_bytes());
        }
        for bits in array.bits() {
            output.extend_from_slice(&bits.to_le_bytes());
        }
    }
    let digest = Sha256::digest(&output);
    output.extend_from_slice(&digest);
    debug_assert_eq!(output.len(), size);
    Ok(output)
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(Error::Domain)?;
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }
}
pub fn decode_model(bytes: &[u8]) -> Result<Vec<(String, Array)>> {
    if bytes.len() > MAX_MODEL_BYTES {
        return Err(Error::Size);
    }
    if bytes.len() < 44 {
        return Err(Error::Domain);
    }
    let (payload, digest) = bytes.split_at(bytes.len() - 32);
    if Sha256::digest(payload).as_slice() != digest {
        return Err(Error::Domain);
    }
    let mut reader = Reader {
        bytes: payload,
        offset: 0,
    };
    if reader.take(8)? != MAGIC {
        return Err(Error::Domain);
    }
    let parameters = u32::from_le_bytes(reader.take(4)?.try_into().unwrap()) as usize;
    if parameters > MAX_MODEL_PARAMETERS {
        return Err(Error::Size);
    }
    // First pass checks the entire wire contract, including all IEEE values,
    // before allocating any numeric pages or publishing an incomplete map.
    let mut entries = Vec::with_capacity(parameters);
    let mut previous = None;
    for _ in 0..parameters {
        let length = u16::from_le_bytes(reader.take(2)?.try_into().unwrap()) as usize;
        if length == 0 || length > 128 {
            return Err(Error::Domain);
        }
        let name = std::str::from_utf8(reader.take(length)?).map_err(|_| Error::Domain)?;
        if name.contains('\0') || previous.is_some_and(|p| p >= name) {
            return Err(Error::Domain);
        }
        previous = Some(name);
        let rank = reader.take(1)?[0] as usize;
        if rank > MAX_RANK {
            return Err(Error::Shape);
        }
        let mut shape = Vec::with_capacity(rank);
        for _ in 0..rank {
            shape.push(
                usize::try_from(u64::from_le_bytes(reader.take(8)?.try_into().unwrap()))
                    .map_err(|_| Error::Size)?,
            );
        }
        let elements = count(&shape)?;
        let data = reader.take(elements.checked_mul(8).ok_or(Error::Size)?)?;
        for chunk in data.chunks_exact(8) {
            if !f64::from_le_bytes(chunk.try_into().unwrap()).is_finite() {
                return Err(Error::NonFinite);
            }
        }
        entries.push((name, shape, data));
    }
    if reader.offset != payload.len() {
        return Err(Error::Domain);
    }
    entries
        .into_iter()
        .map(|(name, shape, data)| {
            Ok((
                name.to_owned(),
                Array::from_bits(
                    DType::Float64,
                    shape,
                    data.chunks_exact(8)
                        .map(|b| u64::from_le_bytes(b.try_into().unwrap())),
                )?,
            ))
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn views_and_ieee_bits_round_trip_in_canonical_order() {
        let a = Array::floats(vec![2, 2], &[-0., 1., 2., 3.]).unwrap();
        let t = a.transpose(&[1, 0]).unwrap();
        let bytes = encode_model(&[("日本語", &t)]).unwrap();
        let model = decode_model(&bytes).unwrap();
        assert_eq!(model[0].0, "日本語");
        assert_eq!(model[0].1.shape(), &[2, 2]);
        assert_eq!(
            model[0].1.bits().collect::<Vec<_>>(),
            t.bits().collect::<Vec<_>>()
        );
        assert_eq!(encode_model(&[("日本語", &model[0].1)]).unwrap(), bytes);
    }
    #[test]
    fn checksum_truncation_and_forged_lengths_are_rejected() {
        let a = Array::floats(vec![1], &[2.]).unwrap();
        let bytes = encode_model(&[("x", &a)]).unwrap();
        for size in 0..bytes.len() {
            assert!(decode_model(&bytes[..size]).is_err());
        }
        let mut corrupt = bytes.clone();
        corrupt[20] ^= 1;
        assert!(matches!(decode_model(&corrupt), Err(Error::Domain)));
        let mut forged = bytes[..bytes.len() - 32].to_vec();
        forged[12..14].copy_from_slice(&u16::MAX.to_le_bytes());
        let digest = Sha256::digest(&forged);
        forged.extend_from_slice(&digest);
        assert!(decode_model(&forged).is_err());
        assert!(encode_model(&[("x", &a), ("x", &a)]).is_err());
        let nan = Array::floats(vec![1], &[f64::NAN]).unwrap();
        assert!(matches!(
            encode_model(&[("x", &nan)]),
            Err(Error::NonFinite)
        ));
    }
}
