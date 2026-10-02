//! Finite PostgreSQL NUMERIC binary values; no float conversion.
use super::{DecimalValue, Error, MAX_PRECISION, MAX_SCALE};
pub fn encode(value: &DecimalValue) -> Result<Vec<u8>, Error> {
    let plain = value.to_string();
    let negative = plain.starts_with('-');
    let magnitude = plain.trim_start_matches('-');
    let (whole, fraction) = magnitude.split_once('.').unwrap_or((magnitude, ""));
    if whole.trim_start_matches('0').len() + fraction.len() > MAX_PRECISION {
        return Err(Error::Precision);
    }
    let mut padded = String::new();
    padded.extend(std::iter::repeat_n('0', (4 - whole.len() % 4) % 4));
    padded.push_str(whole);
    let integer_groups = padded.len() / 4;
    padded.push_str(fraction);
    padded.extend(std::iter::repeat_n('0', (4 - fraction.len() % 4) % 4));
    let groups = padded
        .as_bytes()
        .chunks_exact(4)
        .map(|c| c.iter().fold(0u16, |n, b| n * 10 + u16::from(b - b'0')))
        .collect::<Vec<_>>();
    let first = groups.iter().position(|v| *v != 0).unwrap_or(groups.len());
    let end = groups
        .iter()
        .rposition(|v| *v != 0)
        .map_or(first, |n| n + 1);
    let ndigits = i16::try_from(end - first).map_err(|_| Error::Size)?;
    let weight = if ndigits == 0 {
        0
    } else {
        i16::try_from(integer_groups as i32 - first as i32 - 1).map_err(|_| Error::Size)?
    };
    let mut output = Vec::with_capacity(8 + (end - first) * 2);
    output.extend(ndigits.to_be_bytes());
    output.extend(weight.to_be_bytes());
    output.extend(
        (if negative && ndigits != 0 {
            0x4000u16
        } else {
            0
        })
        .to_be_bytes(),
    );
    output.extend((fraction.len() as u16).to_be_bytes());
    for group in &groups[first..end] {
        output.extend(group.to_be_bytes())
    }
    Ok(output)
}
pub fn decode(bytes: &[u8]) -> Result<DecimalValue, Error> {
    if bytes.len() < 8 || bytes.len() % 2 != 0 {
        return Err(Error::Syntax);
    }
    let ndigits = i16::from_be_bytes(bytes[..2].try_into().unwrap());
    let weight = i16::from_be_bytes(bytes[2..4].try_into().unwrap()) as i32;
    let sign = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
    let scale = u16::from_be_bytes(bytes[6..8].try_into().unwrap()) as usize;
    if ndigits < 0 || bytes.len() != 8 + ndigits as usize * 2 || !matches!(sign, 0 | 0x4000) {
        return Err(Error::Syntax);
    }
    if scale > MAX_SCALE as usize
        || ndigits as usize > MAX_PRECISION.div_ceil(4) + 1
        || weight.unsigned_abs() as usize > MAX_PRECISION.div_ceil(4) + 1
    {
        return Err(Error::Size);
    }
    let groups = bytes[8..]
        .chunks_exact(2)
        .map(|s| u16::from_be_bytes(s.try_into().unwrap()))
        .collect::<Vec<_>>();
    if groups.iter().any(|g| *g >= 10000) {
        return Err(Error::Syntax);
    }
    let integer_digits = if weight >= 0 {
        (weight as usize + 1) * 4
    } else {
        1
    };
    if integer_digits.saturating_add(scale) > MAX_PRECISION + 4 {
        return Err(Error::Precision);
    }
    let group_at = |position: i32| {
        let index = weight - position;
        if index < 0 {
            0
        } else {
            groups.get(index as usize).copied().unwrap_or(0)
        }
    };
    let mut text = String::with_capacity(integer_digits + scale + 2);
    if sign == 0x4000 {
        text.push('-')
    }
    if weight >= 0 {
        for position in (0..=weight).rev() {
            text.push_str(&format!("{:04}", group_at(position)))
        }
    } else {
        text.push('0')
    }
    if scale > 0 {
        text.push('.');
        for i in 1..=scale.div_ceil(4) {
            let part = format!("{:04}", group_at(-(i as i32)));
            text.push_str(&part[..4.min(scale - (i - 1) * 4)]);
        }
    }
    // dscale must not silently truncate a nonzero fractional group or digit.
    let lowest = weight - ndigits as i32 + 1;
    for position in lowest..0 {
        let group = group_at(position);
        let begin = (-position as usize - 1) * 4;
        if begin >= scale && group != 0 {
            return Err(Error::Syntax);
        }
        if begin < scale && begin + 4 > scale && group % 10u16.pow((begin + 4 - scale) as u32) != 0
        {
            return Err(Error::Syntax);
        }
    }
    let negative = text.starts_with('-');
    let magnitude = text.trim_start_matches('-');
    let (whole, fraction) = magnitude.split_once('.').unwrap_or((magnitude, ""));
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let normalized = format!(
        "{}{}{}{}",
        if negative { "-" } else { "" },
        whole,
        if scale > 0 { "." } else { "" },
        fraction
    );
    DecimalValue::parse(&normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_packets_and_exact_roundtrips() {
        assert_eq!(
            encode(&DecimalValue::parse("12345.6789").unwrap()).unwrap(),
            vec![0, 3, 0, 1, 0, 0, 0, 4, 0, 1, 9, 41, 26, 133]
        );
        for text in [
            "0",
            "0.0000",
            "-0.00",
            "1",
            "-1",
            "12345.6789",
            "-0.00000123",
            "12345678901234567890.12340000",
            "1e3",
        ] {
            let value = DecimalValue::parse(text).unwrap();
            let decoded = decode(&encode(&value).unwrap()).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(decoded.scale(), value.scale().max(0));
        }
    }
    #[test]
    fn malformed_special_and_unbounded_values_are_rejected() {
        for bytes in [
            vec![],
            vec![0; 7],
            vec![0, 0, 0, 0, 0xc0, 0, 0, 0],
            vec![0, 1, 0, 0, 0, 0, 0, 0, 0x27, 0x10],
            vec![0, 1, 0xff, 0xff, 0, 0, 0, 0, 0, 1],
            vec![0xff, 0xff, 0, 0, 0, 0, 0, 0],
        ] {
            assert!(decode(&bytes).is_err(), "{bytes:?}")
        }
        let enormous = DecimalValue::parse("1e10000").unwrap();
        assert_eq!(encode(&enormous), Err(Error::Precision));
    }
}
