//! What an editor needs from the language, without running the compiler: the words and symbols of
//! the language, a token classifier that keeps working on broken source, and a flat outline.
//!
//! The classifier is not the compiler's lexer. The lexer stops at the first mistake and throws away
//! comments, which an editor cannot afford while someone is typing. The two are kept in step by a
//! test that compares their tokens over every accepted example.
use super::*;
use serde_json::{json, Value as Json};

/// Words with a fixed meaning in language 4.0.0 (see `is_hard_keyword`), in the order of the
/// documentation. `None` is a literal but not a reserved word.
const KEYWORDS: &[&str] = &[
    "fn", "let", "var", "const", "struct", "record", "enum", "trait", "impl", "type", "import",
    "as", "pub", "private", "async", "mut", "if", "else", "match", "while", "for", "in", "break",
    "continue", "return", "defer", "using", "commit", "revert", "resume", "drop", "publish",
    "branch", "runtime", "external", "begin", "move", "spawn", "await",
];
/// Words that mean something only at a fixed place and stay usable as names everywhere else.
const CONTEXTUAL_KEYWORDS: &[&str] = &[
    "value", "capture", "borrow", "force", "live", "fresh", "test", "where", "effects", "captures",
];
const LITERAL_WORDS: &[&str] = &["true", "false", "None"];
const TWO_CHARACTER_OPERATORS: &[&str] = &[
    "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "%=", "->", "=>", "::", "..", "<<",
    ">>",
];
const OPERATOR_CHARACTERS: &str = "=+-*/%.!<>?:|&^~";
const PUNCTUATION_CHARACTERS: &str = "(),;{}[]";

pub(super) struct Span {
    pub kind: &'static str,
    /// Character offsets into the source (end is exclusive).
    pub start: usize,
    pub end: usize,
    /// 1-based line and column in characters, like the compiler's diagnostics.
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
    /// Set when the token is broken (`unterminated`, `unexpected character`).
    pub error: Option<&'static str>,
}

pub(super) fn info() -> Json {
    let supported = [
        "3.0.0", "3.1.0", "3.2.0", "3.3.0", "3.4.0", "3.5.0", "3.6.0", "4.0.0", "4.1.0", "4.2.0",
        "4.2.1", "4.2.2", "4.3.0", "4.3.1", "4.3.2",
    ]
    .into_iter()
    .filter(|v| project::is_supported_language(v))
    .collect::<Vec<_>>();
    json!({
        "format": 1,
        "compiler": env!("CARGO_PKG_VERSION"),
        "languages": {
            "supported": supported,
            "latest": "4.3.2",
            "defaultForStandaloneFiles": "4.3.2"
        },
        "keywords": KEYWORDS,
        "contextualKeywords": CONTEXTUAL_KEYWORDS,
        "literalWords": LITERAL_WORDS,
        "removedWords": ["null"],
        "operators": TWO_CHARACTER_OPERATORS
            .iter()
            .map(|s| s.to_string())
            .chain(OPERATOR_CHARACTERS.chars().map(|c| c.to_string()))
            .collect::<Vec<_>>(),
        "punctuation": PUNCTUATION_CHARACTERS.chars().map(|c| c.to_string()).collect::<Vec<_>>(),
        "comments": {"line": "//", "blockOpen": "/*", "blockClose": "*/", "blockNests": true},
        "strings": {
            "quote": "\"",
            "escape": "\\",
            "interpolation": "${",
            "raw": "three or more quotes open, the same number close; the closing quotes of a multi-line raw string stand alone on their line"
        },
        "numbers": ["decimal with `_`", "0x", "0b", "0o", "fraction", "exponent"],
        "tokenKinds": ["keyword", "literal", "name", "type_name", "number", "string", "raw_string", "comment", "operator", "punctuation", "unknown"],
        "symbolKinds": ["function", "method", "struct", "enum", "trait", "impl", "const", "type"],
        "diagnostics": {
            "command": "rewind --diagnostic-format json check FILE.rw",
            "stream": "stderr",
            "fields": ["code", "message", "source", "line", "column", "hints", "related", "causes"],
            "positions": "1-based; column counts characters",
            "reports": "the first error only"
        },
        "languageServer": {
            "command": "rewind lsp --root DIR",
            "transport": "stdio",
            "columns": "UTF-16 code units, as the protocol requires"
        },
        "grammar": "docs/grammar/rewind.ebnf"
    })
}

