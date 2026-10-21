//! 字句規則の検査（段階 B）。テスト専用。
//!
//! `docs/grammar/rewind.ebnf` の字句規則（`TOKEN` と `SKIP`）だけを使い、最長一致で入力を切る。
//! その結果を、実装の `lex()`（src/v2.rs）のトークン列と突き合わせる。
//! - 受理側: トークンの数と各 `text` が完全に一致する。
//! - 拒否側: `lex()` が拒否する入力は、EBNF 側も拒否する（どの規則にも当たらない、または `nf` に当たる）。
//!
//! 実装が正で、食い違いは EBNF を直す。文脈自由では書けない差だけを `KnownGap` として数える。

use super::earley::Earley;
use super::ebnf::{Bnf, Grammar};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const EBNF: &str = include_str!("../../../docs/grammar/rewind.ebnf");

/// 文法全体の入口の規則（字句の TOKEN と SKIP、構文の Type、TypeParams、WhereClause、Pattern、Expr）。
/// 字句の検査が使うのは、このうち TOKEN と SKIP だけ。
const ROOTS: [&str; 8] = ["TOKEN", "SKIP", "Type", "TypeParams", "WhereClause", "Pattern", "Expr", "Program"];

fn compiled() -> &'static (Bnf, Bnf) {
    static C: OnceLock<(Bnf, Bnf)> = OnceLock::new();
    C.get_or_init(|| {
        let g = Grammar::parse(EBNF, &ROOTS).unwrap_or_else(|e| panic!("rewind.ebnf の読み込み: {e:#?}"));
        let token = g.compile("TOKEN").unwrap_or_else(|e| panic!("TOKEN の展開: {e:#?}"));
        let skip = g.compile("SKIP").unwrap_or_else(|e| panic!("SKIP の展開: {e:#?}"));
        (token, skip)
    })
}

/// EBNF の字句規則だけで入力を切る駆動部。
pub struct EbnfLexer {
    token: Earley<'static>,
    skip: Earley<'static>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EbnfFail {
    /// この文字の位置（0 起点の文字の番号）で、どの規則にも当たらない。
    Stuck(usize),
    /// TOKEN と SKIP が同じ位置で受理した（文法の不具合）。
    Ambiguous(usize),
}

impl EbnfLexer {
    pub fn new() -> Self {
        let (t, s) = compiled();
        EbnfLexer {
            token: Earley::new(t),
            skip: Earley::new(s),
        }
    }

    /// トークンの text の列を返す（空白とコメントは捨てる。`<eof>` は含めない）。
    pub fn tokenize(&self, src: &str) -> Result<Vec<String>, EbnfFail> {
        let chars: Vec<&str> = src.char_indices().map(|(i, c)| &src[i..i + c.len_utf8()]).collect();
        let mut out = Vec::new();
        let mut pos = 0;
        while pos < chars.len() {
            let rest = &chars[pos..];
            let t = self.token.longest_prefix(rest).filter(|&n| n > 0);
            let s = self.skip.longest_prefix(rest).filter(|&n| n > 0);
            match (t, s) {
                (Some(_), Some(_)) => return Err(EbnfFail::Ambiguous(pos)),
                (Some(n), None) => {
                    out.push(chars[pos..pos + n].concat());
                    pos += n;
                }
                (None, Some(n)) => pos += n,
                (None, None) => return Err(EbnfFail::Stuck(pos)),
            }
        }
        Ok(out)
    }
}

