//! Bounded Thompson/Pike matching, with immutable plans and per-call work caches.
use regex_automata::{
    nfa::thompson::{self, pikevm::PikeVM},
    util::syntax,
    Input, PatternID,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    fmt,
    sync::{Arc, OnceLock},
};
pub const MAX_PATTERN: usize = 1024;
pub const MAX_INPUT: usize = 1024 * 1024;
pub const MAX_NFA: usize = 512 * 1024;
pub const MAX_GROUPS: usize = 32;
pub const COMPILE_SCRATCH: usize = 48 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    PatternLimit,
    InputLimit,
    Syntax,
    Size,
    Groups,
    Index,
    Type,
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    #[serde(deserialize_with = "pattern_text")]
    pattern: String,
    text: bool,
    case_insensitive: bool,
    multi_line: bool,
    dot_all: bool,
}
fn pattern_text<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a bounded regex pattern")
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<String, E> {
            if v.len() > MAX_PATTERN {
                Err(E::custom("RegexPatternLimit"))
            } else {
                Ok(v.into())
            }
        }
        fn visit_string<E: serde::de::Error>(self, v: String) -> std::result::Result<String, E> {
            if v.len() > MAX_PATTERN {
                Err(E::custom("RegexPatternLimit"))
            } else {
                Ok(v)
            }
        }
    }
    d.deserialize_str(Visitor)
}
struct Inner {
    spec: Spec,
    backend: OnceLock<Result<PikeVM>>,
    digest: [u8; 32],
}
#[derive(Clone)]
pub struct Pattern(Arc<Inner>);
impl Pattern {
    fn from_spec(spec: Spec) -> Self {
        let mut hash = Sha256::new();
        hash.update(spec.pattern.as_bytes());
        hash.update([
            spec.text as u8,
            spec.case_insensitive as u8,
            spec.multi_line as u8,
            spec.dot_all as u8,
        ]);
        Self(Arc::new(Inner {
            spec,
            backend: OnceLock::new(),
            digest: hash.finalize().into(),
        }))
    }
    pub fn compile(
        pattern: &str,
        text: bool,
        case_insensitive: bool,
        multi_line: bool,
        dot_all: bool,
    ) -> Result<Self> {
        if pattern.len() > MAX_PATTERN {
            return Err(Error::PatternLimit);
        }
        let value = Self::from_spec(Spec {
            pattern: pattern.into(),
            text,
            case_insensitive,
            multi_line,
            dot_all,
        });
        value.backend()?;
        Ok(value)
    }
    fn backend(&self) -> Result<&PikeVM> {
        self.0
            .backend
            .get_or_init(|| {
                let s = &self.0.spec;
                let mut builder = PikeVM::builder();
                builder.syntax(
                    syntax::Config::new()
                        .unicode(s.text)
                        .utf8(s.text)
                        .case_insensitive(s.case_insensitive)
                        .multi_line(s.multi_line)
                        .dot_matches_new_line(s.dot_all)
                        .nest_limit(32),
                );
                builder.thompson(
                    thompson::Config::new()
                        .utf8(s.text)
                        .nfa_size_limit(Some(MAX_NFA))
                        .shrink(false),
                );
                let vm = builder.build(&s.pattern).map_err(|e| {
                    if e.size_limit().is_some() {
                        Error::Size
                    } else {
                        Error::Syntax
                    }
                })?;
                if vm.get_nfa().group_info().group_len(PatternID::ZERO) > MAX_GROUPS {
                    return Err(Error::Groups);
                }
                Ok(vm)
            })
            .as_ref()
            .map_err(|e| *e)
    }
    pub fn ensure(&self) -> Result<()> {
        self.backend().map(|_| ())
    }
    pub fn is_compiled(&self) -> bool {
        self.0.backend.get().is_some()
    }
    pub fn is_text(&self) -> bool {
        self.0.spec.text
    }
    pub fn flags(&self) -> [bool; 3] {
        let s = &self.0.spec;
        [s.case_insensitive, s.multi_line, s.dot_all]
    }
    pub fn source(&self) -> &str {
        &self.0.spec.pattern
    }
    pub fn retained_bytes(&self) -> usize {
        let backend = match self.0.backend.get() {
            Some(Ok(vm)) => vm.get_nfa().memory_usage(),
            Some(Err(_)) => 0,
            None => MAX_NFA + 8192,
        };
        self.0
            .spec
            .pattern
            .capacity()
            .saturating_add(backend)
            .saturating_add(256)
    }
    pub fn states(&self) -> usize {
        match self.0.backend.get() {
            Some(Ok(vm)) => vm.get_nfa().states().len(),
            Some(Err(_)) => 0,
            None => MAX_NFA / std::mem::size_of::<thompson::State>(),
        }
    }
    pub fn search_scratch(&self) -> usize {
        self.states()
            .saturating_mul(MAX_GROUPS * 32 + 128)
            .saturating_add(COMPILE_SCRATCH * (self.0.backend.get().is_none() as usize))
            .saturating_add(8192)
    }
    pub fn find(&self, input: &[u8], start: usize, text: bool) -> Result<Option<Vec<i64>>> {
        if self.is_text() != text {
            return Err(Error::Type);
        }
        if input.len() > MAX_INPUT {
            return Err(Error::InputLimit);
        }
        if start > input.len() {
            return Err(Error::Index);
        }
        if text {
            let value = std::str::from_utf8(input).map_err(|_| Error::Type)?;
            if !value.is_char_boundary(start) {
                return Err(Error::Index);
            }
        }
        let vm = self.backend()?;
        let mut cache = vm.create_cache();
        let mut captures = vm.create_captures();
        vm.search(
            &mut cache,
            &Input::new(input).span(start..input.len()),
            &mut captures,
        );
        let Some(found) = captures.get_match() else {
            return Ok(None);
        };
        let next = if found.start() == found.end() {
            if found.end() == input.len() {
                -1
            } else if text {
                let tail = std::str::from_utf8(&input[found.end()..]).map_err(|_| Error::Type)?;
                (found.end() + tail.chars().next().ok_or(Error::Index)?.len_utf8()) as i64
            } else {
                found.end() as i64 + 1
            }
        } else {
            found.end() as i64
        };
        let mut output = vec![found.start() as i64, found.end() as i64, next];
        for index in 0..captures.group_len() {
            match captures.get_group(index) {
                Some(span) => {
                    output.push(span.start as i64);
                    output.push(span.end as i64);
                }
                None => {
                    output.push(-1);
                    output.push(-1);
                }
            }
        }
        Ok(Some(output))
    }
    pub fn names(&self) -> Result<Vec<Option<String>>> {
        Ok(self
            .backend()?
            .get_nfa()
            .group_info()
            .pattern_names(PatternID::ZERO)
            .map(|v| v.map(str::to_owned))
            .collect())
    }
}
impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.digest == other.0.digest && self.0.spec == other.0.spec)
    }
}
impl Eq for Pattern {}
impl PartialOrd for Pattern {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Pattern {
    fn cmp(&self, other: &Self) -> Ordering {
        if Arc::ptr_eq(&self.0, &other.0) {
            Ordering::Equal
        } else {
            self.0.spec.cmp(&other.0.spec)
        }
    }
}
impl fmt::Debug for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Regex(text={},bytes={},sha256=",
            self.is_text(),
            self.source().len()
        )?;
        for b in self.0.digest {
            write!(f, "{b:02x}")?;
        }
        write!(f, ")")
    }
}
impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Regex({})",
            if self.is_text() { "text" } else { "bytes" }
        )
    }
}
impl Serialize for Pattern {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.0.spec.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let spec = Spec::deserialize(d)?;
        Ok(Self::from_spec(spec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_optional_captures_use_utf8_byte_offsets() {
        let re =
            Pattern::compile(r"(?P<word>\p{L}+)(-([0-9]+))?", true, false, false, false).unwrap();
        let found = re.find("!日本語".as_bytes(), 0, true).unwrap().unwrap();
        assert_eq!(&found[..3], &[1, 10, 10]);
        assert_eq!(&found[3..], &[1, 10, 1, 10, -1, -1, -1, -1]);
        assert_eq!(
            re.names().unwrap(),
            vec![None, Some("word".into()), None, None]
        );
    }
    #[test]
    fn bytes_and_text_are_explicit_and_offsets_checked() {
        let bytes = Pattern::compile(r"\xFF+", false, false, false, false).unwrap();
        assert_eq!(
            &bytes.find(&[0, 255, 255], 0, false).unwrap().unwrap()[..3],
            &[1, 3, 3]
        );
        assert_eq!(bytes.find(b"x", 0, true), Err(Error::Type));
        let text = Pattern::compile(".", true, false, false, false).unwrap();
        assert_eq!(text.find("é".as_bytes(), 1, true), Err(Error::Index));
        assert_eq!(
            &text.find("é".as_bytes(), 0, true).unwrap().unwrap()[..2],
            &[0, 2]
        );
    }
    #[test]
    fn empty_matches_advance_and_search_retains_context() {
        let re = Pattern::compile("", true, false, false, false).unwrap();
        assert_eq!(
            &re.find("é".as_bytes(), 0, true).unwrap().unwrap()[..3],
            &[0, 0, 2]
        );
        assert_eq!(
            &re.find("é".as_bytes(), 2, true).unwrap().unwrap()[..3],
            &[2, 2, -1]
        );
        let anchor = Pattern::compile("^a", true, false, false, false).unwrap();
        assert_eq!(anchor.find(b"ba", 1, true).unwrap(), None);
    }
    #[test]
    fn bounded_build_and_non_backtracking_nested_repetition() {
        assert!(Pattern::compile(r"(a)\1", true, false, false, false).is_err());
        assert!(Pattern::compile(r"(?=a)", true, false, false, false).is_err());
        assert!(Pattern::compile("a{1000000}", true, false, false, false).is_err());
        assert!(Pattern::compile(&"a".repeat(MAX_PATTERN + 1), true, false, false, false).is_err());
        let re = Pattern::compile("(a+)+b", true, false, false, false).unwrap();
        assert_eq!(re.find(&vec![b'a'; 10000], 0, true).unwrap(), None);
    }
    #[test]
    fn serde_restores_specs_without_compiling_or_changing_identity() {
        let re = Pattern::compile("a+", true, true, false, false).unwrap();
        let encoded = serde_json::to_string(&re).unwrap();
        let restored: Pattern = serde_json::from_str(&encoded).unwrap();
        assert!(restored.0.backend.get().is_none());
        assert!(restored.retained_bytes() >= MAX_NFA);
        assert_eq!(re, restored);
        assert!(restored.find(b"AA", 0, true).unwrap().is_some());
        assert_eq!(serde_json::to_string(&restored).unwrap(), encoded);
    }
}
