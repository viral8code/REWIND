//! Finite PostgreSQL timestamptz: signed microseconds since 2000-01-01 UTC.
use super::{Error, Instant, Result};
const EPOCH: i64 = 946_684_800;
pub fn encode(value: Instant) -> Result<i64> {
    if value.nanos() % 1000 != 0 {
        return Err(Error::Range);
    }
    i64::try_from(
        (value.seconds() as i128 - EPOCH as i128) * 1_000_000 + value.nanos() as i128 / 1000,
    )
    .map_err(|_| Error::Range)
}
pub fn decode(micros: i64) -> Result<Instant> {
    if micros == i64::MIN || micros == i64::MAX {
        return Err(Error::Range);
    }
    Instant::new(
        micros
            .div_euclid(1_000_000)
            .checked_add(EPOCH)
            .ok_or(Error::Range)?,
        micros.rem_euclid(1_000_000) * 1000,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn epoch_negative_microseconds_precision_and_infinity() {
        assert_eq!(decode(0).unwrap().format(), "2000-01-01T00:00:00Z");
        assert_eq!(decode(-1).unwrap().format(), "1999-12-31T23:59:59.999999Z");
        for text in [
            "0001-01-01T00:00:00Z",
            "9999-12-31T23:59:59.999999Z",
            "1969-12-31T23:59:59.999999Z",
        ] {
            let value = Instant::parse(text).unwrap();
            assert_eq!(decode(encode(value).unwrap()).unwrap(), value);
        }
        assert!(encode(Instant::new(0, 1).unwrap()).is_err());
        assert!(decode(i64::MAX).is_err());
        assert!(decode(i64::MIN).is_err());
    }
}