/// 実装の `lex()` のトークンの text の列（`<eof>` は含めない）。拒否されたら誤りの文字列。
pub fn impl_tokens(src: &str) -> Result<Vec<String>, String> {
    match super::super::lex(src) {
        Ok(mut v) => {
            let last = v.pop().map(|t| t.text);
            assert_eq!(last.as_deref(), Some("<eof>"));
            Ok(v.into_iter().map(|t| t.text).collect())
        }
        Err(e) => Err(format!("{e:?}")),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// 両方が受理し、トークン列が一致した（トークンの数）。
    Same(usize),
    /// 両方が拒否した。
    BothReject,
    /// 文脈自由では書けない既知の差（理由）。
    KnownGap(&'static str),
    Mismatch(String),
}

/// 文脈自由では書けない差かどうかを、拒否の理由か入力の形から判定する。
fn known_gap(src: &str, impl_err: Option<&str>) -> Option<&'static str> {
    if let Some(e) = impl_err {
        if e.contains("integer literal overflow") {
            return Some("接頭辞つきの整数が u64 に収まらない（値の検査）");
        }
        if e.contains("RawStringIndent") {
            return Some("raw 文字列の字下げの一致（文脈依存）");
        }
    }
    if src.contains("\"\"\"\"\"\"\"") {
        return Some("引用符 7 個以上の raw 文字列（個数の一致は 6 個までを展開）");
    }
    None
}

pub fn compare(lx: &EbnfLexer, src: &str) -> Outcome {
    let imp = impl_tokens(src);
    let ebnf = lx.tokenize(src);
    match (imp, ebnf) {
        (Ok(a), Ok(b)) => {
            if a == b {
                Outcome::Same(a.len())
            } else {
                let i = a.iter().zip(b.iter()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                Outcome::Mismatch(format!(
                    "トークンが違う（{i} 番目）: lex()={:?} / EBNF={:?}（数 {} / {}）",
                    a.get(i),
                    b.get(i),
                    a.len(),
                    b.len()
                ))
            }
        }
        (Err(_), Err(EbnfFail::Stuck(_))) => Outcome::BothReject,
        (Err(_), Err(EbnfFail::Ambiguous(p))) => Outcome::Mismatch(format!("文法が曖昧（{p} 文字目）")),
        (Ok(_), Err(EbnfFail::Ambiguous(p))) => Outcome::Mismatch(format!("文法が曖昧（{p} 文字目）")),
        (Ok(a), Err(EbnfFail::Stuck(p))) => match known_gap(src, None) {
            Some(g) => Outcome::KnownGap(g),
            None => Outcome::Mismatch(format!(
                "lex() は受理（{} トークン）するが EBNF は {p} 文字目で止まる",
                a.len()
            )),
        },
        (Err(e), Ok(b)) => match known_gap(src, Some(&e)) {
            Some(g) => Outcome::KnownGap(g),
            None => Outcome::Mismatch(format!(
                "lex() は拒否（{}）するが EBNF は {} トークンに切れる",
                e.chars().take(120).collect::<String>(),
                b.len()
            )),
        },
    }
}

// ---------------------------------------------------------------- corpus

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    v.sort();
    v
}

/// ディレクトリの下の `.rw` を、パスの辞書順に集める。
fn rw_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for p in sorted_entries(dir) {
        if p.is_dir() {
            rw_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "rw") {
            out.push(p);
        }
    }
}

/// Markdown の ```rewind ... のブロック（`rewind-error`、`rewind-exit` を含む）を取り出す。
/// scripts/check-docs-examples.py の fences() と同じ方法。
pub fn doc_blocks(md: &str) -> Vec<String> {
    let md = md.replace("\r\n", "\n");
    let mut blocks = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in md.split('\n') {
        match &mut current {
            None => {
                if let Some(tag) = line.strip_prefix("```") {
                    current = Some((tag.trim().to_string(), Vec::new()));
                }
            }
            Some((tag, body)) => {
                if line.trim_end() == "```" {
                    if tag.starts_with("rewind") {
                        blocks.push(body.join("\n") + "\n");
                    }
                    current = None;
                } else {
                    body.push(line);
                }
            }
        }
    }
    blocks
}

pub struct Corpus {
    pub docs: Vec<(String, String)>,
    pub std_files: Vec<(String, String)>,
    pub fixtures: Vec<(String, String)>,
    pub examples: Vec<(String, String)>,
}

/// examples は、最上位の項目ごとに先頭の `EXAMPLES_PER_ENTRY` 件（パスの辞書順）を取る。
pub const EXAMPLES_PER_ENTRY: usize = 3;

