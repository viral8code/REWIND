//! Incremental UTF-8 CSV with bounded rows and persistent pending pages.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::VecDeque, fmt, sync::Arc};
pub const CHUNK: usize = 65_536;
pub const FIELD: usize = 1024 * 1024;
pub const ROW: usize = 4 * 1024 * 1024;
pub const COLUMNS: usize = 4096;
pub const QUEUE: usize = 32;
pub const QUEUE_BYTES: usize = 8 * 1024 * 1024;
const PAGE: usize = 4096;
#[derive(Clone, PartialEq, Eq)]
struct Page {
    previous: Option<Arc<Page>>,
    bytes: Vec<u8>,
}
#[derive(Clone, Default, PartialEq, Eq)]
struct Pending {
    pages: Option<Arc<Page>>,
    tail: Vec<u8>,
    len: usize,
}
impl Pending {
    fn push(&mut self, byte: u8) {
        if self.tail.len() == PAGE {
            self.pages = Some(Arc::new(Page {
                previous: self.pages.take(),
                bytes: std::mem::take(&mut self.tail),
            }));
        }
        self.tail.push(byte);
        self.len += 1;
    }
    fn bytes(&self) -> Vec<u8> {
        let mut pages = Vec::new();
        let mut node = self.pages.as_ref();
        while let Some(p) = node {
            pages.push(p);
            node = p.previous.as_ref();
        }
        let mut out = Vec::with_capacity(self.len);
        for p in pages.into_iter().rev() {
            out.extend_from_slice(&p.bytes);
        }
        out.extend_from_slice(&self.tail);
        out
    }
}
impl Serialize for Pending {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.bytes().serialize(s)
    }
}
impl<'de> Deserialize<'de> for Pending {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Pending;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded CSV pending bytes")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Pending, A::Error> {
                let mut out = Pending::default();
                while let Some(b) = a.next_element::<u8>()? {
                    if out.len == FIELD {
                        return Err(serde::de::Error::custom("CsvFieldLimit"));
                    }
                    out.push(b);
                }
                Ok(out)
            }
        }
        d.deserialize_seq(V)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Mode {
    Start,
    Plain,
    Quoted,
    AfterQuote,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    pending: Pending,
    #[serde(deserialize_with = "bounded_fields")]
    fields: Arc<Vec<Arc<String>>>,
    #[serde(deserialize_with = "bounded_queue")]
    ready: VecDeque<Arc<Vec<String>>>,
    mode: Mode,
    cr: bool,
    started: bool,
    closed: bool,
    position: u64,
    field_start: u64,
    row_bytes: usize,
    queue_bytes: usize,
}
struct FieldText(String);
impl<'de> Deserialize<'de> for FieldText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = FieldText;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded UTF-8 CSV field")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> std::result::Result<FieldText, E> {
                if s.len() > FIELD {
                    Err(E::custom("CsvFieldLimit"))
                } else {
                    Ok(FieldText(s.into()))
                }
            }
            fn visit_string<E: serde::de::Error>(
                self,
                s: String,
            ) -> std::result::Result<FieldText, E> {
                if s.len() > FIELD {
                    Err(E::custom("CsvFieldLimit"))
                } else {
                    Ok(FieldText(s))
                }
            }
        }
        d.deserialize_str(V)
    }
}
struct RowFields(Vec<String>);
impl<'de> Deserialize<'de> for RowFields {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = RowFields;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded CSV row")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<RowFields, A::Error> {
                let mut fields = Vec::new();
                let mut bytes = 0usize;
                while let Some(FieldText(s)) = a.next_element::<FieldText>()? {
                    bytes = bytes.saturating_add(s.len());
                    if fields.len() == COLUMNS || bytes > ROW {
                        return Err(serde::de::Error::custom("CsvRowLimit"));
                    }
                    fields.push(s);
                }
                Ok(RowFields(fields))
            }
        }
        d.deserialize_seq(V)
    }
}
fn bounded_fields<'de, D: Deserializer<'de>>(
    d: D,
) -> std::result::Result<Arc<Vec<Arc<String>>>, D::Error> {
    Ok(Arc::new(
        RowFields::deserialize(d)?
            .0
            .into_iter()
            .map(Arc::new)
            .collect(),
    ))
}
fn bounded_queue<'de, D: Deserializer<'de>>(
    d: D,
) -> std::result::Result<VecDeque<Arc<Vec<String>>>, D::Error> {
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = VecDeque<Arc<Vec<String>>>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("bounded CSV queue")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut a: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut rows = VecDeque::new();
            let mut bytes = 0usize;
            while let Some(RowFields(r)) = a.next_element::<RowFields>()? {
                bytes = bytes.saturating_add(r.iter().map(String::len).sum::<usize>());
                if rows.len() == QUEUE || bytes > QUEUE_BYTES || r.is_empty() {
                    return Err(serde::de::Error::custom("CsvQueueLimit"));
                }
                rows.push_back(Arc::new(r));
            }
            Ok(rows)
        }
    }
    d.deserialize_seq(V)
}
#[derive(Clone, PartialEq, Eq)]
pub struct Reader(Arc<State>);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
    pub position: u64,
}
type Result<T> = std::result::Result<T, Error>;
impl State {
    fn error(&self, code: &'static str) -> Error {
        Error {
            code,
            position: self.position,
        }
    }
    fn push(&mut self, b: u8) -> Result<()> {
        if self.pending.len == FIELD {
            return Err(self.error("CsvFieldLimit"));
        }
        if self.row_bytes == ROW {
            return Err(self.error("CsvRowLimit"));
        }
        self.pending.push(b);
        self.row_bytes += 1;
        Ok(())
    }
    fn field(&mut self) -> Result<()> {
        if self.fields.len() == COLUMNS {
            return Err(self.error("CsvColumns"));
        }
        let bytes = self.pending.bytes();
        let text = String::from_utf8(bytes).map_err(|_| Error {
            code: "CsvUtf8",
            position: self.field_start,
        })?;
        Arc::make_mut(&mut self.fields).push(Arc::new(text));
        self.pending = Pending::default();
        self.mode = Mode::Start;
        self.field_start = self.position.saturating_add(1);
        Ok(())
    }
    fn row(&mut self) -> Result<()> {
        if self.ready.len() == QUEUE
            || self.queue_bytes.saturating_add(self.row_bytes) > QUEUE_BYTES
        {
            return Err(self.error("CsvQueueLimit"));
        }
        self.field()?;
        self.ready.push_back(Arc::new(
            std::mem::take(&mut self.fields)
                .iter()
                .map(|s| (**s).clone())
                .collect(),
        ));
        self.queue_bytes += self.row_bytes;
        self.row_bytes = 0;
        self.started = false;
        Ok(())
    }
    fn byte(&mut self, b: u8) -> Result<()> {
        if self.cr {
            if b != b'\n' {
                return Err(self.error("CsvNewline"));
            }
            self.cr = false;
            self.field_start = self.position.saturating_add(1);
            return Ok(());
        }
        self.started = true;
        match self.mode {
            Mode::Quoted => {
                if b == b'"' {
                    self.mode = Mode::AfterQuote;
                } else {
                    self.push(b)?;
                }
            }
            Mode::AfterQuote if b == b'"' => {
                self.push(b'"')?;
                self.mode = Mode::Quoted;
            }
            Mode::Start if b == b'"' => {
                self.mode = Mode::Quoted;
                self.field_start = self.position.saturating_add(1);
            }
            Mode::Plain if b == b'"' => return Err(self.error("CsvQuote")),
            _ => match b {
                b',' => self.field()?,
                b'\n' => self.row()?,
                b'\r' => {
                    self.row()?;
                    self.cr = true;
                }
                _ if self.mode == Mode::AfterQuote => return Err(self.error("CsvTrailingText")),
                _ => {
                    self.push(b)?;
                    self.mode = Mode::Plain;
                }
            },
        }
        Ok(())
    }
    fn validate(&self) -> bool {
        let row = self
            .pending
            .len
            .saturating_add(self.fields.iter().map(|s| s.len()).sum::<usize>());
        let queued = self
            .ready
            .iter()
            .flat_map(|r| r.iter())
            .map(String::len)
            .sum::<usize>();
        row == self.row_bytes
            && row <= ROW
            && self.fields.len() <= COLUMNS
            && self.ready.len() <= QUEUE
            && queued == self.queue_bytes
            && queued <= QUEUE_BYTES
            && self.ready.iter().all(|r| {
                !r.is_empty()
                    && r.len() <= COLUMNS
                    && r.iter().all(|s| s.len() <= FIELD)
                    && r.iter().map(String::len).sum::<usize>() <= ROW
            })
            && self.fields.iter().all(|s| s.len() <= FIELD)
            && self.field_start <= self.position.saturating_add(1)
            && (!self.closed
                || (self.pending.len == 0
                    && self.fields.is_empty()
                    && !self.started
                    && !self.cr
                    && self.mode == Mode::Start))
    }
}
impl Reader {
    pub fn new() -> Self {
        Self(Arc::new(State {
            pending: Pending::default(),
            fields: Arc::new(vec![]),
            ready: VecDeque::new(),
            mode: Mode::Start,
            cr: false,
            started: false,
            closed: false,
            position: 0,
            field_start: 0,
            row_bytes: 0,
            queue_bytes: 0,
        }))
    }
    pub fn feed(&self, input: &[u8], finish: bool) -> Result<Self> {
        if self.0.closed {
            return Err(self.0.error("CsvClosed"));
        }
        if input.len() > CHUNK {
            return Err(self.0.error("CsvChunkLimit"));
        }
        let mut s = (*self.0).clone();
        for &b in input {
            s.byte(b)?;
            s.position = s
                .position
                .checked_add(1)
                .ok_or_else(|| s.error("CsvPositionLimit"))?;
        }
        if finish {
            if s.cr {
                return Err(s.error("CsvNewline"));
            }
            if s.mode == Mode::Quoted {
                return Err(s.error("CsvUnclosedQuote"));
            }
            if s.started {
                s.row()?;
            }
            s.closed = true;
        }
        Ok(Self(Arc::new(s)))
    }
    pub fn peek(&self) -> Option<&[String]> {
        self.0.ready.front().map(|r| r.as_slice())
    }
    pub fn advance(&self) -> Self {
        if self.0.ready.is_empty() {
            return self.clone();
        }
        let mut s = (*self.0).clone();
        let row = s.ready.pop_front().unwrap();
        s.queue_bytes -= row.iter().map(String::len).sum::<usize>();
        Self(Arc::new(s))
    }
    pub fn cancel(&self) -> Self {
        let mut s = (*self.0).clone();
        s.pending = Pending::default();
        s.fields = Arc::new(vec![]);
        s.ready.clear();
        s.row_bytes = 0;
        s.queue_bytes = 0;
        s.mode = Mode::Start;
        s.cr = false;
        s.started = false;
        s.closed = true;
        Self(Arc::new(s))
    }
    pub fn position(&self) -> u64 {
        self.0.position
    }
    pub fn retained_bytes(&self) -> usize {
        self.0.pending.len
            + self.0.row_bytes
            + self.0.queue_bytes
            + (self.0.fields.len() + self.0.ready.iter().map(|r| r.len()).sum::<usize>()) * 64
            + 8192
    }
    pub fn feed_scratch(&self, len: usize) -> usize {
        self.retained_bytes()
            .saturating_mul(3)
            .saturating_add(len.saturating_mul(128))
            .saturating_add(2 * FIELD)
    }
    pub fn clone_work(&self) -> usize {
        self.0.pending.tail.len() + self.0.ready.len() * 32 + 512
    }
    pub fn completion_work(&self) -> usize {
        self.0.row_bytes.saturating_add(self.0.fields.len() * 64)
    }
    pub fn feed_work(&self, len: usize) -> usize {
        self.0
            .fields
            .len()
            .saturating_mul(32)
            .saturating_add(self.0.pending.len.min(PAGE))
            .saturating_add(len.saturating_mul(32))
            .saturating_add(8192)
    }
}
impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Debug for Reader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CsvStream")
            .field("position", &self.position())
            .field("queued_rows", &self.0.ready.len())
            .field("closed", &self.0.closed)
            .finish()
    }
}
impl fmt::Display for Reader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CsvStream({})", self.position())
    }
}
impl Serialize for Reader {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Reader {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = State::deserialize(d)?;
        if !s.validate() {
            return Err(serde::de::Error::custom("invalid CSV state"));
        }
        Ok(Self(Arc::new(s)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_chunk_boundary_preserves_quotes_utf8_and_crlf() {
        let bytes = "name,note\r\n日本語,\"a\"\"b\n日\"\r\n".as_bytes();
        for split in 0..=bytes.len() {
            let r = Reader::new()
                .feed(&bytes[..split], false)
                .unwrap()
                .feed(&bytes[split..], true)
                .unwrap();
            assert_eq!(r.peek().unwrap(), ["name", "note"]);
            let r = r.advance();
            assert_eq!(r.peek().unwrap(), ["日本語", "a\"b\n日"]);
            assert!(r.advance().peek().is_none());
        }
    }
    #[test]
    fn failure_is_atomic_and_positions_are_absolute() {
        let r = Reader::new().feed(b"ok,", false).unwrap();
        let before = serde_json::to_string(&r).unwrap();
        assert_eq!(
            r.feed(b"bad\"", true).unwrap_err(),
            Error {
                code: "CsvQuote",
                position: 6
            }
        );
        assert_eq!(serde_json::to_string(&r).unwrap(), before);
        assert_eq!(
            r.feed(b"good", true).unwrap().peek().unwrap(),
            ["ok", "good"]
        );
        assert_eq!(
            Reader::new().feed(b"a\r", true).unwrap_err().code,
            "CsvNewline"
        );
        assert_eq!(
            Reader::new().feed(&[255], true).unwrap_err().code,
            "CsvUtf8"
        );
    }
    #[test]
    fn queue_backpressure_finish_cancel_and_empty_fields() {
        let r = Reader::new().feed(&b"a\n".repeat(QUEUE), false).unwrap();
        assert_eq!(r.feed(b"b\n", false).unwrap_err().code, "CsvQueueLimit");
        let r = r.advance().feed(b"b\n", false).unwrap();
        assert_eq!(r.peek().unwrap(), ["a"]);
        assert_eq!(r.cancel().feed(b"", false).unwrap_err().code, "CsvClosed");
        assert!(Reader::new().feed(b"", true).unwrap().peek().is_none());
        assert_eq!(
            Reader::new().feed(b",", true).unwrap().peek().unwrap(),
            ["", ""]
        );
    }
    #[test]
    fn pages_share_checkpoint_and_serde_validates_state() {
        let r = Reader::new().feed(&vec![b'a'; CHUNK], false).unwrap();
        let pages = r.0.pending.pages.clone().unwrap();
        let next = r.feed(b"b", false).unwrap();
        assert!(Arc::ptr_eq(
            &pages,
            next.0
                .pending
                .pages
                .as_ref()
                .unwrap()
                .previous
                .as_ref()
                .unwrap()
        ));
        assert_eq!(r.0.pending.len, CHUNK);
        let restored: Reader =
            serde_json::from_str(&serde_json::to_string(&next).unwrap()).unwrap();
        assert_eq!(restored, next);
        let mut bad = serde_json::to_value(&next).unwrap();
        bad["row_bytes"] = serde_json::json!(0);
        assert!(serde_json::from_value::<Reader>(bad).is_err());
    }
    #[test]
    fn field_columns_and_wire_bounds_are_enforced() {
        let mut r = Reader::new();
        for _ in 0..FIELD / CHUNK {
            r = r.feed(&vec![b'a'; CHUNK], false).unwrap();
        }
        assert_eq!(r.feed(b"b", false).unwrap_err().code, "CsvFieldLimit");
        assert_eq!(r.feed(b"", true).unwrap().peek().unwrap()[0].len(), FIELD);
        assert_eq!(
            Reader::new()
                .feed(&vec![b','; COLUMNS], true)
                .unwrap_err()
                .code,
            "CsvColumns"
        );
        let r = Reader::new();
        let mut bad = serde_json::to_value(&r).unwrap();
        bad["fields"] = serde_json::json!(vec![""; COLUMNS + 1]);
        assert!(serde_json::from_value::<Reader>(bad).is_err());
        let mut bad = serde_json::to_value(&r).unwrap();
        bad["ready"] = serde_json::json!(vec![vec![""]; QUEUE + 1]);
        assert!(serde_json::from_value::<Reader>(bad).is_err());
    }
}