fn step(c: char, line: &mut usize, col: &mut usize) {
    if c == '\n' {
        *line += 1;
        *col = 1;
    } else {
        *col += 1;
    }
}

/// Classifies every token of `source`. Never fails: a broken token gets `error` and scanning
/// resumes after it.
pub(super) fn classify(source: &str) -> Vec<Span> {
    let chars: Vec<char> = source.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let mut spans = Vec::new();
    let (mut i, mut line, mut col) = (0usize, 1usize, 1usize);
    while let Some(c) = at(i) {
        if c.is_whitespace() {
            step(c, &mut line, &mut col);
            i += 1;
            continue;
        }
        let (start, start_line, start_col) = (i, line, col);
        let mut kind = "operator";
        let mut error = None;
        if c == '/' && at(i + 1) == Some('/') {
            kind = "comment";
            while at(i).is_some_and(|n| n != '\n') {
                step(chars[i], &mut line, &mut col);
                i += 1;
            }
        } else if c == '/' && at(i + 1) == Some('*') {
            kind = "comment";
            let mut depth = 0usize;
            loop {
                let Some(n) = at(i) else {
                    error = Some("unterminated");
                    break;
                };
                if n == '/' && at(i + 1) == Some('*') {
                    depth += 1;
                    i += 2;
                    col += 2;
                } else if n == '*' && at(i + 1) == Some('/') {
                    depth -= 1;
                    i += 2;
                    col += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    step(n, &mut line, &mut col);
                    i += 1;
                }
            }
        } else if c == '"' && at(i + 1) == Some('"') && at(i + 2) == Some('"') {
            kind = "raw_string";
            let mut n = 0usize;
            while at(i) == Some('"') {
                n += 1;
                i += 1;
                col += 1;
            }
            let multiline = {
                let mut j = i;
                loop {
                    match at(j) {
                        Some(' ' | '\t') => j += 1,
                        Some('\n' | '\r') => break true,
                        _ => break false,
                    }
                }
            };
            let mut blank = false;
            loop {
                let Some(ch) = at(i) else {
                    error = Some("unterminated");
                    break;
                };
                if ch == '"' {
                    let mut run = 0usize;
                    let alone = blank;
                    while at(i + run) == Some('"') {
                        run += 1;
                    }
                    if run == n && (!multiline || alone) {
                        i += run;
                        col += run;
                        break;
                    }
                    i += run;
                    col += run;
                    blank = false;
                    continue;
                }
                if ch == '\n' && !multiline {
                    error = Some("unterminated");
                    break;
                }
                if ch == '\n' {
                    blank = true;
                } else if !ch.is_whitespace() {
                    blank = false;
                }
                step(ch, &mut line, &mut col);
                i += 1;
            }
        } else if c == '"' {
            kind = "string";
            i += 1;
            col += 1;
            loop {
                let Some(ch) = at(i) else {
                    error = Some("unterminated");
                    break;
                };
                step(ch, &mut line, &mut col);
                i += 1;
                if ch == '\\' {
                    if let Some(e) = at(i) {
                        step(e, &mut line, &mut col);
                        i += 1;
                    }
                } else if ch == '"' {
                    break;
                }
            }
        } else if c.is_ascii_digit() {
            kind = "number";
            let digits = |i: &mut usize, col: &mut usize| {
                while at(*i).is_some_and(|n| n.is_ascii_digit() || n == '_') {
                    *i += 1;
                    *col += 1;
                }
            };
            if c == '0' && matches!(at(i + 1), Some('x' | 'X' | 'b' | 'B' | 'o' | 'O')) {
                i += 2;
                col += 2;
                while at(i).is_some_and(|n| n.is_ascii_alphanumeric() || n == '_') {
                    i += 1;
                    col += 1;
                }
            } else {
                digits(&mut i, &mut col);
                if at(i) == Some('.') && at(i + 1).is_some_and(|n| n.is_ascii_digit()) {
                    i += 1;
                    col += 1;
                    digits(&mut i, &mut col);
                }
                if matches!(at(i), Some('e' | 'E')) {
                    i += 1;
                    col += 1;
                    if matches!(at(i), Some('+' | '-')) {
                        i += 1;
                        col += 1;
                    }
                    digits(&mut i, &mut col);
                }
            }
        } else if c.is_alphabetic() || c == '_' {
            while at(i).is_some_and(|n| n.is_alphanumeric() || n == '_') {
                i += 1;
                col += 1;
            }
            let word: String = chars[start..i].iter().collect();
            kind = if KEYWORDS.contains(&word.as_str()) {
                "keyword"
            } else if LITERAL_WORDS.contains(&word.as_str()) {
                "literal"
            } else if word == "null" {
                // A reserved word that is no longer a literal: the compiler rejects it by name.
                "keyword"
            } else if c.is_uppercase() {
                "type_name"
            } else {
                "name"
            };
        } else {
            let two: String = [c, at(i + 1).unwrap_or('\0')].iter().collect();
            // `<<=` and `>>=` are `<` + `<=` and `>` + `>=`, as in the lexer.
            let shift_assign = matches!(two.as_str(), "<<" | ">>") && at(i + 2) == Some('=');
            let triple = two == ">>" && at(i + 2) == Some('>') && at(i + 3) != Some('=');
            let width = if triple {
                3
            } else if TWO_CHARACTER_OPERATORS.contains(&two.as_str()) && !shift_assign {
                2
            } else {
                1
            };
            i += width;
            col += width;
            if PUNCTUATION_CHARACTERS.contains(c) {
                kind = "punctuation";
            } else if !OPERATOR_CHARACTERS.contains(c) {
                kind = "unknown";
                error = Some("unexpected character");
            }
        }
        spans.push(Span {
            kind,
            start,
            end: i,
            line: start_line,
            col: start_col,
            end_line: line,
            end_col: col,
            error,
        });
    }
    spans
}

