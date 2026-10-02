//! Exact base-ten values with explicit precision, scale and rounding.
use crate::bigint::IntegerValue;
use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{Signed, Zero};
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, fmt, sync::Arc};
pub mod postgres;
pub const MAX_PRECISION: usize = 10_000;
pub const MAX_SCALE: i32 = 10_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Syntax,
    Size,
    Scale,
    Precision,
    DivisionByZero,
    Inexact,
    Domain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    TowardZero,
    AwayFromZero,
    Floor,
    Ceiling,
    HalfUp,
    HalfDown,
    HalfEven,
    Exact,
}
struct Inner {
    coefficient: IntegerValue,
    scale: i32,
    digits: String,
    normalized_scale: i32,
    normalized_digits: String,
    sign: i8,
    digest: [u8; 32],
    canonical: Option<Arc<Inner>>,
}
#[derive(Clone)]
pub struct DecimalValue(Arc<Inner>);
/// VM storage equality also observes scale, unlike numeric equality.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct StoredDecimal(pub DecimalValue);
impl std::ops::Deref for StoredDecimal {
    type Target = DecimalValue;
    fn deref(&self) -> &DecimalValue {
        &self.0
    }
}
impl From<DecimalValue> for StoredDecimal {
    fn from(v: DecimalValue) -> Self {
        Self(v)
    }
}
impl PartialEq for StoredDecimal {
    fn eq(&self, r: &Self) -> bool {
        self.0.same_representation(&r.0)
    }
}
impl Eq for StoredDecimal {}
impl fmt::Display for StoredDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Debug for DecimalValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Decimal")
            .field("scale", &self.scale())
            .field("coefficient_bits", &self.0.coefficient.bits())
            .field("numeric_sha256", &self.0.digest)
            .finish()
    }
}
impl PartialEq for DecimalValue {
    fn eq(&self, rhs: &Self) -> bool {
        self.cmp(rhs) == Ordering::Equal
    }
}
impl Eq for DecimalValue {}
impl PartialOrd for DecimalValue {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for DecimalValue {
    fn cmp(&self, rhs: &Self) -> Ordering {
        if Arc::ptr_eq(&self.0, &rhs.0) {
            return Ordering::Equal;
        }
        let sign = self.0.sign.cmp(&rhs.0.sign);
        if sign != Ordering::Equal {
            return sign;
        }
        if self.0.sign == 0 {
            return Ordering::Equal;
        }
        let a = &self.0;
        let b = &rhs.0;
        let exponent = (a.normalized_digits.len() as i32 - a.normalized_scale)
            .cmp(&(b.normalized_digits.len() as i32 - b.normalized_scale));
        let abs = exponent.then_with(|| {
            let length = a.normalized_digits.len().max(b.normalized_digits.len());
            a.normalized_digits
                .bytes()
                .chain(std::iter::repeat(b'0'))
                .take(length)
                .cmp(
                    b.normalized_digits
                        .bytes()
                        .chain(std::iter::repeat(b'0'))
                        .take(length),
                )
        });
        if a.sign < 0 {
            abs.reverse()
        } else {
            abs
        }
    }
}
impl fmt::Display for DecimalValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.sign < 0 {
            f.write_str("-")?
        }
        let digits = &self.0.digits;
        let scale = self.scale();
        if scale <= 0 {
            f.write_str(digits)?;
            if self.0.sign != 0 {
                for _ in 0..-scale {
                    f.write_str("0")?
                }
            }
        } else if digits.len() > scale as usize {
            let at = digits.len() - scale as usize;
            f.write_str(&digits[..at])?;
            f.write_str(".")?;
            f.write_str(&digits[at..])?
        } else {
            f.write_str("0.")?;
            for _ in 0..scale as usize - digits.len() {
                f.write_str("0")?
            }
            f.write_str(digits)?
        }
        Ok(())
    }
}
fn ten(exponent: u32) -> BigInt {
    BigInt::from(10).pow(exponent)
}
fn precision(p: usize) -> Result<(), Error> {
    if p == 0 || p > MAX_PRECISION {
        Err(Error::Precision)
    } else {
        Ok(())
    }
}
fn check_scale(scale: i32) -> Result<(), Error> {
    if !(-MAX_SCALE..=MAX_SCALE).contains(&scale) {
        Err(Error::Scale)
    } else {
        Ok(())
    }
}
fn rounded(coefficient: &BigInt, divisor: &BigInt, mode: Rounding) -> Result<BigInt, Error> {
    let (mut q, r) = coefficient.div_rem(divisor);
    if r.is_zero() {
        return Ok(q);
    }
    let positive = coefficient.sign() != Sign::Minus;
    let half = (r.abs() * 2u8).cmp(divisor);
    let up = match mode {
        Rounding::TowardZero => false,
        Rounding::AwayFromZero => true,
        Rounding::Floor => !positive,
        Rounding::Ceiling => positive,
        Rounding::HalfUp => half != Ordering::Less,
        Rounding::HalfDown => half == Ordering::Greater,
        Rounding::HalfEven => half == Ordering::Greater || (half == Ordering::Equal && q.is_odd()),
        Rounding::Exact => return Err(Error::Inexact),
    };
    if up {
        q += if positive { 1 } else { -1 }
    }
    Ok(q)
}
impl DecimalValue {
    fn checked(coefficient: BigInt, scale: i32) -> Result<Self, Error> {
        check_scale(scale)?;
        if coefficient.bits() > MAX_PRECISION as u64 * 4 {
            return Err(Error::Size);
        }
        let text = coefficient.to_str_radix(10);
        let digits = text.trim_start_matches('-');
        if digits.len() > MAX_PRECISION {
            return Err(Error::Precision);
        }
        let sign: i8 = match coefficient.sign() {
            Sign::Minus => -1,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        };
        let normalized_digits = if sign == 0 {
            "0"
        } else {
            digits.trim_end_matches('0')
        };
        let normalized_scale = if sign == 0 {
            0
        } else {
            scale - (digits.len() - normalized_digits.len()) as i32
        };
        let mut h = Sha256::new();
        h.update(sign.to_le_bytes());
        h.update(normalized_digits.as_bytes());
        h.update(normalized_scale.to_le_bytes());
        let canonical_scale = if sign == 0 {
            0
        } else {
            normalized_scale.max(-MAX_SCALE)
        };
        let canonical = if canonical_scale != scale {
            let coefficient = if sign == 0 {
                BigInt::zero()
            } else {
                &coefficient / ten((scale - canonical_scale) as u32)
            };
            Some(Self::checked(coefficient, canonical_scale)?.0)
        } else {
            None
        };
        Ok(Self(Arc::new(Inner {
            coefficient: IntegerValue::checked(coefficient).map_err(|_| Error::Size)?,
            scale,
            digits: digits.into(),
            normalized_scale,
            normalized_digits: normalized_digits.into(),
            sign,
            digest: h.finalize().into(),
            canonical,
        })))
    }
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.len() > MAX_PRECISION * 2 + 32 {
            return Err(Error::Size);
        }
        let (mantissa, exponent) = match text.find(['e', 'E']) {
            Some(at) => {
                let exp = &text[at + 1..];
                if exp.is_empty() {
                    return Err(Error::Syntax);
                }
                (&text[..at], exp.parse::<i32>().map_err(|_| Error::Syntax)?)
            }
            None => (text, 0),
        };
        let negative = mantissa.starts_with('-');
        let mantissa = mantissa.strip_prefix(['+', '-']).unwrap_or(mantissa);
        let (whole, fraction) = match mantissa.split_once('.') {
            Some(parts) => parts,
            None => (mantissa, ""),
        };
        if whole.is_empty() && fraction.is_empty()
            || !whole
                .bytes()
                .chain(fraction.bytes())
                .all(|c| c.is_ascii_digit())
        {
            return Err(Error::Syntax);
        }
        let scale = (fraction.len() as i32)
            .checked_sub(exponent)
            .ok_or(Error::Scale)?;
        check_scale(scale)?;
        let mut digits = format!("{whole}{fraction}");
        if negative {
            digits.insert(0, '-')
        }
        Self::checked(
            BigInt::parse_bytes(digits.as_bytes(), 10).ok_or(Error::Syntax)?,
            scale,
        )
    }
    pub fn from_coefficient(coefficient: &IntegerValue, scale: i32) -> Result<Self, Error> {
        Self::checked(coefficient.native().clone(), scale)
    }
    pub fn coefficient(&self) -> IntegerValue {
        self.0.coefficient.clone()
    }
    pub fn same_representation(&self, r: &Self) -> bool {
        Arc::ptr_eq(&self.0, &r.0)
            || (self.scale() == r.scale() && self.0.coefficient == r.0.coefficient)
    }
    pub fn canonical(&self) -> Self {
        Self(self.0.canonical.clone().unwrap_or_else(|| self.0.clone()))
    }
    pub fn scale(&self) -> i32 {
        self.0.scale
    }
    pub fn representation(&self) -> String {
        if self.scale() >= 0 {
            self.to_string()
        } else {
            format!(
                "{}{}e{}",
                if self.0.sign < 0 { "-" } else { "" },
                self.0.digits,
                -self.scale()
            )
        }
    }
    pub fn digits(&self) -> usize {
        self.0.digits.len()
    }
    pub fn formatted_len(&self) -> usize {
        let digits = self.0.digits.len();
        let scale = self.scale();
        if scale <= 0 && self.0.sign == 0 {
            return 1;
        }
        (if scale <= 0 {
            digits + (-scale) as usize
        } else {
            digits.max(scale as usize + 1) + 1
        }) + usize::from(self.0.sign < 0)
    }
    pub fn retained_bytes(&self) -> usize {
        self.0.coefficient.retained_bytes()
            + self.0.digits.capacity()
            + self.0.normalized_digits.capacity()
            + 192
            + self
                .0
                .canonical
                .as_ref()
                .map_or(0, |inner| Self(inner.clone()).retained_bytes())
    }
    pub fn numeric_hash(&self) -> u64 {
        u64::from_le_bytes(self.0.digest[..8].try_into().unwrap())
    }
    fn with_precision(
        coefficient: BigInt,
        mut scale: i32,
        p: usize,
        mode: Rounding,
    ) -> Result<Self, Error> {
        precision(p)?;
        let length = coefficient.to_str_radix(10).trim_start_matches('-').len();
        let mut coefficient = if length > p {
            let drop = (length - p) as u32;
            scale -= drop as i32;
            rounded(&coefficient, &ten(drop), mode)?
        } else {
            coefficient
        };
        // Carry after rounding can introduce one extra significant digit.
        if coefficient.to_str_radix(10).trim_start_matches('-').len() > p {
            coefficient /= 10;
            scale -= 1
        }
        Self::checked(coefficient, scale)
    }
    pub fn binary(&self, op: &str, rhs: &Self, p: usize, mode: Rounding) -> Result<Self, Error> {
        precision(p)?;
        let a = self.0.coefficient.native();
        let b = rhs.0.coefficient.native();
        let (value, scale) = match op {
            "add" | "sub" => {
                let scale = self.scale().max(rhs.scale());
                let a = a * ten((scale - self.scale()) as u32);
                let b = b * ten((scale - rhs.scale()) as u32);
                (if op == "add" { a + b } else { a - b }, scale)
            }
            "mul" => (a * b, self.scale() + rhs.scale()),
            _ => return Err(Error::Domain),
        };
        Self::with_precision(value, scale, p, mode)
    }
    pub fn quantize(&self, scale: i32, p: usize, mode: Rounding) -> Result<Self, Error> {
        precision(p)?;
        check_scale(scale)?;
        let delta = scale - self.scale();
        let a = self.0.coefficient.native();
        let value = if delta >= 0 {
            a * ten(delta as u32)
        } else {
            rounded(a, &ten((-delta) as u32), mode)?
        };
        if value.to_str_radix(10).trim_start_matches('-').len() > p {
            return Err(Error::Precision);
        }
        Self::checked(value, scale)
    }
    pub fn divide(&self, rhs: &Self, scale: i32, p: usize, mode: Rounding) -> Result<Self, Error> {
        precision(p)?;
        check_scale(scale)?;
        let a = self.0.coefficient.native();
        let b = rhs.0.coefficient.native();
        if b.is_zero() {
            return Err(Error::DivisionByZero);
        }
        let delta = rhs.scale() + scale - self.scale();
        let (numerator, denominator) = if delta >= 0 {
            (a * ten(delta as u32), b.clone())
        } else {
            (a.clone(), b * ten((-delta) as u32))
        };
        let (numerator, denominator) = if denominator.sign() == Sign::Minus {
            (-numerator, -denominator)
        } else {
            (numerator, denominator)
        };
        let value = rounded(&numerator, &denominator, mode)?;
        if value.to_str_radix(10).trim_start_matches('-').len() > p {
            return Err(Error::Precision);
        }
        Self::checked(value, scale)
    }
}
impl serde::Serialize for DecimalValue {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (&self.0.coefficient, self.scale()).serialize(s)
    }
}
impl<'de> serde::Deserialize<'de> for DecimalValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (coefficient, scale) = <(IntegerValue, i32)>::deserialize(d)?;
        Self::from_coefficient(&coefficient, scale)
            .map_err(|e| serde::de::Error::custom(format!("Decimal {e:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dec(s: &str) -> DecimalValue {
        DecimalValue::parse(s).unwrap()
    }
    #[test]
    fn exact_arithmetic_scale_and_no_binary_float() {
        assert_eq!(
            dec("0.1")
                .binary("add", &dec("0.2"), 28, Rounding::Exact)
                .unwrap()
                .to_string(),
            "0.3"
        );
        assert_eq!(
            dec("12345678901234567890.123456789")
                .binary("sub", &dec("12345678901234567890"), 28, Rounding::Exact)
                .unwrap()
                .to_string(),
            "0.123456789"
        );
        assert_eq!(dec("1.2300").scale(), 4);
        assert_eq!(dec("1.2300"), dec("1.23"));
        assert_eq!(dec("1.2300").numeric_hash(), dec("1.23").numeric_hash());
        assert_eq!(dec("-0.00"), dec("0"));
        assert_eq!(dec("1e3").to_string(), "1000");
        for text in ["1e10000", "0e10000", "1.2300", "1e-10000"] {
            let value = dec(text);
            let restored = dec(&value.representation());
            assert_eq!(restored, value);
            assert_eq!(restored.scale(), value.scale());
        }
        assert_eq!(dec(".5").to_string(), "0.5");
        for values in [
            ("9.999", "10"),
            ("-9.999", "-10"),
            ("0.009", "0.01"),
            ("1.23", "1.2301"),
        ] {
            assert!(dec(values.0).abs_cmp(&dec(values.1)) == Ordering::Less)
        }
    }
    impl DecimalValue {
        fn abs_cmp(&self, r: &Self) -> Ordering {
            if self.0.sign < 0 {
                self.cmp(r).reverse()
            } else {
                self.cmp(r)
            }
        }
    }
    #[test]
    fn rounding_ties_direction_carry_and_precision() {
        for (mode, positive, negative) in [
            (Rounding::TowardZero, "2", "-2"),
            (Rounding::AwayFromZero, "3", "-3"),
            (Rounding::Floor, "2", "-3"),
            (Rounding::Ceiling, "3", "-2"),
            (Rounding::HalfUp, "3", "-3"),
            (Rounding::HalfDown, "2", "-2"),
            (Rounding::HalfEven, "2", "-2"),
        ] {
            assert_eq!(
                dec("2.5").quantize(0, 10, mode).unwrap().to_string(),
                positive
            );
            assert_eq!(
                dec("-2.5").quantize(0, 10, mode).unwrap().to_string(),
                negative
            );
        }
        assert_eq!(
            dec("3.5")
                .quantize(0, 10, Rounding::HalfEven)
                .unwrap()
                .to_string(),
            "4"
        );
        assert_eq!(
            dec("99.9")
                .binary("add", &dec("0"), 2, Rounding::HalfEven)
                .unwrap()
                .to_string(),
            "100"
        );
        assert_eq!(
            dec("1.01").quantize(1, 10, Rounding::Exact),
            Err(Error::Inexact)
        );
        assert_eq!(
            dec("100").quantize(2, 3, Rounding::Exact),
            Err(Error::Precision)
        );
    }
    #[test]
    fn vm_storage_tracks_scale_and_map_keys_are_canonical() {
        let a = dec("1.00");
        let b = dec("1.0");
        assert_eq!(a, b);
        assert_ne!(StoredDecimal(a.clone()), StoredDecimal(b.clone()));
        let ka = crate::MapKey::from_value(&crate::Value::Decimal(a.into())).unwrap();
        let kb = crate::MapKey::from_value(&crate::Value::Decimal(b.into())).unwrap();
        assert_eq!(ka, kb);
        assert_eq!(ka.value().to_string(), "1");
        assert!(serde_json::from_str::<crate::MapKey>("{\"Decimal\":[\"64\",2]}").is_err());
    }
    #[test]
    fn division_and_limits_are_explicit() {
        assert_eq!(
            dec("1")
                .divide(&dec("3"), 5, 10, Rounding::HalfEven)
                .unwrap()
                .to_string(),
            "0.33333"
        );
        assert_eq!(
            dec("1")
                .divide(&dec("-8"), 2, 10, Rounding::HalfEven)
                .unwrap()
                .to_string(),
            "-0.12"
        );
        assert_eq!(
            dec("1").divide(&dec("0"), 2, 10, Rounding::Exact),
            Err(Error::DivisionByZero)
        );
        assert_eq!(
            dec("1").divide(&dec("3"), 2, 10, Rounding::Exact),
            Err(Error::Inexact)
        );
        assert_eq!(DecimalValue::parse("1e10001"), Err(Error::Scale));
        assert_eq!(DecimalValue::parse("NaN"), Err(Error::Syntax));
        assert_eq!(
            dec("1").binary("add", &dec("1"), 0, Rounding::Exact),
            Err(Error::Precision)
        );
    }
    #[test]
    fn wire_preserves_representation_and_rejects_invalid_scale_or_size() {
        let value = dec("1.2300");
        let encoded = serde_json::to_string(&value).unwrap();
        let restored: DecimalValue = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.scale(), 4);
        assert_eq!(restored.to_string(), "1.2300");
        assert!(serde_json::from_str::<DecimalValue>("[\"1\",10001]").is_err());
        let huge = IntegerValue::from_int(2).pow(50_000).unwrap();
        let json = serde_json::to_string(&(huge, 0)).unwrap();
        assert!(serde_json::from_str::<DecimalValue>(&json).is_err());
    }
}
