//! Exact POSIX instants and durations; pure calendar conversion uses bundled IANA data.
pub mod postgres;
use chrono::{
    DateTime, Datelike, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, Offset, SecondsFormat,
    TimeZone, Timelike, Utc,
};
use chrono_tz::Tz;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

const BILLION: i128 = 1_000_000_000;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Range,
    Syntax,
    Calendar,
    Zone,
    Offset,
    Ambiguous,
    Gap,
    Policy,
    Type,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Instant {
    seconds: i64,
    nanos: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Duration {
    seconds: i64,
    nanos: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parts {
    seconds: i64,
    nanos: u32,
}
impl<'de> Deserialize<'de> for Instant {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let p = Parts::deserialize(d)?;
        Self::new(p.seconds, p.nanos as i64)
            .map_err(|e| serde::de::Error::custom(format!("DateTime{e:?}")))
    }
}
impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let p = Parts::deserialize(d)?;
        if p.nanos >= 1_000_000_000 {
            return Err(serde::de::Error::custom("noncanonical duration"));
        }
        Ok(Self {
            seconds: p.seconds,
            nanos: p.nanos,
        })
    }
}
impl Instant {
    pub fn new(seconds: i64, nanos: i64) -> Result<Self> {
        let nanos = u32::try_from(nanos).map_err(|_| Error::Range)?;
        if nanos >= 1_000_000_000 {
            return Err(Error::Range);
        }
        let dt = DateTime::from_timestamp(seconds, nanos).ok_or(Error::Range)?;
        if !(1..=9999).contains(&dt.year()) {
            return Err(Error::Range);
        }
        Ok(Self { seconds, nanos })
    }
    pub fn seconds(self) -> i64 {
        self.seconds
    }
    pub fn nanos(self) -> i64 {
        self.nanos as i64
    }
    fn datetime(self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.seconds, self.nanos).expect("validated instant")
    }
    fn from_datetime<T: TimeZone>(dt: DateTime<T>) -> Result<Self> {
        Self::new(dt.timestamp(), dt.timestamp_subsec_nanos() as i64)
    }
    pub fn parse(text: &str) -> Result<Self> {
        // Require a full RFC3339 offset, ASCII, bounded fractional precision; reject leap seconds.
        if text.len() > 40
            || !text.is_ascii()
            || text.len() < 20
            || text.as_bytes().get(10) != Some(&b'T')
        {
            return Err(Error::Syntax);
        }
        let bytes = text.as_bytes();
        for (i, c) in [(4, b'-'), (7, b'-'), (13, b':'), (16, b':')] {
            if bytes.get(i) != Some(&c) {
                return Err(Error::Syntax);
            }
        }
        for i in [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18] {
            if !bytes[i].is_ascii_digit() {
                return Err(Error::Syntax);
            }
        }
        let mut suffix = 19;
        if bytes.get(suffix) == Some(&b'.') {
            suffix += 1;
            let start = suffix;
            while bytes.get(suffix).is_some_and(u8::is_ascii_digit) {
                suffix += 1;
            }
            if suffix - start == 0 || suffix - start > 9 {
                return Err(Error::Syntax);
            }
        }
        let ending = &text[suffix..];
        if ending != "Z" {
            let e = ending.as_bytes();
            if e.len() != 6
                || !matches!(e[0], b'+' | b'-')
                || e[3] != b':'
                || ![e[1], e[2], e[4], e[5]].iter().all(u8::is_ascii_digit)
                || ending == "-00:00"
            {
                return Err(Error::Syntax);
            }
        }
        let dt = DateTime::parse_from_rfc3339(text).map_err(|_| Error::Syntax)?;
        if dt.nanosecond() >= 1_000_000_000 {
            return Err(Error::Calendar);
        }
        // Chrono accepts excess fractional digits by truncation; this API never does.
        if let Some(dot) = text.find('.') {
            let count = text[dot + 1..]
                .bytes()
                .take_while(u8::is_ascii_digit)
                .count();
            if count == 0 || count > 9 {
                return Err(Error::Syntax);
            }
        }
        Self::from_datetime(dt)
    }
    pub fn format(self) -> String {
        self.datetime().to_rfc3339_opts(SecondsFormat::AutoSi, true)
    }
    pub fn format_offset(self, offset: i64) -> Result<String> {
        if offset % 60 != 0 {
            return Err(Error::Offset);
        }
        let dt = self.datetime().with_timezone(&fixed(offset)?);
        if !(1..=9999).contains(&dt.year()) {
            return Err(Error::Range);
        }
        Ok(dt.to_rfc3339_opts(SecondsFormat::AutoSi, false))
    }
    pub fn add(self, duration: Duration) -> Result<Self> {
        let total = self.total() + duration.total();
        Self::new(
            i64::try_from(total.div_euclid(BILLION)).map_err(|_| Error::Range)?,
            total.rem_euclid(BILLION) as i64,
        )
    }
    pub fn difference(self, other: Self) -> Result<Duration> {
        Duration::from_total(self.total() - other.total())
    }
    fn total(self) -> i128 {
        self.seconds as i128 * BILLION + self.nanos as i128
    }
    pub fn calendar(self, zone: &str) -> Result<Vec<i64>> {
        let zone = parse_zone(zone)?;
        fields(self.datetime().with_timezone(&zone))
    }
    pub fn calendar_offset(self, offset: i64) -> Result<Vec<i64>> {
        fields(self.datetime().with_timezone(&fixed(offset)?))
    }
}
impl Duration {
    pub fn new(seconds: i64, nanos: i64) -> Result<Self> {
        Self::from_total(seconds as i128 * BILLION + nanos as i128)
    }
    fn from_total(total: i128) -> Result<Self> {
        Ok(Self {
            seconds: i64::try_from(total.div_euclid(BILLION)).map_err(|_| Error::Range)?,
            nanos: total.rem_euclid(BILLION) as u32,
        })
    }
    pub fn seconds(self) -> i64 {
        self.seconds
    }
    pub fn nanos(self) -> i64 {
        self.nanos as i64
    }
    fn total(self) -> i128 {
        self.seconds as i128 * BILLION + self.nanos as i128
    }
    pub fn add(self, other: Self) -> Result<Self> {
        Self::from_total(self.total() + other.total())
    }
    pub fn subtract(self, other: Self) -> Result<Self> {
        Self::from_total(self.total() - other.total())
    }
    pub fn negate(self) -> Result<Self> {
        Self::from_total(-self.total())
    }
}
impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format())
    }
}
impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Duration({}, {})", self.seconds, self.nanos)
    }
}
fn fixed(offset: i64) -> Result<FixedOffset> {
    FixedOffset::east_opt(i32::try_from(offset).map_err(|_| Error::Offset)?).ok_or(Error::Offset)
}
fn parse_zone(zone: &str) -> Result<Tz> {
    if zone.len() > 128 || !zone.is_ascii() {
        return Err(Error::Zone);
    }
    zone.parse().map_err(|_| Error::Zone)
}
fn local(values: &[i64]) -> Result<NaiveDateTime> {
    if values.len() != 7
        || !(1..=9999).contains(&values[0])
        || !(0..1_000_000_000).contains(&values[6])
    {
        return Err(Error::Calendar);
    }
    let part = |i: usize| u32::try_from(values[i]).map_err(|_| Error::Calendar);
    NaiveDate::from_ymd_opt(values[0] as i32, part(1)?, part(2)?)
        .and_then(|d| {
            d.and_hms_nano_opt(part(3).ok()?, part(4).ok()?, part(5).ok()?, part(6).ok()?)
        })
        .ok_or(Error::Calendar)
}
fn fields<T: TimeZone>(dt: DateTime<T>) -> Result<Vec<i64>> {
    if !(1..=9999).contains(&dt.year()) {
        return Err(Error::Range);
    }
    Ok(vec![
        dt.year() as i64,
        dt.month() as i64,
        dt.day() as i64,
        dt.hour() as i64,
        dt.minute() as i64,
        dt.second() as i64,
        dt.nanosecond() as i64,
        dt.offset().fix().local_minus_utc() as i64,
        dt.weekday().number_from_monday() as i64,
        dt.ordinal() as i64,
    ])
}
pub fn resolve(values: &[i64], zone: &str, policy: i64) -> Result<Instant> {
    if !(0..=2).contains(&policy) {
        return Err(Error::Policy);
    }
    match parse_zone(zone)?.from_local_datetime(&local(values)?) {
        LocalResult::None => Err(Error::Gap),
        LocalResult::Single(dt) => Instant::from_datetime(dt),
        LocalResult::Ambiguous(a, b) => match policy {
            1 => Instant::from_datetime(a.min(b)),
            2 => Instant::from_datetime(a.max(b)),
            _ => Err(Error::Ambiguous),
        },
    }
}
pub fn resolve_offset(values: &[i64], offset: i64) -> Result<Instant> {
    Instant::from_datetime(
        fixed(offset)?
            .from_local_datetime(&local(values)?)
            .single()
            .ok_or(Error::Range)?,
    )
}
pub fn database_version() -> &'static str {
    chrono_tz::IANA_TZDB_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_subsecond_pre_epoch_and_checked_duration() {
        let a = Instant::parse("1969-12-31T23:59:59.999999999Z").unwrap();
        assert_eq!(a.seconds(), -1);
        assert_eq!(a.nanos(), 999_999_999);
        assert_eq!(
            a.add(Duration::new(0, 1).unwrap()).unwrap(),
            Instant::new(0, 0).unwrap()
        );
        assert_eq!(
            Duration::new(0, -1).unwrap(),
            Duration::new(-1, 999_999_999).unwrap()
        );
        assert!(Duration::new(i64::MAX, 1_000_000_000).is_err());
        assert!(Duration::new(i64::MIN, 0).unwrap().negate().is_err());
        assert_eq!(
            a.difference(Instant::new(0, 0).unwrap()).unwrap(),
            Duration::new(0, -1).unwrap()
        );
    }
    #[test]
    fn strict_parse_and_calendar() {
        for s in [
            "2024-01-01T00:00:00",
            "2024-02-30T00:00:00Z",
            "2016-12-31T23:59:60Z",
            "2024-01-01T00:00:00.1234567890Z",
            "0000-01-01T00:00:00Z",
        ] {
            assert!(Instant::parse(s).is_err(), "{s}");
        }
        let a = Instant::parse("2024-02-29T12:34:56.123456789+09:00").unwrap();
        assert_eq!(a.format(), "2024-02-29T03:34:56.123456789Z");
        assert_eq!(
            a.format_offset(32400).unwrap(),
            "2024-02-29T12:34:56.123456789+09:00"
        );
        assert_eq!(
            a.calendar("Asia/Tokyo").unwrap(),
            vec![2024, 2, 29, 12, 34, 56, 123456789, 32400, 4, 60]
        );
        assert!(resolve(&[2024, 1, 1, 0, 0, 60, 0], "UTC", 0).is_err());
        assert!(a.format_offset(1).is_err());
    }
    #[test]
    fn dst_overlap_gap_and_non_hour_transitions() {
        let c = [2024, 11, 3, 1, 30, 0, 0];
        assert_eq!(resolve(&c, "America/New_York", 0), Err(Error::Ambiguous));
        let earlier = resolve(&c, "America/New_York", 1).unwrap();
        let later = resolve(&c, "America/New_York", 2).unwrap();
        assert_eq!(
            later.difference(earlier).unwrap(),
            Duration::new(3600, 0).unwrap()
        );
        assert_eq!(
            resolve(&[2024, 3, 10, 2, 30, 0, 0], "America/New_York", 1),
            Err(Error::Gap)
        );
        let c = [2024, 4, 7, 1, 45, 0, 0];
        let a = resolve(&c, "Australia/Lord_Howe", 1).unwrap();
        let b = resolve(&c, "Australia/Lord_Howe", 2).unwrap();
        assert_eq!(b.difference(a).unwrap(), Duration::new(1800, 0).unwrap());
        assert_eq!(
            resolve(&[2011, 12, 30, 12, 0, 0, 0], "Pacific/Apia", 0),
            Err(Error::Gap)
        );
    }
    #[test]
    fn serde_rejects_invalid_instants_and_noncanonical_durations() {
        assert!(serde_json::from_str::<Instant>(r#"{"seconds":0,"nanos":1000000000}"#).is_err());
        assert!(serde_json::from_str::<Duration>(r#"{"seconds":0,"nanos":1000000000}"#).is_err());
        let a = Instant::new(-1, 1).unwrap();
        assert_eq!(
            serde_json::from_str::<Instant>(&serde_json::to_string(&a).unwrap()).unwrap(),
            a
        );
    }
}