pub fn corpus() -> Corpus {
    let r = root();
    let mut docs = Vec::new();
    let mut md_files = vec![r.join("docs/language-reference.md")];
    for p in sorted_entries(&r.join("docs/book/chapters")) {
        if p.extension().is_some_and(|e| e == "md") {
            md_files.push(p);
        }
    }
    for f in &md_files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        for (i, b) in doc_blocks(&read(f)).into_iter().enumerate() {
            docs.push((format!("{name}#{i}"), b));
        }
    }
    let load = |files: Vec<PathBuf>| -> Vec<(String, String)> {
        files
            .into_iter()
            .map(|p| {
                let name = p.strip_prefix(&r).unwrap_or(&p).display().to_string();
                (name, read(&p))
            })
            .collect()
    };
    let mut std_files = Vec::new();
    for p in sorted_entries(&r.join("libraries/std")) {
        if p.extension().is_some_and(|e| e == "rw") {
            std_files.push(p);
        }
    }
    let mut fixtures = Vec::new();
    rw_files(&r.join("tests/fixtures"), &mut fixtures);
    let mut examples = Vec::new();
    for p in sorted_entries(&r.join("examples")) {
        let mut v = Vec::new();
        if p.is_dir() {
            rw_files(&p, &mut v);
        } else if p.extension().is_some_and(|e| e == "rw") {
            v.push(p);
        }
        examples.extend(v.into_iter().take(EXAMPLES_PER_ENTRY));
    }
    Corpus {
        docs,
        std_files: load(std_files),
        fixtures: load(fixtures),
        examples: load(examples),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 手で置く端の例。`Some(トークン列)` は両方がその列に切る。`None` は両方が拒否する。
    fn edge_cases() -> Vec<(&'static str, Option<Vec<&'static str>>)> {
        vec![
            // 数
            ("0x1g", None),
            ("0x", None),
            ("0x_1", None),
            ("0x1_", None),
            ("0x1__2", None),
            ("0b12", None),
            ("0o8", None),
            ("0bar", None),
            ("0b", None),
            ("1_", None),
            ("1__0", None),
            ("1e", None),
            ("1e+", None),
            ("1E_5", None),
            ("1e5_", None),
            ("1.5e", None),
            ("1.5_", None),
            ("1_.5", None),
            ("1else", None),
            ("1_e5", None),
            ("1e+_5", None),
            ("1..2", Some(vec!["1", "..", "2"])),
            ("1.0.2", Some(vec!["1.0", ".", "2"])),
            ("a.0.1", Some(vec!["a", ".", "0.1"])),
            ("1.", Some(vec!["1", "."])),
            ("1.e5", Some(vec!["1", ".", "e5"])),
            ("0x1.5", Some(vec!["0x1", ".", "5"])),
            ("1abc", Some(vec!["1", "abc"])),
            ("1e5x", Some(vec!["1e5", "x"])),
            ("0_1 00", Some(vec!["0_1", "00"])),
            ("1_000.5_5e+1_0", Some(vec!["1_000.5_5e+1_0"])),
            ("0x1F_ff 0XAB 0B101 0O17", Some(vec!["0x1F_ff", "0XAB", "0B101", "0O17"])),
            ("1e5 1E-5 1.5e3", Some(vec!["1e5", "1E-5", "1.5e3"])),
            // 記号
            (">=", Some(vec![">="])),
            (">>", Some(vec![">", ">"])),
            ("a>>=b", Some(vec!["a", ">", ">=", "b"])),
            ("-> => :: ..", Some(vec!["->", "=>", "::", ".."])),
            ("->>", Some(vec!["->", ">"])),
            ("=== !== <<", Some(vec!["==", "=", "!=", "=", "<", "<"])),
            ("a/b a/=b a /*x*/ / b", Some(vec!["a", "/", "b", "a", "/=", "b", "a", "/", "b"])),
            (
                "[]{}();,.!?:|&%*+-",
                Some(vec![
                    "[", "]", "{", "}", "(", ")", ";", ",", ".", "!", "?", ":", "|", "&", "%", "*", "+", "-",
                ]),
            ),
            ("a ||| b &&& c", Some(vec!["a", "||", "|", "b", "&&", "&", "c"])),
            // 文字列
            (r#""a\"b""#, Some(vec![r#""a\"b""#])),
            (r#""a\\""#, Some(vec![r#""a\\""#])),
            (r#""\z""#, Some(vec![r#""\z""#])),
            ("\"\"", Some(vec!["\"\""])),
            ("\"\" \"\"", Some(vec!["\"\"", "\"\""])),
            ("\"a\"\"b\"", Some(vec!["\"a\"", "\"b\""])),
            ("\"line1\nline2\"", Some(vec!["\"line1\nline2\""])),
            ("\"abc", None),
            ("\"a\\\"", None),
            ("\"\\", None),
            // raw 文字列
            ("\"\"\"x\"\"\"", Some(vec!["\"\"\"x\"\"\""])),
            ("\"\"\"x\"\"\" y", Some(vec!["\"\"\"x\"\"\"", "y"])),
            ("\"\"\"a\"b\"\"c\"\"\"", Some(vec!["\"\"\"a\"b\"\"c\"\"\""])),
            ("\"\"\"\"x\"\"\"\"", Some(vec!["\"\"\"\"x\"\"\"\""])),
            ("\"\"\"\"a\"\"\"b\"\"\"\"", Some(vec!["\"\"\"\"a\"\"\"b\"\"\"\""])),
            ("\"\"\"\n  a\n  \"\"\"", Some(vec!["\"\"\"\n  a\n  \"\"\""])),
            ("\"\"\"  \n  a\"b\n  \"\"\" z", Some(vec!["\"\"\"  \n  a\"b\n  \"\"\"", "z"])),
            ("\"\"\"\n\"\"\"", Some(vec!["\"\"\"\n\"\"\""])),
            ("\"\"\"\r\n  a\r\n  \"\"\"", Some(vec!["\"\"\"\r\n  a\r\n  \"\"\""])),
            ("\"\"\"\n  a \"\"\"\n  \"\"\"", None),
            ("\"\"\"\nx\"\"\"\n\"\"\"", None),
            ("\"\"\"x", None),
            ("\"\"\"x\"\"\"\"", None),
            ("\"\"\"x\"\"", None),
            ("\"\"\"x\ny\"\"\"", None),
            ("\"\"\"x\"\"\" \"\"\"", None),
            ("\"\"\"\"\"\"", None),
            ("\"\"\"\"\"\"\"\"", None),
            // コメント
            ("/* a /* b */ c */ x", Some(vec!["x"])),
            ("/**/x", Some(vec!["x"])),
            ("/***/ /*/ */ y", Some(vec!["y"])),
            ("/* a // b */ x", Some(vec!["x"])),
            ("// c /* d\nx", Some(vec!["x"])),
            ("//", Some(vec![])),
            ("a//b", Some(vec!["a"])),
            ("x // y\r\nz", Some(vec!["x", "z"])),
            ("/*", None),
            ("/* /* */", None),
            ("/*/", None),
            ("/*/*/ */", None),
            ("x /* \"unclosed */ y", Some(vec!["x", "y"])),
            // 名前と文字
            ("日本語 変数1 _x _1 é1_", Some(vec!["日本語", "変数1", "_x", "_1", "é1_"])),
            ("x² x①", Some(vec!["x²", "x①"])),
            ("𝒜 ℵ", Some(vec!["𝒜", "ℵ"])),
            ("a\u{3000}b\u{a0}c\r\nd\u{2028}e", Some(vec!["a", "b", "c", "d", "e"])),
            ("'a'", None),
            ("$", None),
            ("@", None),
            ("#", None),
            ("~", None),
            ("^", None),
            ("`", None),
            ("\\", None),
            ("😀", Some(vec!["😀"])),
            ("x = 😀", Some(vec!["x", "=", "😀"])),
            ("a ¬ b", Some(vec!["a", "¬", "b"])),
            ("٣", Some(vec!["٣"])),
            ("²", Some(vec!["²"])),
            ("①", Some(vec!["①"])),
            ("\u{feff}x", Some(vec!["\u{feff}", "x"])),
            ("\u{200b}", Some(vec!["\u{200b}"])),
            ("x\u{0}", None),
        ]
    }

    #[test]
    fn grammar_loads_and_has_disjoint_entries() {
        let lx = EbnfLexer::new();
        assert_eq!(lx.tokenize(""), Ok(vec![]));
        assert_eq!(lx.tokenize("  /* c */ \n").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn edge_cases_agree_with_lex_and_expectations() {
        let cases = edge_cases();
        assert!(cases.len() >= 40, "端の例は 40 件以上: {}", cases.len());
        let lx = EbnfLexer::new();
        let mut bad = Vec::new();
        for (src, expect) in &cases {
            let imp = impl_tokens(src);
            let ebnf = lx.tokenize(src);
            match expect {
                Some(toks) => {
                    let want: Vec<String> = toks.iter().map(|s| s.to_string()).collect();
                    if imp.as_ref() != Ok(&want) {
                        bad.push(format!("{src:?}: lex() が期待と違う: {imp:?}"));
                    }
                    if ebnf.as_ref() != Ok(&want) {
                        bad.push(format!("{src:?}: EBNF が期待と違う: {ebnf:?}"));
                    }
                }
                None => {
                    if imp.is_ok() {
                        bad.push(format!("{src:?}: lex() が受理した: {imp:?}"));
                    }
                    if !matches!(ebnf, Err(EbnfFail::Stuck(_))) {
                        bad.push(format!("{src:?}: EBNF が拒否しない: {ebnf:?}"));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// 文脈自由では書けない差。lex() だけが拒否する（EBNF は受理する）ことを固定する。
    #[test]
    fn known_gaps_are_exactly_the_documented_ones() {
        let lx = EbnfLexer::new();
        let gaps = [
            "0xFFFFFFFFFFFFFFFFF",
            "0b1111111111111111111111111111111111111111111111111111111111111111111",
            "\"\"\"\n  a\n b\n  \"\"\"",
            "\"\"\"\n  a\n\tb\n  \"\"\"",
        ];
        for src in gaps {
            assert!(impl_tokens(src).is_err(), "lex() が拒否する: {src:?}");
            assert!(lx.tokenize(src).is_ok(), "EBNF は受理する（既知の差）: {src:?}");
            assert!(matches!(compare(&lx, src), Outcome::KnownGap(_)), "{src:?}");
        }
        // 引用符 7 個の raw 文字列は lex() が受理し、EBNF は受理しない。
        let seven = "\"\"\"\"\"\"\"x\"\"\"\"\"\"\"";
        assert_eq!(impl_tokens(seven), Ok(vec![seven.to_string()]));
        assert!(lx.tokenize(seven).is_err());
        assert!(matches!(compare(&lx, seven), Outcome::KnownGap(_)));
        // 空の本文でも 3〜6 個は受理する。
        for n in 3..=6 {
            let q = "\"".repeat(n);
            let s = format!("{q}ab{q}");
            assert_eq!(lx.tokenize(&s), Ok(vec![s.clone()]), "{n} 個");
            assert_eq!(impl_tokens(&s), Ok(vec![s.clone()]), "{n} 個");
        }
    }

    fn check_all(label: &str, items: &[(String, String)]) -> (usize, usize, usize, usize) {
        let lx = EbnfLexer::new();
        let (mut same, mut reject, mut gap) = (0, 0, 0);
        let mut tokens = 0;
        let mut bad = Vec::new();
        for (name, src) in items {
            match compare(&lx, src) {
                Outcome::Same(n) => {
                    same += 1;
                    tokens += n;
                }
                Outcome::BothReject => reject += 1,
                Outcome::KnownGap(_) => gap += 1,
                Outcome::Mismatch(m) => bad.push(format!("{name}: {m}")),
            }
        }
        println!("corpus {label}: {} 件（一致 {same}、両方拒否 {reject}、既知の差 {gap}、トークン {tokens}）", items.len());
        assert!(bad.is_empty(), "{label} の食い違い:\n{}", bad.join("\n"));
        (items.len(), same, reject, gap)
    }

    #[test]
    fn docs_blocks_agree() {
        let c = corpus();
        let (n, ..) = check_all("docs", &c.docs);
        assert!(n >= 150, "docs のブロック数: {n}");
    }

    #[test]
    fn std_libraries_agree() {
        let c = corpus();
        let (n, same, ..) = check_all("libraries/std", &c.std_files);
        assert!(n >= 100 && same == n, "std: {n} 件、一致 {same}");
    }

    #[test]
    fn fixtures_agree() {
        let c = corpus();
        let (n, ..) = check_all("tests/fixtures", &c.fixtures);
        assert!(n >= 200, "fixtures: {n}");
    }

    #[test]
    fn examples_agree() {
        let c = corpus();
        let (n, ..) = check_all("examples", &c.examples);
        assert!(n >= 100, "examples: {n}");
    }

    /// 決定的な擬似乱数で入力を壊し、lex() と EBNF の受理・拒否とトークン列が一致することを見る。
    #[test]
    fn mutated_inputs_agree() {
        let c = corpus();
        let mut seeds: Vec<String> = edge_cases().into_iter().map(|(s, _)| s.to_string()).collect();
        for (_, b) in c.docs.iter().filter(|(_, b)| b.len() <= 400).take(120) {
            seeds.push(b.clone());
        }
        let inserts: Vec<&str> = vec![
            "\"", "\"\"\"", "\\", "/", "*", "/*", "*/", "//", "_", "e", "E", "x", "b", "o", ".", "0", "1", "'", "$", "\n", "\r",
            " ", "é", "😀", "²", "=", ">", "&", "|", ":", "-", "+", "\u{3000}",
        ];
        let mut state: u64 = 0x2545F4914F6CDD1D;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let lx = EbnfLexer::new();
        let (mut total, mut same, mut reject, mut gap) = (0, 0, 0, 0);
        let mut bad = Vec::new();
        for seed in &seeds {
            let chars: Vec<char> = seed.chars().collect();
            for _ in 0..12 {
                let mut cs = chars.clone();
                let rounds = 1 + (next() % 3) as usize;
                for _ in 0..rounds {
                    let at = if cs.is_empty() { 0 } else { (next() as usize) % cs.len() };
                    match next() % 3 {
                        0 if !cs.is_empty() => {
                            cs.remove(at);
                        }
                        1 if !cs.is_empty() => {
                            let ins = inserts[(next() as usize) % inserts.len()];
                            cs.remove(at);
                            for (k, ch) in ins.chars().enumerate() {
                                cs.insert(at + k, ch);
                            }
                        }
                        _ => {
                            let ins = inserts[(next() as usize) % inserts.len()];
                            for (k, ch) in ins.chars().enumerate() {
                                cs.insert((at + k).min(cs.len()), ch);
                            }
                        }
                    }
                }
                let src: String = cs.into_iter().collect();
                total += 1;
                match compare(&lx, &src) {
                    Outcome::Same(_) => same += 1,
                    Outcome::BothReject => reject += 1,
                    Outcome::KnownGap(_) => gap += 1,
                    Outcome::Mismatch(m) => bad.push(format!("{src:?}: {m}")),
                }
            }
        }
        println!("mutation: {total} 件（一致 {same}、両方拒否 {reject}、既知の差 {gap}）");
        assert!(reject >= total / 20 && same >= total / 20, "変異の偏り: {same} / {reject}");
        assert!(bad.is_empty(), "食い違い {} 件:\n{}", bad.len(), bad.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
    }

    /// Unicode の表（付録 A）が、標準ライブラリの判定と全コードポイントで一致する。
    #[test]
    fn unicode_tables_match_std() {
        let g = Grammar::parse(EBNF, &ROOTS).unwrap();
        let class = |name: &str| match &g.rule(name).unwrap().body {
            super::super::ebnf::Expr::Class(c) => c.clone(),
            other => panic!("{name} は文字クラスのはず: {other:?}"),
        };
        let tables: [(&str, fn(char) -> bool); 3] = [
            ("UNICODE_ALPHABETIC", |c| c.is_alphabetic()),
            ("UNICODE_ALPHANUMERIC", |c| c.is_alphanumeric()),
            ("WS_CHAR", |c| c.is_whitespace()),
        ];
        for (name, f) in tables {
            let class = class(name);
            assert!(!class.negated);
            assert!(class.ranges.windows(2).all(|w| w[0].1 < w[1].0), "{name}: 範囲が昇順でない");
            let has = |c: u32| {
                let i = class.ranges.partition_point(|&(_, b)| b < c);
                i < class.ranges.len() && class.ranges[i].0 <= c
            };
            for cp in 0..=0x10FFFFu32 {
                if let Some(ch) = char::from_u32(cp) {
                    assert_eq!(has(cp), f(ch), "{name}: U+{cp:04X}");
                }
            }
        }
    }
}
