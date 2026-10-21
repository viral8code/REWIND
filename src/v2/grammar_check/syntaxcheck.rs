//! 構文の検査（段階 C: 型・パターン・式）。テスト専用。
//!
//! `docs/grammar/rewind.ebnf` の構文規則（`Type`、`TypeParams`、`WhereClause`、`Pattern`、`Expr`）を、
//! 実装の Parser（src/v2.rs）と、**部分の入口**で突き合わせる。
//! - 入力は本物の `lex()` のトークン列。
//! - Parser 側: `Parser { toks, pos: 0, type_depth: 0 }` を作り、`ty()`、`type_params()`、`where_bounds()`、
//!   `pattern()`、`expr(0)` を呼ぶ。そのあと、トークンが `<eof>` だけ（`where` では追加した `{` だけ）に
//!   なったことまで確かめて、受理か拒否かを得る。
//! - EBNF 側: 開始記号から Earley 認識器で受理か拒否かを得る。`>=` は前処理で 2 つのトークンに分ける。
//!
//! 実装が正で、食い違いは EBNF を直す。構文解析器が意味の検査で拒否するもの（`NON_SYNTACTIC`）は、
//! EBNF が受理していても食い違いとは数えない。

use super::earley::Earley;
use super::ebnf::{Bnf, Grammar, Token};
use super::lexcheck::{corpus, impl_tokens};
use super::super::{Parser, Tok};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;

const EBNF: &str = include_str!("../../../docs/grammar/rewind.ebnf");

/// 入口になる構文規則。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Entry {
    Type,
    TypeParams,
    Where,
    Pattern,
    Expr,
}

impl Entry {
    pub const ALL: [Entry; 5] = [Entry::Type, Entry::TypeParams, Entry::Where, Entry::Pattern, Entry::Expr];

    fn rule(self) -> &'static str {
        match self {
            Entry::Type => "Type",
            Entry::TypeParams => "TypeParams",
            Entry::Where => "WhereClause",
            Entry::Pattern => "Pattern",
            Entry::Expr => "Expr",
        }
    }
}

/// 構文解析器が意味の検査で拒否するときの診断の文言（EBNF が受理していても食い違いとしない）。
/// どれも、トークンの並び（文脈自由な形）ではなく、値・名前・深さなどの検査である。
const NON_SYNTACTIC: [&str; 13] = [
    "integer literal overflow",
    "invalid escape",
    "InvalidUnicodeEscape",
    "RawStringIndent",
    "duplicate type parameter",
    "duplicate generic bound",
    "where names an unknown type parameter",
    "syntax nesting",
    "TypeExpansionBudgetExceeded",
    "expected a tuple index such as .0",
    "invalid float",
    "let tuple pattern must contain",
    "duplicate pattern binding",
];

/// 段階 D で文の規則を書くまで、EBNF が受理しない文の語。これを含む断片は食い違いにせず数える。
const PENDING_WORDS: [&str; 12] = [
    "while", "for", "defer", "commit", "revert", "resume", "drop", "publish", "branch", "runtime", "external",
    "using",
];

pub struct GTok {
    pub kind: &'static str,
    pub text: String,
}

impl Token for GTok {
    fn text(&self) -> &str {
        &self.text
    }
    fn kind(&self) -> &str {
        self.kind
    }
}

/// トークンの text から、字句規則の種別を決める（`>=` の分割は `earley_tokens` が行う）。
fn kind_of(text: &str) -> &'static str {
    let first = text.chars().next();
    match first {
        Some('"') => {
            if text.starts_with("\"\"\"") {
                "RAW_STRING"
            } else {
                "STRING"
            }
        }
        Some(c) if c.is_ascii_digit() => {
            let lower = text.to_ascii_lowercase();
            let prefixed = lower.starts_with("0x") || lower.starts_with("0b") || lower.starts_with("0o");
            if !prefixed && (text.contains('.') || lower.contains('e')) {
                "FLOAT"
            } else {
                "INT"
            }
        }
        Some('_') if text.chars().all(|c| c == '_' || c.is_ascii_digit()) => "IDENT UNDERSCORE_NUM",
        Some(c) if c.is_alphabetic() || c == '_' => "IDENT",
        _ => "SYMBOL",
    }
}

/// `>=` を `>`（GE_FIRST）と `=`（GE_SECOND）に分け、各トークンに種別をつける。
pub fn earley_tokens(texts: &[String]) -> Vec<GTok> {
    let mut out = Vec::with_capacity(texts.len() + 4);
    for t in texts {
        if t == ">=" {
            out.push(GTok { kind: "GE_FIRST", text: ">".into() });
            out.push(GTok { kind: "GE_SECOND", text: "=".into() });
        } else {
            out.push(GTok { kind: kind_of(t), text: t.clone() });
        }
    }
    out
}

