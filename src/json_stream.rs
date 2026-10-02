//! Bounded event parser: the input document need not fit in memory.
use crate::csv_stream::{bounded_text, Pending};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::VecDeque, fmt, sync::Arc};
pub const CHUNK: usize = 65_536;
pub const TOKEN: usize = 1024 * 1024;
pub const DEPTH: usize = 64;
pub const QUEUE: usize = 256;
pub const QUEUE_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    ObjectStart(u64),
    ObjectEnd(u64),
    ArrayStart(u64),
    ArrayEnd(u64),
    Key(#[serde(deserialize_with = "bounded_text")] String, u64),
    Text(#[serde(deserialize_with = "bounded_text")] String, u64),
    Number(#[serde(deserialize_with = "bounded_text")] String, u64),
    Bool(bool, u64),
    Null(u64),
}
impl Event {
    pub fn position(&self) -> u64 {
        match self {
            Self::ObjectStart(p)
            | Self::ObjectEnd(p)
            | Self::ArrayStart(p)
            | Self::ArrayEnd(p)
            | Self::Key(_, p)
            | Self::Text(_, p)
            | Self::Number(_, p)
            | Self::Bool(_, p)
            | Self::Null(p) => *p,
        }
    }
    pub fn bytes(&self) -> usize {
        match self {
            Self::Key(s, _) | Self::Text(s, _) | Self::Number(s, _) => s.len() + 128,
            _ => 128,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Frame {
    ObjectKeyOrEnd,
    ObjectKey,
    ObjectColon,
    ObjectValue,
    ObjectComma,
    ArrayValueOrEnd,
    ArrayValue,
    ArrayComma,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Literal {
    True,
    False,
    Null,
}
impl Literal {
    fn bytes(self) -> &'static [u8] {
        match self {
            Self::True => b"true",
            Self::False => b"false",
            Self::Null => b"null",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Lex {
    None,
    String { key: bool, escaped: bool },
    Number,
    Literal { kind: Literal, index: usize },
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    pending: Pending,
    lex: Lex,
    #[serde(deserialize_with = "frames")]
    stack: Vec<Frame>,
    #[serde(deserialize_with = "events")]
    ready: VecDeque<Arc<Event>>,
    position: u64,
    start: u64,
    queue_bytes: usize,
    root_complete: bool,
    closed: bool,
}
fn frames<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Vec<Frame>, D::Error> {
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = Vec<Frame>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("bounded JSON grammar stack")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut a: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut out = vec![];
            while let Some(f) = a.next_element::<Frame>()? {
                if out.len() == DEPTH {
                    return Err(serde::de::Error::custom("JsonDepthLimit"));
                }
                out.push(f);
            }
            Ok(out)
        }
    }
    d.deserialize_seq(V)
}
fn events<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<VecDeque<Arc<Event>>, D::Error> {
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = VecDeque<Arc<Event>>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("bounded JSON event queue")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut a: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut out = VecDeque::new();
            let mut bytes = 0usize;
            while let Some(e) = a.next_element::<Event>()? {
                bytes = bytes.saturating_add(e.bytes());
                if out.len() == QUEUE || bytes > QUEUE_BYTES {
                    return Err(serde::de::Error::custom("JsonQueueLimit"));
                }
                out.push_back(Arc::new(e));
            }
            Ok(out)
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
fn space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}
fn number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut p = 0;
    if b.get(p) == Some(&b'-') {
        p += 1;
    }
    match b.get(p) {
        Some(b'0') => p += 1,
        Some(b'1'..=b'9') => {
            p += 1;
            while b.get(p).is_some_and(u8::is_ascii_digit) {
                p += 1;
            }
        }
        _ => return false,
    }
    if b.get(p) == Some(&b'.') {
        p += 1;
        let start = p;
        while b.get(p).is_some_and(u8::is_ascii_digit) {
            p += 1;
        }
        if p == start {
            return false;
        }
    }
    if b.get(p).is_some_and(|b| matches!(b, b'e' | b'E')) {
        p += 1;
        if b.get(p).is_some_and(|b| matches!(b, b'+' | b'-')) {
            p += 1;
        }
        let start = p;
        while b.get(p).is_some_and(u8::is_ascii_digit) {
            p += 1;
        }
        if p == start {
            return false;
        }
    }
    p == b.len()
}
impl State {
    fn error(&self, code: &'static str) -> Error {
        Error {
            code,
            position: self.position,
        }
    }
    fn emit(&mut self, e: Event) -> Result<()> {
        if self.ready.len() == QUEUE || self.queue_bytes.saturating_add(e.bytes()) > QUEUE_BYTES {
            return Err(self.error("JsonQueueLimit"));
        }
        self.queue_bytes += e.bytes();
        self.ready.push_back(Arc::new(e));
        Ok(())
    }
    fn push(&mut self, b: u8) -> Result<()> {
        if self.pending.len() == TOKEN {
            return Err(self.error("JsonTokenLimit"));
        }
        self.pending.push(b);
        Ok(())
    }
    fn accept(&mut self) -> Result<()> {
        match self.stack.last_mut() {
            Some(f @ Frame::ObjectValue) => *f = Frame::ObjectComma,
            Some(f @ Frame::ArrayValueOrEnd) | Some(f @ Frame::ArrayValue) => {
                *f = Frame::ArrayComma
            }
            None if !self.root_complete => self.root_complete = true,
            _ => return Err(self.error("JsonSyntax")),
        }
        Ok(())
    }
    fn end(&mut self, b: u8) -> Result<()> {
        let valid = matches!(
            (b, self.stack.last()),
            (b'}', Some(Frame::ObjectKeyOrEnd | Frame::ObjectComma))
                | (b']', Some(Frame::ArrayValueOrEnd | Frame::ArrayComma))
        );
        if !valid {
            return Err(self.error("JsonSyntax"));
        }
        self.stack.pop();
        self.emit(if b == b'}' {
            Event::ObjectEnd(self.position)
        } else {
            Event::ArrayEnd(self.position)
        })
    }
    fn idle(&mut self, b: u8) -> Result<()> {
        if space(b) {
            return Ok(());
        }
        match self.stack.last().copied() {
            Some(Frame::ObjectKeyOrEnd | Frame::ObjectKey) => {
                if b == b'}' {
                    return self.end(b);
                }
                if b != b'"' {
                    return Err(self.error("JsonSyntax"));
                }
                self.start = self.position;
                self.push(b)?;
                self.lex = Lex::String {
                    key: true,
                    escaped: false,
                };
                return Ok(());
            }
            Some(Frame::ObjectColon) => {
                if b != b':' {
                    return Err(self.error("JsonSyntax"));
                }
                *self.stack.last_mut().unwrap() = Frame::ObjectValue;
                return Ok(());
            }
            Some(Frame::ObjectComma) => {
                if b == b'}' {
                    return self.end(b);
                }
                if b != b',' {
                    return Err(self.error("JsonSyntax"));
                }
                *self.stack.last_mut().unwrap() = Frame::ObjectKey;
                return Ok(());
            }
            Some(Frame::ArrayComma) => {
                if b == b']' {
                    return self.end(b);
                }
                if b != b',' {
                    return Err(self.error("JsonSyntax"));
                }
                *self.stack.last_mut().unwrap() = Frame::ArrayValue;
                return Ok(());
            }
            Some(Frame::ArrayValueOrEnd) if b == b']' => return self.end(b),
            None if self.root_complete => return Err(self.error("JsonTrailingData")),
            _ => {}
        }
        self.start = self.position;
        match b {
            b'{' | b'[' => {
                if self.stack.len() == DEPTH {
                    return Err(self.error("JsonDepthLimit"));
                }
                self.accept()?;
                self.stack.push(if b == b'{' {
                    Frame::ObjectKeyOrEnd
                } else {
                    Frame::ArrayValueOrEnd
                });
                self.emit(if b == b'{' {
                    Event::ObjectStart(self.position)
                } else {
                    Event::ArrayStart(self.position)
                })
            }
            b'"' => {
                self.accept()?;
                self.push(b)?;
                self.lex = Lex::String {
                    key: false,
                    escaped: false,
                };
                Ok(())
            }
            b'-' | b'0'..=b'9' => {
                self.accept()?;
                self.push(b)?;
                self.lex = Lex::Number;
                Ok(())
            }
            b't' | b'f' | b'n' => {
                self.accept()?;
                self.lex = Lex::Literal {
                    kind: match b {
                        b't' => Literal::True,
                        b'f' => Literal::False,
                        _ => Literal::Null,
                    },
                    index: 1,
                };
                Ok(())
            }
            _ => Err(self.error("JsonSyntax")),
        }
    }
    fn numeric(&mut self) -> Result<()> {
        let bytes = self.pending.bytes();
        let text = String::from_utf8(bytes).map_err(|_| Error {
            code: "JsonSyntax",
            position: self.start,
        })?;
        if !number(&text) {
            return Err(Error {
                code: "JsonNumberSyntax",
                position: self.start,
            });
        }
        self.emit(Event::Number(text, self.start))?;
        self.pending = Pending::default();
        self.lex = Lex::None;
        Ok(())
    }
    fn byte(&mut self, b: u8) -> Result<()> {
        match self.lex {
            Lex::None => self.idle(b),
            Lex::Number => {
                if matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.push(b)
                } else {
                    self.numeric()?;
                    self.idle(b)
                }
            }
            Lex::Literal { kind, index } => {
                if kind.bytes().get(index) != Some(&b) {
                    return Err(self.error("JsonSyntax"));
                }
                if index + 1 == kind.bytes().len() {
                    self.emit(match kind {
                        Literal::True => Event::Bool(true, self.start),
                        Literal::False => Event::Bool(false, self.start),
                        Literal::Null => Event::Null(self.start),
                    })?;
                    self.lex = Lex::None;
                } else {
                    self.lex = Lex::Literal {
                        kind,
                        index: index + 1,
                    };
                }
                Ok(())
            }
            Lex::String { key, escaped } => {
                if b < 32 {
                    return Err(self.error("JsonStringSyntax"));
                }
                self.push(b)?;
                if b == b'"' && !escaped {
                    let text: String =
                        serde_json::from_slice(&self.pending.bytes()).map_err(|_| Error {
                            code: "JsonStringSyntax",
                            position: self.start,
                        })?;
                    if key {
                        if !matches!(
                            self.stack.last(),
                            Some(Frame::ObjectKeyOrEnd | Frame::ObjectKey)
                        ) {
                            return Err(self.error("JsonSyntax"));
                        }
                        *self.stack.last_mut().unwrap() = Frame::ObjectColon;
                        self.emit(Event::Key(text, self.start))?;
                    } else {
                        self.emit(Event::Text(text, self.start))?;
                    }
                    self.pending = Pending::default();
                    self.lex = Lex::None;
                } else {
                    self.lex = Lex::String {
                        key,
                        escaped: b == b'\\' && !escaped,
                    };
                }
                Ok(())
            }
        }
    }
    fn validate(&self) -> bool {
        if self.position > i64::MAX as u64
            || self.start > self.position
            || self.pending.len() > TOKEN
            || self.ready.len() > QUEUE
            || self.queue_bytes != self.ready.iter().map(|e| e.bytes()).sum::<usize>()
            || self.queue_bytes > QUEUE_BYTES
        {
            return false;
        }
        if !self.ready.iter().all(|e| {
            e.position() < self.position
                && match e.as_ref() {
                    Event::Number(s, _) => number(s),
                    _ => true,
                }
        }) {
            return false;
        }
        let lexical = match self.lex {
            Lex::None => self.pending.len() == 0,
            Lex::String { key, .. } => {
                self.pending.len() > 0
                    && self.pending.bytes().first() == Some(&b'"')
                    && (!key
                        || matches!(
                            self.stack.last(),
                            Some(Frame::ObjectKeyOrEnd | Frame::ObjectKey)
                        ))
            }
            Lex::Number => {
                self.pending.len() > 0
                    && self
                        .pending
                        .bytes()
                        .iter()
                        .all(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
            }
            Lex::Literal { kind, index } => {
                self.pending.len() == 0 && index > 0 && index < kind.bytes().len()
            }
        };
        let parent = matches!(
            self.stack.last(),
            None | Some(Frame::ObjectComma | Frame::ArrayComma)
        );
        let source_len = self.position - self.start;
        let progress = match self.lex {
            Lex::None => true,
            Lex::String { key, .. } => self.pending.len() as u64 == source_len && (key || parent),
            Lex::Number => self.pending.len() as u64 == source_len && parent,
            Lex::Literal { index, .. } => index as u64 == source_len && parent,
        };
        let ancestors = self
            .stack
            .iter()
            .take(self.stack.len().saturating_sub(1))
            .all(|f| matches!(f, Frame::ObjectComma | Frame::ArrayComma));
        lexical
            && progress
            && ancestors
            && (self.root_complete || (self.stack.is_empty() && self.lex == Lex::None))
            && (!self.closed
                || (self.root_complete && self.stack.is_empty() && self.lex == Lex::None))
    }
}
impl Reader {
    pub fn new() -> Self {
        Self(Arc::new(State {
            pending: Pending::default(),
            lex: Lex::None,
            stack: vec![],
            ready: VecDeque::new(),
            position: 0,
            start: 0,
            queue_bytes: 0,
            root_complete: false,
            closed: false,
        }))
    }
    pub fn feed(&self, input: &[u8], finish: bool) -> Result<Self> {
        if self.0.closed {
            return Err(self.0.error("JsonClosed"));
        }
        if input.len() > CHUNK {
            return Err(self.0.error("JsonChunkLimit"));
        }
        let mut s = (*self.0).clone();
        for &b in input {
            if s.position == i64::MAX as u64 {
                return Err(s.error("JsonPositionLimit"));
            }
            s.byte(b)?;
            s.position += 1;
        }
        if finish {
            if s.lex == Lex::Number {
                s.numeric()?;
            }
            if s.lex != Lex::None || !s.stack.is_empty() || !s.root_complete {
                return Err(s.error("JsonIncomplete"));
            }
            s.closed = true;
        }
        Ok(Self(Arc::new(s)))
    }
    pub fn peek(&self) -> Option<&Event> {
        self.0.ready.front().map(Arc::as_ref)
    }
    pub fn advance(&self) -> Self {
        if self.0.ready.is_empty() {
            return self.clone();
        }
        let mut s = (*self.0).clone();
        let e = s.ready.pop_front().unwrap();
        s.queue_bytes -= e.bytes();
        Self(Arc::new(s))
    }
    pub fn cancel(&self) -> Self {
        let mut r = Self::new();
        let s = Arc::get_mut(&mut r.0).unwrap();
        s.position = self.0.position;
        s.root_complete = true;
        s.closed = true;
        r
    }
    pub fn position(&self) -> u64 {
        self.0.position
    }
    pub fn retained_bytes(&self) -> usize {
        self.0.pending.len() + self.0.queue_bytes + self.0.stack.len() * 32 + 8192
    }
    pub fn clone_work(&self) -> usize {
        self.0.pending.len().min(4096) + self.0.stack.len() * 32 + self.0.ready.len() * 32 + 512
    }
    pub fn feed_work(&self, input: &[u8], finish: bool) -> usize {
        self.clone_work()
            + input.len() * 32
            + if finish
                || input
                    .iter()
                    .any(|b| space(*b) || matches!(b, b'"' | b',' | b']' | b'}'))
            {
                self.0.pending.len()
            } else {
                0
            }
    }
    pub fn feed_scratch(&self, len: usize) -> usize {
        self.retained_bytes()
            .saturating_mul(3)
            .saturating_add(len * 128)
            .saturating_add(2 * TOKEN)
    }
}
impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Debug for Reader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("JsonStream")
            .field("position", &self.position())
            .field("queued_events", &self.0.ready.len())
            .field("closed", &self.0.closed)
            .finish()
    }
}
impl fmt::Display for Reader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "JsonStream({})", self.position())
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
            return Err(serde::de::Error::custom("invalid JSON stream state"));
        }
        Ok(Self(Arc::new(s)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn drain(mut r: Reader) -> Vec<Event> {
        let mut out = vec![];
        while let Some(e) = r.peek() {
            out.push(e.clone());
            r = r.advance();
        }
        out
    }
    #[test]
    fn every_split_keeps_utf8_escapes_surrogates_and_number_tokens() {
        let b = r#"{"name":"日本語\uD83D\uDE00","items":[-12.30e+400,true,false,null]}"#.as_bytes();
        let expected = drain(Reader::new().feed(b, true).unwrap());
        for split in 0..=b.len() {
            let r = Reader::new()
                .feed(&b[..split], false)
                .unwrap()
                .feed(&b[split..], true)
                .unwrap();
            assert_eq!(drain(r), expected);
        }
        assert!(expected
            .iter()
            .any(|e| matches!(e,Event::Text(s,_) if s=="日本語😀")));
        assert!(expected
            .iter()
            .any(|e| matches!(e,Event::Number(s,_) if s=="-12.30e+400")));
    }
    #[test]
    fn grammar_errors_and_failed_feed_are_atomic() {
        for b in [
            b"[1,]".as_slice(),
            b"{\"a\":}",
            b"{\"a\":1,}",
            b"01",
            b"1e",
            b"true false",
            b"\"\\uD800\"",
            b"[}",
            b"",
            b"[",
            b"nul",
        ] {
            assert!(Reader::new().feed(b, true).is_err(), "{:?}", b);
        }
        let r = Reader::new().feed(b"[1,", false).unwrap();
        assert_eq!(
            r.feed(b"?]", true).unwrap_err(),
            Error {
                code: "JsonSyntax",
                position: 3
            }
        );
        assert_eq!(drain(r.feed(b"2]", true).unwrap()).len(), 4);
    }
    #[test]
    fn queue_backpressure_and_large_array_do_not_keep_all_events() {
        let mut r = Reader::new();
        r = r.feed(b"[", false).unwrap().advance();
        for _ in 0..1000 {
            r = r.feed(b"123,", false).unwrap();
            assert!(matches!(r.peek(),Some(Event::Number(s,_)) if s=="123"));
            r = r.advance();
            assert!(r.retained_bytes() < 16384);
        }
        r = r.feed(b"0]", true).unwrap();
        assert_eq!(drain(r).len(), 2);
        let r = Reader::new()
            .feed(&format!("[{}", "0,".repeat(QUEUE - 1)).into_bytes(), false)
            .unwrap();
        assert_eq!(r.feed(b"0,", false).unwrap_err().code, "JsonQueueLimit");
        assert!(r.advance().feed(b"0,", false).is_ok());
    }
    #[test]
    fn serde_bounds_and_pending_restore() {
        let r = Reader::new().feed(b"{\"a\":\"part", false).unwrap();
        let restored: Reader = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(r, restored);
        assert_eq!(drain(restored.feed(b"ial\"}", true).unwrap()).len(), 4);
        let mut bad = serde_json::to_value(&r).unwrap();
        bad["queue_bytes"] = serde_json::json!(0);
        assert!(serde_json::from_value::<Reader>(bad).is_err());
        assert_eq!(r.cancel().feed(b"", false).unwrap_err().code, "JsonClosed");
        assert_eq!(
            Reader::new()
                .feed(&vec![b'['; DEPTH + 1], false)
                .unwrap_err()
                .code,
            "JsonDepthLimit"
        );
    }
    #[test]
    fn token_limit_wire_queue_and_reference_documents() {
        let mut r = Reader::new().feed(b"\"", false).unwrap();
        for _ in 0..15 {
            r = r.feed(&vec![b'a'; CHUNK], false).unwrap();
        }
        r = r.feed(&vec![b'a'; CHUNK - 1], false).unwrap();
        assert_eq!(r.feed(b"a", false).unwrap_err().code, "JsonTokenLimit");
        let mut bad = serde_json::to_value(Reader::new()).unwrap();
        bad["ready"] = serde_json::json!(vec![Event::Null(0); QUEUE + 1]);
        assert!(serde_json::from_value::<Reader>(bad).is_err());
        for text in [
            "{}",
            "[]",
            "true",
            "false",
            "null",
            "-0",
            "1.25e-4",
            r#"{"a":[],"b":[{"c":"\\\"\n"},2]}"#,
        ] {
            assert!(serde_json::from_str::<serde_json::Value>(text).is_ok());
            let expected = drain(Reader::new().feed(text.as_bytes(), true).unwrap());
            let mut r = Reader::new();
            let mut got = vec![];
            for &b in text.as_bytes() {
                r = r.feed(&[b], false).unwrap();
                while let Some(e) = r.peek() {
                    got.push(e.clone());
                    r = r.advance();
                }
            }
            r = r.feed(b"", true).unwrap();
            got.extend(drain(r));
            assert_eq!(got, expected);
        }
        let events = drain(Reader::new().feed(b"{\"a\":1,\"a\":2}", true).unwrap());
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Key(..)))
                .count(),
            2
        );
    }
}
