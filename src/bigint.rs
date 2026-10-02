//! Bounded immutable exact integers. No VM heap references or external state.
use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};
use sha2::{Digest, Sha256};
use std::{fmt, sync::Arc};

pub const MAX_BITS: u64 = 262_144;
pub const MAX_INPUT: usize = MAX_BITS as usize;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Syntax,
    Radix,
    Size,
    DivisionByZero,
    Domain,
    Overflow,
}
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Inner {
    number: BigInt,
    digest: [u8; 32],
}
#[derive(Clone)]
pub struct IntegerValue(Arc<Inner>);
impl PartialEq for IntegerValue {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0.number == other.0.number
    }
}
impl Eq for IntegerValue {}
impl PartialOrd for IntegerValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for IntegerValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.number.cmp(&other.0.number)
    }
}
impl fmt::Debug for IntegerValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BigInt")
            .field("bits", &self.bits())
            .field("sha256", &self.0.digest)
            .finish()
    }
}
impl fmt::Display for IntegerValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.number.fmt(f)
    }
}
impl IntegerValue {
    pub(crate) fn native(&self) -> &BigInt {
        &self.0.number
    }
    pub fn from_int(n: i64) -> Self {
        Self::checked(BigInt::from(n)).unwrap()
    }
    pub fn checked(number: BigInt) -> Result<Self, Error> {
        if number.bits() > MAX_BITS {
            return Err(Error::Size);
        }
        let (sign, bytes) = number.to_bytes_le();
        let mut digest = Sha256::new();
        digest.update([match sign {
            Sign::Minus => 2,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        }]);
        digest.update(&bytes);
        // Arithmetic can leave a limb buffer with spare capacity after cancellation.
        // Retain only the canonical magnitude, so retained_bytes follows storage size.
        let number = BigInt::from_bytes_le(sign, &bytes);
        Ok(Self(Arc::new(Inner {
            number,
            digest: digest.finalize().into(),
        })))
    }
    pub fn bits(&self) -> u64 {
        self.0.number.bits()
    }
    pub fn retained_bytes(&self) -> usize {
        (self.bits() as usize).div_ceil(8) + 128
    }
    pub fn parse(text: &str, radix: u32) -> Result<Self, Error> {
        if !(2..=36).contains(&radix) {
            return Err(Error::Radix);
        }
        if text.len() > MAX_INPUT {
            return Err(Error::Size);
        }
        let digits = text
            .strip_prefix('-')
            .or_else(|| text.strip_prefix('+'))
            .unwrap_or(text);
        if digits.is_empty()
            || !digits
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() && (c as char).to_digit(radix).is_some())
        {
            return Err(Error::Syntax);
        }
        Self::checked(BigInt::parse_bytes(text.as_bytes(), radix).ok_or(Error::Syntax)?)
    }
    pub fn format(&self, radix: u32) -> Result<String, Error> {
        if !(2..=36).contains(&radix) {
            return Err(Error::Radix);
        }
        Ok(self.0.number.to_str_radix(radix))
    }
    pub fn to_int(&self) -> Result<i64, Error> {
        self.0.number.to_i64().ok_or(Error::Overflow)
    }
    pub fn binary(&self, op: &str, rhs: &Self) -> Result<Self, Error> {
        let a = &self.0.number;
        let b = &rhs.0.number;
        if matches!(op, "div" | "rem" | "mod") && b.is_zero() {
            return Err(Error::DivisionByZero);
        }
        if op == "mul"
            && !a.is_zero()
            && !b.is_zero()
            && self.bits().saturating_add(rhs.bits()).saturating_sub(1) > MAX_BITS
        {
            return Err(Error::Size);
        }
        Self::checked(match op {
            "add" => a + b,
            "sub" => a - b,
            "mul" => a * b,
            "div" => a / b,
            "rem" => a % b,
            "mod" => {
                if b.sign() != Sign::Plus {
                    return Err(Error::Domain);
                }
                a.mod_floor(b)
            }
            "and" => a & b,
            "or" => a | b,
            "xor" => a ^ b,
            "gcd" => a.gcd(b),
            _ => return Err(Error::Domain),
        })
    }
    pub fn unary(&self, op: &str) -> Result<Self, Error> {
        Self::checked(match op {
            "neg" => -&self.0.number,
            "not" => !&self.0.number,
            _ => return Err(Error::Domain),
        })
    }
    pub fn shift(&self, left: bool, count: u64) -> Result<Self, Error> {
        if left && !self.0.number.is_zero() && self.bits().saturating_add(count) > MAX_BITS {
            return Err(Error::Size);
        }
        if !left && count >= self.bits() + 1 {
            return Ok(Self::from_int(if self.0.number.sign() == Sign::Minus {
                -1
            } else {
                0
            }));
        }
        if self.0.number.is_zero() {
            return Ok(self.clone());
        }
        let count = usize::try_from(count).map_err(|_| Error::Size)?;
        Self::checked(if left {
            &self.0.number << count
        } else {
            &self.0.number >> count
        })
    }
    pub fn pow(&self, exponent: u32) -> Result<Self, Error> {
        let abs_bits = self.bits();
        if abs_bits > 1
            && abs_bits
                .saturating_sub(1)
                .saturating_mul(exponent as u64)
                .saturating_add(1)
                > MAX_BITS
        {
            return Err(Error::Size);
        }
        Self::checked(self.0.number.pow(exponent))
    }
    pub fn mod_pow(&self, exponent: &Self, modulus: &Self) -> Result<Self, Error> {
        if exponent.0.number.sign() == Sign::Minus || modulus.0.number <= BigInt::zero() {
            return Err(Error::Domain);
        }
        Self::checked(self.0.number.modpow(&exponent.0.number, &modulus.0.number))
    }
}
impl serde::Serialize for IntegerValue {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.number.to_str_radix(16))
    }
}
impl<'de> serde::Deserialize<'de> for IntegerValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = IntegerValue;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded canonical signed hexadecimal integer")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if s.len() > (MAX_BITS as usize).div_ceil(4) + 1 {
                    return Err(E::custom("BigInt wire size"));
                }
                let value =
                    IntegerValue::parse(s, 16).map_err(|e| E::custom(format!("BigInt {e:?}")))?;
                if value.format(16).unwrap() != s {
                    return Err(E::custom("noncanonical BigInt wire"));
                }
                Ok(value)
            }
        }
        d.deserialize_str(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_arithmetic_division_and_signed_bits() {
        let a = IntegerValue::parse("123456789012345678901234567890", 10).unwrap();
        assert_eq!(
            a.binary("mul", &IntegerValue::from_int(9))
                .unwrap()
                .format(10)
                .unwrap(),
            "1111111101111111110111111111010"
        );
        for x in -100..=100 {
            for y in -17..=17 {
                if y == 0 {
                    continue;
                }
                let a = IntegerValue::from_int(x);
                let b = IntegerValue::from_int(y);
                let q = a.binary("div", &b).unwrap();
                let r = a.binary("rem", &b).unwrap();
                assert_eq!(q.binary("mul", &b).unwrap().binary("add", &r).unwrap(), a);
                assert_eq!(q.to_int().unwrap(), x / y);
                assert_eq!(r.to_int().unwrap(), x % y);
            }
        }
        assert_eq!(
            IntegerValue::from_int(-3)
                .shift(false, 1)
                .unwrap()
                .to_int()
                .unwrap(),
            -2
        );
        assert_eq!(
            IntegerValue::from_int(-3)
                .binary("mod", &IntegerValue::from_int(5))
                .unwrap()
                .to_int()
                .unwrap(),
            2
        );
        assert_eq!(
            IntegerValue::from_int(-1)
                .unary("not")
                .unwrap()
                .to_int()
                .unwrap(),
            0
        );
    }
    #[test]
    fn powers_limits_and_conversions() {
        let two = IntegerValue::from_int(2);
        let value = two.pow(100).unwrap();
        assert_eq!(value.format(10).unwrap(), "1267650600228229401496703205376");
        assert_eq!(value.to_int(), Err(Error::Overflow));
        assert_eq!(
            two.mod_pow(&IntegerValue::from_int(100), &IntegerValue::from_int(97))
                .unwrap()
                .to_int()
                .unwrap(),
            16
        );
        assert_eq!(two.shift(true, MAX_BITS), Err(Error::Size));
        assert_eq!(two.pow(MAX_BITS as u32), Err(Error::Size));
        assert_eq!(
            two.binary("div", &IntegerValue::from_int(0)),
            Err(Error::DivisionByZero)
        );
        assert_eq!(IntegerValue::parse("1_000", 10), Err(Error::Syntax));
        assert_eq!(IntegerValue::parse(" 1", 10), Err(Error::Syntax));
        assert_eq!(IntegerValue::parse("1", 1), Err(Error::Radix));
        assert_eq!(IntegerValue::from_int(i64::MIN).to_int().unwrap(), i64::MIN);
    }
    #[test]
    fn wire_is_bounded_canonical_and_shares_snapshots() {
        let original = IntegerValue::from_int(-255);
        let clone = original.clone();
        assert!(Arc::ptr_eq(&original.0, &clone.0));
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"-ff\"");
        assert_eq!(
            serde_json::from_str::<IntegerValue>(&json).unwrap(),
            original
        );
        for s in ["+ff", "FF", "00", "-0", "0x10"] {
            assert!(serde_json::from_str::<IntegerValue>(&format!("\"{s}\"")).is_err())
        }
        assert!(serde_json::from_str::<IntegerValue>(&format!(
            "\"{}\"",
            "1".repeat(MAX_BITS as usize / 4 + 2)
        ))
        .is_err());
    }
}