fn compiled() -> &'static HashMap<Entry, Bnf> {
    static C: OnceLock<HashMap<Entry, Bnf>> = OnceLock::new();
    C.get_or_init(|| {
        let mut roots = vec!["TOKEN", "SKIP"];
        roots.extend(Entry::ALL.iter().map(|e| e.rule()));
        let g = Grammar::parse(EBNF, &roots).unwrap_or_else(|e| panic!("rewind.ebnf の読み込み: {e:#?}"));
        Entry::ALL
            .iter()
            .map(|&e| {
                let b = g.compile(e.rule()).unwrap_or_else(|x| panic!("{} の展開: {x:#?}", e.rule()));
                (e, b)
            })
            .collect()
    })
}

pub fn earley_accepts(entry: Entry, texts: &[String]) -> bool {
    let bnf = &compiled()[&entry];
    Earley::new(bnf).recognize(&earley_tokens(texts)).is_ok()
}

/// Parser の受理（`Ok`）か拒否（診断の文字列）。2 つ目は、`>=` が `=` に書き換えられたまま残ったか。
pub fn parser_verdict(entry: Entry, texts: &[String]) -> (Result<(), String>, bool) {
    let mut toks: Vec<Tok> = texts
        .iter()
        .enumerate()
        .map(|(i, t)| Tok { source: String::new(), text: t.clone(), line: 1, col: i + 1 })
        .collect();
    if entry == Entry::Where {
        toks.push(Tok { source: String::new(), text: "{".into(), line: 1, col: texts.len() + 1 });
    }
    toks.push(Tok { source: String::new(), text: "<eof>".into(), line: 1, col: texts.len() + 2 });
    let mut p = Parser { toks, pos: 0, type_depth: 0 };
    let r = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        match entry {
            Entry::Type => p.ty().map(|_| ()),
            Entry::TypeParams => p.type_params().map(|_| ()),
            Entry::Where => {
                let mut params: Vec<(String, Option<String>)> =
                    ["T", "U", "V"].iter().map(|n| (n.to_string(), None)).collect();
                p.where_bounds(&mut params)
            }
            Entry::Pattern => p.pattern().map(|_| ()),
            Entry::Expr => p.expr(0).map(|_| ()),
        }
        .map_err(|e| format!("{e:?}"))
    }));
    let mutated = p
        .toks
        .iter()
        .zip(texts.iter())
        .any(|(now, orig)| orig == ">=" && now.text == "=");
    let res = match r {
        Err(_) => Err("panic".to_string()),
        Ok(Err(e)) => Err(e),
        Ok(Ok(())) => {
            if p.pos == texts.len() {
                Ok(())
            } else {
                Err(format!("入力が残る（{} 番目のトークン）", p.pos))
            }
        }
    };
    (res, mutated)
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Class {
    /// 両方が受理した、または両方が拒否した。
    Agree,
    /// Parser は意味の検査で拒否し、EBNF は受理した。
    NonSyntactic,
    /// 既知の差（理由）。
    KnownGap(&'static str),
    /// 段階 D で文の規則を書くまでの差。
    Pending,
    Mismatch(String),
}

pub struct Res {
    pub parser_ok: bool,
    pub earley_ok: bool,
    pub class: Class,
}

fn has_pending(texts: &[String]) -> bool {
    texts.iter().any(|t| PENDING_WORDS.contains(&t.as_str()))
}

pub fn compare(entry: Entry, texts: &[String]) -> Res {
    let (pr, mutated) = parser_verdict(entry, texts);
    let ok_e = earley_accepts(entry, texts);
    let parser_ok = pr.is_ok();
    let class = match (&pr, ok_e) {
        (Ok(()), true) | (Err(_), false) => Class::Agree,
        (Ok(()), false) => {
            if has_pending(texts) {
                Class::Pending
            } else {
                Class::Mismatch("Parser は受理、EBNF は拒否".into())
            }
        }
        (Err(e), true) => {
            if NON_SYNTACTIC.iter().any(|m| e.contains(m)) {
                Class::NonSyntactic
            } else if mutated {
                Class::KnownGap("名前の先読みが `>=` を `=` に書き換えたまま位置を戻す（癖 1）")
            } else if has_pending(texts) {
                Class::Pending
            } else {
                Class::Mismatch(format!("Parser は拒否（{}）、EBNF は受理", e.chars().take(100).collect::<String>()))
            }
        }
    };
    Res { parser_ok, earley_ok: ok_e, class }
}

fn toks(src: &str) -> Vec<String> {
    impl_tokens(src).unwrap_or_else(|e| panic!("lex() が拒否: {src:?}: {e}"))
}

// ---------------------------------------------------------------- 断片の抜き出し

const MAX_FRAGMENT: usize = 600;

fn is_open(t: &str) -> bool {
    matches!(t, "(" | "[" | "{")
}

fn is_close(t: &str) -> bool {
    matches!(t, ")" | "]" | "}")
}

type Fragment = (Entry, Vec<String>);

/// `from` から、深さ 0 の `stop` のどれかか、深さが負になる直前までの終了位置（その位置は含まない）。
fn scan_until(t: &[String], from: usize, stop: &[&str]) -> Option<usize> {
    let mut depth = 0i32;
    let mut j = from;
    while j < t.len() {
        let x = t[j].as_str();
        if is_close(x) {
            if depth == 0 {
                return if stop.contains(&x) { Some(j) } else { None };
            }
            depth -= 1;
        } else if is_open(x) {
            depth += 1;
        } else if depth == 0 && stop.contains(&x) {
            return Some(j);
        }
        j += 1;
    }
    None
}

pub fn fragments(t: &[String]) -> Vec<Fragment> {
    let mut out: Vec<Fragment> = Vec::new();
    let mut add = |entry: Entry, s: &[String]| {
        if !s.is_empty() && s.len() <= MAX_FRAGMENT {
            out.push((entry, s.to_vec()));
        }
    };
    let n = t.len();
    for i in 0..n {
        match t[i].as_str() {
            "let" | "var" => {
                let Some(end) = scan_until(t, i + 1, &[";"]) else { continue };
                // 深さ 0 の最初の `:` と `=`（`>=` は型の終わりと `=`）
                let mut depth = 0i32;
                let (mut colon, mut eq, mut ge) = (None, None, None);
                for j in i + 1..end {
                    let x = t[j].as_str();
                    if is_open(x) {
                        depth += 1;
                    } else if is_close(x) {
                        depth -= 1;
                    } else if depth == 0 {
                        if x == ":" && colon.is_none() && eq.is_none() && ge.is_none() {
                            colon = Some(j);
                        } else if x == "=" && eq.is_none() && ge.is_none() {
                            eq = Some(j);
                        } else if x == ">=" && eq.is_none() && ge.is_none() {
                            ge = Some(j);
                        }
                    }
                }
                let (ty_end, init_start) = match (eq, ge) {
                    (Some(e), _) => (e, e + 1),
                    (None, Some(g)) => (g, g + 1),
                    _ => continue,
                };
                if let Some(c) = colon {
                    let mut ty: Vec<String> = t[c + 1..ty_end].to_vec();
                    if ge.is_some() && eq.is_none() {
                        ty.push(">".into());
                    }
                    add(Entry::Type, &ty);
                }
                if init_start <= end {
                    let mut init: Vec<String> = Vec::new();
                    if ge.is_some() && eq.is_none() {
                        init.push("=".into());
                        init.extend_from_slice(&t[init_start..end]);
                        // 先頭が `=` の式は Parser が拒否するので、`>=` の後ろだけを式の断片にする。
                        init.remove(0);
                    } else {
                        init.extend_from_slice(&t[init_start..end]);
                    }
                    add(Entry::Expr, &init);
                }
                if t.get(i + 1).map(|s| s.as_str()) == Some("(") {
                    let pend = colon.or(eq).or(ge).unwrap_or(end);
                    add(Entry::Pattern, &t[i + 1..pend]);
                }
            }
            "return" => {
                if let Some(end) = scan_until(t, i + 1, &[";"]) {
                    add(Entry::Expr, &t[i + 1..end]);
                }
            }
            "if" | "while" if t.get(i + 1).map(|s| s.as_str()) == Some("let") => {
                if let Some(eq) = scan_until(t, i + 2, &["="]) {
                    add(Entry::Pattern, &t[i + 2..eq]);
                }
            }
            "=>" => {
                // 腕のパターン: 直前の `,`、`{`、前の腕の `}` まで戻る。
                let mut depth = 0i32;
                let mut start = 0;
                for j in (0..i).rev() {
                    let x = t[j].as_str();
                    if is_close(x) {
                        if depth == 0 && x == "}" && j + 1 != i {
                            start = j + 1;
                            break;
                        }
                        depth += 1;
                    } else if is_open(x) {
                        if depth == 0 {
                            start = j + 1;
                            break;
                        }
                        depth -= 1;
                    } else if depth == 0 && x == "," {
                        start = j + 1;
                        break;
                    }
                }
                let seg = &t[start..i];
                let mut cut = seg.len();
                let mut d = 0i32;
                for (k, x) in seg.iter().enumerate() {
                    if is_open(x) {
                        d += 1;
                    } else if is_close(x) {
                        d -= 1;
                    } else if d == 0 && x == "if" {
                        cut = k;
                        break;
                    }
                }
                add(Entry::Pattern, &seg[..cut]);
                // 腕の本体
                let from = i + 1;
                if t.get(from).map(|s| s.as_str()) == Some("{") {
                    let mut d = 0i32;
                    for (k, x) in t[from..].iter().enumerate() {
                        if is_open(x) {
                            d += 1;
                        } else if is_close(x) {
                            d -= 1;
                            if d == 0 {
                                add(Entry::Expr, &t[from..from + k + 1]);
                                break;
                            }
                        }
                    }
                } else if let Some(end) = scan_until(t, from, &[",", "}"]) {
                    add(Entry::Expr, &t[from..end]);
                }
            }
            "fn" => {
                // fn 名前 <型パラメータ> ( 引数 ) -> 戻り値 where ... {
                let mut j = i + 1;
                if t.get(j).is_none() {
                    continue;
                }
                j += 1;
                if t.get(j).map(|s| s.as_str()) == Some("<") {
                    let mut d = 0i32;
                    let st = j;
                    while j < n {
                        match t[j].as_str() {
                            "<" => d += 1,
                            ">" => {
                                d -= 1;
                                if d == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    if j >= n {
                        continue;
                    }
                    add(Entry::TypeParams, &t[st..=j]);
                    j += 1;
                }
                if t.get(j).map(|s| s.as_str()) != Some("(") {
                    continue;
                }
                let Some(close) = scan_until(t, j + 1, &[")"]) else { continue };
                // 引数ごとの型
                let mut k = j + 1;
                while k < close {
                    let end = scan_until(t, k, &[",", ")"]).unwrap_or(close).min(close);
                    if t.get(k + 1).map(|s| s.as_str()) == Some(":") && k + 2 <= end {
                        let mut e2 = end;
                        if let Some(eqp) = (k + 2..end).find(|&x| t[x] == "=") {
                            e2 = eqp;
                        }
                        add(Entry::Type, &t[k + 2..e2]);
                    }
                    k = end + 1;
                }
                let mut after = close + 1;
                if t.get(after).map(|s| s.as_str()) == Some("->") {
                    let end = scan_until(t, after + 1, &["{", "where", ";"]).unwrap_or(n);
                    add(Entry::Type, &t[after + 1..end]);
                    after = end;
                }
                if t.get(after).map(|s| s.as_str()) == Some("where") {
                    if let Some(end) = scan_until(t, after + 1, &["{", "=", ";"]) {
                        add(Entry::Where, &t[after..end]);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn check_list(entry: Entry, list: &[(&str, bool)]) {
        let accepts = list.iter().filter(|(_, a)| *a).count();
        let rejects = list.len() - accepts;
        assert!(list.len() >= 50, "{entry:?}: 断片は 50 件以上（{}）", list.len());
        assert!(accepts >= 30, "{entry:?}: 受理は 30 件以上（{accepts}）");
        assert!(rejects >= 20, "{entry:?}: 拒否は 20 件以上（{rejects}）");
        let mut bad = Vec::new();
        for (src, expect) in list {
            let t = toks(src);
            let r = compare(entry, &t);
            let ok = !matches!(r.class, Class::Mismatch(_) | Class::Pending) && r.parser_ok == *expect;
            if !ok {
                bad.push(format!(
                    "{src:?}: 期待 {} / Parser {} / EBNF {} / {:?}",
                    if *expect { "受理" } else { "拒否" },
                    r.parser_ok,
                    r.earley_ok,
                    r.class
                ));
            }
        }
        assert!(bad.is_empty(), "{entry:?}:\n{}", bad.join("\n"));
    }

    const TYPES: &[(&str, bool)] = &[
        ("Int", true),
        ("List<Int>", true),
        ("Map<String, Int>", true),
        ("Map<String, List<Int>>", true),
        ("Map<String,Int,>", true),
        ("List<List<List<Int>>>", true),
        ("()", true),
        ("(Int)", true),
        ("(((Int)))", true),
        ("(Int,)", true),
        ("(Int, String)", true),
        ("(Int, String,)", true),
        ("&Int", true),
        ("&mut Int", true),
        ("& mut Int", true),
        ("&mut List<Int>", true),
        ("&&mutx", false),
        ("List<&Int>", true),
        ("fn() -> Int", true),
        ("fn(Int) -> Int", true),
        ("fn(Int,) -> Int", true),
        ("fn(Int, String) -> Bool", true),
        ("fn() -> ()", true),
        ("fn(fn(Int) -> Int) -> Int", true),
        ("fn() -> fn() -> Int", true),
        ("fn() -> Int effects Clock", true),
        ("fn() -> Int effects { Clock, Random }", true),
        ("fn() -> Int effects {}", true),
        ("fn() -> Int effects { Clock, }", true),
        ("fn() -> Int captures { Send }", true),
        ("fn() -> Int captures { Send, Share, }", true),
        ("fn() -> Int effects { Clock } captures { Share }", true),
        ("fn() -> Int effects Clock captures { Send }", true),
        ("A.B", true),
        ("A.B::C<T,>", true),
        ("a::b::c", true),
        ("Result<Int, String>", true),
        ("Option<(Int, String)>", true),
        ("List<fn(Int) -> Int>", true),
        ("Map<String, fn() -> Int effects Clock>", true),
        ("_", true),
        ("x", true),
        ("Self", true),
        ("", false),
        ("List<>", false),
        ("List<Int", false),
        ("List<Int,,>", false),
        ("List<,Int>", false),
        ("(,)", false),
        ("(Int Int)", false),
        ("(Int,,)", false),
        ("&", false),
        ("&mut", false),
        ("&&Int", false),
        ("fn", false),
        ("fn(Int)", false),
        ("fn() Int", false),
        ("fn() ->", false),
        ("fn(,) -> Int", false),
        ("fn() -> Int effects", false),
        ("fn() -> Int captures { Foo }", false),
        ("fn() -> Int captures Send", false),
        ("begin", false),
        ("A.", false),
        ("A::", false),
        ("A<B>.C", false),
        ("1", false),
        ("\"s\"", false),
        ("Int Int", false),
        ("List<Int>>", false),
        ("->", false),
        ("List<Int>=", false),
    ];

    const TYPE_PARAMS: &[(&str, bool)] = &[
        ("", true),
        ("<T>", true),
        ("<T,>", true),
        ("<T, U>", true),
        ("<T: Eq>", true),
        ("<T: Eq + Hash, U>", true),
        ("<T: Eq + Hash,>", true),
        ("<>", false),
        ("<T", false),
        ("<T U>", false),
        ("<T: >", false),
        ("<T, T>", false),
        ("<T: Eq + Eq>", false),
        ("<1>", false),
    ];

    const WHERES: &[(&str, bool)] = &[
        ("where T: Eq", true),
        ("where T: Eq,", true),
        ("where T: Eq + Hash", true),
        ("where T: Eq, U: Hash", true),
        ("where T: Eq, U: Hash,", true),
        ("where", false),
        ("where T", false),
        ("where T:", false),
        ("where W: Eq", false),
        ("where T: Eq,, U: Hash", false),
        ("where T: Eq + Eq", false),
    ];

    const PATTERNS: &[(&str, bool)] = &[
        ("_", true),
        ("x", true),
        ("_x", true),
        ("x.y", true),
        ("true", true),
        ("false", true),
        ("\"a\"", true),
        ("\"\"\"raw\"\"\"", true),
        ("0", true),
        ("42", true),
        ("-1", true),
        ("1..5", true),
        ("0..0", true),
        ("-5..-1", true),
        ("0x10..0xff", true),
        ("()", true),
        ("(a)", true),
        ("(a, b)", true),
        ("(a,)", true),
        ("(a, _)", true),
        ("(1, \"a\", true)", true),
        ("[]", true),
        ("[a]", true),
        ("[a, b,]", true),
        ("[_, _]", true),
        ("None", true),
        ("Some(x)", true),
        ("Some(_)", true),
        ("Some(Some(x))", true),
        ("Some([a, b])", true),
        ("Ok((a, b))", true),
        ("E::A", true),
        ("E::A()", true),
        ("E::A(x, y)", true),
        ("E::A(x,)", true),
        ("Shape::Circle { r: x }", true),
        ("Point { x: a, y: b }", true),
        ("Point { x: a, y: b, }", true),
        ("Point {}", true),
        ("x { a: p }", true),
        ("List<Int>::Empty", true),
        ("Option<Int>::Some(x)", true),
        ("Result<Int, String>::Ok(v)", true),
        ("a.b::C", true),
        ("", false),
        ("(", false),
        ("(a b)", false),
        ("(,)", false),
        ("(a,,b)", false),
        ("[,]", false),
        ("[a b]", false),
        ("[a,,]", false),
        ("E::", false),
        ("::A", false),
        ("Some(", false),
        ("Point { x }", false),
        ("Point { x: }", false),
        ("Point { : a }", false),
        ("E::A { x: 1,, }", false),
        ("1..", false),
        ("..5", false),
        ("-", false),
        ("- x", false),
        ("-a", false),
        ("1.5", false),
        ("begin", false),
        ("x::", false),
        ("_.x", false),
        ("true.x", false),
        ("\"a\" + \"b\"", false),
        ("\"a\".x", false),
        ("a b", false),
        ("Some(x) y", false),
        ("1 2", false),
        ("99999999999999999999", false),
    ];

    const EXPRS: &[(&str, bool)] = &[
        ("1", true),
        ("1.5", true),
        ("1e3", true),
        ("0xff", true),
        ("\"s\"", true),
        ("\"\"\"raw\"\"\"", true),
        ("true", true),
        ("None", true),
        ("null", true),
        ("x", true),
        ("begin", true),
        ("x.y", true),
        ("x.0", true),
        ("x.0.1", true),
        ("p.1.0", true),
        ("x.y.z", true),
        ("f()", true),
        ("f(a)", true),
        ("f(a,)", true),
        ("f(a, b)", true),
        ("f(a: 1, b: 2)", true),
        ("f(a)(b)", true),
        ("f(a).g(b)", true),
        ("a + b * c", true),
        ("a * b + c", true),
        ("(a + b) * c", true),
        ("1 + 2 - 3", true),
        ("a % b / c", true),
        ("-a", true),
        ("!a", true),
        ("- - a", true),
        ("-a.b", true),
        ("!f(x)", true),
        ("a + -b", true),
        ("-9223372036854775808", true),
        ("&a", true),
        ("&a.b", true),
        ("&mut a", true),
        ("& mut a", true),
        ("a || b", true),
        ("a && b || c", true),
        ("a == b", true),
        ("a != b", true),
        ("a < b", true),
        ("a <= b", true),
        ("a > b", true),
        ("a >= b", true),
        ("a < b > c", true),
        ("a < b || c > d", true),
        ("List<Int>()", true),
        ("Map<String,Int,>()", true),
        ("Map<String, Int>::new()", true),
        ("E::A", true),
        ("E::A(1)", true),
        ("E::A { x: 1 }", true),
        ("E::A {}", true),
        ("a.b::C", true),
        ("a.b::C { x: 1 }", true),
        ("(a)::B", true),
        ("(a.b)::C", true),
        ("((a))::B { x: 1 }", true),
        ("(a)::B(1)", true),
        ("-(a)::B.c", true),
        ("(a.b.c)::D", false),
        ("(1)::B", false),
        ("(a + b)::C", false),
        ("(a(1))::C", false),
        ("x.f<Int>(1)", true),
        ("Json.parse<Int>(s)", true),
        ("x?", true),
        ("x?.y", true),
        ("f(x)?", true),
        ("xs[0]", true),
        ("xs[i][j]", true),
        ("xs[0].y", true),
        ("()", true),
        ("(a)", true),
        ("(1,)", true),
        ("(1, 2)", true),
        ("[1, 2, 3]", true),
        ("[1,]", true),
        ("{ a: 1 }", true),
        ("{ a: 1, b: 2, }", true),
        ("{ \"k\": v, f(x): 2 }", true),
        ("{ 1: 2 }", true),
        ("{ }", true),
        ("{ a }", true),
        ("{ 1 }", true),
        ("{ let x = 1; x }", true),
        ("{ let (a, b) = p; a }", true),
        ("{ x = 1; }", true),
        ("{ x += 1; x }", true),
        ("{ if a { 1 } else { 2 } }", true),
        ("|a, b| a + b", true),
        ("|| 1", true),
        ("| | 1", true),
        ("|a| a", true),
        ("|a: Int| a", true),
        ("|a: Int, b: Int,| a", true),
        ("|a| -> Int { return a; }", true),
        ("|| -> Int { return 1; }", true),
        ("a || || 1", true),
        ("if a { 1 } else { 2 }", true),
        ("if a { 1 }", true),
        ("if let Some(x) = y { x } else { 0 }", true),
        ("if a { 1 } else if b { 2 } else { 3 }", true),
        ("match x { }", true),
        ("match x { 1 => 2, _ => 3 }", true),
        ("match x { A => 1, B => { 2 } C => 3 }", true),
        ("match x { A => [1] B => 2 }", true),
        ("match x { Some(v) if v > 0 => v, _ => 0 }", true),
        ("match (E::A) { _ => 1 }", true),
        ("await x", true),
        ("spawn f()", true),
        ("move x", true),
        ("capture value |a| a", true),
        ("capture move |a| -> Int { return a; }", true),
        ("capture borrow || 1", true),
        ("", false),
        ("1 +", false),
        ("+ 1", false),
        ("(", false),
        (")", false),
        ("(1", false),
        ("f(", false),
        ("f(a b)", false),
        ("f(,)", false),
        ("f(a: )", false),
        ("[]", false),
        ("[,]", false),
        ("[1 2]", false),
        ("{ a: }", false),
        ("{ : 1 }", false),
        ("{ f(x): 2 }", false),
        ("match E::A { _ => 1 }", false),
        ("if x == E::A { 1 }", false),
        ("if { 1 }", false),
        ("if a { 1 } else", false),
        ("match x { 1 => 2 3 => 4 }", false),
        ("match x { A => if c { 1 } else { 2 } B => 3 }", false),
        ("match x { A => { 1 } -2 => 3 }", false),
        ("capture value 1", false),
        ("capture value |a| -> Int { return a; }(1)", false),
        ("&&a", false),
        ("&mut", false),
        ("a b", false),
        ("x.", false),
        ("x::", false),
        (".x", false),
        ("x.begin", false),
        ("E::begin", false),
        ("-", false),
        ("!", false),
        ("a >=", false),
        ("|a|", false),
        ("|a b| a", false),
        ("|a:| a", false),
        ("|a| ->", false),
        ("x.0x1", false),
        ("x.1e5", false),
        ("1 = 2", false),
        ("a = b", false),
        ("9223372036854775808", false),
        ("\"a\\q\"", false),
        ("a < b >= c", false),
        ("a.b < c >= d", false),
    ];

    #[test]
    fn empty_input_is_rejected_without_panic() {
        for entry in [Entry::Type, Entry::Pattern, Entry::Expr] {
            let (r, _) = parser_verdict(entry, &[]);
            let e = r.expect_err("空の入力は拒否される");
            assert_ne!(e, "panic", "{entry:?}: Parser が panic した");
            assert!(!earley_accepts(entry, &[]));
        }
        // 空でも受理される入口（型パラメータなし）。
        assert!(parser_verdict(Entry::TypeParams, &[]).0.is_ok());
        assert!(earley_accepts(Entry::TypeParams, &[]));
    }

    #[test]
    fn grammar_has_syntax_entries() {
        let _ = compiled();
    }

    #[test]
    fn type_fragments_agree() {
        check_list(Entry::Type, TYPES);
    }

    #[test]
    fn type_param_fragments_agree() {
        // 型パラメータと where は断片が少ないので、件数の下限は課さない。
        for (entry, list) in [(Entry::TypeParams, TYPE_PARAMS), (Entry::Where, WHERES)] {
            let mut bad = Vec::new();
            for (src, expect) in list {
                let t = toks(src);
                let r = compare(entry, &t);
                if matches!(r.class, Class::Mismatch(_)) || r.parser_ok != *expect {
                    bad.push(format!("{src:?}: 期待 {expect} / Parser {} / EBNF {} / {:?}", r.parser_ok, r.earley_ok, r.class));
                }
            }
            assert!(bad.is_empty(), "{entry:?}:\n{}", bad.join("\n"));
        }
    }

    #[test]
    fn pattern_fragments_agree() {
        check_list(Entry::Pattern, PATTERNS);
    }

    #[test]
    fn expr_fragments_agree() {
        check_list(Entry::Expr, EXPRS);
    }

    #[test]
    fn fragment_counts_are_reported() {
        for (name, list) in [("Type", TYPES), ("Pattern", PATTERNS), ("Expr", EXPRS)] {
            let a = list.iter().filter(|(_, a)| *a).count();
            println!("{name}: {} 件（受理 {a}、拒否 {}）", list.len(), list.len() - a);
        }
    }

    #[test]
    fn ge_split_is_modeled() {
        // `>=` は常に 2 トークンに分ける。比較演算子は GE_FIRST GE_SECOND、型引数の閉じは `>`。
        let t = toks("a >= b");
        let g = earley_tokens(&t);
        let v: Vec<(&str, &str)> = g.iter().map(|x| (x.kind, x.text.as_str())).collect();
        assert_eq!(v, [("IDENT", "a"), ("GE_FIRST", ">"), ("GE_SECOND", "="), ("IDENT", "b")]);
        // `> =`（空白あり）は比較演算子にならない。
        assert!(!earley_accepts(Entry::Expr, &toks("a > = b")));
        assert!(earley_accepts(Entry::Expr, &toks("a >= b")));
    }

    fn excerpt(t: &[String]) -> String {
        t.iter().take(40).cloned().collect::<Vec<_>>().join(" ")
    }

    fn run_corpus(label: &str, files: &[(String, String)]) -> BTreeMap<&'static str, usize> {
        let mut tally: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut bad: Vec<String> = Vec::new();
        for (name, src) in files {
            let Ok(t) = impl_tokens(src) else { continue };
            for (entry, frag) in fragments(&t) {
                let r = compare(entry, &frag);
                let key = match (&r.class, r.parser_ok) {
                    (Class::Agree, true) => "受理で一致",
                    (Class::Agree, false) => "拒否で一致",
                    (Class::NonSyntactic, _) => "意味の検査",
                    (Class::KnownGap(_), _) => "既知の差",
                    (Class::Pending, _) => "段階 D 待ち",
                    (Class::Mismatch(_), _) => "食い違い",
                };
                *tally.entry(key).or_default() += 1;
                if std::env::var("GC_DEBUG").is_ok() && matches!(r.class, Class::Pending | Class::KnownGap(_) | Class::NonSyntactic) {
                    println!("{:?} {name} [{entry:?}] p={} e={}: {}", r.class, r.parser_ok, r.earley_ok, excerpt(&frag));
                }
                if let Class::Mismatch(m) = &r.class {
                    if bad.len() < 12 {
                        bad.push(format!("{name} [{entry:?}] {m}: {}", excerpt(&frag)));
                    }
                }
            }
        }
        println!("{label}: {tally:?}");
        assert!(bad.is_empty(), "{label}: 食い違い\n{}", bad.join("\n"));
        tally
    }


    const VOCAB: [&str; 60] = [
        "(", ")", "{", "}", "[", "]", ",", ":", "::", ".", "<", ">", ">=", "=", "|", "||", "&", "-", "?", "if", "match",
        "mut", "fn", "_", "begin", "x", "1", "\"s\"", "=>", ";", "->", "effects", "captures", "let", "capture", "value",
        "move", "await", "true", "None", "else", "&&", "!", "..", "+", "1.5", "Int", "E", "borrow", "spawn", "return",
        "<=", "==", "*", "0.1", "Send", "where", "_1", "__", "null",
    ];

    fn seeds() -> Vec<Fragment> {
        let mut seeds: Vec<Fragment> = Vec::new();
        for (entry, list) in [(Entry::Type, TYPES), (Entry::Pattern, PATTERNS), (Entry::Expr, EXPRS)] {
            for (src, _) in list {
                seeds.push((entry, toks(src)));
            }
        }
        let c = corpus();
        let mut picked = 0;
        for (_, src) in c.docs.iter().chain(c.std_files.iter()).chain(c.fixtures.iter()) {
            if let Ok(t) = impl_tokens(src) {
                for (entry, f) in fragments(&t).into_iter().step_by(37) {
                    if f.len() <= 24 && picked < 400 {
                        seeds.push((entry, f));
                        picked += 1;
                    }
                }
            }
        }
        seeds
    }

    /// 変異した断片を比べ、結果を数えて、食い違いを集める。
    struct Fuzz {
        tally: BTreeMap<&'static str, usize>,
        bad: BTreeMap<String, String>,
    }

    impl Fuzz {
        fn new() -> Self {
            Fuzz { tally: BTreeMap::new(), bad: BTreeMap::new() }
        }

        fn run(&mut self, entry: Entry, f: Vec<String>) {
            let r = compare(entry, &f);
            let key = match (&r.class, r.parser_ok) {
                (Class::Agree, true) => "受理で一致",
                (Class::Agree, false) => "拒否で一致",
                (Class::NonSyntactic, _) => "意味の検査",
                (Class::KnownGap(_), _) => "既知の差",
                (Class::Pending, _) => "段階 D 待ち",
                (Class::Mismatch(m), _) => {
                    self.bad
                        .entry(format!("{entry:?} {m} {}", f.join(" ")))
                        .or_insert_with(|| format!("[{entry:?}] p={} e={}: {}", r.parser_ok, r.earley_ok, f.join(" ")));
                    "食い違い"
                }
            };
            *self.tally.entry(key).or_default() += 1;
        }

        fn finish(self, label: &str) {
            println!("{label}: {:?}", self.tally);
            let msgs: Vec<&String> = self.bad.values().take(40).collect();
            assert!(
                self.bad.is_empty(),
                "{label}: 食い違い {} 種:
{}",
                self.bad.len(),
                msgs.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("
")
            );
        }
    }

    /// 受理される断片のトークンを 1 つ消す、置き換える、足す変異を作り、Parser と EBNF を突き合わせる。
    #[test]
    fn mutated_fragments_agree() {
        let mut fz = Fuzz::new();
        for (entry, seed) in &seeds() {
            for i in 0..seed.len() {
                let mut d = seed.clone();
                d.remove(i);
                fz.run(*entry, d);
                for v in VOCAB {
                    let mut r = seed.clone();
                    r[i] = v.to_string();
                    fz.run(*entry, r);
                    let mut a = seed.clone();
                    a.insert(i, v.to_string());
                    fz.run(*entry, a);
                }
            }
        }
        fz.finish("1 変異");
    }

    /// 2 つの変異を重ねる（決まった乱数列で選ぶ）。
    #[test]
    fn double_mutated_fragments_agree() {
        // 既定は決まった種と回数。GC_FUZZ_SEED と GC_FUZZ_N で変えて、長く探せる。
        let env = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let mut state: u64 = env("GC_FUZZ_SEED", 0x9E37_79B9_7F4A_7C15);
        let mut next = move |n: usize| -> usize {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % n as u64) as usize
        };
        let mut fz = Fuzz::new();
        let seeds = seeds();
        for _ in 0..env("GC_FUZZ_N", 250_000) {
            let (entry, seed) = &seeds[next(seeds.len())];
            let mut f = seed.clone();
            for _ in 0..2 {
                if f.is_empty() {
                    break;
                }
                let i = next(f.len());
                match next(3) {
                    0 => {
                        f.remove(i);
                    }
                    1 => f[i] = VOCAB[next(VOCAB.len())].to_string(),
                    _ => f.insert(i, VOCAB[next(VOCAB.len())].to_string()),
                }
            }
            fz.run(*entry, f);
        }
        fz.finish("2 変異");
    }

    #[test]
    fn docs_fragments_agree() {
        let c = corpus();
        let t = run_corpus("docs", &c.docs);
        assert!(t.get("受理で一致").copied().unwrap_or(0) > 50);
    }

    #[test]
    fn std_fragments_agree() {
        let c = corpus();
        let t = run_corpus("std", &c.std_files);
        assert!(t.get("受理で一致").copied().unwrap_or(0) > 200);
    }

    #[test]
    fn fixtures_fragments_agree() {
        let c = corpus();
        let t = run_corpus("fixtures", &c.fixtures);
        assert!(t.get("受理で一致").copied().unwrap_or(0) > 20);
    }

    #[test]
    fn examples_fragments_agree() {
        let c = corpus();
        let t = run_corpus("examples", &c.examples);
        assert!(t.get("受理で一致").copied().unwrap_or(0) > 20);
    }
}