pub(super) fn tokens_json(source: &str) -> Json {
    let spans = classify(source);
    json!({
        "format": 1,
        "tokens": spans.iter().map(|s| {
            let mut token = json!({
                "kind": s.kind,
                "line": s.line, "column": s.col,
                "endLine": s.end_line, "endColumn": s.end_col,
            });
            if let Some(error) = s.error {
                token["error"] = json!(error);
            }
            token
        }).collect::<Vec<_>>(),
        "symbols": symbols(source),
    })
}

/// Flat outline of the declarations in `source`, found from the token stream so that it keeps
/// working while the file does not compile.
pub(super) fn symbols(source: &str) -> Vec<Json> {
    let spans = classify(source);
    let chars: Vec<char> = source.chars().collect();
    let text = |s: &Span| chars[s.start..s.end].iter().collect::<String>();
    let code: Vec<&Span> = spans.iter().filter(|s| s.kind != "comment").collect();
    let mut out = Vec::new();
    // One entry per open `{`: whether it is the body of an impl or a trait.
    let mut braces: Vec<bool> = Vec::new();
    let mut container_pending = false;
    for (index, span) in code.iter().enumerate() {
        match (span.kind, text(span).as_str()) {
            ("punctuation", "{") => {
                braces.push(std::mem::take(&mut container_pending));
            }
            ("punctuation", "}") => {
                braces.pop();
            }
            ("punctuation", ";") => container_pending = false,
            (
                "keyword",
                word @ ("fn" | "struct" | "record" | "enum" | "trait" | "impl" | "const" | "type"),
            ) => {
                let Some(name) = code
                    .get(index + 1)
                    .filter(|n| matches!(n.kind, "name" | "type_name"))
                else {
                    continue;
                };
                if matches!(word, "trait" | "impl") {
                    container_pending = true;
                }
                let kind = match word {
                    "fn" if braces.last() == Some(&true) => "method",
                    "fn" => "function",
                    "record" => "struct",
                    other => other,
                };
                out.push(json!({
                    "name": text(name),
                    "kind": kind,
                    "line": name.line, "column": name.col,
                    "endLine": name.end_line, "endColumn": name.end_col,
                }));
            }
            _ => {}
        }
    }
    out
}

