//! W3C 形式の EBNF の読み込み、層の検査、BNF への展開。
//!
//! 名前の付け方で層を分ける。`UPPER_CASE`（小文字を含まない）は字句規則、
//! `CamelCase`（小文字を含む）は構文規則である。大文字 1 字の名前（`E` など）は字句規則になる。

use std::collections::{HashMap, HashSet};

/// 入れ子の深さの上限（読み込み器の再帰でスタックを使い切らないため）。
const MAX_NEST: usize = 500;

#[derive(Clone, Debug, PartialEq)]
pub struct CharClass {
    pub ranges: Vec<(u32, u32)>,
    pub negated: bool,
}

impl CharClass {
    pub fn contains(&self, c: u32) -> bool {
        self.ranges.iter().any(|&(a, b)| a <= c && c <= b) != self.negated
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Empty,
    Lit(String),
    Class(CharClass),
    Ref(String),
    Seq(Vec<Expr>),
    Alt(Vec<Expr>),
    Opt(Box<Expr>),
    Star(Box<Expr>),
    Plus(Box<Expr>),
    Diff(Box<Expr>, Box<Expr>),
    /// `[ ns: X ]` 次のトークンが X でない。
    NotStart(Box<Expr>),
    /// `[ nf: X ]` 直後に X が来ない。
    NotFollow(Box<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Lexical,
    Syntax,
}

pub fn layer_of(name: &str) -> Option<Layer> {
    let mut chars = name.chars();
    let first = chars.next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    if name.chars().any(|c| c.is_ascii_lowercase()) {
        Some(Layer::Syntax)
    } else {
        Some(Layer::Lexical)
    }
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub name: String,
    pub body: Expr,
    pub line: usize,
}

// ---------------------------------------------------------------- 終端と入力

/// 認識器へ渡すトークン。字句規則を種別（`kind`）として参照する構文規則に使う。
pub trait Token {
    fn text(&self) -> &str;
    fn kind(&self) -> &str {
        ""
    }
}

impl Token for &str {
    fn text(&self) -> &str {
        self
    }
}

impl Token for String {
    fn text(&self) -> &str {
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Terminal {
    /// トークンの文字列がこれと等しい。
    Text(String),
    /// トークンの種別がこれと等しい。
    Kind(String),
    /// 1 文字のトークンで、その文字がクラスに入る。
    Class(CharClass),
    Any(Vec<Terminal>),
    Diff(Box<Terminal>, Box<Terminal>),
}

impl Terminal {
    pub fn matches<T: Token>(&self, tok: &T) -> bool {
        match self {
            Terminal::Text(s) => tok.text() == s,
            Terminal::Kind(k) => tok.kind() == k,
            Terminal::Class(c) => {
                let mut it = tok.text().chars();
                match (it.next(), it.next()) {
                    (Some(ch), None) => c.contains(ch as u32),
                    _ => false,
                }
            }
            Terminal::Any(v) => v.iter().any(|t| t.matches(tok)),
            Terminal::Diff(a, b) => a.matches(tok) && !b.matches(tok),
        }
    }
}

// ---------------------------------------------------------------- 字句解析

#[derive(Clone, Debug, PartialEq)]
enum Tk {
    Name(String),
    Define,
    Bar,
    Quest,
    Star,
    Plus,
    LParen,
    RParen,
    Minus,
    Str(String),
    Class(CharClass),
    /// `[ ns:`（true）または `[ nf:`（false）
    AnnOpen(bool),
    RBracket,
}

fn read_hex(chars: &[char], i: &mut usize, line: usize) -> Result<u32, String> {
    // chars[*i] は '#'
    if chars.get(*i + 1) != Some(&'x') {
        return Err(format!("{line} 行: `#x` の形式が必要です"));
    }
    let mut j = *i + 2;
    let mut v: u32 = 0;
    let start = j;
    while let Some(d) = chars.get(j).and_then(|c| c.to_digit(16)) {
        v = v
            .checked_mul(16)
            .and_then(|v| v.checked_add(d))
            .ok_or_else(|| format!("{line} 行: `#x` の値が大きすぎます"))?;
        j += 1;
    }
    if j == start {
        return Err(format!("{line} 行: `#x` のあとに 16 進数が必要です"));
    }
    if char::from_u32(v).is_none() {
        return Err(format!("{line} 行: `#x{v:X}` は Unicode のスカラ値ではありません"));
    }
    *i = j;
    Ok(v)
}

fn read_class_atom(chars: &[char], i: &mut usize, line: usize) -> Result<u32, String> {
    match chars.get(*i) {
        None => Err(format!("{line} 行: 文字クラスが閉じていません")),
        Some('\\') => {
            let c = *chars
                .get(*i + 1)
                .ok_or_else(|| format!("{line} 行: 文字クラスが閉じていません"))?;
            *i += 2;
            Ok(match c {
                'n' => '\n' as u32,
                't' => '\t' as u32,
                'r' => '\r' as u32,
                c => c as u32,
            })
        }
        Some('#') => read_hex(chars, i, line),
        Some(&c) => {
            *i += 1;
            Ok(c as u32)
        }
    }
}

/// `[` の次の位置から読み、`]` の次へ進める。
fn read_class(chars: &[char], i: &mut usize, line: usize) -> Result<CharClass, String> {
    let negated = chars.get(*i) == Some(&'^');
    if negated {
        *i += 1;
    }
    let mut ranges = Vec::new();
    loop {
        match chars.get(*i) {
            None => return Err(format!("{line} 行: 文字クラスが閉じていません")),
            Some(']') => {
                *i += 1;
                break;
            }
            _ => {}
        }
        let a = read_class_atom(chars, i, line)?;
        if chars.get(*i) == Some(&'-') && chars.get(*i + 1).is_some_and(|c| *c != ']') {
            *i += 1;
            let b = read_class_atom(chars, i, line)?;
            if a > b {
                return Err(format!("{line} 行: 文字クラスの範囲が逆順です"));
            }
            ranges.push((a, b));
        } else {
            ranges.push((a, a));
        }
    }
    if ranges.is_empty() {
        return Err(format!("{line} 行: 空の文字クラスです"));
    }
    Ok(CharClass { ranges, negated })
}

fn lex(src: &str) -> Result<Vec<(Tk, usize)>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            c if c.is_whitespace() => i += 1,
            '/' if chars.get(i + 1) == Some(&'*') => {
                let start_line = line;
                i += 2;
                loop {
                    match chars.get(i) {
                        None => return Err(format!("{start_line} 行: コメントが閉じていません")),
                        Some('*') if chars.get(i + 1) == Some(&'/') => {
                            i += 2;
                            break;
                        }
                        Some('\n') => {
                            line += 1;
                            i += 1;
                        }
                        _ => i += 1,
                    }
                }
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while chars
                    .get(i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
                {
                    i += 1;
                }
                out.push((Tk::Name(chars[start..i].iter().collect()), line));
            }
            ':' => {
                if chars.get(i + 1) == Some(&':') && chars.get(i + 2) == Some(&'=') {
                    out.push((Tk::Define, line));
                    i += 3;
                } else {
                    return Err(format!("{line} 行: `::=` が必要です"));
                }
            }
            '|' => {
                out.push((Tk::Bar, line));
                i += 1;
            }
            '?' => {
                out.push((Tk::Quest, line));
                i += 1;
            }
            '*' => {
                out.push((Tk::Star, line));
                i += 1;
            }
            '+' => {
                out.push((Tk::Plus, line));
                i += 1;
            }
            '(' => {
                out.push((Tk::LParen, line));
                i += 1;
            }
            ')' => {
                out.push((Tk::RParen, line));
                i += 1;
            }
            '-' => {
                out.push((Tk::Minus, line));
                i += 1;
            }
            ']' => {
                out.push((Tk::RBracket, line));
                i += 1;
            }
            '\'' | '"' => {
                let q = c;
                let mut j = i + 1;
                let start = j;
                while chars.get(j).is_some_and(|c| *c != q) {
                    if chars[j] == '\n' {
                        return Err(format!("{line} 行: 文字列リテラルが閉じていません"));
                    }
                    j += 1;
                }
                if j >= chars.len() {
                    return Err(format!("{line} 行: 文字列リテラルが閉じていません"));
                }
                out.push((Tk::Str(chars[start..j].iter().collect()), line));
                i = j + 1;
            }
            '#' => {
                let v = read_hex(&chars, &mut i, line)?;
                out.push((
                    Tk::Class(CharClass {
                        ranges: vec![(v, v)],
                        negated: false,
                    }),
                    line,
                ));
            }
            '[' => {
                // `[ ns: ` / `[ nf: ` は注釈、それ以外は文字クラス。
                let mut j = i + 1;
                while chars.get(j).is_some_and(|c| *c == ' ' || *c == '\t') {
                    j += 1;
                }
                let kw: String = chars[j..chars.len().min(j + 2)].iter().collect();
                let mut k = j + 2;
                while chars.get(k).is_some_and(|c| *c == ' ' || *c == '\t') {
                    k += 1;
                }
                if (kw == "ns" || kw == "nf") && chars.get(k) == Some(&':') {
                    out.push((Tk::AnnOpen(kw == "ns"), line));
                    i = k + 1;
                } else {
                    i += 1;
                    let cls = read_class(&chars, &mut i, line)?;
                    out.push((Tk::Class(cls), line));
                }
            }
            c => return Err(format!("{line} 行: 想定外の文字 `{c}` です")),
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- 構文解析

struct Parser {
    toks: Vec<(Tk, usize)>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tk> {
        self.toks.get(self.pos).map(|t| &t.0)
    }

    fn line(&self) -> usize {
        self.toks
            .get(self.pos)
            .or(self.toks.last())
            .map_or(1, |t| t.1)
    }

    fn at_rule_start(&self) -> bool {
        matches!(self.toks.get(self.pos), Some((Tk::Name(_), _)))
            && matches!(self.toks.get(self.pos + 1), Some((Tk::Define, _)))
    }

    fn starts_item(&self) -> bool {
        match self.peek() {
            Some(Tk::Name(_)) => !self.at_rule_start(),
            Some(Tk::Str(_) | Tk::Class(_) | Tk::LParen | Tk::AnnOpen(_)) => true,
            _ => false,
        }
    }

    fn parse_grammar(&mut self) -> Result<Vec<Rule>, String> {
        let mut rules = Vec::new();
        while self.pos < self.toks.len() {
            if !self.at_rule_start() {
                return Err(format!("{} 行: `名前 ::=` が必要です", self.line()));
            }
            let line = self.line();
            let name = match &self.toks[self.pos].0 {
                Tk::Name(n) => n.clone(),
                _ => unreachable!(),
            };
            self.pos += 2;
            let body = self.parse_alt()?;
            if self.pos < self.toks.len() && !self.at_rule_start() {
                return Err(format!(
                    "{} 行: 規則 {name} の本体に想定外のトークンがあります",
                    self.line()
                ));
            }
            rules.push(Rule { name, body, line });
        }
        Ok(rules)
    }

    fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_NEST {
            return Err(format!(
                "{} 行: 入れ子が深すぎます（上限 {MAX_NEST}）",
                self.line()
            ));
        }
        Ok(())
    }

    fn parse_alt(&mut self) -> Result<Expr, String> {
        self.enter()?;
        let mut alts = vec![self.parse_seq()?];
        while self.peek() == Some(&Tk::Bar) {
            self.pos += 1;
            alts.push(self.parse_seq()?);
        }
        self.depth -= 1;
        Ok(if alts.len() == 1 {
            alts.pop().unwrap()
        } else {
            Expr::Alt(alts)
        })
    }

    fn parse_seq(&mut self) -> Result<Expr, String> {
        let mut items = Vec::new();
        while self.starts_item() {
            items.push(self.parse_item()?);
        }
        Ok(match items.len() {
            0 => Expr::Empty,
            1 => items.pop().unwrap(),
            _ => Expr::Seq(items),
        })
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let line = self.line();
        let tok = self
            .toks
            .get(self.pos)
            .map(|t| t.0.clone())
            .ok_or_else(|| format!("{line} 行: 式が途中で終わっています"))?;
        self.pos += 1;
        match tok {
            Tk::Name(n) => Ok(Expr::Ref(n)),
            Tk::Str(s) => Ok(Expr::Lit(s)),
            Tk::Class(c) => Ok(Expr::Class(c)),
            Tk::LParen => {
                let e = self.parse_alt()?;
                if self.peek() != Some(&Tk::RParen) {
                    return Err(format!("{} 行: `)` が必要です", self.line()));
                }
                self.pos += 1;
                Ok(e)
            }
            Tk::AnnOpen(ns) => {
                self.enter()?;
                let operand = self.parse_primary()?;
                self.depth -= 1;
                if self.peek() != Some(&Tk::RBracket) {
                    return Err(format!("{} 行: 注釈の `]` が必要です", self.line()));
                }
                self.pos += 1;
                Ok(if ns {
                    Expr::NotStart(Box::new(operand))
                } else {
                    Expr::NotFollow(Box::new(operand))
                })
            }
            _ => Err(format!("{line} 行: 式が必要です")),
        }
    }

    fn parse_item(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_primary()?;
        if self.peek() == Some(&Tk::Minus) {
            self.pos += 1;
            let rhs = self.parse_primary()?;
            e = Expr::Diff(Box::new(e), Box::new(rhs));
        }
        loop {
            e = match self.peek() {
                Some(Tk::Quest) => Expr::Opt(Box::new(e)),
                Some(Tk::Star) => Expr::Star(Box::new(e)),
                Some(Tk::Plus) => Expr::Plus(Box::new(e)),
                _ => break,
            };
            self.pos += 1;
        }
        Ok(e)
    }
}

// ---------------------------------------------------------------- 文法

#[derive(Debug)]
pub struct Grammar {
    pub rules: Vec<Rule>,
    index: HashMap<String, usize>,
}

impl Grammar {
    /// EBNF を読み、層・未定義・到達性を検査する。エラーはすべて集めて返す。
    /// `roots` は入口にする規則（字句規則でも構文規則でもよい）。
    pub fn parse(src: &str, roots: &[&str]) -> Result<Grammar, Vec<String>> {
        let toks = lex(src).map_err(|e| vec![e])?;
        let mut p = Parser {
            toks,
            pos: 0,
            depth: 0,
        };
        let rules = p.parse_grammar().map_err(|e| vec![e])?;
        let mut errors = Vec::new();
        let mut index = HashMap::new();
        for (i, r) in rules.iter().enumerate() {
            if index.insert(r.name.clone(), i).is_some() {
                errors.push(format!("{} 行: 規則 {} が二重に定義されています", r.line, r.name));
            }
        }
        let g = Grammar { rules, index };
        g.validate(roots, &mut errors);
        if errors.is_empty() {
            Ok(g)
        } else {
            Err(errors)
        }
    }

    pub fn rule(&self, name: &str) -> Option<&Rule> {
        self.index.get(name).map(|&i| &self.rules[i])
    }

    fn is_terminal_like(&self, e: &Expr) -> bool {
        match e {
            Expr::Lit(_) | Expr::Class(_) => true,
            Expr::Ref(n) => layer_of(n) == Some(Layer::Lexical),
            _ => false,
        }
    }

    fn walk(&self, e: &Expr, rule: &Rule, layer: Layer, refs: &mut Vec<String>, errors: &mut Vec<String>) {
        let at = |msg: String| format!("{} 行: 規則 {}: {msg}", rule.line, rule.name);
        match e {
            Expr::Empty | Expr::Lit(_) => {}
            Expr::Class(_) => {
                if layer == Layer::Syntax {
                    errors.push(at("構文規則の本体に文字クラスや #x は書けません（字句規則へ移してください）".into()));
                }
            }
            Expr::Ref(n) => match (layer_of(n), self.index.contains_key(n)) {
                (_, false) => errors.push(at(format!("未定義の記号 {n} です"))),
                (None, true) => {}
                (Some(l), true) => {
                    if layer == Layer::Lexical && l == Layer::Syntax {
                        errors.push(at(format!("字句規則から構文規則 {n} は参照できません")));
                    }
                    refs.push(n.clone());
                }
            },
            Expr::Seq(v) | Expr::Alt(v) => {
                for x in v {
                    self.walk(x, rule, layer, refs, errors);
                }
            }
            Expr::Opt(x) | Expr::Star(x) | Expr::Plus(x) => self.walk(x, rule, layer, refs, errors),
            Expr::Diff(a, b) => {
                if layer == Layer::Syntax {
                    errors.push(at("構文規則の本体に差 `A - B` は書けません".into()));
                }
                for x in [a, b] {
                    if !self.is_terminal_like(x) {
                        errors.push(at("差の項は終端（リテラル、文字クラス、字句規則）に限ります".into()));
                    }
                    self.walk(x, rule, layer, refs, errors);
                }
            }
            Expr::NotStart(x) | Expr::NotFollow(x) => {
                if !self.is_terminal_like(x) {
                    errors.push(at("注釈の項は終端（リテラル、文字クラス、字句規則）に限ります".into()));
                }
                self.walk(x, rule, layer, refs, errors);
            }
        }
    }

    fn validate(&self, roots: &[&str], errors: &mut Vec<String>) {
        let mut edges: HashMap<&str, Vec<String>> = HashMap::new();
        for r in &self.rules {
            let layer = match layer_of(&r.name) {
                Some(l) => l,
                None => {
                    errors.push(format!(
                        "{} 行: 規則名 {} は UPPER_CASE（字句）か CamelCase（構文）で書いてください",
                        r.line, r.name
                    ));
                    continue;
                }
            };
            let mut refs = Vec::new();
            self.walk(&r.body, r, layer, &mut refs, errors);
            edges.insert(&r.name, refs);
        }
        let mut seen: HashSet<String> = HashSet::new();
        let mut stack: Vec<String> = Vec::new();
        for root in roots {
            if self.index.contains_key(*root) {
                stack.push((*root).to_string());
            } else {
                errors.push(format!("入口の規則 {root} が定義されていません"));
            }
        }
        while let Some(n) = stack.pop() {
            if !seen.insert(n.clone()) {
                continue;
            }
            if let Some(next) = edges.get(n.as_str()) {
                stack.extend(next.iter().cloned());
            }
        }
        for r in &self.rules {
            if !seen.contains(&r.name) && layer_of(&r.name).is_some() {
                errors.push(format!("{} 行: 規則 {} は入口から到達しません", r.line, r.name));
            }
        }
    }

    /// `start` から BNF を作る。`start` が字句規則なら文字単位（文字列リテラルは 1 文字ずつの終端）、
    /// 構文規則ならトークン単位（字句規則への参照は種別を比べる終端、文字列リテラルは 1 トークン）。
    pub fn compile(&self, start: &str) -> Result<Bnf, Vec<String>> {
        let layer = match (self.index.contains_key(start), layer_of(start)) {
            (true, Some(l)) => l,
            _ => return Err(vec![format!("入口の規則 {start} が定義されていません")]),
        };
        let mut c = Compiler {
            g: self,
            char_mode: layer == Layer::Lexical,
            bnf: Bnf {
                nt_names: Vec::new(),
                terms: Vec::new(),
                rules: Vec::new(),
                by_lhs: Vec::new(),
                start: 0,
            },
            memo: HashMap::new(),
            work: Vec::new(),
            aux: 0,
            errors: Vec::new(),
        };
        c.bnf.start = c.nt_of(start);
        while let Some((name, id)) = c.work.pop() {
            let body = c.g.rule(&name).expect("検査済み").body.clone();
            match body {
                Expr::Alt(v) => {
                    for a in &v {
                        let mut rhs = Vec::new();
                        c.lower(a, &mut rhs);
                        c.add_rule(id, rhs);
                    }
                }
                other => {
                    let mut rhs = Vec::new();
                    c.lower(&other, &mut rhs);
                    c.add_rule(id, rhs);
                }
            }
        }
        if !c.errors.is_empty() {
            return Err(c.errors);
        }
        let n = c.bnf.nt_names.len();
        c.bnf.by_lhs = vec![Vec::new(); n];
        for (i, r) in c.bnf.rules.iter().enumerate() {
            c.bnf.by_lhs[r.lhs as usize].push(i as u32);
        }
        Ok(c.bnf)
    }
}

// ---------------------------------------------------------------- BNF

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sym {
    Nt(u32),
    T(u32),
    /// 次のトークンが終端に当てはまらないこと（長さ 0）。
    Ns(u32),
    Nf(u32),
}

#[derive(Clone, Debug)]
pub struct BnfRule {
    pub lhs: u32,
    pub rhs: Vec<Sym>,
}

#[derive(Debug)]
pub struct Bnf {
    pub nt_names: Vec<String>,
    pub terms: Vec<Terminal>,
    pub rules: Vec<BnfRule>,
    pub by_lhs: Vec<Vec<u32>>,
    pub start: u32,
}

struct Compiler<'g> {
    g: &'g Grammar,
    char_mode: bool,
    bnf: Bnf,
    memo: HashMap<String, u32>,
    work: Vec<(String, u32)>,
    aux: usize,
    errors: Vec<String>,
}

impl Compiler<'_> {
    fn new_nt(&mut self, name: String) -> u32 {
        self.bnf.nt_names.push(name);
        (self.bnf.nt_names.len() - 1) as u32
    }

    fn nt_of(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.memo.get(name) {
            return id;
        }
        let id = self.new_nt(name.to_string());
        self.memo.insert(name.to_string(), id);
        self.work.push((name.to_string(), id));
        id
    }

    fn aux_nt(&mut self) -> u32 {
        self.aux += 1;
        self.new_nt(format!("_aux{}", self.aux))
    }

    fn term(&mut self, t: Terminal) -> u32 {
        if let Some(i) = self.bnf.terms.iter().position(|x| *x == t) {
            return i as u32;
        }
        self.bnf.terms.push(t);
        (self.bnf.terms.len() - 1) as u32
    }

    fn add_rule(&mut self, lhs: u32, rhs: Vec<Sym>) {
        self.bnf.rules.push(BnfRule { lhs, rhs });
    }

    /// 1 トークン（文字モードでは 1 文字）に当てはまる述語へ変える。
    fn to_terminal(&mut self, e: &Expr, depth: usize) -> Option<Terminal> {
        if depth > 50 {
            self.errors.push("終端の定義が深すぎるか循環しています".into());
            return None;
        }
        match e {
            Expr::Lit(s) => {
                if self.char_mode && s.chars().count() != 1 {
                    self.errors
                        .push(format!("文字単位では差・注釈の項に使うリテラルは 1 文字に限ります: '{s}'"));
                    None
                } else {
                    Some(Terminal::Text(s.clone()))
                }
            }
            Expr::Class(c) => Some(Terminal::Class(c.clone())),
            Expr::Ref(n) => {
                if self.char_mode {
                    let body = self.g.rule(n)?.body.clone();
                    match self.to_terminal(&body, depth + 1) {
                        Some(t) => Some(t),
                        None => {
                            self.errors
                                .push(format!("字句規則 {n} は 1 文字の終端として使えません"));
                            None
                        }
                    }
                } else {
                    Some(Terminal::Kind(n.clone()))
                }
            }
            Expr::Alt(v) => {
                let mut ts = Vec::new();
                for x in v {
                    ts.push(self.to_terminal(x, depth + 1)?);
                }
                Some(Terminal::Any(ts))
            }
            Expr::Diff(a, b) => {
                let a = self.to_terminal(a, depth + 1)?;
                let b = self.to_terminal(b, depth + 1)?;
                Some(Terminal::Diff(Box::new(a), Box::new(b)))
            }
            _ => None,
        }
    }

    fn lower_vec(&mut self, e: &Expr) -> Vec<Sym> {
        let mut v = Vec::new();
        self.lower(e, &mut v);
        v
    }

    fn lower(&mut self, e: &Expr, out: &mut Vec<Sym>) {
        match e {
            Expr::Empty => {}
            Expr::Lit(s) => {
                if self.char_mode {
                    for ch in s.chars() {
                        let t = self.term(Terminal::Text(ch.to_string()));
                        out.push(Sym::T(t));
                    }
                } else {
                    let t = self.term(Terminal::Text(s.clone()));
                    out.push(Sym::T(t));
                }
            }
            Expr::Class(c) => {
                let t = self.term(Terminal::Class(c.clone()));
                out.push(Sym::T(t));
            }
            Expr::Diff(..) => {
                if let Some(t) = self.to_terminal(e, 0) {
                    let t = self.term(t);
                    out.push(Sym::T(t));
                }
            }
            Expr::Ref(n) => {
                if !self.char_mode && layer_of(n) == Some(Layer::Lexical) {
                    let t = self.term(Terminal::Kind(n.clone()));
                    out.push(Sym::T(t));
                } else {
                    let id = self.nt_of(n);
                    out.push(Sym::Nt(id));
                }
            }
            Expr::Seq(v) => {
                for x in v {
                    self.lower(x, out);
                }
            }
            Expr::Alt(v) => {
                let aux = self.aux_nt();
                for a in v {
                    let rhs = self.lower_vec(a);
                    self.add_rule(aux, rhs);
                }
                out.push(Sym::Nt(aux));
            }
            Expr::Opt(x) => {
                let aux = self.aux_nt();
                let rhs = self.lower_vec(x);
                self.add_rule(aux, rhs);
                self.add_rule(aux, Vec::new());
                out.push(Sym::Nt(aux));
            }
            // 繰り返しは左再帰に展開する（右再帰は Leo の最適化が無いと 2 乗になる）。
            Expr::Star(x) | Expr::Plus(x) => {
                let aux = self.aux_nt();
                let body = self.lower_vec(x);
                if matches!(e, Expr::Star(_)) {
                    self.add_rule(aux, Vec::new());
                } else {
                    self.add_rule(aux, body.clone());
                }
                let mut rec = vec![Sym::Nt(aux)];
                rec.extend(body);
                self.add_rule(aux, rec);
                out.push(Sym::Nt(aux));
            }
            Expr::NotStart(x) | Expr::NotFollow(x) => {
                if let Some(t) = self.to_terminal(x, 0) {
                    let t = self.term(t);
                    out.push(if matches!(e, Expr::NotStart(_)) {
                        Sym::Ns(t)
                    } else {
                        Sym::Nf(t)
                    });
                }
            }
        }
    }
}
