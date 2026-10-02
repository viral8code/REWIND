//! Bounded Unicode transforms. Existing scalar and byte indexing stay unchanged.
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;
pub const MAX_INPUT: usize = 1024 * 1024;
pub const MAX_OUTPUT: usize = 8 * 1024 * 1024;
pub const MAX_ITEMS: usize = 65_536;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InputLimit,
    OutputLimit,
    Items,
    Index,
    Form,
    Operation,
}
pub type Result<T> = std::result::Result<T, Error>;
fn input(text: &str) -> Result<()> {
    if text.len() > MAX_INPUT {
        Err(Error::InputLimit)
    } else {
        Ok(())
    }
}
fn collect(chars: impl Iterator<Item = char>) -> Result<String> {
    let mut output = String::new();
    for c in chars {
        if output.len().saturating_add(c.len_utf8()) > MAX_OUTPUT {
            return Err(Error::OutputLimit);
        }
        output.push(c);
    }
    Ok(output)
}
pub fn normalize(text: &str, form: &str) -> Result<String> {
    input(text)?;
    match form {
        "NFC" => collect(text.nfc()),
        "NFD" => collect(text.nfd()),
        "NFKC" => collect(text.nfkc()),
        "NFKD" => collect(text.nfkd()),
        _ => Err(Error::Form),
    }
}
pub fn is_normalized(text: &str, form: &str) -> Result<bool> {
    input(text)?;
    Ok(match form {
        "NFC" => unicode_normalization::is_nfc(text),
        "NFD" => unicode_normalization::is_nfd(text),
        "NFKC" => unicode_normalization::is_nfkc(text),
        "NFKD" => unicode_normalization::is_nfkd(text),
        _ => return Err(Error::Form),
    })
}
pub fn grapheme_count(text: &str) -> Result<usize> {
    input(text)?;
    Ok(text.graphemes(true).count())
}
pub fn grapheme_slice(text: &str, start: usize, end: usize) -> Result<String> {
    input(text)?;
    if start > end {
        return Err(Error::Index);
    }
    let mut first = 0;
    let mut count = 0;
    for (i, (offset, _)) in text.grapheme_indices(true).enumerate() {
        if i == start {
            first = offset;
        }
        if i == end {
            return Ok(text[first..offset].into());
        }
        count = i + 1;
    }
    if end == count && start <= count {
        if start == count {
            first = text.len();
        }
        return Ok(text[first..].into());
    }
    Err(Error::Index)
}
fn segments<'a>(iter: impl Iterator<Item = &'a str>) -> Result<Vec<String>> {
    let mut output = Vec::new();
    for segment in iter {
        if output.len() == MAX_ITEMS {
            return Err(Error::Items);
        }
        output.push(segment.into());
    }
    Ok(output)
}
pub fn split(text: &str, kind: &str) -> Result<Vec<String>> {
    input(text)?;
    match kind {
        "graphemes" => segments(text.graphemes(true)),
        "words" => segments(text.unicode_words()),
        "wordBounds" => segments(text.split_word_bounds()),
        "sentences" => segments(text.split_sentence_bounds()),
        _ => Err(Error::Operation),
    }
}
pub fn grapheme_offsets(text: &str) -> Result<Vec<i64>> {
    input(text)?;
    let mut output = Vec::new();
    for (offset, _) in text.grapheme_indices(true) {
        if output.len() == MAX_ITEMS {
            return Err(Error::Items);
        }
        output.push(offset as i64);
    }
    if output.len() == MAX_ITEMS {
        return Err(Error::Items);
    }
    output.push(text.len() as i64);
    Ok(output)
}
pub fn case(text: &str, operation: &str) -> Result<String> {
    input(text)?;
    let output = match operation {
        "lower" => text.to_lowercase(),
        "upper" => text.to_uppercase(),
        _ => return Err(Error::Operation),
    };
    if output.len() > MAX_OUTPUT {
        Err(Error::OutputLimit)
    } else {
        Ok(output)
    }
}
pub fn versions() -> String {
    let n = unicode_normalization::UNICODE_VERSION;
    let g = unicode_segmentation::UNICODE_VERSION;
    let c = std::char::UNICODE_VERSION;
    format!(
        "normalization={}.{}.{};segmentation={}.{}.{};case={}.{}.{}",
        n.0, n.1, n.2, g.0, g.1, g.2, c.0, c.1, c.2
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_handles_composition_compatibility_and_ordering() {
        assert_eq!(normalize("e\u{301}", "NFC").unwrap(), "é");
        assert_eq!(normalize("é", "NFD").unwrap(), "e\u{301}");
        assert_eq!(normalize("Ａ①ﬃ", "NFKC").unwrap(), "A1ffi");
        assert_eq!(
            normalize("a\u{315}\u{300}", "NFD").unwrap(),
            "a\u{300}\u{315}"
        );
        assert_eq!(normalize("각", "NFD").unwrap(), "각");
        assert!(!is_normalized("e\u{301}", "NFC").unwrap());
        assert_eq!(normalize("hello", "UNKNOWN"), Err(Error::Form));
    }
    #[test]
    fn graphemes_cover_crlf_combining_emoji_flags_and_indic() {
        for text in ["e\u{301}", "👩‍👩‍👧‍👦", "🇯🇵", "\r\n", "क्‍ष"] {
            assert_eq!(grapheme_count(text).unwrap(), 1, "{text}");
            assert_eq!(grapheme_offsets(text).unwrap(), vec![0, text.len() as i64]);
        }
        let text = "ae\u{301}🇯🇵z";
        assert_eq!(grapheme_count(text).unwrap(), 4);
        assert_eq!(grapheme_slice(text, 1, 3).unwrap(), "e\u{301}🇯🇵");
        assert_eq!(grapheme_slice(text, 4, 4).unwrap(), "");
        assert_eq!(grapheme_slice("", 0, 0).unwrap(), "");
        assert_eq!(grapheme_slice(text, 0, 5), Err(Error::Index));
    }
    #[test]
    fn case_is_full_locale_independent_and_context_sensitive() {
        assert_eq!(case("straße ﬃ", "upper").unwrap(), "STRASSE FFI");
        assert_eq!(case("ΟΣ İ", "lower").unwrap(), "ος i\u{307}");
        assert_eq!(case("ı", "lower").unwrap(), "ı");
    }
    #[test]
    fn caps_are_checked_and_sequences_keep_boundaries() {
        assert_eq!(
            grapheme_count(&"a".repeat(MAX_INPUT + 1)),
            Err(Error::InputLimit)
        );
        assert_eq!(
            split(&"a".repeat(MAX_ITEMS + 1), "graphemes"),
            Err(Error::Items)
        );
        assert_eq!(
            split("Hello, world!", "words").unwrap(),
            vec!["Hello", "world"]
        );
        let text = "One. Two!";
        assert_eq!(split(text, "sentences").unwrap().concat(), text);
        assert_eq!(split(text, "wordBounds").unwrap().concat(), text);
    }
}