/// LSP semantic tokens (`textDocument/semanticTokens/full`), UTF-16 columns, one entry per line of
/// a token. The legend is `SEMANTIC_LEGEND`.
pub(super) const SEMANTIC_LEGEND: &[&str] = &[
    "keyword", "comment", "string", "number", "operator", "type", "variable",
];
pub(super) fn semantic_tokens(source: &str) -> Vec<u32> {
    let chars: Vec<char> = source.chars().collect();
    let mut data = Vec::new();
    let (mut previous_line, mut previous_start) = (0u32, 0u32);
    for span in classify(source) {
        let kind = match span.kind {
            "keyword" | "literal" => 0,
            "comment" => 1,
            "string" | "raw_string" => 2,
            "number" => 3,
            "operator" => 4,
            "type_name" => 5,
            "name" => 6,
            _ => continue,
        };
        let utf16 = |range: &[char]| range.iter().map(|c| c.len_utf16() as u32).sum::<u32>();
        let mut segment_start = span.start;
        let mut line = span.line as u32 - 1;
        loop {
            let segment_end = chars[segment_start..span.end]
                .iter()
                .position(|c| *c == '\n')
                .map_or(span.end, |p| segment_start + p);
            let line_start = chars[..segment_start]
                .iter()
                .rposition(|c| *c == '\n')
                .map_or(0, |p| p + 1);
            let length = utf16(&chars[segment_start..segment_end]);
            if length > 0 {
                let start = utf16(&chars[line_start..segment_start]);
                let delta_line = line - previous_line;
                let delta_start = if delta_line == 0 {
                    start - previous_start
                } else {
                    start
                };
                data.extend([delta_line, delta_start, length, kind, 0]);
                (previous_line, previous_start) = (line, start);
            }
            if segment_end >= span.end {
                break;
            }
            segment_start = segment_end + 1;
            line += 1;
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<(&'static str, String)> {
        let chars: Vec<char> = source.chars().collect();
        classify(source)
            .into_iter()
            .map(|s| (s.kind, chars[s.start..s.end].iter().collect()))
            .collect()
    }

    #[test]
    fn keyword_list_is_the_compilers_reserved_words() {
        for word in KEYWORDS {
            assert!(is_hard_keyword(word), "{word}");
        }
        for word in ["true", "false", "null"] {
            assert!(is_hard_keyword(word));
        }
        // Every reserved word is either listed or a literal word, and nothing else.
        let probe = [
            "fn", "let", "var", "const", "struct", "record", "enum", "trait", "impl", "type",
            "import", "as", "pub", "private", "async", "mut", "if", "else", "match", "while",
            "for", "in", "break", "continue", "return", "defer", "using", "commit", "revert",
            "resume", "drop", "publish", "branch", "runtime", "external", "begin", "move", "spawn",
            "await", "true", "false", "null", "None", "value", "capture", "borrow", "force",
            "live", "fresh", "test", "where", "effects", "captures",
        ];
        for word in probe {
            let listed = KEYWORDS.contains(&word) || matches!(word, "true" | "false" | "null");
            assert_eq!(is_hard_keyword(word), listed, "{word}");
        }
        for word in CONTEXTUAL_KEYWORDS {
            assert!(!is_hard_keyword(word), "{word}");
        }
    }

    #[test]
    fn classifies_the_basic_kinds() {
        let tokens = kinds("fn main() -> Int { let x = 0x1F + 2.5e3; // note\n return x; }");
        assert_eq!(tokens[0], ("keyword", "fn".into()));
        assert_eq!(tokens[1], ("name", "main".into()));
        assert!(tokens.contains(&("operator", "->".into())));
        assert!(tokens.contains(&("type_name", "Int".into())));
        assert!(tokens.contains(&("number", "0x1F".into())));
        assert!(tokens.contains(&("number", "2.5e3".into())));
        assert!(tokens.contains(&("comment", "// note".into())));
    }

    #[test]
    fn broken_source_still_gets_classified() {
        let spans = classify("let s = \"open\nlet t = 1;");
        assert_eq!(
            spans.iter().find(|s| s.kind == "string").unwrap().error,
            Some("unterminated")
        );
        let spans = classify("let a = 1 @ 2; /* open");
        assert!(spans
            .iter()
            .any(|s| s.kind == "unknown" && s.error.is_some()));
        assert_eq!(spans.last().unwrap().kind, "comment");
        assert_eq!(spans.last().unwrap().error, Some("unterminated"));
        // Empty and whitespace-only sources are fine.
        assert!(classify("").is_empty());
        assert!(classify("  \n\t").is_empty());
    }

    #[test]
    fn multi_line_tokens_report_their_end() {
        let spans = classify("/* a\nb */ x");
        assert_eq!(
            (
                spans[0].line,
                spans[0].col,
                spans[0].end_line,
                spans[0].end_col
            ),
            (1, 1, 2, 5)
        );
        assert_eq!((spans[1].line, spans[1].col), (2, 6));
    }

    #[test]
    fn raw_strings_follow_the_compilers_closing_rule() {
        let source = "let a = \"\"\"\n    one \"\"\" two\n    \"\"\";\nlet b = 1;";
        let tokens = kinds(source);
        assert_eq!(tokens.iter().filter(|t| t.0 == "raw_string").count(), 1);
        assert!(tokens.contains(&("name", "b".into())));
    }

    #[test]
    fn outline_marks_methods_and_survives_errors() {
        let source = "struct P { x: Int }\nimpl P { fn len(self) -> Int { return 1; } }\nfn main() -> Int { let s = \"oops\n";
        let found = symbols(source);
        let names: Vec<(String, String)> = found
            .iter()
            .map(|s| {
                (
                    s["kind"].as_str().unwrap().into(),
                    s["name"].as_str().unwrap().into(),
                )
            })
            .collect();
        assert_eq!(
            names,
            [
                ("struct".to_string(), "P".to_string()),
                ("impl".to_string(), "P".to_string()),
                ("method".to_string(), "len".to_string()),
                ("function".to_string(), "main".to_string()),
            ]
        );
    }

    #[test]
    fn semantic_tokens_use_utf16_columns_and_split_lines() {
        // "あ" is one UTF-16 unit; the emoji is two.
        let data = semantic_tokens("let s = \"あ😀\";\n/* a\nb */");
        // let(keyword) s(variable) =(operator) "..."(string) ;(punctuation, skipped)
        assert_eq!(&data[0..5], [0, 0, 3, 0, 0]);
        let string = data.chunks(5).find(|c| c[3] == 2).unwrap();
        assert_eq!(string[2], 5);
        // The block comment spans two lines and becomes two entries.
        let comments: Vec<&[u32]> = data.chunks(5).filter(|c| c[3] == 1).collect();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0][0], 1);
        assert_eq!(comments[1][0], 1);
    }

    /// The classifier and the compiler's lexer must agree on where tokens are, for every source the
    /// repository accepts.
    #[test]
    fn classifier_agrees_with_the_lexer() {
        fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(directory) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect(&path, files);
                } else if path.extension().is_some_and(|e| e == "rw") {
                    files.push(path);
                }
            }
        }
        let mut files = Vec::new();
        for directory in ["examples", "tests/grammar/accept", "libraries/std"] {
            collect(Path::new(directory), &mut files);
        }
        assert!(files.len() > 20, "found {} files", files.len());
        let mut compared = 0;
        for path in files {
            let source = fs::read_to_string(&path).unwrap();
            let Ok(tokens) = lex(&source) else { continue };
            let mine: Vec<(usize, usize)> = classify(&source)
                .into_iter()
                .filter(|s| s.kind != "comment")
                .map(|s| (s.line, s.col))
                .collect();
            let theirs: Vec<(usize, usize)> = tokens
                .iter()
                .filter(|t| t.text != "<eof>")
                .map(|t| (t.line, t.col))
                .collect();
            assert_eq!(mine, theirs, "{}", path.display());
            compared += 1;
        }
        assert!(compared > 20, "compared {compared} files");
    }
}
