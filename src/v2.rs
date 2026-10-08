use rewind::{Error, FileFailure, MapKey, ResourceBudget, Result, Runtime, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
mod effects;
mod namespacing;
mod packages;
mod pattern_space;
mod project;
mod v05;
mod v06;
mod v07;
mod v08;
mod v09;
mod v091;
mod v092;
mod v100;
mod v11;
mod v15;
fn program_v09(p: &Program) -> bool {
    matches!(
        p.language.as_str(),
        "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
            | "1.1.0"
            | "1.2.0"
            | "1.3.0"
            | "1.4.0"
            | "1.5.0"
            | "1.6.0"
            | "1.6.1"
            | "1.7.0"
            | "1.7.1"
            | "1.8.0"
            | "1.8.1"
            | "1.8.2"
            | "1.8.3"
            | "1.8.4"
            | "1.8.5"
            | "1.8.6"
            | "1.8.7"
            | "1.8.8"
            | "1.9.0"
            | "1.9.1"
            | "1.9.2"
            | "1.9.3"
            | "1.9.4"
            | "1.9.5"
            | "1.9.6"
            | "1.9.7"
            | "1.9.8"
            | "1.9.9"
            | "1.9.10"
            | "1.9.11"
            | "1.9.12"
            | "1.9.13"
            | "1.9.14"
            | "1.9.15"
            | "1.9.16"
            | "1.9.17"
            | "1.9.18"
            | "1.9.19"
            | "1.9.20"
            | "1.9.21"
            | "1.9.22"
            | "1.9.23"
            | "1.9.24"
            | "1.9.25"
            | "1.9.26"
            | "1.9.27"
            | "1.9.28"
            | "1.9.29"
            | "1.9.30"
            | "1.9.31"
            | "1.9.32"
            | "1.9.33"
            | "1.9.34"
            | "1.9.35"
            | "1.9.36"
            | "1.9.37"
            | "1.9.38"
            | "1.9.39"
            | "1.9.40"
            | "1.9.41"
            | "1.9.42"
            | "1.9.43"
            | "1.9.44"
            | "1.9.45"
            | "1.9.46"
            | "1.9.47"
            | "1.9.48"
            | "1.9.49"
            | "1.9.50"
            | "1.9.51"
            | "2.0.0"
    )
}
mod vm;
fn program_v07(p: &Program) -> bool {
    matches!(
        p.language.as_str(),
        "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
            | "1.1.0"
            | "1.2.0"
            | "1.3.0"
            | "1.4.0"
            | "1.5.0"
            | "1.6.0"
            | "1.6.1"
            | "1.7.0"
            | "1.7.1"
            | "1.8.0"
            | "1.8.1"
            | "1.8.2"
            | "1.8.3"
            | "1.8.4"
            | "1.8.5"
            | "1.8.6"
            | "1.8.7"
            | "1.8.8"
            | "1.9.0"
            | "1.9.1"
            | "1.9.2"
            | "1.9.3"
            | "1.9.4"
            | "1.9.5"
            | "1.9.6"
            | "1.9.7"
            | "1.9.8"
            | "1.9.9"
            | "1.9.10"
            | "1.9.11"
            | "1.9.12"
            | "1.9.13"
            | "1.9.14"
            | "1.9.15"
            | "1.9.16"
            | "1.9.17"
            | "1.9.18"
            | "1.9.19"
            | "1.9.20"
            | "1.9.21"
            | "1.9.22"
            | "1.9.23"
            | "1.9.24"
            | "1.9.25"
            | "1.9.26"
            | "1.9.27"
            | "1.9.28"
            | "1.9.29"
            | "1.9.30"
            | "1.9.31"
            | "1.9.32"
            | "1.9.33"
            | "1.9.34"
            | "1.9.35"
            | "1.9.36"
            | "1.9.37"
            | "1.9.38"
            | "1.9.39"
            | "1.9.40"
            | "1.9.41"
            | "1.9.42"
            | "1.9.43"
            | "1.9.44"
            | "1.9.45"
            | "1.9.46"
            | "1.9.47"
            | "1.9.48"
            | "1.9.49"
            | "1.9.50"
            | "1.9.51"
            | "2.0.0"
    )
}
thread_local! {
    // Parsed modules are reused within a compiler session by content identity.
    // Persistent entries are authenticated outside the project before reuse.
    static MODULE_CACHE: std::cell::RefCell<BTreeMap<String,Program>> = const { std::cell::RefCell::new(BTreeMap::new()) };
}

#[derive(Clone, Default)]
pub struct RunOptions {
    pub record_compact: Option<bool>,
    pub history_budget: Option<ResourceBudget>,
    pub standalone: bool,
    pub gui_events: Option<PathBuf>,
    pub gui_window_events: Option<PathBuf>,
    pub native_work: Option<usize>,
    pub execution_steps: Option<usize>,
    pub recorded_test: Option<String>,
    pub arguments: Vec<String>,
    pub allowed_env: BTreeSet<String>,
    pub secret_env: BTreeSet<String>,
    pub secret_input: Option<PathBuf>,
    pub locale: Option<String>,
    pub test_filter: Option<String>,
    pub trace_json: bool,
    pub output: Option<PathBuf>,
    pub record: Option<PathBuf>,
    pub replay: Option<serde_json::Value>,
    pub inspect: bool,
    pub profile: bool,
    pub explore: usize,
    pub choices: Vec<usize>,
    pub virtual_publish: bool,
    pub task_steps: Option<usize>,
    pub artifact: Option<serde_json::Value>,
    pub artifact_path: Option<String>,
    pub allowed_effects: BTreeSet<String>,
    pub verify_key: Option<String>,
    pub pause_after: Option<usize>,
}

pub fn standalone_effects() -> BTreeSet<String> {
    ["input", "output", "args", "locale", "random", "tasks"]
        .into_iter()
        .map(str::to_string)
        .collect()
}
fn language_at_least(actual: &str, minimum: &str) -> bool {
    semver::Version::parse(actual)
        .is_ok_and(|v| v >= semver::Version::parse(minimum).expect("valid minimum language"))
}

fn standalone_config(
    root: &Path,
    entry: PathBuf,
    extra: &BTreeSet<String>,
) -> Result<project::ProjectConfig> {
    let mut effects = standalone_effects();
    effects.extend(extra.iter().cloned());
    let mut config = project::ProjectConfig::artifact(root, entry, effects);
    config.language = env!("CARGO_PKG_VERSION").into();
    let base = root.join(".rewind");
    if fs::symlink_metadata(&base).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::InvalidPath(base.display().to_string()));
    }
    fs::create_dir_all(&base)?;
    let std_root = base.join(format!("std-{}", env!("CARGO_PKG_VERSION")));
    if fs::symlink_metadata(&std_root).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::InvalidPath(std_root.display().to_string()));
    }
    fs::create_dir_all(&std_root)?;
    for (name, source) in v092::std_modules() {
        if name == "tests" {
            continue;
        }
        let path = std_root.join(format!("{name}.rw"));
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::InvalidPath(path.display().to_string()));
        }
        // Embedded compiler sources are the trust anchor; verify/repair every cache entry.
        if fs::read(&path).ok().as_deref() != Some(source.as_bytes()) {
            fs::write(path, source)?;
        }
    }
    let metadata = std_root.join("rewind.package.json");
    if fs::symlink_metadata(&metadata).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::InvalidPath(metadata.display().to_string()));
    }
    fs::write(metadata, serde_json::to_vec(&serde_json::json!({"name":"std", "version":env!("CARGO_PKG_VERSION"), "effects":["gui","external","live","clock","network","db","tasks","random","fileRead","fileWrite"], "dependencies":{}})).unwrap())?;
    config.imports.insert("std".into(), std_root);
    Ok(config)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Tok {
    source: String,
    text: String,
    line: usize,
    col: usize,
}

fn diagnostic(t: &Tok, message: impl AsRef<str>) -> Error {
    Error::Diagnostic(Box::new(rewind::DiagnosticRecord {
        code: v06::diagnostics::code(message.as_ref()).into(),
        message: message.as_ref().into(),
        source: t.source.clone(),
        line: t.line,
        column: t.col,
        task_id: None,
        frames: vec![],
        hints: vec![],
        frames_truncated: false,
        causes: vec![],
        wait_edges: vec![],
    }))
}

fn integer_magnitude(text: &str) -> Option<u64> {
    let (digits, radix) = if text.starts_with("0x") || text.starts_with("0X") {
        (&text[2..], 16)
    } else if text.starts_with("0b") || text.starts_with("0B") {
        (&text[2..], 2)
    } else if text.starts_with("0o") || text.starts_with("0O") {
        (&text[2..], 8)
    } else {
        (text, 10)
    };
    if !digits.contains('_') {
        return u64::from_str_radix(digits, radix).ok();
    }
    digits
        .bytes()
        .filter(|b| *b != b'_')
        .try_fold(0u64, |value, byte| {
            value
                .checked_mul(radix as u64)?
                .checked_add((byte as char).to_digit(radix)? as u64)
        })
}
fn integer_literal(at: &Tok, negative: bool) -> Result<i64> {
    let magnitude =
        integer_magnitude(&at.text).ok_or_else(|| diagnostic(at, "integer literal overflow"))?;
    let value = if negative {
        -(magnitude as i128)
    } else {
        magnitude as i128
    };
    i64::try_from(value).map_err(|_| diagnostic(at, "integer literal overflow"))
}

fn lex(src: &str) -> Result<Vec<Tok>> {
    let mut out = Vec::new();
    let mut it = src.chars().peekable();
    let (mut line, mut col) = (1, 1);
    while let Some(c) = it.next() {
        let (start_line, start_col) = (line, col);
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
        if c.is_whitespace() {
            continue;
        }
        if c == '/' && it.peek() == Some(&'/') {
            for n in it.by_ref() {
                if n == '\n' {
                    line += 1;
                    col = 1;
                    break;
                }
                col += 1;
            }
            continue;
        }
        if c == '/' && it.peek() == Some(&'*') {
            it.next();
            col += 1;
            let mut depth = 1usize;
            while let Some(n) = it.next() {
                if n == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                if n == '/' && it.peek() == Some(&'*') {
                    it.next();
                    col += 1;
                    depth += 1;
                } else if n == '*' && it.peek() == Some(&'/') {
                    it.next();
                    col += 1;
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            if depth != 0 {
                return Err(diagnostic(
                    &Tok {
                        source: String::new(),
                        text: "/*".into(),
                        line: start_line,
                        col: start_col,
                    },
                    "UnterminatedComment: close the block comment with */",
                ));
            }
            continue;
        }
        let mut text = c.to_string();
        if c == '"' {
            let mut closed = false;
            while let Some(n) = it.next() {
                text.push(n);
                if n == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                if n == '\\' {
                    if let Some(e) = it.next() {
                        text.push(e);
                        if e == '\n' {
                            line += 1;
                            col = 1;
                        } else {
                            col += 1;
                        }
                    } else {
                        break;
                    }
                } else if n == '"' {
                    closed = true;
                    break;
                }
            }
            if !closed {
                return Err(diagnostic(
                    &Tok {
                        source: String::new(),
                        text,
                        line: start_line,
                        col: start_col,
                    },
                    "unterminated string; add a closing quote",
                ));
            }
        } else if c.is_ascii_digit() {
            if c == '0' && matches!(it.peek(), Some('x' | 'X' | 'b' | 'B' | 'o' | 'O')) {
                let prefix = it.next().unwrap();
                text.push(prefix);
                col += 1;
                while it
                    .peek()
                    .is_some_and(|n| n.is_ascii_alphanumeric() || *n == '_')
                {
                    text.push(it.next().unwrap());
                    col += 1;
                }
                let radix = match prefix {
                    'x' | 'X' => 16,
                    'b' | 'B' => 2,
                    _ => 8,
                };
                let digits = &text[2..];
                let chars = digits.as_bytes();
                let valid = !chars.is_empty()
                    && chars.iter().enumerate().all(|(i, c)| {
                        (*c as char).is_digit(radix)
                            || (*c == b'_'
                                && i > 0
                                && i + 1 < chars.len()
                                && (chars[i - 1] as char).is_digit(radix)
                                && (chars[i + 1] as char).is_digit(radix))
                    });
                let token = Tok {
                    source: String::new(),
                    text: text.clone(),
                    line: start_line,
                    col: start_col,
                };
                if !valid {
                    return Err(diagnostic(&token,"InvalidNumericLiteral: expected base digits, with underscores only between digits"));
                }
                integer_magnitude(&text)
                    .ok_or_else(|| diagnostic(&token, "integer literal overflow"))?;
            } else {
                while it.peek().is_some_and(|n| n.is_ascii_digit() || *n == '_') {
                    text.push(it.next().unwrap());
                    col += 1;
                }
                if it.peek() == Some(&'.') {
                    let mut look = it.clone();
                    look.next();
                    if look.peek().is_some_and(|n| n.is_ascii_digit()) {
                        text.push(it.next().unwrap());
                        col += 1;
                        while it.peek().is_some_and(|n| n.is_ascii_digit() || *n == '_') {
                            text.push(it.next().unwrap());
                            col += 1;
                        }
                    }
                }
                if matches!(it.peek(), Some('e' | 'E')) {
                    text.push(it.next().unwrap());
                    col += 1;
                    if matches!(it.peek(), Some('+' | '-')) {
                        text.push(it.next().unwrap());
                        col += 1;
                    }
                    while it.peek().is_some_and(|n| n.is_ascii_digit() || *n == '_') {
                        text.push(it.next().unwrap());
                        col += 1;
                    }
                    if !text.chars().last().is_some_and(|n| n.is_ascii_digit()) {
                        return Err(diagnostic(
                            &Tok {
                                source: String::new(),
                                text,
                                line: start_line,
                                col: start_col,
                            },
                            "InvalidNumericLiteral: exponent requires digits",
                        ));
                    }
                }
                let chars = text.as_bytes();
                if chars.iter().enumerate().any(|(i, c)| {
                    *c == b'_'
                        && (i == 0
                            || i + 1 == chars.len()
                            || !chars[i - 1].is_ascii_digit()
                            || !chars[i + 1].is_ascii_digit())
                }) {
                    return Err(diagnostic(
                        &Tok {
                            source: String::new(),
                            text,
                            line: start_line,
                            col: start_col,
                        },
                        "InvalidNumericLiteral: underscores must separate digits",
                    ));
                }
            }
        } else if c.is_alphabetic() || c == '_' {
            while it.peek().is_some_and(|n| n.is_alphanumeric() || *n == '_') {
                text.push(it.next().unwrap());
                col += 1;
            }
        } else {
            if let Some(n) = it.peek().copied() {
                if matches!(
                    (c, n),
                    ('=', '=')
                        | ('!', '=')
                        | ('<', '=')
                        | ('>', '=')
                        | ('&', '&')
                        | ('|', '|')
                        | ('+', '=')
                        | ('-', '=')
                        | ('*', '=')
                        | ('/', '=')
                        | ('%', '=')
                        | ('-', '>')
                        | ('=', '>')
                        | (':', ':')
                        | ('.', '.')
                ) {
                    text.push(it.next().unwrap());
                    col += 1;
                }
            }
            if text.len() == 1 && !"=+-*/%(),;{}.!<>?:[]|&".contains(c) {
                return Err(diagnostic(
                    &Tok {
                        source: String::new(),
                        text,
                        line: start_line,
                        col: start_col,
                    },
                    "unexpected character",
                ));
            }
        }
        out.push(Tok {
            source: String::new(),
            text,
            line: start_line,
            col: start_col,
        });
    }
    out.push(Tok {
        source: String::new(),
        text: "<eof>".into(),
        line,
        col,
    });
    Ok(out)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Expr {
    kind: ExprKind,
    at: Tok,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
enum ExprKind {
    Value(Value),
    Name(String),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Member(Box<Expr>, String),
    Try(Box<Expr>),
    Match(Box<Expr>, Vec<(Pattern, Option<Expr>, Expr)>),
    Closure(Vec<(String, String)>, String, Vec<Stmt>),
    NamedConstructor(String, Vec<(String, Expr)>),
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
enum Pattern {
    Wildcard,
    Bind(String),
    Literal(Value),
    Range(i64, i64),
    Variant(String, Vec<Pattern>),
    List(Vec<Pattern>),
    Struct(String, Vec<(String, Pattern)>),
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Stmt {
    kind: StmtKind,
    at: Tok,
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum LiveExternalMode {
    Live,
}
// Preserve the boolean wire used by existing recorded/fresh AST and IR.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum ExternalMode {
    Recorded(bool),
    Live(LiveExternalMode),
}
impl ExternalMode {
    fn is_live(self) -> bool {
        matches!(self, Self::Live(_))
    }
    fn is_fresh(self) -> bool {
        matches!(self, Self::Recorded(true))
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
enum StmtKind {
    Let(String, bool, Option<String>, Expr),
    Using(String, Expr),
    Assign(Expr, String, Expr),
    Expr(Expr),
    Block(Vec<Stmt>),
    External(ExternalMode, Vec<Stmt>),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    For(String, Expr, Expr, Vec<Stmt>),
    Break,
    Continue,
    Return(Option<Expr>),
    Defer(Expr),
    Commit(String),
    Revert(String),
    Resume(String),
    Drop(String),
    Publish(bool),
    Branch(String, Vec<Stmt>),
    Runtime(ResourceBudget, Option<usize>),
    Match(Expr, Vec<(Pattern, Option<Expr>, Stmt)>),
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Function {
    asynchronous: bool,
    effects: Option<BTreeSet<String>>,
    type_params: Vec<(String, Option<String>)>,
    params: Vec<(String, String)>,
    ret: String,
    body: Vec<Stmt>,
    at: Tok,
    test: bool,
    public: bool,
    origin: PathBuf,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct StructDef {
    #[serde(default)]
    private_fields: BTreeSet<String>,
    #[serde(default)]
    bounds: BTreeMap<String, String>,
    #[serde(default)]
    immutable: bool,
    type_params: Vec<String>,
    fields: Vec<(String, String)>,
    public: bool,
    origin: PathBuf,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct TraitDef {
    #[serde(default)]
    defaults: BTreeMap<String, Function>,
    #[serde(default)]
    method_effects: BTreeMap<String, BTreeSet<String>>,
    associated: BTreeSet<String>,
    methods: BTreeMap<String, (Vec<String>, String)>,
    public: bool,
    origin: PathBuf,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct EnumDef {
    type_params: Vec<String>,
    variants: BTreeMap<String, Vec<(String, String)>>,
    public: bool,
    origin: PathBuf,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct AliasDef {
    params: Vec<String>,
    ty: String,
    public: bool,
    origin: PathBuf,
    at: Tok,
}
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Program {
    #[serde(default)]
    aliases: BTreeMap<String, AliasDef>,
    #[serde(default, with = "v05::pairs")]
    impl_generics: BTreeMap<(String, String), Vec<(String, Option<String>)>>,
    language: String,
    #[serde(default)]
    inferred_effects: BTreeMap<String, BTreeSet<String>>,
    stmts: Vec<Stmt>,
    functions: BTreeMap<String, Function>,
    structs: BTreeMap<String, StructDef>,
    traits: BTreeMap<String, TraitDef>,
    #[serde(with = "v05::pairs")]
    impls: BTreeMap<(String, String), BTreeMap<String, String>>,
    #[serde(with = "v05::pairs")]
    impl_origins: BTreeMap<(String, String), PathBuf>,
    #[serde(with = "v05::pairs")]
    impl_associated: BTreeMap<(String, String), BTreeMap<String, String>>,
    enums: BTreeMap<String, EnumDef>,
    consts: BTreeMap<String, String>,
    const_origins: BTreeMap<String, (bool, PathBuf)>,
    stmt_origins: Vec<PathBuf>,
    strict_visibility: bool,
    root_origin: PathBuf,
    import_aliases: BTreeMap<String, String>,
    #[serde(with = "v05::pairs")]
    import_exposure: BTreeMap<(PathBuf, PathBuf), BTreeSet<String>>,
    included_modules: BTreeSet<PathBuf>,
    module_effects: BTreeMap<PathBuf, BTreeSet<String>>,
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    type_depth: usize,
}
impl Parser {
    fn current(&self) -> &Tok {
        &self.toks[self.pos]
    }
    fn effect_set(&mut self) -> Result<Option<BTreeSet<String>>> {
        if !self.eat("effects") {
            return Ok(None);
        }
        if !self.is("{") {
            return Ok(Some(BTreeSet::from([self.name()?])));
        }
        self.need("{")?;
        let mut effects = BTreeSet::new();
        while !self.eat("}") {
            effects.insert(self.name()?);
            if !self.is("}") {
                self.need(",")?;
            }
        }
        Ok(Some(effects))
    }
    fn is(&self, s: &str) -> bool {
        self.current().text == s
    }
    fn eat(&mut self, s: &str) -> bool {
        if s == ">" && self.is(">=") {
            self.toks[self.pos].text = "=".into();
            self.toks[self.pos].col += 1;
            return true;
        }
        if self.is(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn need(&mut self, s: &str) -> Result<()> {
        if self.eat(s) {
            Ok(())
        } else {
            Err(diagnostic(
                self.current(),
                format!("expected '{s}'; check punctuation"),
            ))
        }
    }
    fn name(&mut self) -> Result<String> {
        let t = self.current().clone();
        if t.text == "begin" {
            return Err(diagnostic(
                &t,
                "ReservedLabel: begin is the immutable initial checkpoint",
            ));
        }
        if t.text
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
            && t.text != "<eof>"
        {
            self.pos += 1;
            Ok(t.text)
        } else {
            Err(diagnostic(&t, "expected a name"))
        }
    }
    fn ty(&mut self) -> Result<String> {
        if self.type_depth >= 64 {
            return Err(diagnostic(
                &self.toks[self.pos],
                "TypeExpansionBudgetExceeded: nesting",
            ));
        }
        self.type_depth += 1;
        let result = self.ty_inner();
        self.type_depth -= 1;
        result
    }
    fn ty_inner(&mut self) -> Result<String> {
        if self.eat("(") {
            if self.eat(")") {
                return Ok("Unit".into());
            }
            let first = self.ty()?;
            if self.eat(")") {
                return Ok(first);
            }
            self.need(",")?;
            let mut items = vec![first];
            while !self.eat(")") {
                items.push(self.ty()?);
                if !self.is(")") {
                    self.need(",")?;
                }
            }
            return Ok(format!("Tuple<{}>", items.join(",")));
        }
        if self.eat("&") {
            return Ok(format!(
                "&{}{}",
                if self.eat("mut") { "mut " } else { "" },
                self.ty()?
            ));
        }
        if self.eat("fn") {
            self.need("(")?;
            let mut args = Vec::new();
            if !self.eat(")") {
                loop {
                    args.push(self.ty()?);
                    if self.eat(")") {
                        break;
                    }
                    self.need(",")?;
                }
            }
            self.need("->")?;
            let ret = self.ty()?;
            let effects = self.effect_set()?;
            let captures = if self.eat("captures") {
                self.need("{")?;
                let mut flags = BTreeSet::new();
                while !self.eat("}") {
                    let flag = self.name()?;
                    if flag != "Send" && flag != "Share" {
                        return Err(diagnostic(
                            self.current(),
                            "capture contract must use Send or Share",
                        ));
                    }
                    flags.insert(flag);
                    if !self.is("}") {
                        self.need(",")?;
                    }
                }
                if flags.contains("Share") {
                    flags.insert("Send".into());
                }
                Some(flags)
            } else {
                None
            };
            return Ok(format!(
                "fn({})->{ret}{}{}",
                args.join(","),
                effects
                    .map(|e| format!("!{{{}}}", e.into_iter().collect::<Vec<_>>().join("+")))
                    .unwrap_or_else(|| if captures.is_some() {
                        format!("!{{{}}}", v06::KNOWN.join("+"))
                    } else {
                        String::new()
                    }),
                captures
                    .map(|c| format!("~{{{}}}", c.into_iter().collect::<Vec<_>>().join("+")))
                    .unwrap_or_default()
            ));
        }
        let mut name = self.name()?;
        while self.is(".") || self.is("::") {
            let separator = self.current().text.clone();
            self.pos += 1;
            name.push_str(&separator);
            name.push_str(&self.name()?);
        }
        if self.eat("<") {
            name.push('<');
            loop {
                name.push_str(&self.ty()?);
                if self.eat(">") {
                    name.push('>');
                    break;
                }
                self.need(",")?;
                name.push(',');
            }
        }
        Ok(name)
    }
    fn bounds_list(&mut self) -> Result<String> {
        let mut bounds = vec![self.name()?];
        while self.eat("+") {
            let bound = self.name()?;
            if bounds.contains(&bound) {
                return Err(diagnostic(self.current(), "duplicate generic bound"));
            }
            bounds.push(bound);
        }
        Ok(bounds.join("+"))
    }
    fn where_bounds(&mut self, params: &mut [(String, Option<String>)]) -> Result<()> {
        if self.eat("where") {
            loop {
                let name = self.name()?;
                self.need(":")?;
                let bound = self.bounds_list()?;
                let Some((_, current)) = params.iter_mut().find(|(p, _)| p == &name) else {
                    return Err(diagnostic(
                        self.current(),
                        "where names an unknown type parameter",
                    ));
                };
                *current = Some(match current.take() {
                    Some(previous) => format!("{previous}+{bound}"),
                    None => bound,
                });
                if !self.eat(",") {
                    break;
                }
            }
        }
        Ok(())
    }
    fn type_params(&mut self) -> Result<Vec<(String, Option<String>)>> {
        let mut params = Vec::new();
        if self.eat("<") {
            loop {
                let name = self.name()?;
                let bound = if self.eat(":") {
                    Some(self.bounds_list()?)
                } else {
                    None
                };
                if params.iter().any(|(n, _)| n == &name) {
                    return Err(diagnostic(
                        self.current(),
                        format!("duplicate type parameter {name}"),
                    ));
                }
                params.push((name, bound));
                if self.eat(">") {
                    break;
                }
                self.need(",")?;
            }
        }
        Ok(params)
    }
    fn program(&mut self) -> Result<Program> {
        let mut p = Program::default();
        while !self.is("<eof>") {
            let public = self.eat("pub");
            if self.eat("effects") {
                self.need("{")?;
                let mut declared = BTreeSet::new();
                while !self.eat("}") {
                    declared.insert(self.name()?);
                    if !self.is("}") {
                        self.need(",")?;
                    }
                }
                self.eat(";");
                if p.module_effects.insert(PathBuf::new(), declared).is_some() {
                    return Err(diagnostic(self.current(), "duplicate effects declaration"));
                }
            } else if self.eat("import") {
                if public {
                    return Err(diagnostic(self.current(), "import cannot be public"));
                }
                let mut path = self.name()?;
                let mut selected = Vec::new();
                while self.eat(".") {
                    if self.eat("{") {
                        while !self.eat("}") {
                            let item = self.name()?;
                            let alias = if self.eat("as") {
                                self.name()?
                            } else {
                                item.clone()
                            };
                            selected.push((item, alias));
                            if !self.is("}") {
                                self.need(",")?;
                            }
                        }
                        break;
                    }
                    path.push('/');
                    path.push_str(&self.name()?);
                }
                if self.eat("::") {
                    self.need("{")?;
                    while !self.eat("}") {
                        let item = self.name()?;
                        let alias = if self.eat("as") {
                            self.name()?
                        } else {
                            item.clone()
                        };
                        selected.push((item, alias));
                        if !self.is("}") {
                            self.need(",")?;
                        }
                    }
                }
                let alias = if self.eat("as") {
                    self.name()?
                } else {
                    String::new()
                };
                if !alias.is_empty() && !selected.is_empty() {
                    return Err(diagnostic(
                        self.current(),
                        "choose module alias or selective import",
                    ));
                }
                self.need(";")?;
                let at = self.toks[self.pos - 1].clone();
                p.stmts.push(Stmt {
                    kind: StmtKind::Expr(Expr {
                        kind: ExprKind::Name(format!(
                            "@import:{path}|{alias}|{}",
                            selected
                                .iter()
                                .map(|(a, b)| format!("{a}={b}"))
                                .collect::<Vec<_>>()
                                .join(",")
                        )),
                        at: at.clone(),
                    }),
                    at,
                });
            } else if self.eat("type") {
                let at = self.current().clone();
                let name = self.name()?;
                let params = self.type_params()?.into_iter().map(|(n, _)| n).collect();
                self.need("=")?;
                let ty = self.ty()?;
                self.need(";")?;
                if p.aliases
                    .insert(
                        name.clone(),
                        AliasDef {
                            params,
                            ty,
                            public,
                            origin: PathBuf::new(),
                            at,
                        },
                    )
                    .is_some()
                {
                    return Err(diagnostic(
                        self.current(),
                        format!("duplicate type alias {name}"),
                    ));
                }
            } else if self.eat("enum") {
                let name = self.name()?;
                let type_params = self.type_params()?.into_iter().map(|(n, _)| n).collect();
                self.need("{")?;
                let mut variants = BTreeMap::new();
                while !self.eat("}") {
                    let variant = self.name()?;
                    let mut fields = Vec::new();
                    if self.eat("(") {
                        if !self.eat(")") {
                            loop {
                                fields.push((fields.len().to_string(), self.ty()?));
                                if self.eat(")") {
                                    break;
                                }
                                self.need(",")?;
                            }
                        }
                    } else if self.eat("{") {
                        while !self.eat("}") {
                            let field = self.name()?;
                            self.need(":")?;
                            fields.push((field, self.ty()?));
                            if !self.is("}") {
                                self.need(",")?;
                            }
                        }
                    }
                    if variants.insert(variant.clone(), fields).is_some() {
                        return Err(diagnostic(
                            self.current(),
                            format!("duplicate variant {variant}"),
                        ));
                    }
                    if !self.is("}") {
                        self.need(",")?;
                    }
                }
                if p.enums
                    .insert(
                        name.clone(),
                        EnumDef {
                            type_params,
                            variants,
                            public,
                            origin: PathBuf::new(),
                        },
                    )
                    .is_some()
                {
                    return Err(diagnostic(self.current(), format!("duplicate enum {name}")));
                }
            } else if self.eat("trait") {
                let name = self.name()?;
                self.need("{")?;
                let mut methods = BTreeMap::new();
                let mut method_effects = BTreeMap::new();
                let mut defaults = BTreeMap::new();
                let mut associated = BTreeSet::new();
                while !self.eat("}") {
                    if self.eat("type") {
                        associated.insert(self.name()?);
                        self.need(";")?;
                        continue;
                    }
                    let at = self.current().clone();
                    self.need("fn")?;
                    let method = self.name()?;
                    self.need("(")?;
                    let mut args = Vec::new();
                    let mut params = Vec::new();
                    if !self.eat(")") {
                        loop {
                            let arg = self.name()?;
                            self.need(":")?;
                            let ty = self.ty()?;
                            args.push(ty.clone());
                            params.push((arg, ty));
                            if self.eat(")") {
                                break;
                            }
                            self.need(",")?;
                        }
                    }
                    self.need("->")?;
                    let ret = self.ty()?;
                    if let Some(effects) = self.effect_set()? {
                        method_effects.insert(method.clone(), effects);
                    }
                    if self.is("{") {
                        let body = self.block()?;
                        defaults.insert(
                            method.clone(),
                            Function {
                                asynchronous: false,
                                effects: method_effects
                                    .get(&method)
                                    .cloned()
                                    .or_else(|| Some(BTreeSet::new())),
                                type_params: Vec::new(),
                                params,
                                ret: ret.clone(),
                                body,
                                at,
                                test: false,
                                public: false,
                                origin: PathBuf::new(),
                            },
                        );
                    } else {
                        self.need(";")?;
                    }
                    if methods.insert(method.clone(), (args, ret)).is_some() {
                        return Err(diagnostic(
                            self.current(),
                            format!("duplicate trait method {method}"),
                        ));
                    }
                }
                if p.traits
                    .insert(
                        name.clone(),
                        TraitDef {
                            defaults,
                            method_effects,
                            associated,
                            methods,
                            public,
                            origin: PathBuf::new(),
                        },
                    )
                    .is_some()
                {
                    return Err(diagnostic(
                        self.current(),
                        format!("duplicate trait {name}"),
                    ));
                }
            } else if self.eat("impl") {
                if public {
                    return Err(diagnostic(self.current(), "impl cannot be public"));
                }
                let type_params = self.type_params()?;
                let tr = self.name()?;
                self.need("for")?;
                let target = self.ty()?;
                self.need("{")?;
                let mut methods = BTreeMap::new();
                let mut associated = BTreeMap::new();
                while !self.eat("}") {
                    if self.eat("type") {
                        let name = self.name()?;
                        self.need("=")?;
                        let ty = self.ty()?;
                        self.need(";")?;
                        if associated.insert(name, ty).is_some() {
                            return Err(diagnostic(self.current(), "duplicate associated type"));
                        }
                        continue;
                    }
                    let at = self.current().clone();
                    self.need("fn")?;
                    let method = self.name()?;
                    self.need("(")?;
                    let mut params = Vec::new();
                    if !self.eat(")") {
                        loop {
                            let arg = self.name()?;
                            self.need(":")?;
                            let mut ty = self.ty()?;
                            for (n, t) in &associated {
                                ty = ty.replace(&format!("Self::{n}"), t);
                            }
                            let ty = ty.replace("Self", &target);
                            params.push((arg, ty));
                            if self.eat(")") {
                                break;
                            }
                            self.need(",")?;
                        }
                    }
                    self.need("->")?;
                    let mut ret = self.ty()?;
                    for (n, t) in &associated {
                        ret = ret.replace(&format!("Self::{n}"), t);
                    }
                    let ret = ret.replace("Self", &target);
                    let effects = self.effect_set()?;
                    let body = self.block()?;
                    let symbol = format!(
                        "$impl${tr}${}${method}",
                        target
                            .replace('<', "$lt$")
                            .replace('>', "$gt$")
                            .replace(',', "$comma$")
                    );
                    if methods.insert(method, symbol.clone()).is_some() {
                        return Err(diagnostic(&at, "duplicate impl method"));
                    }
                    p.functions.insert(
                        symbol,
                        Function {
                            type_params: type_params.clone(),
                            asynchronous: false,
                            effects,
                            params,
                            ret,
                            body,
                            at,
                            test: false,
                            public: false,
                            origin: PathBuf::new(),
                        },
                    );
                }
                if p.impls
                    .insert((tr.clone(), target.clone()), methods)
                    .is_some()
                {
                    return Err(diagnostic(self.current(), "duplicate trait implementation"));
                }
                p.impl_associated
                    .insert((tr.clone(), target.clone()), associated);
                p.impl_generics
                    .insert((tr.clone(), target.clone()), type_params);
                p.impl_origins.insert((tr, target), PathBuf::new());
            } else if self.is("struct") || self.is("record") {
                let immutable = self.eat("record");
                if !immutable {
                    self.need("struct")?;
                }
                let name = self.name()?;
                let mut parameters = self.type_params()?;
                self.where_bounds(&mut parameters)?;
                let bounds = parameters
                    .iter()
                    .filter_map(|(n, b)| b.clone().map(|b| (n.clone(), b)))
                    .collect();
                let type_params = parameters.into_iter().map(|(n, _)| n).collect();
                self.need("{")?;
                let mut fields = Vec::new();
                let mut private_fields = BTreeSet::new();
                while !self.eat("}") {
                    let private = self.eat("private");
                    if !private {
                        self.eat("pub");
                    }
                    let field = self.name()?;
                    if fields.iter().any(|(n, _)| n == &field) {
                        return Err(diagnostic(self.current(), "duplicate struct field"));
                    }
                    if private {
                        private_fields.insert(field.clone());
                    }
                    self.need(":")?;
                    let ty = self.ty()?;
                    fields.push((field, ty));
                    if !self.is("}") {
                        self.need(",")?;
                    }
                }
                if p.structs
                    .insert(
                        name.clone(),
                        StructDef {
                            private_fields,
                            bounds,
                            immutable,
                            type_params,
                            fields,
                            public,
                            origin: PathBuf::new(),
                        },
                    )
                    .is_some()
                {
                    return Err(diagnostic(
                        self.current(),
                        format!("duplicate struct {name}"),
                    ));
                }
            } else if self.is("fn")
                || self.is("async")
                || (self.is("test") && self.toks.get(self.pos + 1).is_some_and(|t| t.text == "fn"))
            {
                let test = self.eat("test");
                let asynchronous = self.eat("async");
                let at = self.current().clone();
                self.need("fn")?;
                let name = self.name()?;
                let mut type_params = self.type_params()?;
                self.need("(")?;
                let mut params = Vec::new();
                if !self.eat(")") {
                    loop {
                        let arg = self.name()?;
                        self.need(":")?;
                        let ty = self.ty()?;
                        params.push((arg, ty));
                        if self.eat(")") {
                            break;
                        }
                        self.need(",")?;
                    }
                }
                let annotated = self.eat("->");
                if public && !annotated {
                    return Err(diagnostic(
                        &at,
                        "public function requires an explicit return type; add '-> Unit'",
                    ));
                }
                let ret = if annotated { self.ty()? } else { "Unit".into() };
                let effects = self.effect_set()?;
                self.where_bounds(&mut type_params)?;
                let body = self.block()?;
                if p.functions
                    .insert(
                        name.clone(),
                        Function {
                            type_params,
                            asynchronous,
                            effects,
                            params,
                            ret,
                            body,
                            at,
                            test,
                            public,
                            origin: PathBuf::new(),
                        },
                    )
                    .is_some()
                {
                    return Err(diagnostic(
                        self.current(),
                        format!("duplicate function {name}"),
                    ));
                }
            } else if self.eat("const") {
                let at = self.current().clone();
                let name = self.name()?;
                self.need(":")?;
                let ty = self.ty()?;
                self.need("=")?;
                let expr = self.expr(0)?;
                self.need(";")?;
                p.const_origins
                    .insert(name.clone(), (public, PathBuf::new()));
                if public {
                    p.consts.insert(name.clone(), ty.clone());
                }
                p.stmts.push(Stmt {
                    kind: StmtKind::Let(name, false, Some(ty), expr),
                    at,
                });
            } else {
                if public {
                    return Err(diagnostic(self.current(), "pub requires a declaration"));
                }
                let stmt = self.stmt()?;
                Self::push_stmt(&mut p.stmts, stmt);
            }
        }
        Ok(p)
    }
    fn push_stmt(body: &mut Vec<Stmt>, stmt: Stmt) {
        if stmt.at.text == "$destructure" {
            if let StmtKind::Block(stmts) = stmt.kind {
                body.extend(stmts);
            }
        } else {
            body.push(stmt);
        }
    }
    fn block(&mut self) -> Result<Vec<Stmt>> {
        if self.type_depth >= 64 {
            return Err(diagnostic(
                self.current(),
                "CompilerBudgetExceeded: syntax nesting (64)",
            ));
        }
        self.type_depth += 1;
        let result = self.block_inner();
        self.type_depth -= 1;
        result
    }
    fn block_inner(&mut self) -> Result<Vec<Stmt>> {
        self.need("{")?;
        let mut body = Vec::new();
        while !self.eat("}") {
            if self.is("<eof>") {
                return Err(diagnostic(self.current(), "unclosed block; add '}'"));
            }
            let stmt = self.stmt()?;
            Self::push_stmt(&mut body, stmt);
        }
        Ok(body)
    }
    fn pattern(&mut self) -> Result<Pattern> {
        if self.type_depth >= 64 {
            return Err(diagnostic(
                self.current(),
                "CompilerBudgetExceeded: syntax nesting (64)",
            ));
        }
        self.type_depth += 1;
        let result = self.pattern_inner();
        self.type_depth -= 1;
        result
    }
    fn pattern_inner(&mut self) -> Result<Pattern> {
        let at = self.current().clone();
        if self.eat("_") {
            return Ok(Pattern::Wildcard);
        }
        if self.eat("(") {
            let mut parts = Vec::new();
            while !self.eat(")") {
                parts.push(self.pattern()?);
                if self.eat(")") {
                    if parts.len() == 1 {
                        return Ok(parts.remove(0));
                    }
                    break;
                }
                self.need(",")?;
            }
            return Ok(Pattern::Variant("$tuple".into(), parts));
        }
        if self.eat("[") {
            let mut fields = Vec::new();
            while !self.eat("]") {
                fields.push(self.pattern()?);
                if !self.is("]") {
                    self.need(",")?;
                }
            }
            return Ok(Pattern::List(fields));
        }
        if at.text == "true" || at.text == "false" {
            self.pos += 1;
            return Ok(Pattern::Literal(Value::Bool(at.text == "true")));
        }
        if at.text.starts_with('"') {
            let expr = self.expr(0)?;
            if let ExprKind::Value(v) = expr.kind {
                return Ok(Pattern::Literal(v));
            }
            return Err(diagnostic(&at, "invalid literal pattern"));
        }
        if at.text.chars().next().is_some_and(|c| c.is_ascii_digit()) || at.text == "-" {
            let negative = self.eat("-");
            let token = self.current().clone();
            self.pos += 1;
            let first = integer_literal(&token, negative)?;
            if self.eat("..") {
                let negative = self.eat("-");
                let end = self.current().clone();
                self.pos += 1;
                return Ok(Pattern::Range(first, integer_literal(&end, negative)?));
            }
            return Ok(Pattern::Literal(Value::Int(first)));
        }
        let mut name = self.name()?;
        while self.eat(".") {
            name.push('.');
            name.push_str(&self.name()?);
        }
        if self.eat("<") {
            name.push('<');
            loop {
                name.push_str(&self.ty()?);
                if self.eat(">") {
                    name.push('>');
                    break;
                }
                self.need(",")?;
                name.push(',');
            }
        }
        while self.eat("::") {
            name.push_str("::");
            name.push_str(&self.name()?);
        }
        if self.eat("(") {
            let mut fields = Vec::new();
            while !self.eat(")") {
                fields.push(self.pattern()?);
                if !self.is(")") {
                    self.need(",")?;
                }
            }
            return Ok(Pattern::Variant(name, fields));
        }
        if self.eat("{") {
            let mut fields = Vec::new();
            while !self.eat("}") {
                let field = self.name()?;
                self.need(":")?;
                fields.push((field, self.pattern()?));
                if !self.is("}") {
                    self.need(",")?;
                }
            }
            return Ok(Pattern::Struct(name, fields));
        }
        if name.contains("::") || matches!(name.as_str(), "None") {
            Ok(Pattern::Variant(name, Vec::new()))
        } else {
            Ok(Pattern::Bind(name))
        }
    }
    fn stmt(&mut self) -> Result<Stmt> {
        if self.type_depth >= 64 {
            return Err(diagnostic(
                self.current(),
                "CompilerBudgetExceeded: syntax nesting (64)",
            ));
        }
        self.type_depth += 1;
        let result = self.stmt_inner();
        self.type_depth -= 1;
        result
    }
    fn stmt_inner(&mut self) -> Result<Stmt> {
        let at = self.current().clone();
        let kind = if self.eat("let") || self.eat("var") {
            let mutable = at.text == "var";
            if self.is("(") {
                let pattern = self.pattern()?;
                let ty = if self.eat(":") {
                    Some(self.ty()?)
                } else {
                    None
                };
                self.need("=")?;
                let init = self.expr(0)?;
                self.need(";")?;
                let temp = format!("$destructure${}${}", at.line, at.col);
                let mut body = vec![Stmt {
                    kind: StmtKind::Let(temp.clone(), false, ty, init),
                    at: at.clone(),
                }];
                v06::language::destructure(
                    &pattern,
                    Expr {
                        kind: ExprKind::Name(temp),
                        at: at.clone(),
                    },
                    mutable,
                    &at,
                    &mut body,
                )?;
                let mut marker = at;
                marker.text = "$destructure".into();
                return Ok(Stmt {
                    kind: StmtKind::Block(body),
                    at: marker,
                });
            }
            let name = self.name()?;
            let ty = if self.eat(":") {
                Some(self.ty()?)
            } else {
                None
            };
            self.need("=")?;
            let init = self.expr(0)?;
            self.need(";")?;
            StmtKind::Let(name, mutable, ty, init)
        } else if self.eat("using") {
            let name = self.name()?;
            self.need("=")?;
            let init = self.expr(0)?;
            self.need(";")?;
            StmtKind::Using(name, init)
        } else if self.eat("if") {
            let cond = self.expr(0)?;
            let yes = self.block()?;
            let no = if self.eat("else") {
                if self.is("if") {
                    vec![self.stmt()?]
                } else {
                    self.block()?
                }
            } else {
                Vec::new()
            };
            StmtKind::If(cond, yes, no)
        } else if self.eat("while") {
            let cond = self.expr(0)?;
            StmtKind::While(cond, self.block()?)
        } else if self.eat("for") {
            let name = self.name()?;
            self.need("in")?;
            let start = self.expr(1)?;
            if self.eat("..") {
                let end = self.expr(0)?;
                StmtKind::For(name, start, end, self.block()?)
            } else {
                let body = self.block()?;
                let iterator = format!("$iterator{}", self.pos);
                let next = format!("$next{}", self.pos);
                let named = |name: String| Expr {
                    kind: ExprKind::Name(name),
                    at: at.clone(),
                };
                let method = |base: Expr, name: &str| Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: ExprKind::Member(Box::new(base), name.into()),
                            at: at.clone(),
                        }),
                        Vec::new(),
                    ),
                    at: at.clone(),
                };
                StmtKind::Block(vec![
                    Stmt {
                        kind: StmtKind::Let(iterator.clone(), false, None, method(start, "iter")),
                        at: at.clone(),
                    },
                    Stmt {
                        kind: StmtKind::While(
                            Expr {
                                kind: ExprKind::Value(Value::Bool(true)),
                                at: at.clone(),
                            },
                            vec![
                                Stmt {
                                    kind: StmtKind::Let(
                                        next.clone(),
                                        false,
                                        None,
                                        method(named(iterator), "next"),
                                    ),
                                    at: at.clone(),
                                },
                                Stmt {
                                    kind: StmtKind::Match(
                                        named(next),
                                        vec![
                                            (
                                                Pattern::Variant(
                                                    "Some".into(),
                                                    vec![Pattern::Bind(name)],
                                                ),
                                                None,
                                                Stmt {
                                                    kind: StmtKind::Block(body),
                                                    at: at.clone(),
                                                },
                                            ),
                                            (
                                                Pattern::Variant("None".into(), Vec::new()),
                                                None,
                                                Stmt {
                                                    kind: StmtKind::Break,
                                                    at: at.clone(),
                                                },
                                            ),
                                        ],
                                    ),
                                    at: at.clone(),
                                },
                            ],
                        ),
                        at: at.clone(),
                    },
                ])
            }
        } else if self.eat("break") {
            self.need(";")?;
            StmtKind::Break
        } else if self.eat("continue") {
            self.need(";")?;
            StmtKind::Continue
        } else if self.eat("return") {
            let expr = if self.is(";") {
                None
            } else {
                Some(self.expr(0)?)
            };
            self.need(";")?;
            StmtKind::Return(expr)
        } else if self.eat("defer") {
            let expr = self.expr(0)?;
            self.need(";")?;
            StmtKind::Defer(expr)
        } else if self.is("external")
            && self
                .toks
                .get(self.pos + 1)
                .is_some_and(|t| matches!(t.text.as_str(), "{" | "fresh" | "live"))
        {
            self.pos += 1;
            let fresh = if self.eat("live") {
                ExternalMode::Live(LiveExternalMode::Live)
            } else {
                ExternalMode::Recorded(self.eat("fresh"))
            };
            StmtKind::External(fresh, self.block()?)
        } else if self.eat("commit") {
            let n = self.name()?;
            self.need(";")?;
            StmtKind::Commit(n)
        } else if self.eat("revert") {
            let n = if self.eat("begin") {
                "begin".into()
            } else {
                self.name()?
            };
            self.need(";")?;
            StmtKind::Revert(n)
        } else if self.eat("resume") {
            let n = if self.eat("begin") {
                "begin".into()
            } else {
                self.name()?
            };
            self.need(";")?;
            StmtKind::Resume(n)
        } else if self.eat("drop") {
            let n = self.name()?;
            self.need(";")?;
            StmtKind::Drop(n)
        } else if self.eat("publish") {
            let force = self.eat("force");
            self.need(";")?;
            StmtKind::Publish(force)
        } else if self.eat("branch") {
            let n = self.name()?;
            StmtKind::Branch(n, self.block()?)
        } else if self.eat("runtime") {
            self.need("{")?;
            let mut b = ResourceBudget::default();
            let mut steps = None;
            while !self.eat("}") {
                let name = self.name()?;
                self.need("=")?;
                let t = self.current().clone();
                self.pos += 1;
                let mut v: usize = t
                    .text
                    .parse()
                    .map_err(|_| diagnostic(&t, "expected nonnegative size"))?;
                if matches!(
                    self.current().text.as_str(),
                    "KB" | "KiB" | "MB" | "MiB" | "GB" | "GiB"
                ) {
                    let unit = self.name()?;
                    v = v
                        .checked_mul(match unit.as_str() {
                            "KB" | "KiB" => 1024,
                            "MB" | "MiB" => 1024 * 1024,
                            _ => 1024 * 1024 * 1024,
                        })
                        .ok_or_else(|| diagnostic(&t, "size overflow"))?;
                }
                self.need(";")?;
                match name.as_str() {
                    "historyMemory" => b.history_memory = v,
                    "historyStorage" => b.history_storage = v,
                    "spillThreshold" => b.spill_threshold = v,
                    "executionSteps" => steps = Some(v),
                    _ => return Err(diagnostic(&at, "unknown runtime setting")),
                }
            }
            StmtKind::Runtime(b, steps)
        } else if self.eat("match") {
            let value = self.expr(0)?;
            self.need("{")?;
            let mut arms = Vec::new();
            while !self.eat("}") {
                let pattern = self.pattern()?;
                let guard = if self.eat("if") {
                    Some(self.expr(0)?)
                } else {
                    None
                };
                self.need("=>")?;
                let arm_at = self.current().clone();
                let body = if self.eat("return") {
                    let value = if self.is(",") || self.is("}") {
                        None
                    } else {
                        Some(self.expr(0)?)
                    };
                    self.eat(";");
                    Stmt {
                        kind: StmtKind::Return(value),
                        at: arm_at,
                    }
                } else if self.is("{") {
                    Stmt {
                        kind: StmtKind::Block(self.block()?),
                        at: arm_at,
                    }
                } else {
                    Stmt {
                        kind: StmtKind::Expr(self.expr(0)?),
                        at: arm_at,
                    }
                };
                arms.push((pattern, guard, body));
                if !self.is("}") {
                    self.need(",")?;
                }
            }
            StmtKind::Match(value, arms)
        } else if self.is("{") {
            StmtKind::Block(self.block()?)
        } else {
            let lhs = self.expr(0)?;
            if matches!(
                self.current().text.as_str(),
                "=" | "+=" | "-=" | "*=" | "/=" | "%="
            ) {
                let op = self.current().text.clone();
                self.pos += 1;
                let rhs = self.expr(0)?;
                self.need(";")?;
                StmtKind::Assign(lhs, op, rhs)
            } else {
                self.need(";")?;
                StmtKind::Expr(lhs)
            }
        };
        Ok(Stmt { kind, at })
    }
    fn expr(&mut self, min_bp: u8) -> Result<Expr> {
        if self.type_depth >= 64 {
            return Err(diagnostic(
                self.current(),
                "CompilerBudgetExceeded: syntax nesting (64)",
            ));
        }
        self.type_depth += 1;
        let result = self.expr_inner(min_bp);
        self.type_depth -= 1;
        result
    }
    fn expr_inner(&mut self, min_bp: u8) -> Result<Expr> {
        let at = self.current().clone();
        self.pos += 1;
        let mut lhs = match at.text.as_str() {
            "(" => {
                if self.eat(")") {
                    Expr {
                        kind: ExprKind::Value(Value::Null),
                        at: at.clone(),
                    }
                } else {
                    let e = self.expr(0)?;
                    if self.eat(",") {
                        let mut items = vec![e];
                        while !self.eat(")") {
                            items.push(self.expr(0)?);
                            if !self.is(")") {
                                self.need(",")?;
                            }
                        }
                        Expr {
                            kind: ExprKind::Call(
                                Box::new(Expr {
                                    kind: ExprKind::Name("$tuple".into()),
                                    at: at.clone(),
                                }),
                                items,
                            ),
                            at: at.clone(),
                        }
                    } else {
                        self.need(")")?;
                        e
                    }
                }
            }
            "-" if integer_magnitude(&self.current().text) == Some(1u64 << 63) => {
                self.pos += 1;
                Expr {
                    kind: ExprKind::Value(Value::Int(i64::MIN)),
                    at: at.clone(),
                }
            }
            "&" => {
                let op = if self.eat("mut") {
                    "borrowMut"
                } else {
                    "borrow"
                };
                Expr {
                    kind: ExprKind::Unary(op.into(), Box::new(self.expr(13)?)),
                    at: at.clone(),
                }
            }
            "-" | "!" | "await" | "spawn" | "move" => Expr {
                kind: ExprKind::Unary(at.text.clone(), Box::new(self.expr(13)?)),
                at: at.clone(),
            },
            "capture" if self.is("value") || self.is("move") || self.is("borrow") => {
                let mode = self.name()?;
                if !matches!(mode.as_str(), "value" | "move" | "borrow") {
                    return Err(diagnostic(
                        &at,
                        "capture mode must be value, move, or borrow",
                    ));
                }
                let inner = self.expr(13)?;
                if !matches!(inner.kind, ExprKind::Closure(..)) {
                    return Err(diagnostic(&at, "capture requires a closure"));
                }
                Expr {
                    kind: ExprKind::Unary(format!("$capture:{mode}"), Box::new(inner)),
                    at: at.clone(),
                }
            }
            "true" => Expr {
                kind: ExprKind::Value(Value::Bool(true)),
                at: at.clone(),
            },
            "false" => Expr {
                kind: ExprKind::Value(Value::Bool(false)),
                at: at.clone(),
            },
            "None" => Expr {
                kind: ExprKind::Value(Value::Option(None)),
                at: at.clone(),
            },
            "null" => Expr {
                kind: ExprKind::Value(Value::Null),
                at: at.clone(),
            },
            "match" => {
                let value = self.expr(0)?;
                self.need("{")?;
                let mut arms = Vec::new();
                while !self.eat("}") {
                    let pattern = self.pattern()?;
                    let guard = if self.eat("if") {
                        Some(self.expr(0)?)
                    } else {
                        None
                    };
                    self.need("=>")?;
                    let result = self.expr(0)?;
                    arms.push((pattern, guard, result));
                    if !self.is("}") {
                        self.need(",")?;
                    }
                }
                Expr {
                    kind: ExprKind::Match(Box::new(value), arms),
                    at: at.clone(),
                }
            }
            "||" | "|" => {
                let mut params = Vec::new();
                if at.text == "|" && !self.eat("|") {
                    loop {
                        let name = self.name()?;
                        self.need(":")?;
                        params.push((name, self.ty()?));
                        if self.eat("|") {
                            break;
                        }
                        self.need(",")?;
                    }
                }
                self.need("->")?;
                let ret = self.ty()?;
                let body = self.block()?;
                Expr {
                    kind: ExprKind::Closure(params, ret, body),
                    at: at.clone(),
                }
            }
            _ if at.text.starts_with('"') => {
                let raw = &at.text[1..at.text.len() - 1];
                let mut s = String::new();
                let mut it = raw.chars();
                while let Some(c) = it.next() {
                    if c == '\\' {
                        match it.next() {
                            Some('n') => s.push('\n'),
                            Some('r') => s.push('\r'),
                            Some('t') => s.push('\t'),
                            Some('"') => s.push('"'),
                            Some('\\') => s.push('\\'),
                            Some('0') => s.push('\0'),
                            Some('u') => {
                                if it.next() != Some('{') {
                                    return Err(diagnostic(
                                        &at,
                                        "InvalidUnicodeEscape: expected \\u{HEX}",
                                    ));
                                }
                                let mut digits = String::new();
                                let mut closed = false;
                                for c in it.by_ref() {
                                    if c == '}' {
                                        closed = true;
                                        break;
                                    }
                                    if !c.is_ascii_hexdigit() || digits.len() >= 6 {
                                        return Err(diagnostic(&at,"InvalidUnicodeEscape: use 1 to 6 hex digits for a Unicode scalar"));
                                    }
                                    digits.push(c);
                                }
                                let scalar = u32::from_str_radix(&digits, 16)
                                    .ok()
                                    .and_then(char::from_u32);
                                if !closed || scalar.is_none() {
                                    return Err(diagnostic(
                                        &at,
                                        "InvalidUnicodeEscape: invalid Unicode scalar",
                                    ));
                                }
                                s.push(scalar.unwrap());
                            }
                            _ => return Err(diagnostic(&at, "invalid escape")),
                        }
                    } else {
                        s.push(c);
                    }
                }
                Expr {
                    kind: ExprKind::Value(Value::Text(s.into())),
                    at: at.clone(),
                }
            }
            _ if at.text.chars().next().is_some_and(|c| c.is_ascii_digit()) => {
                let v = if !(at.text.starts_with("0x")
                    || at.text.starts_with("0X")
                    || at.text.starts_with("0b")
                    || at.text.starts_with("0B")
                    || at.text.starts_with("0o")
                    || at.text.starts_with("0O"))
                    && (at.text.contains('.') || at.text.contains('e') || at.text.contains('E'))
                {
                    Value::Float(
                        at.text
                            .replace('_', "")
                            .parse::<f64>()
                            .map_err(|_| diagnostic(&at, "invalid float"))?
                            .to_bits(),
                    )
                } else {
                    Value::Int(integer_literal(&at, false)?)
                };
                Expr {
                    kind: ExprKind::Value(v),
                    at: at.clone(),
                }
            }
            _ if at
                .text
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_') =>
            {
                let mut name = at.text.clone();
                let saved = self.pos;
                if self.eat("<") {
                    name.push('<');
                    let parsed = (|| -> Result<()> {
                        loop {
                            name.push_str(&self.ty()?);
                            if self.eat(">") {
                                name.push('>');
                                break;
                            }
                            self.need(",")?;
                            name.push(',');
                        }
                        Ok(())
                    })();
                    if parsed.is_err() || !self.is("(") {
                        self.pos = saved;
                        name = at.text.clone();
                    }
                }
                while self.eat("::") {
                    name.push_str("::");
                    name.push_str(&self.name()?);
                }
                if name.contains("::") && self.eat("{") {
                    let mut fields = Vec::new();
                    while !self.eat("}") {
                        let field = self.name()?;
                        self.need(":")?;
                        fields.push((field, self.expr(0)?));
                        if !self.is("}") {
                            self.need(",")?;
                        }
                    }
                    Expr {
                        kind: ExprKind::NamedConstructor(name, fields),
                        at: at.clone(),
                    }
                } else {
                    Expr {
                        kind: ExprKind::Name(name),
                        at: at.clone(),
                    }
                }
            }
            _ => return Err(diagnostic(&at, "expected an expression")),
        };
        let mut chain_length = 0usize;
        loop {
            chain_length += 1;
            if chain_length > 128 {
                return Err(diagnostic(
                    &at,
                    "CompilerBudgetExceeded: expression chain (128)",
                ));
            }
            if self.eat("::") {
                let variant = self.name()?;
                let prefix = match &lhs.kind {
                    ExprKind::Name(name) => name.clone(),
                    ExprKind::Member(base, field) => {
                        if let ExprKind::Name(name) = &base.kind {
                            format!("{name}.{field}")
                        } else {
                            return Err(diagnostic(&at, "invalid enum path"));
                        }
                    }
                    _ => return Err(diagnostic(&at, "invalid enum path")),
                };
                let name = format!("{prefix}::{variant}");
                if self.eat("{") {
                    let mut fields = Vec::new();
                    while !self.eat("}") {
                        let field = self.name()?;
                        self.need(":")?;
                        fields.push((field, self.expr(0)?));
                        if !self.is("}") {
                            self.need(",")?;
                        }
                    }
                    lhs = Expr {
                        kind: ExprKind::NamedConstructor(name, fields),
                        at: at.clone(),
                    };
                } else {
                    lhs = Expr {
                        kind: ExprKind::Name(name),
                        at: at.clone(),
                    };
                }
                continue;
            }
            if self.eat(".") {
                let field = self.name()?;
                if let ExprKind::Name(module) = &lhs.kind {
                    let saved = self.pos;
                    if self.eat("<") {
                        let mut name = format!("{module}.{field}<");
                        let parsed = (|| -> Result<()> {
                            loop {
                                name.push_str(&self.ty()?);
                                if self.eat(">") {
                                    name.push('>');
                                    break;
                                }
                                self.need(",")?;
                                name.push(',');
                            }
                            Ok(())
                        })();
                        if parsed.is_ok() && self.is("(") {
                            lhs = Expr {
                                kind: ExprKind::Name(name),
                                at: at.clone(),
                            };
                            continue;
                        }
                        self.pos = saved;
                    }
                }
                lhs = Expr {
                    kind: ExprKind::Member(Box::new(lhs), field),
                    at: at.clone(),
                };
                continue;
            }
            if self.eat("(") {
                let mut args = Vec::new();
                if !self.eat(")") {
                    loop {
                        args.push(self.expr(0)?);
                        if self.eat(")") {
                            break;
                        }
                        self.need(",")?;
                    }
                }
                lhs = Expr {
                    kind: ExprKind::Call(Box::new(lhs), args),
                    at: at.clone(),
                };
                continue;
            }
            if self.eat("?") {
                lhs = Expr {
                    kind: ExprKind::Try(Box::new(lhs)),
                    at: at.clone(),
                };
                continue;
            }
            let op = self.current().text.clone();
            let bp = match op.as_str() {
                "||" => 1,
                "&&" => 2,
                "==" | "!=" => 3,
                "<" | "<=" | ">" | ">=" => 4,
                "+" | "-" => 5,
                "*" | "/" | "%" => 6,
                _ => break,
            };
            if bp < min_bp {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(bp + 1)?;
            lhs = Expr {
                kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
                at: at.clone(),
            };
        }
        Ok(lhs)
    }
}

fn rename_symbol(name: &str, names: &BTreeMap<String, String>) -> String {
    if let Some(replacement) = names.get(name) {
        return replacement.clone();
    }
    if name.contains('<') {
        return rename_type(name, names);
    }
    let split = name.find(['<', ':', '.']).unwrap_or(name.len());
    if let Some(replacement) = names.get(&name[..split]) {
        return format!("{replacement}{}", &name[split..]);
    }
    name.into()
}
fn rename_type(ty: &str, names: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut token = String::new();
    for c in ty.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() || c == '_' || c == '$' || c == '.' {
            token.push(c);
        } else {
            if !token.is_empty() {
                out.push_str(names.get(&token).unwrap_or(&token));
                token.clear();
            }
            out.push(c);
        }
    }
    out.pop();
    out
}
fn collect_pattern_bindings(pattern: &Pattern, out: &mut BTreeSet<String>) {
    match pattern {
        Pattern::Bind(name) => {
            out.insert(name.clone());
        }
        Pattern::Variant(_, parts) | Pattern::List(parts) => {
            for part in parts {
                collect_pattern_bindings(part, out);
            }
        }
        Pattern::Struct(_, fields) => {
            for (_, part) in fields {
                collect_pattern_bindings(part, out);
            }
        }
        _ => {}
    }
}
fn collect_local_bindings(body: &[Stmt], out: &mut BTreeSet<String>) {
    for stmt in body {
        match &stmt.kind {
            StmtKind::Let(name, _, _, _) | StmtKind::Using(name, _) => {
                out.insert(name.clone());
            }
            StmtKind::For(name, _, _, body) => {
                out.insert(name.clone());
                collect_local_bindings(body, out);
            }
            StmtKind::External(_, body)
            | StmtKind::Block(body)
            | StmtKind::While(_, body)
            | StmtKind::Branch(_, body) => collect_local_bindings(body, out),
            StmtKind::If(_, a, b) => {
                collect_local_bindings(a, out);
                collect_local_bindings(b, out);
            }
            StmtKind::Match(_, arms) => {
                for (pattern, _, stmt) in arms {
                    collect_pattern_bindings(pattern, out);
                    collect_local_bindings(std::slice::from_ref(stmt), out);
                }
            }
            _ => {}
        }
    }
}
fn rename_pattern(pattern: &mut Pattern, names: &BTreeMap<String, String>) {
    match pattern {
        Pattern::Variant(name, parts) => {
            *name = rename_symbol(name, names);
            for part in parts {
                rename_pattern(part, names);
            }
        }
        Pattern::Struct(name, fields) => {
            *name = rename_symbol(name, names);
            for (_, part) in fields {
                rename_pattern(part, names);
            }
        }
        Pattern::List(parts) => {
            for part in parts {
                rename_pattern(part, names);
            }
        }
        _ => {}
    }
}
fn rename_expr(expr: &mut Expr, names: &BTreeMap<String, String>) {
    match &mut expr.kind {
        ExprKind::Name(name) => *name = rename_symbol(name, names),
        ExprKind::Unary(_, value) | ExprKind::Try(value) | ExprKind::Member(value, _) => {
            rename_expr(value, names)
        }
        ExprKind::Binary(_, a, b) => {
            rename_expr(a, names);
            rename_expr(b, names);
        }
        ExprKind::Call(callee, args) => {
            rename_expr(callee, names);
            for arg in args {
                rename_expr(arg, names);
            }
        }
        ExprKind::Match(value, arms) => {
            rename_expr(value, names);
            for (pattern, guard, result) in arms {
                rename_pattern(pattern, names);
                if let Some(guard) = guard {
                    rename_expr(guard, names);
                }
                rename_expr(result, names);
            }
        }
        ExprKind::Closure(params, ret, body) => {
            for (_, ty) in params.iter_mut() {
                *ty = rename_type(ty, names);
            }
            *ret = rename_type(ret, names);
            let mut locals = params
                .iter()
                .map(|(n, _)| n.clone())
                .collect::<BTreeSet<_>>();
            collect_local_bindings(body, &mut locals);
            let filtered = names
                .iter()
                .filter(|(n, _)| !locals.contains(*n))
                .map(|(n, v)| (n.clone(), v.clone()))
                .collect();
            for stmt in body {
                rename_stmt(stmt, &filtered);
            }
        }
        ExprKind::NamedConstructor(name, fields) => {
            *name = rename_symbol(name, names);
            for (_, expr) in fields {
                rename_expr(expr, names);
            }
        }
        ExprKind::Value(_) => {}
    }
}
fn rename_stmt(stmt: &mut Stmt, names: &BTreeMap<String, String>) {
    match &mut stmt.kind {
        StmtKind::Let(_, _, ty, e) => {
            if let Some(ty) = ty {
                *ty = rename_type(ty, names);
            }
            rename_expr(e, names);
        }
        StmtKind::Using(_, e) | StmtKind::Expr(e) | StmtKind::Defer(e) => rename_expr(e, names),
        StmtKind::Assign(a, _, b) => {
            rename_expr(a, names);
            rename_expr(b, names);
        }
        StmtKind::External(_, body) | StmtKind::Block(body) | StmtKind::Branch(_, body) => {
            for stmt in body {
                rename_stmt(stmt, names);
            }
        }
        StmtKind::If(e, a, b) => {
            rename_expr(e, names);
            for stmt in a.iter_mut().chain(b) {
                rename_stmt(stmt, names);
            }
        }
        StmtKind::While(e, body) => {
            rename_expr(e, names);
            for stmt in body {
                rename_stmt(stmt, names);
            }
        }
        StmtKind::For(_, a, b, body) => {
            rename_expr(a, names);
            rename_expr(b, names);
            for stmt in body {
                rename_stmt(stmt, names);
            }
        }
        StmtKind::Return(Some(e)) => rename_expr(e, names),
        StmtKind::Match(e, arms) => {
            rename_expr(e, names);
            for (pattern, guard, body) in arms {
                rename_pattern(pattern, names);
                if let Some(guard) = guard {
                    rename_expr(guard, names);
                }
                rename_stmt(body, names);
            }
        }
        _ => {}
    }
}
fn namespace_symbols(program: &mut Program, module: &str) -> BTreeMap<String, String> {
    let prefix = format!("$import${}$", module.replace('/', "$"));
    let mut names = program
        .functions
        .iter()
        .filter(|(_, f)| f.origin == program.root_origin)
        .map(|(n, _)| n.clone())
        .chain(
            program
                .structs
                .iter()
                .filter(|(_, d)| d.origin == program.root_origin)
                .map(|(n, _)| n.clone()),
        )
        .chain(
            program
                .enums
                .iter()
                .filter(|(_, d)| d.origin == program.root_origin)
                .map(|(n, _)| n.clone()),
        )
        .chain(
            program
                .traits
                .iter()
                .filter(|(_, d)| d.origin == program.root_origin)
                .map(|(n, _)| n.clone()),
        )
        .chain(
            program
                .aliases
                .iter()
                .filter(|(_, d)| d.origin == program.root_origin)
                .map(|(n, _)| n.clone()),
        )
        .chain(
            program
                .const_origins
                .iter()
                .filter(|(_, (_, origin))| origin == &program.root_origin)
                .map(|(n, _)| n.clone()),
        )
        .map(|name| (name.clone(), format!("{prefix}{name}")))
        .collect::<BTreeMap<_, _>>();
    for (stmt, origin) in program.stmts.iter().zip(&program.stmt_origins) {
        if origin == &program.root_origin {
            if let StmtKind::Let(n, _, _, _) | StmtKind::Using(n, _) = &stmt.kind {
                names
                    .entry(n.clone())
                    .or_insert_with(|| format!("{prefix}{n}"));
            }
        }
    }
    // Imported aliases are lexical to this module, including selected imports.
    let scoped_aliases = program
        .import_aliases
        .keys()
        .filter(|n| !n.starts_with("$import$"))
        .cloned()
        .collect::<Vec<_>>();
    for alias in &scoped_aliases {
        let base = alias.split('.').next().unwrap();
        names
            .entry(base.to_string())
            .or_insert_with(|| format!("{prefix}{base}"));
    }
    for f in program
        .functions
        .values_mut()
        .filter(|f| f.origin == program.root_origin)
    {
        namespacing::function(f, &names);
    }
    for def in program
        .structs
        .values_mut()
        .filter(|d| d.origin == program.root_origin)
    {
        let names = names
            .iter()
            .filter(|(name, _)| !def.type_params.contains(name))
            .map(|(a, b)| (a.clone(), b.clone()))
            .collect();
        for (_, ty) in &mut def.fields {
            *ty = rename_type(ty, &names);
        }
        for bound in def.bounds.values_mut() {
            *bound = rename_type(bound, &names);
        }
    }
    for def in program
        .enums
        .values_mut()
        .filter(|d| d.origin == program.root_origin)
    {
        let names = names
            .iter()
            .filter(|(name, _)| !def.type_params.contains(name))
            .map(|(a, b)| (a.clone(), b.clone()))
            .collect();
        for fields in def.variants.values_mut() {
            for (_, ty) in fields {
                *ty = rename_type(ty, &names);
            }
        }
    }
    for def in program
        .traits
        .values_mut()
        .filter(|d| d.origin == program.root_origin)
    {
        for (args, ret) in def.methods.values_mut() {
            for ty in args {
                *ty = rename_type(ty, &names);
            }
            *ret = rename_type(ret, &names);
        }
    }
    for alias in program
        .aliases
        .values_mut()
        .filter(|d| d.origin == program.root_origin)
    {
        let names = names
            .iter()
            .filter(|(name, _)| !alias.params.contains(name))
            .map(|(a, b)| (a.clone(), b.clone()))
            .collect();
        alias.ty = rename_type(&alias.ty, &names);
    }
    for def in program
        .traits
        .values_mut()
        .filter(|d| d.origin == program.root_origin)
    {
        for f in def.defaults.values_mut() {
            namespacing::function(f, &names);
        }
    }
    let old = std::mem::take(&mut program.aliases);
    program.aliases = old
        .into_iter()
        .map(|(n, d)| (names.get(&n).cloned().unwrap_or(n), d))
        .collect();
    let old = std::mem::take(&mut program.impl_generics);
    program.impl_generics = old
        .into_iter()
        .map(|((tr, ty), params)| ((rename_type(&tr, &names), rename_type(&ty, &names)), params))
        .collect();
    let mut top_bound = BTreeSet::new();
    for (stmt, origin) in program.stmts.iter_mut().zip(&program.stmt_origins) {
        if origin == &program.root_origin {
            namespacing::top_statement(stmt, &names, &mut top_bound);
        }
    }
    let old = std::mem::take(&mut program.functions);
    program.functions = old
        .into_iter()
        .map(|(name, f)| (names.get(&name).cloned().unwrap_or(name), f))
        .collect();
    let old = std::mem::take(&mut program.structs);
    program.structs = old
        .into_iter()
        .map(|(name, d)| (names.get(&name).cloned().unwrap_or(name), d))
        .collect();
    let old = std::mem::take(&mut program.enums);
    program.enums = old
        .into_iter()
        .map(|(name, d)| (names.get(&name).cloned().unwrap_or(name), d))
        .collect();
    let old = std::mem::take(&mut program.traits);
    program.traits = old
        .into_iter()
        .map(|(name, d)| (names.get(&name).cloned().unwrap_or(name), d))
        .collect();
    let old = std::mem::take(&mut program.consts);
    program.consts = old
        .into_iter()
        .map(|(name, ty)| {
            (
                names.get(&name).cloned().unwrap_or(name),
                rename_type(&ty, &names),
            )
        })
        .collect();
    let old = std::mem::take(&mut program.const_origins);
    program.const_origins = old
        .into_iter()
        .map(|(name, info)| (names.get(&name).cloned().unwrap_or(name), info))
        .collect();
    let old = std::mem::take(&mut program.impls);
    program.impls = old
        .into_iter()
        .map(|((trait_name, target), methods)| {
            (
                (
                    rename_type(&trait_name, &names),
                    rename_type(&target, &names),
                ),
                methods,
            )
        })
        .collect();
    let old = std::mem::take(&mut program.impl_origins);
    program.impl_origins = old
        .into_iter()
        .map(|((trait_name, target), origin)| {
            (
                (
                    rename_type(&trait_name, &names),
                    rename_type(&target, &names),
                ),
                origin,
            )
        })
        .collect();
    let associated = std::mem::take(&mut program.impl_associated);
    program.impl_associated = associated
        .into_iter()
        .map(|((tr, target), types)| {
            (
                (rename_type(&tr, &names), rename_type(&target, &names)),
                types
                    .into_iter()
                    .map(|(n, t)| (n, rename_type(&t, &names)))
                    .collect(),
            )
        })
        .collect();
    for methods in program.impls.values_mut() {
        for symbol in methods.values_mut() {
            if let Some(new) = names.get(symbol) {
                *symbol = new.clone();
            }
        }
    }
    let scoped = |alias: &str| {
        if let Some((base, member)) = alias.split_once('.') {
            format!(
                "{}.{}",
                names.get(base).map(String::as_str).unwrap_or(base),
                member
            )
        } else {
            names
                .get(alias)
                .cloned()
                .unwrap_or_else(|| alias.to_string())
        }
    };
    let aliases = std::mem::take(&mut program.import_aliases);
    program.import_aliases = aliases
        .into_iter()
        .map(|(name, target)| {
            let name = if scoped_aliases.contains(&name) {
                scoped(&name)
            } else {
                name
            };
            (name, rename_symbol(&target, &names))
        })
        .collect();
    for ((origin, _), exposure) in &mut program.import_exposure {
        if *origin == program.root_origin {
            *exposure = exposure.iter().map(|name| scoped(name)).collect();
        }
    }
    names
}
fn load_program(
    path: &Path,
    root: &Path,
    imports: &BTreeMap<String, PathBuf>,
    visiting: &mut BTreeSet<PathBuf>,
    loaded: &mut BTreeMap<PathBuf, Program>,
) -> Result<Program> {
    load_program_overlay(path, root, imports, visiting, loaded, &BTreeMap::new())
}
fn load_program_overlay(
    path: &Path,
    root: &Path,
    imports: &BTreeMap<String, PathBuf>,
    visiting: &mut BTreeSet<PathBuf>,
    loaded: &mut BTreeMap<PathBuf, Program>,
    overlay: &BTreeMap<PathBuf, String>,
) -> Result<Program> {
    let full = if overlay.contains_key(path) && !path.exists() {
        fs::canonicalize(
            path.parent()
                .ok_or_else(|| Error::InvalidPath(path.display().to_string()))?,
        )?
        .join(
            path.file_name()
                .ok_or_else(|| Error::InvalidPath(path.display().to_string()))?,
        )
    } else {
        fs::canonicalize(path)?
    };
    let project_root = fs::canonicalize(root)?;
    if !full.starts_with(&project_root) {
        return Err(Error::InvalidPath(path.display().to_string()));
    }
    if visiting.contains(&full) {
        return Err(Error::InvalidOperation(format!(
            "cyclic import: {}",
            path.display()
        )));
    }
    if let Some(program) = loaded.get(&full) {
        return Ok(program.clone());
    }
    if visiting.len().saturating_add(loaded.len()) >= 256 {
        return Err(Error::InvalidOperation(
            "CompilerBudgetExceeded: module count (256)".into(),
        ));
    }
    if match overlay.get(&full) {
        Some(s) => s.len() as u64,
        None => fs::metadata(&full)?.len(),
    } > 1024 * 1024
    {
        return Err(Error::InvalidOperation(
            "CompilerBudgetExceeded: module source (1 MiB)".into(),
        ));
    }
    visiting.insert(full.clone());
    let source = overlay
        .get(&full)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| fs::read_to_string(&full))?;
    let module_id = full
        .strip_prefix(&project_root)
        .map_err(|_| Error::InvalidPath(full.display().to_string()))?
        .to_string_lossy()
        .replace('\\', "/");
    let module_key = packages::hash(format!("{}\0{module_id}\0{source}", v06::cache::stamp()));
    let cached = MODULE_CACHE
        .with(|cache| cache.borrow().get(&module_key).cloned())
        .or_else(|| {
            if overlay.is_empty() {
                v06::cache::get(root, "module", &module_key)
            } else {
                None
            }
        });
    let mut own = if let Some(program) = cached {
        program
    } else {
        let mut parser = Parser {
            toks: {
                let mut tokens = lex(&source).map_err(|mut error| {
                    if let Error::Diagnostic(d) = &mut error {
                        d.source = module_id.clone();
                    }
                    error
                })?;
                for t in &mut tokens {
                    t.source = full
                        .strip_prefix(fs::canonicalize(root)?)
                        .map_err(|_| Error::InvalidPath(full.display().to_string()))?
                        .to_string_lossy()
                        .replace('\\', "/");
                }
                tokens
            },
            pos: 0,
            type_depth: 0,
        };
        let parsed = parser.program()?;
        v100::syntax_budget(&parsed)?;
        if overlay.is_empty() {
            v06::cache::put(root, "module", &module_key, &parsed);
        }
        if source.len() <= 128 * 1024 {
            MODULE_CACHE.with(|cache| {
                let mut cache = cache.borrow_mut();
                if cache.len() >= 128 {
                    cache.clear();
                }
                cache.insert(module_key, parsed.clone());
            });
        }
        parsed
    };
    own.root_origin = full.clone();
    if let Some(declared) = own.module_effects.remove(&PathBuf::new()) {
        own.module_effects.insert(full.clone(), declared);
    }
    own.stmt_origins = vec![full.clone(); own.stmts.len()];
    for def in own.functions.values_mut() {
        def.origin = full.clone();
    }
    for def in own.structs.values_mut() {
        def.origin = full.clone();
    }
    for def in own.enums.values_mut() {
        def.origin = full.clone();
    }
    for def in own.traits.values_mut() {
        def.origin = full.clone();
        for f in def.defaults.values_mut() {
            f.origin = full.clone();
        }
    }
    for def in own.aliases.values_mut() {
        def.origin = full.clone();
    }
    for (_, origin) in own.const_origins.values_mut() {
        *origin = full.clone();
    }
    for origin in own.impl_origins.values_mut() {
        *origin = full.clone();
    }
    let mut program = Program {
        root_origin: full.clone(),
        ..Program::default()
    };
    program.included_modules.insert(full.clone());
    for stmt in &own.stmts {
        if let StmtKind::Expr(Expr {
            kind: ExprKind::Name(name),
            ..
        }) = &stmt.kind
        {
            if let Some(spec) = name.strip_prefix("@import:") {
                let mut parts = spec.splitn(3, '|');
                let module = parts.next().unwrap();
                let alias = parts.next().unwrap_or("");
                let selected = parts
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.split_once('='))
                    .collect::<Vec<_>>();
                let (base, relative) = module
                    .split_once('/')
                    .and_then(|(name, rest)| imports.get(name).map(|root| (root, rest)))
                    .unwrap_or_else(|| {
                        let local = imports
                            .values()
                            .filter(|base| full.starts_with(base))
                            .max_by_key(|base| base.components().count())
                            .unwrap_or(&imports[""]);
                        (local, module)
                    });
                if imports.get("std") == Some(base)
                    && base
                        == &project_root
                            .join(".rewind")
                            .join(format!("std-{}", env!("CARGO_PKG_VERSION")))
                    && !v092::std_modules().contains_key(relative)
                {
                    return Err(Error::InvalidOperation(format!(
                        "unknown bundled std module {relative}"
                    )));
                }
                let mut imported = load_program_overlay(
                    &base.join(format!("{relative}.rw")),
                    root,
                    imports,
                    visiting,
                    loaded,
                    overlay,
                )?;
                let target_origin = imported.root_origin.clone();
                let renamed = if alias.is_empty() && selected.is_empty() {
                    BTreeMap::new()
                } else {
                    let package = imports
                        .iter()
                        .filter(|(name, path)| !name.is_empty() && *path == base)
                        .map(|(name, _)| name.as_str())
                        .next();
                    let canonical = package
                        .map(|name| format!("{name}/{relative}"))
                        .unwrap_or_else(|| relative.to_string());
                    namespace_symbols(&mut imported, &canonical)
                };
                let original = |name: &String| {
                    renamed
                        .iter()
                        .find(|(_, symbol)| *symbol == name)
                        .map(|(original, _)| original.clone())
                        .unwrap_or_else(|| name.clone())
                };
                let mut exposure = BTreeSet::new();
                let exports = imported
                    .functions
                    .iter()
                    .filter(|(_, f)| f.origin == target_origin && f.public)
                    .map(|(n, _)| original(n))
                    .chain(
                        imported
                            .structs
                            .iter()
                            .filter(|(_, f)| f.origin == target_origin && f.public)
                            .map(|(n, _)| original(n)),
                    )
                    .chain(
                        imported
                            .enums
                            .iter()
                            .filter(|(_, f)| f.origin == target_origin && f.public)
                            .map(|(n, _)| original(n)),
                    )
                    .chain(
                        imported
                            .traits
                            .iter()
                            .filter(|(_, f)| f.origin == target_origin && f.public)
                            .map(|(n, _)| original(n)),
                    )
                    .chain(
                        imported
                            .aliases
                            .iter()
                            .filter(|(_, d)| d.origin == target_origin && d.public)
                            .map(|(n, _)| original(n)),
                    )
                    .chain(
                        imported
                            .const_origins
                            .iter()
                            .filter(|(_, (public, origin))| *public && origin == &target_origin)
                            .map(|(n, _)| original(n)),
                    )
                    .collect::<BTreeSet<_>>();
                if !alias.is_empty() {
                    for export in &exports {
                        let visible = format!("{alias}.{export}");
                        if program
                            .import_aliases
                            .insert(
                                visible.clone(),
                                renamed
                                    .get(export)
                                    .cloned()
                                    .unwrap_or_else(|| export.clone()),
                            )
                            .is_some()
                        {
                            return Err(diagnostic(
                                &stmt.at,
                                format!("duplicate import alias {visible}"),
                            ));
                        }
                        exposure.insert(visible);
                    }
                } else if !selected.is_empty() {
                    for (export, local) in selected {
                        if !exports.contains(export) {
                            return Err(diagnostic(
                                &stmt.at,
                                format!("{export} is not a public export of {module}"),
                            ));
                        }
                        if program
                            .import_aliases
                            .insert(
                                local.into(),
                                renamed
                                    .get(export)
                                    .cloned()
                                    .unwrap_or_else(|| export.to_string()),
                            )
                            .is_some()
                        {
                            return Err(diagnostic(
                                &stmt.at,
                                format!("duplicate import alias {local}"),
                            ));
                        }
                        exposure.insert(local.into());
                    }
                } else {
                    exposure.extend(exports);
                }
                program
                    .import_exposure
                    .entry((full.clone(), target_origin))
                    .or_default()
                    .extend(exposure);
                program
                    .import_aliases
                    .extend(imported.import_aliases.clone());
                program
                    .import_exposure
                    .extend(imported.import_exposure.clone());
                if program.included_modules.contains(&imported.root_origin) {
                    continue;
                }
                let already_included = program.included_modules.clone();
                imported
                    .aliases
                    .retain(|_, d| !already_included.contains(&d.origin));
                imported
                    .functions
                    .retain(|_, f| !already_included.contains(&f.origin));
                imported
                    .structs
                    .retain(|_, f| !already_included.contains(&f.origin));
                imported
                    .traits
                    .retain(|_, f| !already_included.contains(&f.origin));
                imported
                    .enums
                    .retain(|_, f| !already_included.contains(&f.origin));
                imported.impls.retain(|key, _| {
                    !imported
                        .impl_origins
                        .get(key)
                        .is_some_and(|origin| already_included.contains(origin))
                });
                imported.consts.retain(|name, _| {
                    !imported
                        .const_origins
                        .get(name)
                        .is_some_and(|(_, origin)| already_included.contains(origin))
                });
                let statements = imported
                    .stmts
                    .into_iter()
                    .zip(imported.stmt_origins)
                    .filter(|(_, origin)| !already_included.contains(origin))
                    .collect::<Vec<_>>();
                imported.stmts = statements.iter().map(|(s, _)| s.clone()).collect();
                imported.stmt_origins = statements.into_iter().map(|(_, o)| o).collect();
                program
                    .included_modules
                    .extend(imported.included_modules.clone());
                for (name, f) in imported.functions {
                    if program.functions.insert(name.clone(), f).is_some() {
                        return Err(diagnostic(
                            &stmt.at,
                            format!("duplicate imported function {name}"),
                        ));
                    }
                }
                for (name, s) in imported.structs {
                    if program.structs.insert(name.clone(), s).is_some() {
                        return Err(diagnostic(
                            &stmt.at,
                            format!("duplicate imported struct {name}"),
                        ));
                    }
                }
                for (name, tr) in imported.traits {
                    if program.traits.insert(name.clone(), tr).is_some() {
                        return Err(diagnostic(
                            &stmt.at,
                            format!("duplicate imported trait {name}"),
                        ));
                    }
                }
                for (name, en) in imported.enums {
                    if program.enums.insert(name.clone(), en).is_some() {
                        return Err(diagnostic(
                            &stmt.at,
                            format!("duplicate imported enum {name}"),
                        ));
                    }
                }
                for (key, imp) in imported.impls {
                    if program.impls.insert(key, imp).is_some() {
                        return Err(diagnostic(
                            &stmt.at,
                            "duplicate imported trait implementation",
                        ));
                    }
                }
                for (name, alias) in imported.aliases {
                    if let Some(existing) = program.aliases.get(&name) {
                        if existing.origin != alias.origin {
                            return Err(diagnostic(
                                &stmt.at,
                                format!("type alias collision: {name}"),
                            ));
                        }
                    } else {
                        program.aliases.insert(name, alias);
                    }
                }
                program.impl_generics.extend(imported.impl_generics);
                program.impl_origins.extend(imported.impl_origins);
                program.impl_associated.extend(imported.impl_associated);
                program.module_effects.extend(imported.module_effects);
                program.const_origins.extend(imported.const_origins);
                for (name, ty) in imported.consts {
                    if program.consts.insert(name.clone(), ty).is_some() {
                        return Err(diagnostic(
                            &stmt.at,
                            format!("duplicate imported const {name}"),
                        ));
                    }
                }
                program.stmts.extend(imported.stmts);
                program.stmt_origins.extend(imported.stmt_origins);
            }
        }
    }
    let kept = own.stmts.into_iter().zip(own.stmt_origins).filter(|(s,_)| !matches!(&s.kind, StmtKind::Expr(Expr { kind: ExprKind::Name(n), .. }) if n.starts_with("@import:"))).collect::<Vec<_>>();
    own.stmts = kept.iter().map(|(s, _)| s.clone()).collect();
    own.stmt_origins = kept.into_iter().map(|(_, o)| o).collect();
    for f in own.functions.values_mut() {
        for (_, ty) in &mut f.params {
            *ty = rename_type(ty, &program.import_aliases);
        }
        f.ret = rename_type(&f.ret, &program.import_aliases);
        for (_, bound) in &mut f.type_params {
            if let Some(bound) = bound {
                *bound = rename_type(bound, &program.import_aliases);
            }
        }
    }
    for def in own.structs.values_mut() {
        for (_, ty) in &mut def.fields {
            *ty = rename_type(ty, &program.import_aliases);
        }
        for bound in def.bounds.values_mut() {
            *bound = rename_type(bound, &program.import_aliases);
        }
    }
    for def in own.enums.values_mut() {
        for fields in def.variants.values_mut() {
            for (_, ty) in fields {
                *ty = rename_type(ty, &program.import_aliases);
            }
        }
    }
    for alias in own.aliases.values_mut() {
        alias.ty = rename_type(&alias.ty, &program.import_aliases);
    }
    for def in own.traits.values_mut() {
        for (args, ret) in def.methods.values_mut() {
            for ty in args {
                *ty = rename_type(ty, &program.import_aliases);
            }
            *ret = rename_type(ret, &program.import_aliases);
        }
        for f in def.defaults.values_mut() {
            v06::language::rename_function(f, &program.import_aliases);
        }
    }
    for (name, d) in own.aliases {
        if program.aliases.insert(name.clone(), d).is_some() {
            return Err(Error::InvalidOperation(format!(
                "duplicate type alias {name}"
            )));
        }
    }
    program.impl_generics.extend(own.impl_generics);
    for (name, f) in own.functions {
        if program.functions.insert(name.clone(), f).is_some() {
            return Err(Error::InvalidOperation(format!(
                "duplicate function {name}"
            )));
        }
    }
    for (name, s) in own.structs {
        if program.structs.insert(name.clone(), s).is_some() {
            return Err(Error::InvalidOperation(format!("duplicate struct {name}")));
        }
    }
    for (name, tr) in own.traits {
        if program.traits.insert(name.clone(), tr).is_some() {
            return Err(Error::InvalidOperation(format!("duplicate trait {name}")));
        }
    }
    for (name, en) in own.enums {
        if program.enums.insert(name.clone(), en).is_some() {
            return Err(Error::InvalidOperation(format!("duplicate enum {name}")));
        }
    }
    for (key, imp) in own.impls {
        if program.impls.insert(key, imp).is_some() {
            return Err(Error::InvalidOperation(
                "duplicate trait implementation".into(),
            ));
        }
    }
    program.impl_origins.extend(own.impl_origins);
    program.impl_associated.extend(own.impl_associated);
    program.module_effects.extend(own.module_effects);
    program.const_origins.extend(own.const_origins);
    for (name, ty) in own.consts {
        if program.consts.insert(name.clone(), ty).is_some() {
            return Err(Error::InvalidOperation(format!("duplicate const {name}")));
        }
    }
    program.stmts.extend(own.stmts);
    program.stmt_origins.extend(own.stmt_origins);
    visiting.remove(&full);
    loaded.insert(full, program.clone());
    Ok(program)
}

pub fn cli(mode: &str, file: &str, root: &Path, trace: bool, options: RunOptions) -> Result<()> {
    let mut manifest = if mode == "test" {
        project::ProjectConfig::load_for_test(root)?
    } else {
        project::ProjectConfig::load(root)?
    };
    if let Some(config) = &manifest {
        config.lock(root, false)?;
    }
    if manifest.is_none() && options.standalone {
        let entry = if file.is_empty() {
            root.join("main.rw")
        } else {
            fs::canonicalize(file)?
        };
        manifest = Some(standalone_config(root, entry, &options.allowed_effects)?);
    }
    let file = if let Some(config) = &manifest {
        if file.is_empty()
            || (!Path::new(file).exists()
                && Path::new(file)
                    .file_name()
                    .is_some_and(|name| name == "main.rw"))
        {
            config.entry.to_string_lossy().into_owned()
        } else {
            file.into()
        }
    } else if file.is_empty() {
        root.join("main.rw").to_string_lossy().into_owned()
    } else {
        file.into()
    };
    let mut imports = BTreeMap::new();
    imports.insert(
        String::new(),
        manifest
            .as_ref()
            .map(|m| m.source_root.clone())
            .unwrap_or(fs::canonicalize(root)?),
    );
    if let Some(config) = &manifest {
        imports.extend(config.imports.clone());
    }
    let mut program = load_program(
        Path::new(&file),
        root,
        &imports,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    program.strict_visibility = manifest.as_ref().is_some_and(|m| {
        matches!(
            m.language.as_str(),
            "0.3"
                | "0.4"
                | "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        )
    });
    program.language = manifest
        .as_ref()
        .map(|m| m.language.clone())
        .unwrap_or_default();
    v05::prepare(&mut program)?;
    check_program(&program)?;
    if let Some(config) = &manifest {
        if matches!(
            config.language.as_str(),
            "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) {
            v05::validate(&program, config)?;
        } else if config.language == "0.4" {
            effects::validate(&program, config)?;
        }
    }
    vm::validate(&program)?;
    if mode == "build" {
        let artifact = if matches!(
            program.language.as_str(),
            "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) {
            v05::artifact(
                &program,
                root,
                manifest
                    .as_ref()
                    .ok_or_else(|| Error::InvalidOperation("artifact requires manifest".into()))?,
            )?
        } else {
            vm::build_artifact(&program, root)?
        };
        fs::write(
            options
                .output
                .as_ref()
                .cloned()
                .unwrap_or_else(|| root.join("rewind.build.json")),
            serde_json::to_string_pretty(&artifact)
                .map_err(|e| Error::InvalidOperation(e.to_string()))?
                + "\n",
        )?;
        return Ok(());
    }
    if mode == "check" {
        return Ok(());
    }
    vm::execute(program, root, trace, mode, options)
}
pub fn documentation(file: &str, root: &Path) -> Result<String> {
    documentation_mode(file, root, false)
}
fn documentation_mode(file: &str, root: &Path, include_dev: bool) -> Result<String> {
    let manifest = if include_dev {
        project::ProjectConfig::load_for_test(root)?
    } else {
        project::ProjectConfig::load(root)?
    };
    let mut imports = BTreeMap::new();
    imports.insert(
        String::new(),
        manifest
            .as_ref()
            .map(|m| m.source_root.clone())
            .unwrap_or(fs::canonicalize(root)?),
    );
    if let Some(config) = &manifest {
        imports.extend(config.imports.clone());
        config.lock(root, false)?;
    }
    let mut program = load_program(
        Path::new(file),
        root,
        &imports,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    program.strict_visibility = manifest.as_ref().is_some_and(|m| {
        matches!(
            m.language.as_str(),
            "0.3"
                | "0.4"
                | "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        )
    });
    program.language = manifest
        .as_ref()
        .map(|m| m.language.clone())
        .unwrap_or_default();
    v05::prepare(&mut program)?;
    check_program(&program)?;
    if let Some(config) = &manifest {
        if matches!(
            config.language.as_str(),
            "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) {
            v05::validate(&program, config)?;
        } else if config.language == "0.4" {
            effects::validate(&program, config)?;
        }
    }
    let mut docs = BTreeMap::new();
    let mut pending = Vec::new();
    for line in fs::read_to_string(file)?.lines() {
        let line = line.trim();
        if let Some(comment) = line.strip_prefix("///") {
            pending.push(comment.trim_start().to_string());
            continue;
        }
        if let Some(declaration) = line.strip_prefix("pub ") {
            let declaration = declaration.strip_prefix("async ").unwrap_or(declaration);
            let mut parts = declaration.split_whitespace();
            if let (Some(kind), Some(name)) = (parts.next(), parts.next()) {
                let name = name.split(['<', '(', '{', ':']).next().unwrap_or(name);
                docs.insert(format!("{kind}:{name}"), pending.join("\n"));
            }
        }
        pending.clear();
    }
    fn append_doc(out: &mut String, docs: &BTreeMap<String, String>, kind: &str, name: &str) {
        if let Some(doc) = docs.get(&format!("{kind}:{name}")) {
            if !doc.is_empty() {
                for line in doc.lines() {
                    out.push_str(&format!("  {line}\n"));
                }
            }
        }
    }
    let mut out = String::from("# REWIND public API\n\n");
    for (name, ty) in &program.consts {
        if program
            .const_origins
            .get(name)
            .is_none_or(|(_, origin)| origin != &program.root_origin)
        {
            continue;
        }
        out.push_str(&format!("- `pub const {name}: {ty}`\n"));
        append_doc(&mut out, &docs, "const", name);
    }
    for (name, def) in &program.structs {
        if def.public && def.origin == program.root_origin {
            out.push_str(&format!(
                "- `pub struct {name}{} {{ {} }}`\n",
                if def.type_params.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", def.type_params.join(", "))
                },
                def.fields
                    .iter()
                    .filter(|(n, _)| !def.private_fields.contains(n))
                    .map(|(n, t)| format!("{n}: {t}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            append_doc(&mut out, &docs, "struct", name);
        }
    }
    for (name, def) in &program.enums {
        if def.public && def.origin == program.root_origin {
            out.push_str(&format!(
                "- `pub enum {name}{} {{ {} }}`\n",
                if def.type_params.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", def.type_params.join(", "))
                },
                def.variants.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
            append_doc(&mut out, &docs, "enum", name);
        }
    }
    for (name, def) in &program.traits {
        if def.public && def.origin == program.root_origin {
            out.push_str(&format!("- `pub trait {name}`\n"));
            append_doc(&mut out, &docs, "trait", name);
            for (method, (args, ret)) in &def.methods {
                out.push_str(&format!(
                    "  - `fn {method}({}) -> {ret}`\n",
                    args.join(", ")
                ));
            }
        }
    }
    for (name, def) in &program.functions {
        if def.public && def.origin == program.root_origin {
            out.push_str(&format!(
                "- `pub {}fn {name}{}({}) -> {}{}`\n",
                if def.asynchronous { "async " } else { "" },
                if def.type_params.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        def.type_params
                            .iter()
                            .map(|(n, b)| if let Some(b) = b {
                                format!("{n}: {b}")
                            } else {
                                n.clone()
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
                def.params
                    .iter()
                    .map(|(n, t)| format!("{n}: {t}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                def.ret,
                def.effects
                    .as_ref()
                    .map(|effects| format!(
                        " effects {{{}}}",
                        effects.iter().cloned().collect::<Vec<_>>().join(", ")
                    ))
                    .unwrap_or_default()
            ));
            append_doc(&mut out, &docs, "fn", name);
        }
    }
    Ok(out)
}
pub fn sign_package(path: &Path, key: &Path, output: &Path) -> Result<()> {
    packages::sign_package(path, key, output)
}
pub fn run_compiled(path: &Path, root: &Path, mut options: RunOptions) -> Result<()> {
    let policy = project::RuntimePolicy::load(root)?;
    if let Some(policy) = &policy {
        options.allowed_effects = policy.effects.clone();
    } else {
        options.allowed_effects.extend(standalone_effects());
    }
    v05::run_compiled(path, root, options, policy.as_ref())
}
pub fn run_artifact(path: &Path, root: &Path, options: RunOptions) -> Result<()> {
    v05::run_artifact(path, root, options)
}
pub fn keygen(path: &Path) -> Result<()> {
    packages::keygen(path)
}
pub fn sign_file(path: &Path, key: &Path, output: &Path, kind: &str) -> Result<()> {
    packages::sign_file(path, key, output, kind)
}
pub fn verify_file(path: &Path, signature: &Path, public: &str, kind: &str) -> Result<()> {
    packages::verify_file(path, signature, public, kind)
}
pub fn lsp(root: &Path) -> Result<()> {
    v05::lsp(root)
}
pub fn debug_session(path: &Path, root: &Path, options: RunOptions) -> Result<()> {
    v05::debug_session(path, root, options)
}
pub fn replay_trace(path: &Path, root: &Path, mut options: RunOptions) -> Result<()> {
    if let Some(key) = &options.verify_key {
        packages::verify_file(
            path,
            &PathBuf::from(format!("{}.signature", path.display())),
            key,
            "trace",
        )?;
    }
    if fs::metadata(path)?.len() > 128 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "replay trace exceeds 128 MiB budget".into(),
        ));
    }
    let trace: serde_json::Value = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if trace["format"] != 1 || trace["compiler"] != env!("CARGO_PKG_VERSION") {
        return Err(Error::InvalidOperation(
            "ReplayMismatch: trace format/compiler".into(),
        ));
    }
    if trace.get("artifact_entry").is_some_and(|v| !v.is_string()) {
        return Err(Error::InvalidOperation(
            "ReplayMismatch: invalid artifact entry".into(),
        ));
    }
    let artifact = trace
        .get("artifact_entry")
        .and_then(serde_json::Value::as_str);
    let file = artifact
        .or_else(|| trace["entry"].as_str())
        .ok_or_else(|| Error::InvalidOperation("ReplayMismatch: missing entry".into()))?;
    let relative = Path::new(file);
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(Error::InvalidPath(file.into()));
    }
    let entry = root.join(relative).to_string_lossy().into_owned();
    let artifact_replay = artifact.is_some();
    let task_steps = trace["task_steps"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or_else(|| Error::InvalidOperation("ReplayMismatch: invalid task budget".into()))?;
    if options.task_steps.is_some_and(|n| n != task_steps) {
        return Err(Error::InvalidOperation(
            "ReplayMismatch: task budget differs from recording".into(),
        ));
    }
    options.task_steps = Some(task_steps);
    if let Some(choices) = trace["schedule_choices"].as_array() {
        options.choices = choices
            .iter()
            .map(|v| {
                v.as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| {
                        Error::InvalidOperation("ReplayMismatch: invalid schedule choice".into())
                    })
            })
            .collect::<Result<_>>()?;
    }
    options.recorded_test = trace["test"].as_str().map(str::to_string);
    let mode = if options.recorded_test.is_some() {
        "test"
    } else {
        "run"
    };
    options.standalone = trace["standalone"] == true;
    options.replay = Some(trace);
    options.virtual_publish = options.replay.as_ref().unwrap()["virtual_publish"] == true;
    if artifact_replay {
        run_compiled(Path::new(&entry), root, options)
    } else {
        cli(mode, &entry, root, false, options)
    }
}
pub fn debug_trace(path: &Path) -> Result<()> {
    if fs::metadata(path)?.len() > 128 * 1024 * 1024 {
        return Err(Error::InvalidOperation("trace exceeds budget".into()));
    }
    let trace: serde_json::Value = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if trace["record_mode"] == "compact" {
        return Err(Error::InvalidOperation(
            "CompactTraceNoDebugHistory: record with --record-mode debug for inspection".into(),
        ));
    }
    if trace["format"] != 1 {
        return Err(Error::InvalidOperation("unsupported trace format".into()));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&trace["debug"])
            .map_err(|e| Error::InvalidOperation(e.to_string()))?
    );
    Ok(())
}
pub fn lock_project(root: &Path) -> Result<()> {
    let project = project::ProjectConfig::load(root)?
        .ok_or_else(|| Error::InvalidOperation("rewind.toml is required".into()))?;
    if matches!(
        project.language.as_str(),
        "0.4"
            | "0.5"
            | "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
            | "1.1.0"
            | "1.2.0"
            | "1.3.0"
            | "1.4.0"
            | "1.5.0"
            | "1.6.0"
            | "1.6.1"
            | "1.7.0"
            | "1.7.1"
            | "1.8.0"
            | "1.8.1"
            | "1.8.2"
            | "1.8.3"
            | "1.8.4"
            | "1.8.5"
            | "1.8.6"
            | "1.8.7"
            | "1.8.8"
            | "1.9.0"
            | "1.9.1"
            | "1.9.2"
            | "1.9.3"
            | "1.9.4"
            | "1.9.5"
            | "1.9.6"
            | "1.9.7"
            | "1.9.8"
            | "1.9.9"
            | "1.9.10"
            | "1.9.11"
            | "1.9.12"
            | "1.9.13"
            | "1.9.14"
            | "1.9.15"
            | "1.9.16"
            | "1.9.17"
            | "1.9.18"
            | "1.9.19"
            | "1.9.20"
            | "1.9.21"
            | "1.9.22"
            | "1.9.23"
            | "1.9.24"
            | "1.9.25"
            | "1.9.26"
            | "1.9.27"
            | "1.9.28"
            | "1.9.29"
            | "1.9.30"
            | "1.9.31"
            | "1.9.32"
            | "1.9.33"
            | "1.9.34"
            | "1.9.35"
            | "1.9.36"
            | "1.9.37"
            | "1.9.38"
            | "1.9.39"
            | "1.9.40"
            | "1.9.41"
            | "1.9.42"
            | "1.9.43"
            | "1.9.44"
            | "1.9.45"
            | "1.9.46"
            | "1.9.47"
            | "1.9.48"
            | "1.9.49"
            | "1.9.50"
            | "1.9.51"
            | "2.0.0"
    ) {
        return Err(Error::InvalidOperation(
            "use 'rewind update' to update v0.4 dependencies".into(),
        ));
    }
    project.lock(root, true)
}
pub fn update_preview(root: &Path, output: Option<&Path>) -> Result<()> {
    v06::update::preview(root, output)
}
pub fn update_apply(root: &Path, path: &Path) -> Result<()> {
    v06::update::apply(root, path)
}
pub fn compatibility(path: &Path) -> Result<()> {
    v06::update::compatibility(path)
}
pub fn update_project(root: &Path) -> Result<()> {
    let project = project::ProjectConfig::load_for_update(root)?
        .ok_or_else(|| Error::InvalidOperation("rewind.toml is required".into()))?;
    project.lock(root, true)
}
pub fn migrate_project(root: &Path, write: bool) -> Result<()> {
    let mut config = project::ProjectConfig::load(root)?
        .ok_or_else(|| Error::InvalidOperation("migration requires rewind.toml".into()))?;
    let old = config.language.clone();
    println!("migration preview: language {old} -> 0.5; revalidate ownership/effects; regenerate rewind.lock");
    if old == "0.5" {
        return Ok(());
    }
    let manifest = root.join("rewind.toml");
    let source = fs::read_to_string(&manifest)?;
    let proposed = source
        .lines()
        .map(|line| {
            if line
                .split_once('=')
                .is_some_and(|(k, _)| k.trim() == "language")
            {
                "language = \"0.5\""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    println!("--- rewind.toml\n{proposed}");
    config.language = "0.5".into();
    let mut program = load_program(
        &config.entry,
        root,
        &config.imports,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    program.language = "0.5".into();
    program.strict_visibility = true;
    v05::prepare(&mut program)?;
    if let Err(error) = check_program(&program).and_then(|_| v05::validate(&program, &config)) {
        eprintln!("migration needs source changes: {error}");
        if write {
            return Err(Error::InvalidOperation(
                "migration not written: resolve source diagnostics first".into(),
            ));
        }
        return Ok(());
    }
    if write {
        config.lock(root, true)?;
        fs::write(manifest, proposed)?;
        println!("migration written");
    } else {
        println!("source checks passed; use --write to apply this proposal");
    }
    Ok(())
}
pub fn format_source(source: &str) -> Result<String> {
    let mut parser = Parser {
        toks: lex(source)?,
        pos: 0,
        type_depth: 0,
    };
    parser.program()?;
    let mut out = String::new();
    let mut depth = 0usize;
    let mut quoted = false;
    let mut comment_depth = 0usize;
    for line in source.lines() {
        // Preserve whitespace inside multiline String literals.
        let line = if quoted || comment_depth != 0 || line.contains('"') {
            line
        } else {
            line.trim_end()
        };
        if quoted || comment_depth != 0 {
            out.push_str(line);
            out.push('\n');
        } else if line.trim().is_empty() {
            out.push('\n');
        } else {
            let content = line.trim_start();
            let indent = depth.saturating_sub(usize::from(content.starts_with('}')));
            out.push_str(&"    ".repeat(indent));
            out.push_str(content);
            out.push('\n');
        }
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if !quoted && c == '/' && chars.peek() == Some(&'*') {
                chars.next();
                comment_depth += 1;
                continue;
            }
            if comment_depth != 0 {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    comment_depth -= 1;
                }
                continue;
            }
            if c == '\\' && quoted {
                chars.next();
                continue;
            }
            if c == '"' {
                quoted = !quoted;
                continue;
            }
            if !quoted && c == '/' && chars.peek() == Some(&'/') {
                break;
            }
            if !quoted && c == '{' {
                depth += 1;
            }
            if !quoted && c == '}' {
                depth = depth.saturating_sub(1);
            }
        }
    }
    Ok(out)
}

fn check_program(program: &Program) -> Result<()> {
    v15::validate(program)?;
    v07::validate_records(program)?;
    v09::record_signatures(program)?;
    if matches!(
        program.language.as_str(),
        "0.5"
            | "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
            | "1.1.0"
            | "1.2.0"
            | "1.3.0"
            | "1.4.0"
            | "1.5.0"
            | "1.6.0"
            | "1.6.1"
            | "1.7.0"
            | "1.7.1"
            | "1.8.0"
            | "1.8.1"
            | "1.8.2"
            | "1.8.3"
            | "1.8.4"
            | "1.8.5"
            | "1.8.6"
            | "1.8.7"
            | "1.8.8"
            | "1.9.0"
            | "1.9.1"
            | "1.9.2"
            | "1.9.3"
            | "1.9.4"
            | "1.9.5"
            | "1.9.6"
            | "1.9.7"
            | "1.9.8"
            | "1.9.9"
            | "1.9.10"
            | "1.9.11"
            | "1.9.12"
            | "1.9.13"
            | "1.9.14"
            | "1.9.15"
            | "1.9.16"
            | "1.9.17"
            | "1.9.18"
            | "1.9.19"
            | "1.9.20"
            | "1.9.21"
            | "1.9.22"
            | "1.9.23"
            | "1.9.24"
            | "1.9.25"
            | "1.9.26"
            | "1.9.27"
            | "1.9.28"
            | "1.9.29"
            | "1.9.30"
            | "1.9.31"
            | "1.9.32"
            | "1.9.33"
            | "1.9.34"
            | "1.9.35"
            | "1.9.36"
            | "1.9.37"
            | "1.9.38"
            | "1.9.39"
            | "1.9.40"
            | "1.9.41"
            | "1.9.42"
            | "1.9.43"
            | "1.9.44"
            | "1.9.45"
            | "1.9.46"
            | "1.9.47"
            | "1.9.48"
            | "1.9.49"
            | "1.9.50"
            | "1.9.51"
            | "2.0.0"
    ) {
        let entries = program.impls.iter().collect::<Vec<_>>();
        for (i, ((tr, target), methods)) in entries.iter().enumerate() {
            for ((other_tr, other_target), other_methods) in entries.iter().skip(i + 1) {
                if tr == other_tr {
                    let params = methods
                        .values()
                        .next()
                        .and_then(|n| program.functions.get(n))
                        .map(|f| {
                            f.type_params
                                .iter()
                                .map(|(n, _)| n.clone())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let other_params = other_methods
                        .values()
                        .next()
                        .and_then(|n| program.functions.get(n))
                        .map(|f| {
                            f.type_params
                                .iter()
                                .map(|(n, _)| n.clone())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if unify_type(target, other_target, &params, &mut BTreeMap::new())
                        || unify_type(other_target, target, &other_params, &mut BTreeMap::new())
                    {
                        return Err(Error::InvalidOperation(format!(
                            "overlapping impls: {tr} for {target} and {other_target}"
                        )));
                    }
                }
            }
        }
    }
    for ((trait_name, target), methods) in &program.impls {
        let impl_origin = &program.impl_origins[&(trait_name.clone(), target.clone())];
        let trait_local = program
            .traits
            .get(trait_name)
            .is_some_and(|tr| &tr.origin == impl_origin);
        let target_base = target.split('<').next().unwrap_or(target);
        let type_local = program
            .structs
            .get(target_base)
            .is_some_and(|ty| &ty.origin == impl_origin)
            || program
                .enums
                .get(target_base)
                .is_some_and(|ty| &ty.origin == impl_origin);
        if !trait_local && !type_local {
            return Err(Error::InvalidOperation(format!(
                "orphan impl: {trait_name} for {target}"
            )));
        }
        let standard = standard_trait(trait_name);
        let tr = program
            .traits
            .get(trait_name)
            .or(standard.as_ref())
            .ok_or_else(|| Error::InvalidOperation(format!("unknown trait {trait_name}")))?;
        let associated = program
            .impl_associated
            .get(&(trait_name.clone(), target.clone()))
            .cloned()
            .unwrap_or_default();
        if associated.keys().cloned().collect::<BTreeSet<_>>() != tr.associated {
            return Err(Error::InvalidOperation(format!(
                "impl {trait_name} for {target}: associated types do not match trait"
            )));
        }
        let signature_type = |ty: &str| {
            let mut ty = ty.to_string();
            for (n, t) in &associated {
                ty = ty.replace(&format!("Self::{n}"), t);
            }
            ty.replace("Self", target)
        };
        if !builtin_key_type(program, target.split('<').next().unwrap_or(target))
            && !program
                .structs
                .contains_key(target.split('<').next().unwrap_or(target))
            && !program
                .enums
                .contains_key(target.split('<').next().unwrap_or(target))
        {
            return Err(Error::InvalidOperation(format!(
                "unknown impl target {target}"
            )));
        }
        if methods.len() != tr.methods.len() || tr.methods.keys().any(|m| !methods.contains_key(m))
        {
            return Err(Error::InvalidOperation(format!(
                "impl {trait_name} for {target} does not define every trait method"
            )));
        }
        for (method, symbol) in methods {
            let f = &program.functions[symbol];
            let (args, ret) = &tr.methods[method];
            if f.params.len() != args.len()
                || f.params
                    .iter()
                    .zip(args)
                    .any(|((_, actual), expected)| actual != &signature_type(expected))
                || f.ret != signature_type(ret)
            {
                return Err(diagnostic(
                    &f.at,
                    format!("method {method} does not match trait signature"),
                ));
            }
        }
    }
    for (name, f) in &program.functions {
        let mut seen = BTreeSet::new();
        for (arg, _) in &f.params {
            if !seen.insert(arg) {
                return Err(diagnostic(
                    &f.at,
                    format!("duplicate parameter {arg} in {name}"),
                ));
            }
        }
        if f.test && (!f.params.is_empty() || f.ret != "Unit") {
            return Err(diagnostic(
                &f.at,
                "test functions must have no arguments and return Unit",
            ));
        }
    }
    let mut checker = Checker {
        program,
        scopes: vec![BTreeMap::new()],
        return_ty: None,
        loop_depth: 0,
        bounds: BTreeMap::new(),
        origin: program.root_origin.clone(),
    };
    for (stmt, origin) in program.stmts.iter().zip(&program.stmt_origins) {
        checker.origin = origin.clone();
        checker.stmt(stmt)?;
    }
    let globals = checker.scopes[0].clone();
    let global_origins: BTreeMap<_, _> = program
        .stmts
        .iter()
        .zip(&program.stmt_origins)
        .filter_map(|(stmt, origin)| match &stmt.kind {
            StmtKind::Let(name, ..) | StmtKind::Using(name, ..) => {
                Some((name.clone(), origin.clone()))
            }
            _ => None,
        })
        .collect();

    let mut interfaces = program.clone();
    interfaces.stmts.clear();
    for f in interfaces.functions.values_mut() {
        f.body.clear();
    }
    let interface_key = v06::cache::key(&(interfaces, &globals)).unwrap_or_default();
    for f in program.functions.values() {
        let root = program.root_origin.parent().unwrap_or(Path::new("."));
        let key = v06::cache::key(&(f, &interface_key)).unwrap_or_default();
        if matches!(
            program.language.as_str(),
            "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) && v06::cache::get::<bool>(root, "checked", &key) == Some(true)
        {
            continue;
        }
        let mut checker = Checker {
            program,
            scopes: vec![
                if language_at_least(&program.language, "1.9.17") {
                    globals
                        .iter()
                        .filter(|(name, _)| {
                            name.starts_with("$import$")
                                || global_origins.get(*name) == Some(&f.origin)
                        })
                        .map(|(name, value)| (name.clone(), value.clone()))
                        .collect()
                } else {
                    globals.clone()
                },
                BTreeMap::new(),
            ],
            return_ty: Some(f.ret.clone()),
            loop_depth: 0,
            bounds: f
                .type_params
                .iter()
                .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                .collect(),
            origin: f.origin.clone(),
        };
        for (name, ty) in &f.params {
            checker.scopes[1].insert(name.clone(), (ty.clone(), false));
        }
        for stmt in &f.body {
            checker.stmt(stmt)?;
        }
        if f.ret != "Unit" && !guarantees_return(&f.body) {
            return Err(diagnostic(
                &f.at,
                format!("function returning {} needs a return on every path", f.ret),
            ));
        }
        if matches!(
            program.language.as_str(),
            "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) {
            v06::cache::put(root, "checked", &key, &true);
        }
    }
    v07::validate_constants(program)?;
    Ok(())
}
fn guarantees_return(body: &[Stmt]) -> bool {
    body.iter().any(|stmt| match &stmt.kind {
        StmtKind::Return(_) => true,
        StmtKind::External(_, body) | StmtKind::Block(body) => guarantees_return(body),
        StmtKind::If(_, yes, no) => guarantees_return(yes) && guarantees_return(no),
        StmtKind::Match(_, arms) => {
            !arms.is_empty()
                && arms
                    .iter()
                    .all(|(_, _, body)| guarantees_return(std::slice::from_ref(body)))
        }
        StmtKind::Expr(Expr {
            kind: ExprKind::Call(target, _),
            ..
        }) if matches!(&target.kind, ExprKind::Name(name) if name == "panic") => true,
        _ => false,
    })
}

struct Checker<'a> {
    program: &'a Program,
    scopes: Vec<BTreeMap<String, (String, bool)>>,
    return_ty: Option<String>,
    loop_depth: usize,
    bounds: BTreeMap<String, String>,
    origin: PathBuf,
}
impl Checker<'_> {
    fn field_visible(&self, def: &StructDef, field: &str, at: &Tok) -> Result<()> {
        if matches!(
            self.program.language.as_str(),
            "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) && def.origin != self.origin
            && def.private_fields.contains(field)
        {
            return Err(diagnostic(
                at,
                format!("field {field} is private to its module"),
            ));
        }
        Ok(())
    }
    fn constructor_visible(&self, def: &StructDef, at: &Tok) -> Result<()> {
        if def.private_fields.contains("$native") {
            return Err(diagnostic(
                at,
                "opaque type constructor is not available; use its standard-library factory",
            ));
        }
        if matches!(
            self.program.language.as_str(),
            "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) && def.origin != self.origin
            && !def.private_fields.is_empty()
        {
            return Err(diagnostic(
                at,
                "opaque type constructor is private to its module",
            ));
        }
        Ok(())
    }
    fn visible(&self, public: bool, origin: &Path, at: &Tok, name: &str) -> Result<()> {
        if self.program.strict_visibility && !public && origin != self.origin {
            return Err(diagnostic(at, format!("{name} is private to its module")));
        }
        if self.program.strict_visibility && origin != self.origin {
            if let Some(exposed) = self
                .program
                .import_exposure
                .get(&(self.origin.clone(), origin.to_path_buf()))
            {
                if !exposed.contains(name)
                    && !(matches!(
                        self.program.language.as_str(),
                        "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) && exposed.iter().any(|local| {
                        resolve_alias(self.program, local).split('<').next()
                            == resolve_alias(self.program, name).split('<').next()
                    }))
                {
                    return Err(diagnostic(
                        at,
                        format!("{name} is not imported into this module"),
                    ));
                }
            }
        }
        Ok(())
    }
    fn pattern(
        &self,
        p: &Pattern,
        ty: &str,
        at: &Tok,
        bindings: &mut BTreeMap<String, (String, bool)>,
    ) -> Result<()> {
        if let Pattern::Struct(name, _) | Pattern::Variant(name, _) = p {
            let head = name.split("::").next().unwrap_or(name);
            if self.program.import_aliases.contains_key(head) {
                return self.pattern(
                    &resolve_pattern_alias(p, &self.program.import_aliases),
                    ty,
                    at,
                    bindings,
                );
            }
        }
        match p {
            Pattern::Wildcard => {}
            Pattern::Bind(name) => {
                if bindings.insert(name.clone(), (ty.into(), false)).is_some() {
                    return Err(diagnostic(at, format!("duplicate pattern binding {name}")));
                }
            }
            Pattern::Literal(value) => {
                let actual = match value {
                    Value::Bool(_) => "Bool",
                    Value::Int(_) => "Int",
                    Value::Text(_) => "String",
                    _ => return Err(diagnostic(at, "unsupported pattern literal")),
                };
                if !compatible(ty, actual) {
                    return Err(diagnostic(
                        at,
                        format!("pattern expects {ty}, found {actual}"),
                    ));
                }
            }
            Pattern::Range(_, _) => {
                if ty != "Int" {
                    return Err(diagnostic(at, "range pattern requires Int"));
                }
            }
            Pattern::List(parts) => {
                let Some(inner) = ty.strip_prefix("List<").and_then(|s| s.strip_suffix('>')) else {
                    return Err(diagnostic(at, "List pattern requires List"));
                };
                for part in parts {
                    self.pattern(part, inner, at, bindings)?;
                }
            }
            Pattern::Struct(name, fields) => {
                if name == "FileError" && ty == "FileError" {
                    for (field, p) in fields {
                        let field_ty = match field.as_str() {
                            "code" | "path" => "String",
                            "cause" => "Option<String>",
                            "causes" => "List<String>",
                            _ => {
                                return Err(diagnostic(
                                    at,
                                    format!("unknown FileError field {field}"),
                                ))
                            }
                        };
                        self.pattern(p, field_ty, at, bindings)?;
                    }
                    return Ok(());
                }
                if let Some((pattern_ty, variant)) = name.split_once("::") {
                    let base = ty.split('<').next().unwrap_or(ty);
                    if pattern_ty.split('<').next() != Some(base) {
                        return Err(diagnostic(
                            at,
                            format!("pattern {name} does not match {ty}"),
                        ));
                    }
                    let def = self
                        .program
                        .enums
                        .get(base)
                        .ok_or_else(|| diagnostic(at, format!("unknown enum {base}")))?;
                    let variant_fields = def
                        .variants
                        .get(variant)
                        .ok_or_else(|| diagnostic(at, format!("unknown variant {variant}")))?;
                    let args = ty
                        .split_once('<')
                        .map(|(_, i)| split_type_args(outer_type_end(i)))
                        .unwrap_or_default();
                    let substitutions = def
                        .type_params
                        .iter()
                        .cloned()
                        .zip(args.iter().map(|s| s.to_string()))
                        .collect::<BTreeMap<_, _>>();
                    for (field, p) in fields {
                        let field_ty =
                            variant_fields
                                .iter()
                                .find(|(n, _)| n == field)
                                .ok_or_else(|| {
                                    diagnostic(at, format!("unknown variant field {field}"))
                                })?;
                        self.pattern(
                            p,
                            &substitute_type(&field_ty.1, &substitutions),
                            at,
                            bindings,
                        )?;
                    }
                    return Ok(());
                }
                if ty.split('<').next() != Some(name) {
                    return Err(diagnostic(
                        at,
                        format!("pattern {name} does not match {ty}"),
                    ));
                }
                let def = self
                    .program
                    .structs
                    .get(name)
                    .ok_or_else(|| diagnostic(at, format!("unknown struct {name}")))?;
                if def.private_fields.contains("$native") {
                    return Err(diagnostic(
                        at,
                        "opaque type cannot be destructured; use its standard-library accessors",
                    ));
                }
                let args = ty
                    .split_once('<')
                    .map(|(_, i)| split_type_args(outer_type_end(i)))
                    .unwrap_or_default();
                let map = def
                    .type_params
                    .iter()
                    .cloned()
                    .zip(args.iter().map(|s| s.to_string()))
                    .collect::<BTreeMap<_, _>>();
                for (field, p) in fields {
                    self.field_visible(def, field, at)?;
                    let field_ty = def
                        .fields
                        .iter()
                        .find(|(n, _)| n == field)
                        .ok_or_else(|| diagnostic(at, format!("unknown field {field}")))?;
                    self.pattern(p, &substitute_type(&field_ty.1, &map), at, bindings)?;
                }
            }
            Pattern::Variant(name, parts) => {
                let (head, field_types) = if name == "$tuple" {
                    if !matches!(
                        self.program.language.as_str(),
                        "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) {
                        return Err(diagnostic(at, "tuple pattern requires language 0.6"));
                    }
                    let inner = ty
                        .strip_prefix("Tuple<")
                        .and_then(|s| s.strip_suffix('>'))
                        .ok_or_else(|| diagnostic(at, "tuple pattern requires a tuple"))?;
                    (
                        name.as_str(),
                        split_type_args(inner)
                            .into_iter()
                            .map(str::to_string)
                            .collect(),
                    )
                } else if ty.starts_with("Option<") {
                    let inner = outer_type_end(ty.strip_prefix("Option<").unwrap());
                    (
                        name.as_str(),
                        if name == "Some" {
                            vec![inner.to_string()]
                        } else if name == "None" {
                            Vec::new()
                        } else {
                            return Err(diagnostic(at, "invalid Option pattern"));
                        },
                    )
                } else if ty.starts_with("Result<") {
                    let inner = outer_type_end(ty.strip_prefix("Result<").unwrap());
                    let args = split_type_args(inner);
                    (
                        name.as_str(),
                        if name == "Ok" {
                            vec![args[0].into()]
                        } else if name == "Err" {
                            vec![args[1].into()]
                        } else {
                            return Err(diagnostic(at, "invalid Result pattern"));
                        },
                    )
                } else {
                    let base = ty.split('<').next().unwrap_or(ty);
                    let def = self
                        .program
                        .enums
                        .get(base)
                        .ok_or_else(|| diagnostic(at, format!("{ty} is not an enum")))?;
                    let (pattern_ty, variant) = name
                        .split_once("::")
                        .ok_or_else(|| diagnostic(at, "enum pattern needs Enum::Variant"))?;
                    if pattern_ty.split('<').next() != Some(base) {
                        return Err(diagnostic(
                            at,
                            format!("pattern {name} does not match {ty}"),
                        ));
                    }
                    let fields = def
                        .variants
                        .get(variant)
                        .ok_or_else(|| diagnostic(at, format!("unknown variant {variant}")))?;
                    let args = ty
                        .split_once('<')
                        .map(|(_, i)| split_type_args(outer_type_end(i)))
                        .unwrap_or_default();
                    let map = def
                        .type_params
                        .iter()
                        .cloned()
                        .zip(args.iter().map(|s| s.to_string()))
                        .collect::<BTreeMap<_, _>>();
                    (
                        name.as_str(),
                        fields
                            .iter()
                            .map(|(_, t)| substitute_type(t, &map))
                            .collect(),
                    )
                };
                let _ = head;
                if parts.len() != field_types.len() {
                    return Err(diagnostic(
                        at,
                        format!("pattern {name} expects {} fields", field_types.len()),
                    ));
                }
                for (part, field_ty) in parts.iter().zip(field_types) {
                    self.pattern(part, &field_ty, at, bindings)?;
                }
            }
        }
        Ok(())
    }
    fn check_match(&self, ty: &str, patterns: &[(&Pattern, bool)], at: &Tok) -> Result<()> {
        if language_at_least(&self.program.language, "1.9.0") {
            return pattern_space::check(self.program, ty, patterns, at);
        }

        fn irrefutable(pattern: &Pattern) -> bool {
            matches!(pattern, Pattern::Wildcard | Pattern::Bind(_))
                || matches!(pattern, Pattern::Variant(n, parts) if n == "$tuple" && parts.iter().all(irrefutable))
                || matches!(pattern,Pattern::Struct(n,fields) if !n.contains("::")&&fields.iter().all(|(_,p)|irrefutable(p)))
        }
        let required: Vec<String> = if ty.starts_with("Option<") {
            vec!["Some".into(), "None".into()]
        } else if ty.starts_with("Result<") {
            vec!["Ok".into(), "Err".into()]
        } else if ty == "Bool" {
            vec!["true".into(), "false".into()]
        } else if let Some(def) = self.program.enums.get(ty.split('<').next().unwrap_or(ty)) {
            def.variants.keys().cloned().collect()
        } else {
            Vec::new()
        };
        let mut covered = BTreeSet::new();
        let mut all = false;
        for (pattern, guarded) in patterns {
            let key = match pattern {
                p if irrefutable(p) => None,
                Pattern::Variant(name, parts) if parts.iter().all(irrefutable) => {
                    Some(name.split("::").last().unwrap().to_string())
                }
                Pattern::Struct(name, fields)
                    if name.contains("::") && fields.iter().all(|(_, p)| irrefutable(p)) =>
                {
                    Some(name.split("::").last().unwrap().to_string())
                }
                Pattern::Literal(Value::Bool(b)) => Some(b.to_string()),
                _ => Some(String::new()),
            };
            if all
                || key
                    .as_ref()
                    .is_some_and(|k| covered.contains(k) && !k.is_empty())
            {
                if program_v07(self.program) {
                    return Err(diagnostic(at, "unreachable match arm"));
                }
                eprintln!("warning: {}:{}: unreachable match arm", at.line, at.col);
            }
            if !guarded {
                if let Some(key) = key {
                    if !key.is_empty() {
                        covered.insert(key);
                    }
                } else {
                    all = true;
                }
            }
        }
        if !all && (required.is_empty() || required.iter().any(|v| !covered.contains(v))) {
            return Err(diagnostic(at, format!("non-exhaustive match for {ty}")));
        }
        Ok(())
    }
    fn find(&self, name: &str) -> Option<&(String, bool)> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
    fn block(&mut self, body: &[Stmt]) -> Result<()> {
        self.scopes.push(BTreeMap::new());
        for stmt in body {
            self.stmt(stmt)?;
        }
        self.scopes.pop();
        Ok(())
    }
    fn expr(&self, e: &Expr) -> Result<String> {
        Ok(match &e.kind {
            ExprKind::Value(v) => match v {
                Value::Bool(_) => "Bool",
                Value::Int(_) => "Int",
                Value::Float(_) => "Float",
                Value::Text(_) => "String",
                Value::Bytes(_) => "Bytes",
                Value::BigInt(_) => "BigInt",
                Value::Decimal(_) => "Decimal",
                Value::Regex(_) => "Regex",
                Value::CsvStream(_) => "CsvStreamState",
                Value::JsonStream(_) => "JsonStreamState",
                Value::Instant(_) => "Instant",
                Value::Duration(_) => "Duration",
                Value::NumericArray(a) => match a.dtype() {
                    rewind::numeric::DType::Float64 => "FloatArray",
                    rewind::numeric::DType::Int64 => "IntArray",
                },
                Value::FileError(_) => "FileError",
                Value::Null => "Unit",
                Value::Option(_) => "Option<Unknown>",
                Value::Result(_) => "Result<Unknown,Unknown>",
                _ => "Unknown",
            }
            .into(),
            ExprKind::Name(n) => {
                let resolved = resolve_alias(self.program, n);
                if let Some((public, origin)) = self.program.const_origins.get(&resolved) {
                    self.visible(*public, origin, &e.at, n)?;
                }
                if let Some(binding) = self.find(&resolved) {
                    if matches!(
                        self.program.language.as_str(),
                        "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) {
                        v06::borrowed_type(&binding.0).into()
                    } else {
                        binding.0.clone()
                    }
                } else if let Some(f) = self
                    .program
                    .functions
                    .get(self.program.import_aliases.get(n).unwrap_or(n))
                {
                    self.visible(f.public, &f.origin, &e.at, n)?;
                    if !f.type_params.is_empty() {
                        return Err(diagnostic(
                            &e.at,
                            "generic function requires a call for type inference",
                        ));
                    }
                    if matches!(
                        self.program.language.as_str(),
                        "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) {
                        let ty = v06::fn_type(
                            &f.params,
                            &if f.asynchronous {
                                format!("Task<{}>", f.ret)
                            } else {
                                f.ret.clone()
                            },
                            &v06::function_effects(self.program, &resolved),
                        );
                        v06::captures::infer(
                            self,
                            &ty,
                            v05::needed_globals(self.program, &resolved),
                        )
                    } else {
                        format!(
                            "fn({})->{}",
                            f.params
                                .iter()
                                .map(|(_, t)| t.as_str())
                                .collect::<Vec<_>>()
                                .join(","),
                            if f.asynchronous {
                                format!("Task<{}>", f.ret)
                            } else {
                                f.ret.clone()
                            }
                        )
                    }
                } else if let Some(ty) = enum_constructor_type(
                    self.program,
                    &resolve_alias(self.program, n),
                    &[],
                    &e.at,
                )? {
                    if let Some((base, _)) = n.split_once("::") {
                        let resolved = resolve_alias(self.program, base);
                        let def =
                            &self.program.enums[resolved.split('<').next().unwrap_or(&resolved)];
                        self.visible(def.public, &def.origin, &e.at, base)?;
                    }
                    ty
                } else {
                    return Err(
                        if matches!(
                            self.program.language.as_str(),
                            "1.1.0"
                                | "1.2.0"
                                | "1.3.0"
                                | "1.4.0"
                                | "1.5.0"
                                | "1.6.0"
                                | "1.6.1"
                                | "1.7.0"
                                | "1.7.1"
                                | "1.8.0"
                                | "1.8.1"
                                | "1.8.2"
                                | "1.8.3"
                                | "1.8.4"
                                | "1.8.5"
                                | "1.8.6"
                                | "1.8.7"
                                | "1.8.8"
                                | "1.9.0"
                                | "1.9.1"
                                | "1.9.2"
                                | "1.9.3"
                                | "1.9.4"
                                | "1.9.5"
                                | "1.9.6"
                                | "1.9.7"
                                | "1.9.8"
                                | "1.9.9"
                                | "1.9.10"
                                | "1.9.11"
                                | "1.9.12"
                                | "1.9.13"
                                | "1.9.14"
                                | "1.9.15"
                                | "1.9.16"
                                | "1.9.17"
                                | "1.9.18"
                                | "1.9.19"
                                | "1.9.20"
                                | "1.9.21"
                                | "1.9.22"
                                | "1.9.23"
                                | "1.9.24"
                                | "1.9.25"
                                | "1.9.26"
                                | "1.9.27"
                                | "1.9.28"
                                | "1.9.29"
                                | "1.9.30"
                                | "1.9.31"
                                | "1.9.32"
                                | "1.9.33"
                                | "1.9.34"
                                | "1.9.35"
                                | "1.9.36"
                                | "1.9.37"
                                | "1.9.38"
                                | "1.9.39"
                                | "1.9.40"
                                | "1.9.41"
                                | "1.9.42"
                                | "1.9.43"
                                | "1.9.44"
                                | "1.9.45"
                                | "1.9.46"
                                | "1.9.47"
                                | "1.9.48"
                                | "1.9.49"
                                | "1.9.50"
                                | "1.9.51"
                                | "2.0.0"
                        ) {
                            v11::unknown_name(
                                &e.at,
                                n,
                                self.scopes.iter().rev().flat_map(|s| s.keys().cloned()),
                            )
                        } else {
                            diagnostic(&e.at, format!("unknown name {n}"))
                        },
                    );
                }
            }
            ExprKind::Unary(op, inner) => {
                let t = self.expr(inner)?;
                if op.starts_with("$capture:") {
                    if !matches!(
                        self.program.language.as_str(),
                        "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) {
                        return Err(diagnostic(&e.at, "explicit capture requires language 0.6"));
                    }
                    return Ok(if op == "$capture:borrow" {
                        v06::captures::with_flags(&t, false, false)
                    } else {
                        t
                    });
                }
                if op == "$unsecret" {
                    return Ok(t
                        .strip_prefix("Secret<")
                        .and_then(|s| s.strip_suffix('>'))
                        .unwrap_or(&t)
                        .into());
                }
                if op == "$unfreeze" {
                    return Ok(t
                        .strip_prefix("Frozen<")
                        .and_then(|s| s.strip_suffix('>'))
                        .unwrap_or(&t)
                        .into());
                }
                if op == "spawn" || op == "await" {
                    let inner = t
                        .strip_prefix("Task<")
                        .and_then(|t| t.strip_suffix('>'))
                        .ok_or_else(|| diagnostic(&e.at, "spawn/await requires Task<T>"))?;
                    return Ok(if op == "spawn" {
                        t
                    } else {
                        format!(
                            "Result<{inner},{}>",
                            if matches!(
                                self.program.language.as_str(),
                                "0.5"
                                    | "0.6"
                                    | "0.7"
                                    | "0.8"
                                    | "0.9"
                                    | "0.9.1"
                                    | "0.9.2"
                                    | "0.9.3"
                                    | "0.9.4"
                                    | "0.9.5"
                                    | "0.9.6"
                                    | "0.9.7"
                                    | "0.9.8"
                                    | "0.9.9"
                                    | "1.0.0"
                                    | "1.1.0"
                                    | "1.2.0"
                                    | "1.3.0"
                                    | "1.4.0"
                                    | "1.5.0"
                                    | "1.6.0"
                                    | "1.6.1"
                                    | "1.7.0"
                                    | "1.7.1"
                                    | "1.8.0"
                                    | "1.8.1"
                                    | "1.8.2"
                                    | "1.8.3"
                                    | "1.8.4"
                                    | "1.8.5"
                                    | "1.8.6"
                                    | "1.8.7"
                                    | "1.8.8"
                                    | "1.9.0"
                                    | "1.9.1"
                                    | "1.9.2"
                                    | "1.9.3"
                                    | "1.9.4"
                                    | "1.9.5"
                                    | "1.9.6"
                                    | "1.9.7"
                                    | "1.9.8"
                                    | "1.9.9"
                                    | "1.9.10"
                                    | "1.9.11"
                                    | "1.9.12"
                                    | "1.9.13"
                                    | "1.9.14"
                                    | "1.9.15"
                                    | "1.9.16"
                                    | "1.9.17"
                                    | "1.9.18"
                                    | "1.9.19"
                                    | "1.9.20"
                                    | "1.9.21"
                                    | "1.9.22"
                                    | "1.9.23"
                                    | "1.9.24"
                                    | "1.9.25"
                                    | "1.9.26"
                                    | "1.9.27"
                                    | "1.9.28"
                                    | "1.9.29"
                                    | "1.9.30"
                                    | "1.9.31"
                                    | "1.9.32"
                                    | "1.9.33"
                                    | "1.9.34"
                                    | "1.9.35"
                                    | "1.9.36"
                                    | "1.9.37"
                                    | "1.9.38"
                                    | "1.9.39"
                                    | "1.9.40"
                                    | "1.9.41"
                                    | "1.9.42"
                                    | "1.9.43"
                                    | "1.9.44"
                                    | "1.9.45"
                                    | "1.9.46"
                                    | "1.9.47"
                                    | "1.9.48"
                                    | "1.9.49"
                                    | "1.9.50"
                                    | "1.9.51"
                                    | "2.0.0"
                            ) {
                                "TaskError"
                            } else {
                                "String"
                            }
                        )
                    });
                }
                if matches!(op.as_str(), "move" | "borrow" | "borrowMut") {
                    if matches!(
                        self.program.language.as_str(),
                        "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) && op != "move"
                    {
                        return Ok(format!(
                            "&{}{}",
                            if op == "borrowMut" { "mut " } else { "" },
                            t
                        ));
                    }
                    return Ok(t);
                }
                if op == "!" && t != "Bool" || op == "-" && !matches!(t.as_str(), "Int" | "Float") {
                    return Err(diagnostic(&e.at, format!("invalid operand {t} for {op}")));
                }
                t
            }
            ExprKind::Binary(op, a, b) => {
                if matches!(
                    self.program.language.as_str(),
                    "0.5"
                        | "0.6"
                        | "0.7"
                        | "0.8"
                        | "0.9"
                        | "0.9.1"
                        | "0.9.2"
                        | "0.9.3"
                        | "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) && (self.expr(a)?.starts_with("Secret<")
                    || self.expr(b)?.starts_with("Secret<"))
                {
                    if matches!(op.as_str(), "&&" | "||") {
                        return Err(diagnostic(
                            &e.at,
                            "reveal Secret<Bool> explicitly before short-circuit logic",
                        ));
                    }
                    let unwrap = |v: &Expr| Expr {
                        kind: ExprKind::Unary("$unsecret".into(), Box::new(v.clone())),
                        at: v.at.clone(),
                    };
                    let expr = Expr {
                        kind: ExprKind::Binary(
                            op.clone(),
                            Box::new(unwrap(a)),
                            Box::new(unwrap(b)),
                        ),
                        at: e.at.clone(),
                    };
                    return Ok(format!("Secret<{}>", self.expr(&expr)?));
                }
                let x = self.expr(a)?;
                let y = self.expr(b)?;
                match op.as_str() {
                    "&&" | "||" => {
                        if x != "Bool" || y != "Bool" {
                            return Err(diagnostic(
                                &e.at,
                                "logical operators require Bool operands",
                            ));
                        }
                        "Bool".into()
                    }
                    "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                        if !compatible(&x, &y) && x != "Unknown" && y != "Unknown" {
                            return Err(diagnostic(&e.at, format!("cannot compare {x} and {y}")));
                        }
                        "Bool".into()
                    }
                    "+" if x == "String" || y == "String" => "String".into(),
                    "+" | "-" | "*" | "/" | "%" => {
                        if x != y || !matches!(x.as_str(), "Int" | "Float") {
                            return Err(diagnostic(
                                &e.at,
                                format!("arithmetic requires matching numbers; found {x} and {y}"),
                            ));
                        }
                        x
                    }
                    _ => "Unknown".into(),
                }
            }
            ExprKind::Member(base, field) => {
                if let ExprKind::Name(n) = &base.kind {
                    let qualified = format!("{n}.{field}");
                    if let Some(symbol) = self
                        .program
                        .import_aliases
                        .get(&qualified)
                        .filter(|_| self.find(n).is_none())
                    {
                        if let Some((public, origin)) = self.program.const_origins.get(symbol) {
                            self.visible(*public, origin, &e.at, &qualified)?;
                            return self
                                .find(symbol)
                                .map(|b| b.0.clone())
                                .ok_or_else(|| diagnostic(&e.at, "constant is not initialized"));
                        }
                        if let Some(f) = self.program.functions.get(symbol) {
                            self.visible(f.public, &f.origin, &e.at, &qualified)?;
                            if !f.type_params.is_empty() {
                                return Err(diagnostic(
                                    &e.at,
                                    "generic function requires a call for type inference",
                                ));
                            }
                            if matches!(
                                self.program.language.as_str(),
                                "0.6"
                                    | "0.7"
                                    | "0.8"
                                    | "0.9"
                                    | "0.9.1"
                                    | "0.9.2"
                                    | "0.9.3"
                                    | "0.9.4"
                                    | "0.9.5"
                                    | "0.9.6"
                                    | "0.9.7"
                                    | "0.9.8"
                                    | "0.9.9"
                                    | "1.0.0"
                                    | "1.1.0"
                                    | "1.2.0"
                                    | "1.3.0"
                                    | "1.4.0"
                                    | "1.5.0"
                                    | "1.6.0"
                                    | "1.6.1"
                                    | "1.7.0"
                                    | "1.7.1"
                                    | "1.8.0"
                                    | "1.8.1"
                                    | "1.8.2"
                                    | "1.8.3"
                                    | "1.8.4"
                                    | "1.8.5"
                                    | "1.8.6"
                                    | "1.8.7"
                                    | "1.8.8"
                                    | "1.9.0"
                                    | "1.9.1"
                                    | "1.9.2"
                                    | "1.9.3"
                                    | "1.9.4"
                                    | "1.9.5"
                                    | "1.9.6"
                                    | "1.9.7"
                                    | "1.9.8"
                                    | "1.9.9"
                                    | "1.9.10"
                                    | "1.9.11"
                                    | "1.9.12"
                                    | "1.9.13"
                                    | "1.9.14"
                                    | "1.9.15"
                                    | "1.9.16"
                                    | "1.9.17"
                                    | "1.9.18"
                                    | "1.9.19"
                                    | "1.9.20"
                                    | "1.9.21"
                                    | "1.9.22"
                                    | "1.9.23"
                                    | "1.9.24"
                                    | "1.9.25"
                                    | "1.9.26"
                                    | "1.9.27"
                                    | "1.9.28"
                                    | "1.9.29"
                                    | "1.9.30"
                                    | "1.9.31"
                                    | "1.9.32"
                                    | "1.9.33"
                                    | "1.9.34"
                                    | "1.9.35"
                                    | "1.9.36"
                                    | "1.9.37"
                                    | "1.9.38"
                                    | "1.9.39"
                                    | "1.9.40"
                                    | "1.9.41"
                                    | "1.9.42"
                                    | "1.9.43"
                                    | "1.9.44"
                                    | "1.9.45"
                                    | "1.9.46"
                                    | "1.9.47"
                                    | "1.9.48"
                                    | "1.9.49"
                                    | "1.9.50"
                                    | "1.9.51"
                                    | "2.0.0"
                            ) {
                                return Ok(v06::fn_type(
                                    &f.params,
                                    &if f.asynchronous {
                                        format!("Task<{}>", f.ret)
                                    } else {
                                        f.ret.clone()
                                    },
                                    &v06::function_effects(self.program, symbol),
                                ));
                            }
                            return Ok(format!(
                                "fn({})->{}",
                                f.params
                                    .iter()
                                    .map(|(_, ty)| ty.as_str())
                                    .collect::<Vec<_>>()
                                    .join(","),
                                if f.asynchronous {
                                    format!("Task<{}>", f.ret)
                                } else {
                                    f.ret.clone()
                                }
                            ));
                        }
                    }
                    if matches!(
                        n.as_str(),
                        "Out"
                            | "Err"
                            | "File"
                            | "Directory"
                            | "Time"
                            | "Random"
                            | "In"
                            | "Args"
                            | "Env"
                            | "Locale"
                    ) {
                        return Ok("Builtin".into());
                    }
                }
                let t = self.expr(base)?;
                if matches!(
                    self.program.language.as_str(),
                    "0.6"
                        | "0.7"
                        | "0.8"
                        | "0.9"
                        | "0.9.1"
                        | "0.9.2"
                        | "0.9.3"
                        | "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) && t.starts_with("Secret<")
                {
                    let inner = Expr {
                        kind: ExprKind::Unary("$unsecret".into(), base.clone()),
                        at: base.at.clone(),
                    };
                    let field_expr = Expr {
                        kind: ExprKind::Member(Box::new(inner), field.clone()),
                        at: e.at.clone(),
                    };
                    return Ok(format!("Secret<{}>", self.expr(&field_expr)?));
                }
                if matches!(
                    self.program.language.as_str(),
                    "0.6"
                        | "0.7"
                        | "0.8"
                        | "0.9"
                        | "0.9.1"
                        | "0.9.2"
                        | "0.9.3"
                        | "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) {
                    if let Some(inner) = t.strip_prefix("Tuple<").and_then(|t| t.strip_suffix('>'))
                    {
                        let index = field
                            .strip_prefix('_')
                            .and_then(|n| n.parse::<usize>().ok())
                            .ok_or_else(|| diagnostic(&e.at, "tuple field must be _0, _1, ..."))?;
                        return split_type_args(inner)
                            .get(index)
                            .map(|t| t.to_string())
                            .ok_or_else(|| diagnostic(&e.at, "tuple field out of range"));
                    }
                }
                if matches!(
                    self.program.language.as_str(),
                    "0.5"
                        | "0.6"
                        | "0.7"
                        | "0.8"
                        | "0.9"
                        | "0.9.1"
                        | "0.9.2"
                        | "0.9.3"
                        | "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) && t.starts_with("Frozen<")
                {
                    let inner = Expr {
                        kind: ExprKind::Unary("$unfreeze".into(), base.clone()),
                        at: base.at.clone(),
                    };
                    let field_expr = Expr {
                        kind: ExprKind::Member(Box::new(inner), field.clone()),
                        at: e.at.clone(),
                    };
                    return Ok(v05::frozen_type_result(&self.expr(&field_expr)?));
                }
                if let Some(def) = self.program.structs.get(t.split('<').next().unwrap_or(&t)) {
                    self.field_visible(def, field, &e.at)?;
                    if self.program.strict_visibility && !def.public && def.origin != self.origin {
                        return Err(diagnostic(&e.at, format!("{t} is private to its module")));
                    }
                    let args = t
                        .split_once('<')
                        .map(|(_, inner)| split_type_args(outer_type_end(inner)))
                        .unwrap_or_default();
                    let substitutions = def
                        .type_params
                        .iter()
                        .cloned()
                        .zip(args.iter().map(|a| a.to_string()))
                        .collect::<BTreeMap<_, _>>();
                    def.fields
                        .iter()
                        .find(|(n, _)| n == field)
                        .map(|(_, t)| substitute_type(t, &substitutions))
                        .ok_or_else(|| diagnostic(&e.at, format!("unknown field {field}")))?
                } else if t == "FileError" {
                    match field.as_str() {
                        "code" | "path" => "String".into(),
                        "cause" => "Option<String>".into(),
                        "causes" => "List<String>".into(),
                        _ => {
                            return Err(diagnostic(
                                &e.at,
                                format!("FileError has no field {field}"),
                            ))
                        }
                    }
                } else if t == "FileHandle" && field == "position" {
                    "Int".into()
                } else {
                    return Err(diagnostic(&e.at, format!("type {t} has no field {field}")));
                }
            }
            ExprKind::Call(target, args) => {
                if matches!(
                    self.program.language.as_str(),
                    "0.6"
                        | "0.7"
                        | "0.8"
                        | "0.9"
                        | "0.9.1"
                        | "0.9.2"
                        | "0.9.3"
                        | "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) {
                    if let ExprKind::Member(base, method) = &target.kind {
                        if let Ok(ty) = self.expr(base) {
                            if let Some(inner) =
                                ty.strip_prefix("Secret<").and_then(|s| s.strip_suffix('>'))
                            {
                                let builtin = inner.split('<').next().unwrap_or(inner);
                                if !matches!(
                                    builtin,
                                    "Bool"
                                        | "Int"
                                        | "Float"
                                        | "String"
                                        | "Bytes"
                                        | "List"
                                        | "Map"
                                        | "Option"
                                        | "Result"
                                        | "Tuple"
                                ) || args.iter().any(|a| {
                                    self.expr(a).is_ok_and(|t| function_signature(&t).is_some())
                                }) {
                                    return Err(diagnostic(
                                        &e.at,
                                        "Secret callbacks and user methods require explicit reveal",
                                    ));
                                }
                                let call = Expr {
                                    kind: ExprKind::Call(
                                        Box::new(Expr {
                                            kind: ExprKind::Member(
                                                Box::new(Expr {
                                                    kind: ExprKind::Unary(
                                                        "$unsecret".into(),
                                                        base.clone(),
                                                    ),
                                                    at: base.at.clone(),
                                                }),
                                                method.clone(),
                                            ),
                                            at: target.at.clone(),
                                        }),
                                        args.clone(),
                                    ),
                                    at: e.at.clone(),
                                };
                                return Ok(format!("Secret<{}>", self.expr(&call)?));
                            }
                        }
                    }
                }
                if matches!(
                    self.program.language.as_str(),
                    "0.5"
                        | "0.6"
                        | "0.7"
                        | "0.8"
                        | "0.9"
                        | "0.9.1"
                        | "0.9.2"
                        | "0.9.3"
                        | "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) {
                    if let ExprKind::Member(base, method) = &target.kind {
                        if self.expr(base).is_ok_and(|t| t.starts_with("Frozen<")) {
                            if !matches!(
                                method.as_str(),
                                "len" | "get" | "keys" | "eq" | "cmp" | "hash" | "display" | "iter"
                            ) {
                                return Err(diagnostic(&e.at, "cannot mutate Frozen<T>; use thaw"));
                            }
                            let view = Expr {
                                kind: ExprKind::Unary("$unfreeze".into(), base.clone()),
                                at: base.at.clone(),
                            };
                            let call = Expr {
                                kind: ExprKind::Call(
                                    Box::new(Expr {
                                        kind: ExprKind::Member(Box::new(view), method.clone()),
                                        at: target.at.clone(),
                                    }),
                                    args.clone(),
                                ),
                                at: e.at.clone(),
                            };
                            let ty = self.expr(&call)?;
                            return Ok(if method == "iter" {
                                ty
                            } else {
                                v05::frozen_type_result(&ty)
                            });
                        }
                    }
                }
                let types = args
                    .iter()
                    .map(|a| self.expr(a))
                    .collect::<Result<Vec<_>>>()?;
                if matches!(&target.kind,ExprKind::Name(n) if n=="$tuple") {
                    if !matches!(
                        self.program.language.as_str(),
                        "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) {
                        return Err(diagnostic(&e.at, "tuple values require language 0.6"));
                    }
                    if types.iter().any(|t| t.starts_with('&')) {
                        return Err(diagnostic(&e.at, "tuple cannot contain a borrow"));
                    }
                    return Ok(format!("Tuple<{}>", types.join(",")));
                }
                if let ExprKind::Member(base, method) = &target.kind {
                    let special = matches!(&base.kind,ExprKind::Name(n) if self.find(n).is_none() && (self.program.import_aliases.keys().any(|alias| alias.starts_with(&format!("{n}."))) || matches!(n.as_str(),"Out"|"Err"|"File"|"Directory"|"Time"|"Random"|"Args"|"Env"|"Locale"|"In")));
                    if !special {
                        let ty = self.expr(base)?;
                        if matches!(
                            self.program.language.as_str(),
                            "0.6"
                                | "0.7"
                                | "0.8"
                                | "0.9"
                                | "0.9.1"
                                | "0.9.2"
                                | "0.9.3"
                                | "0.9.4"
                                | "0.9.5"
                                | "0.9.6"
                                | "0.9.7"
                                | "0.9.8"
                                | "0.9.9"
                                | "1.0.0"
                                | "1.1.0"
                                | "1.2.0"
                                | "1.3.0"
                                | "1.4.0"
                                | "1.5.0"
                                | "1.6.0"
                                | "1.6.1"
                                | "1.7.0"
                                | "1.7.1"
                                | "1.8.0"
                                | "1.8.1"
                                | "1.8.2"
                                | "1.8.3"
                                | "1.8.4"
                                | "1.8.5"
                                | "1.8.6"
                                | "1.8.7"
                                | "1.8.8"
                                | "1.9.0"
                                | "1.9.1"
                                | "1.9.2"
                                | "1.9.3"
                                | "1.9.4"
                                | "1.9.5"
                                | "1.9.6"
                                | "1.9.7"
                                | "1.9.8"
                                | "1.9.9"
                                | "1.9.10"
                                | "1.9.11"
                                | "1.9.12"
                                | "1.9.13"
                                | "1.9.14"
                                | "1.9.15"
                                | "1.9.16"
                                | "1.9.17"
                                | "1.9.18"
                                | "1.9.19"
                                | "1.9.20"
                                | "1.9.21"
                                | "1.9.22"
                                | "1.9.23"
                                | "1.9.24"
                                | "1.9.25"
                                | "1.9.26"
                                | "1.9.27"
                                | "1.9.28"
                                | "1.9.29"
                                | "1.9.30"
                                | "1.9.31"
                                | "1.9.32"
                                | "1.9.33"
                                | "1.9.34"
                                | "1.9.35"
                                | "1.9.36"
                                | "1.9.37"
                                | "1.9.38"
                                | "1.9.39"
                                | "1.9.40"
                                | "1.9.41"
                                | "1.9.42"
                                | "1.9.43"
                                | "1.9.44"
                                | "1.9.45"
                                | "1.9.46"
                                | "1.9.47"
                                | "1.9.48"
                                | "1.9.49"
                                | "1.9.50"
                                | "1.9.51"
                                | "2.0.0"
                        ) {
                            if let Some(result) =
                                v06::checker_method(self, &ty, method, &types, &e.at)?
                            {
                                return Ok(result);
                            }
                        }
                        if matches!(
                            self.program.language.as_str(),
                            "0.5"
                                | "0.6"
                                | "0.7"
                                | "0.8"
                                | "0.9"
                                | "0.9.1"
                                | "0.9.2"
                                | "0.9.3"
                                | "0.9.4"
                                | "0.9.5"
                                | "0.9.6"
                                | "0.9.7"
                                | "0.9.8"
                                | "0.9.9"
                                | "1.0.0"
                                | "1.1.0"
                                | "1.2.0"
                                | "1.3.0"
                                | "1.4.0"
                                | "1.5.0"
                                | "1.6.0"
                                | "1.6.1"
                                | "1.7.0"
                                | "1.7.1"
                                | "1.8.0"
                                | "1.8.1"
                                | "1.8.2"
                                | "1.8.3"
                                | "1.8.4"
                                | "1.8.5"
                                | "1.8.6"
                                | "1.8.7"
                                | "1.8.8"
                                | "1.9.0"
                                | "1.9.1"
                                | "1.9.2"
                                | "1.9.3"
                                | "1.9.4"
                                | "1.9.5"
                                | "1.9.6"
                                | "1.9.7"
                                | "1.9.8"
                                | "1.9.9"
                                | "1.9.10"
                                | "1.9.11"
                                | "1.9.12"
                                | "1.9.13"
                                | "1.9.14"
                                | "1.9.15"
                                | "1.9.16"
                                | "1.9.17"
                                | "1.9.18"
                                | "1.9.19"
                                | "1.9.20"
                                | "1.9.21"
                                | "1.9.22"
                                | "1.9.23"
                                | "1.9.24"
                                | "1.9.25"
                                | "1.9.26"
                                | "1.9.27"
                                | "1.9.28"
                                | "1.9.29"
                                | "1.9.30"
                                | "1.9.31"
                                | "1.9.32"
                                | "1.9.33"
                                | "1.9.34"
                                | "1.9.35"
                                | "1.9.36"
                                | "1.9.37"
                                | "1.9.38"
                                | "1.9.39"
                                | "1.9.40"
                                | "1.9.41"
                                | "1.9.42"
                                | "1.9.43"
                                | "1.9.44"
                                | "1.9.45"
                                | "1.9.46"
                                | "1.9.47"
                                | "1.9.48"
                                | "1.9.49"
                                | "1.9.50"
                                | "1.9.51"
                                | "2.0.0"
                        ) {
                            if method == "iter" && types.is_empty() {
                                if let Some(item) = v05::iterator_item(&ty) {
                                    return Ok(format!("Iterator<{item}>"));
                                }
                            }
                            if method == "next" && types.is_empty() && ty.starts_with("Iterator<") {
                                return Ok(format!(
                                    "Option<{}>",
                                    outer_type_end(ty.strip_prefix("Iterator<").unwrap())
                                ));
                            }
                        }
                        if let Some(inner) = ty
                            .strip_prefix("Channel<")
                            .and_then(|t| t.strip_suffix('>'))
                        {
                            return Ok(match (method.as_str(), types.as_slice()) {
                                ("send", [actual]) if compatible(inner, actual) => {
                                    "Task<Unit>".into()
                                }
                                ("receive", []) => format!("Task<{inner}>"),
                                ("close", []) => "Unit".into(),
                                _ => {
                                    return Err(diagnostic(
                                        &e.at,
                                        "invalid Channel method arguments",
                                    ))
                                }
                            });
                        }
                        if ty.starts_with("Task<") {
                            if method == "selectReady"
                                && language_at_least(&self.program.language, "1.9.18")
                                && types.len() == 1
                                && types[0].starts_with("Task<")
                            {
                                return Ok("Task<Int>".into());
                            }
                            if matches!(
                                self.program.language.as_str(),
                                "0.6"
                                    | "0.7"
                                    | "0.8"
                                    | "0.9"
                                    | "0.9.1"
                                    | "0.9.2"
                                    | "0.9.3"
                                    | "0.9.4"
                                    | "0.9.5"
                                    | "0.9.6"
                                    | "0.9.7"
                                    | "0.9.8"
                                    | "0.9.9"
                                    | "1.0.0"
                                    | "1.1.0"
                                    | "1.2.0"
                                    | "1.3.0"
                                    | "1.4.0"
                                    | "1.5.0"
                                    | "1.6.0"
                                    | "1.6.1"
                                    | "1.7.0"
                                    | "1.7.1"
                                    | "1.8.0"
                                    | "1.8.1"
                                    | "1.8.2"
                                    | "1.8.3"
                                    | "1.8.4"
                                    | "1.8.5"
                                    | "1.8.6"
                                    | "1.8.7"
                                    | "1.8.8"
                                    | "1.9.0"
                                    | "1.9.1"
                                    | "1.9.2"
                                    | "1.9.3"
                                    | "1.9.4"
                                    | "1.9.5"
                                    | "1.9.6"
                                    | "1.9.7"
                                    | "1.9.8"
                                    | "1.9.9"
                                    | "1.9.10"
                                    | "1.9.11"
                                    | "1.9.12"
                                    | "1.9.13"
                                    | "1.9.14"
                                    | "1.9.15"
                                    | "1.9.16"
                                    | "1.9.17"
                                    | "1.9.18"
                                    | "1.9.19"
                                    | "1.9.20"
                                    | "1.9.21"
                                    | "1.9.22"
                                    | "1.9.23"
                                    | "1.9.24"
                                    | "1.9.25"
                                    | "1.9.26"
                                    | "1.9.27"
                                    | "1.9.28"
                                    | "1.9.29"
                                    | "1.9.30"
                                    | "1.9.31"
                                    | "1.9.32"
                                    | "1.9.33"
                                    | "1.9.34"
                                    | "1.9.35"
                                    | "1.9.36"
                                    | "1.9.37"
                                    | "1.9.38"
                                    | "1.9.39"
                                    | "1.9.40"
                                    | "1.9.41"
                                    | "1.9.42"
                                    | "1.9.43"
                                    | "1.9.44"
                                    | "1.9.45"
                                    | "1.9.46"
                                    | "1.9.47"
                                    | "1.9.48"
                                    | "1.9.49"
                                    | "1.9.50"
                                    | "1.9.51"
                                    | "2.0.0"
                            ) {
                                if method == "timeout" && types == ["Int"] {
                                    return Ok(ty);
                                }
                                if method == "select"
                                    && types.len() == 1
                                    && compatible(&ty, &types[0])
                                {
                                    let inner = ty
                                        .strip_prefix("Task<")
                                        .unwrap()
                                        .strip_suffix('>')
                                        .unwrap();
                                    return Ok(format!(
                                        "Task<Tuple<Int,Result<{inner},TaskError>>>"
                                    ));
                                }
                            }
                            if matches!(
                                self.program.language.as_str(),
                                "0.6"
                                    | "0.7"
                                    | "0.8"
                                    | "0.9"
                                    | "0.9.1"
                                    | "0.9.2"
                                    | "0.9.3"
                                    | "0.9.4"
                                    | "0.9.5"
                                    | "0.9.6"
                                    | "0.9.7"
                                    | "0.9.8"
                                    | "0.9.9"
                                    | "1.0.0"
                                    | "1.1.0"
                                    | "1.2.0"
                                    | "1.3.0"
                                    | "1.4.0"
                                    | "1.5.0"
                                    | "1.6.0"
                                    | "1.6.1"
                                    | "1.7.0"
                                    | "1.7.1"
                                    | "1.8.0"
                                    | "1.8.1"
                                    | "1.8.2"
                                    | "1.8.3"
                                    | "1.8.4"
                                    | "1.8.5"
                                    | "1.8.6"
                                    | "1.8.7"
                                    | "1.8.8"
                                    | "1.9.0"
                                    | "1.9.1"
                                    | "1.9.2"
                                    | "1.9.3"
                                    | "1.9.4"
                                    | "1.9.5"
                                    | "1.9.6"
                                    | "1.9.7"
                                    | "1.9.8"
                                    | "1.9.9"
                                    | "1.9.10"
                                    | "1.9.11"
                                    | "1.9.12"
                                    | "1.9.13"
                                    | "1.9.14"
                                    | "1.9.15"
                                    | "1.9.16"
                                    | "1.9.17"
                                    | "1.9.18"
                                    | "1.9.19"
                                    | "1.9.20"
                                    | "1.9.21"
                                    | "1.9.22"
                                    | "1.9.23"
                                    | "1.9.24"
                                    | "1.9.25"
                                    | "1.9.26"
                                    | "1.9.27"
                                    | "1.9.28"
                                    | "1.9.29"
                                    | "1.9.30"
                                    | "1.9.31"
                                    | "1.9.32"
                                    | "1.9.33"
                                    | "1.9.34"
                                    | "1.9.35"
                                    | "1.9.36"
                                    | "1.9.37"
                                    | "1.9.38"
                                    | "1.9.39"
                                    | "1.9.40"
                                    | "1.9.41"
                                    | "1.9.42"
                                    | "1.9.43"
                                    | "1.9.44"
                                    | "1.9.45"
                                    | "1.9.46"
                                    | "1.9.47"
                                    | "1.9.48"
                                    | "1.9.49"
                                    | "1.9.50"
                                    | "1.9.51"
                                    | "2.0.0"
                            ) && types.is_empty()
                            {
                                if method == "isDone"
                                    && language_at_least(&self.program.language, "1.6.0")
                                {
                                    return Ok("Bool".into());
                                }
                                if method == "requestCancel" {
                                    return Ok("Unit".into());
                                }
                                if method == "cancelAndJoin" {
                                    return Ok(ty);
                                }
                            }
                            return Ok(match (method.as_str(), types.as_slice()) {
                                ("cancel" | "ignore" | "detach", []) => "Unit".into(),
                                ("setPriority", [t]) if t == "Int" => "Unit".into(),
                                _ => {
                                    return Err(diagnostic(&e.at, "invalid Task method arguments"))
                                }
                            });
                        }
                        if ty == "TaskGroup" {
                            return Ok(match (method.as_str(), types.as_slice()) {
                                ("add", [t]) if t.starts_with("Task<") => "Unit".into(),
                                ("join", []) => "Task<Unit>".into(),
                                ("cancel", []) => "Unit".into(),
                                _ => {
                                    return Err(diagnostic(
                                        &e.at,
                                        "invalid TaskGroup method arguments",
                                    ))
                                }
                            });
                        }
                    }
                }
                if matches!(&target.kind,ExprKind::Name(n) if n=="property" || n=="propertyCandidates" && program_v09(self.program))
                    && program_v07(self.program)
                {
                    if types.len() != 5 || types[0] != "Int" || types[1] != "Int" {
                        return Err(diagnostic(
                            &e.at,
                            "property requires seed, cases, generator, shrinker, predicate",
                        ));
                    }
                    let (params, ty) = function_signature(&types[2]).ok_or_else(|| {
                        diagnostic(&e.at, "property requires a generator function")
                    })?;
                    if params != vec!["Int".to_string()]
                        || !v05::transfer_bounded(self.program, &ty, true, &self.bounds)
                    {
                        return Err(diagnostic(&e.at, "property input must be Share"));
                    }
                    for (actual, expected) in [
                        (&types[2], format!("fn(Int)->{ty}!{{}}~{{Send+Share}}")),
                        (
                            &types[3],
                            format!(
                                "fn({ty})->{}!{{}}~{{Send+Share}}",
                                if matches!(&target.kind,ExprKind::Name(n) if n=="propertyCandidates")
                                {
                                    format!("Frozen<List<{ty}>>")
                                } else {
                                    ty.clone()
                                }
                            ),
                        ),
                        (&types[4], format!("fn({ty})->Bool!{{}}~{{Send+Share}}")),
                    ] {
                        if !compatible(&expected, actual) {
                            return Err(diagnostic(&e.at,"property callbacks must be pure Share functions with matching types"));
                        }
                    }
                    return Ok(format!("Result<Unit,PropertyCase<{ty}>>"));
                }
                if matches!(&target.kind, ExprKind::Name(n) if n == "propertyInt")
                    && matches!(
                        self.program.language.as_str(),
                        "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    )
                {
                    if types.len() != 5 || types[..4].iter().any(|t| t != "Int") {
                        return Err(diagnostic(
                            &e.at,
                            "propertyInt requires seed, cases, min, max, and predicate",
                        ));
                    }
                    if function_signature(&types[4]) != Some((vec!["Int".into()], "Bool".into()))
                        || !v06::type_effects(&types[4]).is_empty()
                    {
                        return Err(diagnostic(
                            &e.at,
                            "propertyInt predicate must be fn(Int)->Bool effects {}",
                        ));
                    }
                    return Ok("Result<Unit,PropertyFailure>".into());
                }
                if let ExprKind::Name(name) = &target.kind {
                    if let Some(ty) = v092::call_type(self.program, name, &types, &e.at)? {
                        return Ok(ty);
                    }
                    if let Some(ty) = v091::call_type(self.program, name, &types, &e.at)? {
                        return Ok(ty);
                    }
                    if matches!(
                        self.program.language.as_str(),
                        "0.5"
                            | "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) && matches!(name.as_str(), "secret" | "reveal")
                    {
                        if types.len() != 1 {
                            return Err(diagnostic(&e.at, "secret/reveal requires one argument"));
                        }
                        return if name == "secret" {
                            Ok(format!("Secret<{}>", types[0]))
                        } else {
                            types[0]
                                .strip_prefix("Secret<")
                                .and_then(|s| s.strip_suffix('>'))
                                .map(str::to_string)
                                .ok_or_else(|| diagnostic(&e.at, "reveal requires Secret<T>"))
                        };
                    }
                    if matches!(
                        self.program.language.as_str(),
                        "0.5"
                            | "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) && matches!(name.as_str(), "freeze" | "thaw")
                    {
                        if types.len() != 1 {
                            return Err(diagnostic(&e.at, "freeze/thaw requires one argument"));
                        }
                        return if name == "freeze" {
                            if v05::contains_native_type(
                                self.program,
                                &types[0],
                                &mut BTreeSet::new(),
                            ) || !v05::transfer_bounded(
                                self.program,
                                &types[0],
                                false,
                                &self.bounds,
                            ) || types[0].contains("fn(")
                            {
                                return Err(diagnostic(&e.at,"freeze requires snapshot values; resources and function values cannot be frozen"));
                            }
                            Ok(format!("Frozen<{}>", types[0]))
                        } else {
                            types[0]
                                .strip_prefix("Frozen<")
                                .and_then(|t| t.strip_suffix('>'))
                                .map(str::to_string)
                                .ok_or_else(|| diagnostic(&e.at, "thaw requires Frozen<T>"))
                        };
                    }
                    if let Some((params, ret)) =
                        self.find(name).and_then(|(ty, _)| function_signature(ty))
                    {
                        if params.len() != types.len()
                            || params.iter().zip(&types).any(|(p, a)| !compatible(p, a))
                        {
                            return Err(diagnostic(
                                &e.at,
                                format!("invalid arguments for function value {name}"),
                            ));
                        }
                        return Ok(ret);
                    }
                    if let Some(ty) = enum_constructor_type(
                        self.program,
                        &resolve_alias(self.program, name),
                        &types,
                        &e.at,
                    )? {
                        if let Some((base, _)) = name.split_once("::") {
                            let resolved = resolve_alias(self.program, base);
                            let def = &self.program.enums
                                [resolved.split('<').next().unwrap_or(&resolved)];
                            self.visible(def.public, &def.origin, &e.at, base)?;
                        }
                        return Ok(ty);
                    }
                    let resolved_call = resolve_alias(self.program, name);
                    if let Some(f) = self
                        .program
                        .functions
                        .get(resolved_call.split('<').next().unwrap_or(&resolved_call))
                    {
                        self.visible(
                            f.public,
                            &f.origin,
                            &e.at,
                            name.split('<').next().unwrap_or(name),
                        )?;
                        let params = f
                            .type_params
                            .iter()
                            .map(|(n, _)| n.clone())
                            .collect::<Vec<_>>();
                        let substitutions = infer_call_arguments(
                            &resolved_call,
                            &f.params,
                            &params,
                            &types,
                            &e.at,
                        )?;
                        for (param, bound) in &f.type_params {
                            if let Some(bound) = bound {
                                if !trait_satisfied(self.program, bound, &substitutions[param])
                                    && !bound_provided(&self.bounds, &substitutions[param], bound)
                                {
                                    return Err(diagnostic(
                                        &e.at,
                                        format!(
                                            "{} does not implement {bound}",
                                            substitutions[param]
                                        ),
                                    ));
                                }
                            }
                        }
                        let ret = substitute_type(&f.ret, &substitutions);
                        if f.asynchronous {
                            format!("Task<{ret}>")
                        } else {
                            ret
                        }
                    } else if let Some(s) = self.program.structs.get(
                        self.program
                            .import_aliases
                            .get(name)
                            .map(String::as_str)
                            .unwrap_or_else(|| name.split('<').next().unwrap_or(name)),
                    ) {
                        self.visible(s.public, &s.origin, &e.at, name)?;
                        self.constructor_visible(s, &e.at)?;
                        let actual_name = self
                            .program
                            .import_aliases
                            .get(name)
                            .map(String::as_str)
                            .unwrap_or(name);
                        let params = s.type_params.clone();
                        let substitutions = infer_arguments(&s.fields, &params, &types, &e.at)?;
                        v09::record_arguments(
                            self.program,
                            s,
                            &substitutions,
                            &self.bounds,
                            &e.at,
                        )?;
                        if let Some((_, explicit)) = name.split_once('<') {
                            let explicit = split_type_args(outer_type_end(explicit));
                            if explicit.len() != params.len()
                                || params
                                    .iter()
                                    .zip(explicit)
                                    .any(|(p, t)| substitutions[p] != t)
                            {
                                return Err(diagnostic(
                                    &e.at,
                                    "explicit type arguments disagree with constructor arguments",
                                ));
                            }
                        }
                        if params.is_empty() {
                            actual_name.into()
                        } else {
                            format!(
                                "{}<{}>",
                                actual_name.split('<').next().unwrap(),
                                params
                                    .iter()
                                    .map(|p| substitutions[p].as_str())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            )
                        }
                    } else {
                        match name.as_str() {
                            "List" => "List".into(),
                            "Map" => "Map".into(),
                            "Bytes" => "Bytes".into(),
                            "Ok" if types.len() == 1 => format!("Result<{},Unknown>", types[0]),
                            "Err" if types.len() == 1 => format!("Result<Unknown,{}>", types[0]),
                            "Some" if types.len() == 1 => format!("Option<{}>", types[0]),
                            "assert" if types == ["Bool"] => "Unit".into(),
                            "assert_eq" if types.len() == 2 && compatible(&types[0], &types[1]) => {
                                "Unit".into()
                            }
                            "panic" if types == ["String"] => "Never".into(),
                            "TaskGroup" if types.is_empty() => "TaskGroup".into(),
                            _ if name.starts_with("Channel<") && types == ["Int"] => name.clone(),
                            _ if name.starts_with("List<") || name.starts_with("Map<") => {
                                if !types.is_empty() {
                                    return Err(diagnostic(
                                        &e.at,
                                        "collection constructor takes no arguments",
                                    ));
                                }
                                if let Some(inner) =
                                    name.strip_prefix("Map<").and_then(|s| s.strip_suffix('>'))
                                {
                                    let parts = split_type_args(inner);
                                    if parts.len() != 2
                                        || !trait_satisfied(self.program, "Ord", parts[0])
                                    {
                                        return Err(diagnostic(
                                            &e.at,
                                            "Map key type must implement Ord",
                                        ));
                                    }
                                }
                                name.clone()
                            }
                            _ if matches!(
                                self.program.language.as_str(),
                                "1.1.0"
                                    | "1.2.0"
                                    | "1.3.0"
                                    | "1.4.0"
                                    | "1.5.0"
                                    | "1.6.0"
                                    | "1.6.1"
                                    | "1.7.0"
                                    | "1.7.1"
                                    | "1.8.0"
                                    | "1.8.1"
                                    | "1.8.2"
                                    | "1.8.3"
                                    | "1.8.4"
                                    | "1.8.5"
                                    | "1.8.6"
                                    | "1.8.7"
                                    | "1.8.8"
                                    | "1.9.0"
                                    | "1.9.1"
                                    | "1.9.2"
                                    | "1.9.3"
                                    | "1.9.4"
                                    | "1.9.5"
                                    | "1.9.6"
                                    | "1.9.7"
                                    | "1.9.8"
                                    | "1.9.9"
                                    | "1.9.10"
                                    | "1.9.11"
                                    | "1.9.12"
                                    | "1.9.13"
                                    | "1.9.14"
                                    | "1.9.15"
                                    | "1.9.16"
                                    | "1.9.17"
                                    | "1.9.18"
                                    | "1.9.19"
                                    | "1.9.20"
                                    | "1.9.21"
                                    | "1.9.22"
                                    | "1.9.23"
                                    | "1.9.24"
                                    | "1.9.25"
                                    | "1.9.26"
                                    | "1.9.27"
                                    | "1.9.28"
                                    | "1.9.29"
                                    | "1.9.30"
                                    | "1.9.31"
                                    | "1.9.32"
                                    | "1.9.33"
                                    | "1.9.34"
                                    | "1.9.35"
                                    | "1.9.36"
                                    | "1.9.37"
                                    | "1.9.38"
                                    | "1.9.39"
                                    | "1.9.40"
                                    | "1.9.41"
                                    | "1.9.42"
                                    | "1.9.43"
                                    | "1.9.44"
                                    | "1.9.45"
                                    | "1.9.46"
                                    | "1.9.47"
                                    | "1.9.48"
                                    | "1.9.49"
                                    | "1.9.50"
                                    | "1.9.51"
                                    | "2.0.0"
                            ) && matches!(
                                name.as_str(),
                                "assert"
                                    | "assert_eq"
                                    | "panic"
                                    | "Ok"
                                    | "Err"
                                    | "Some"
                                    | "TaskGroup"
                            ) =>
                            {
                                let expected = match name.as_str() {
                                    "assert" => "assert(Bool)",
                                    "assert_eq" => "assert_eq(T, T) with compatible argument types",
                                    "panic" => "panic(String)",
                                    "TaskGroup" => "TaskGroup()",
                                    "Ok" => "Ok(T)",
                                    "Err" => "Err(E)",
                                    _ => "Some(T)",
                                };
                                return Err(diagnostic(
                                    &e.at,
                                    format!(
                                        "InvalidArguments: expected {expected}, found ({})",
                                        types.join(", ")
                                    ),
                                ));
                            }
                            _ => return Err(diagnostic(&e.at, format!("unknown function {name}"))),
                        }
                    }
                } else if let ExprKind::Member(base, method) = &target.kind {
                    if let ExprKind::Name(n) = &base.kind {
                        let qualified = format!("{n}.{method}");
                        if let Some(symbol) = self
                            .program
                            .import_aliases
                            .get(&qualified)
                            .filter(|_| self.find(n).is_none())
                        {
                            if let Some(f) = self.program.functions.get(symbol) {
                                self.visible(f.public, &f.origin, &e.at, &qualified)?;
                                let params = f
                                    .type_params
                                    .iter()
                                    .map(|(n, _)| n.clone())
                                    .collect::<Vec<_>>();
                                let substitutions =
                                    infer_arguments(&f.params, &params, &types, &e.at)?;
                                for (param, bound) in &f.type_params {
                                    if let Some(bound) = bound {
                                        if !trait_satisfied(
                                            self.program,
                                            bound,
                                            &substitutions[param],
                                        ) && !bound_provided(
                                            &self.bounds,
                                            &substitutions[param],
                                            bound,
                                        ) {
                                            return Err(diagnostic(
                                                &e.at,
                                                format!(
                                                    "{} does not implement {bound}",
                                                    substitutions[param]
                                                ),
                                            ));
                                        }
                                    }
                                }
                                let ret = substitute_type(&f.ret, &substitutions);
                                return Ok(if f.asynchronous {
                                    format!("Task<{ret}>")
                                } else {
                                    ret
                                });
                            }
                            if let Some(s) = self.program.structs.get(symbol) {
                                self.visible(s.public, &s.origin, &e.at, &qualified)?;
                                self.constructor_visible(s, &e.at)?;
                                let substitutions =
                                    infer_arguments(&s.fields, &s.type_params, &types, &e.at)?;
                                v09::record_arguments(
                                    self.program,
                                    s,
                                    &substitutions,
                                    &self.bounds,
                                    &e.at,
                                )?;
                                return Ok(if s.type_params.is_empty() {
                                    symbol.clone()
                                } else {
                                    format!(
                                        "{symbol}<{}>",
                                        s.type_params
                                            .iter()
                                            .map(|p| substitutions[p].as_str())
                                            .collect::<Vec<_>>()
                                            .join(",")
                                    )
                                });
                            }
                        }
                        if matches!(
                            n.as_str(),
                            "Out"
                                | "Err"
                                | "File"
                                | "Directory"
                                | "Time"
                                | "Random"
                                | "In"
                                | "Args"
                                | "Env"
                                | "Locale"
                        ) {
                            {
                                if !matches!(
                                    self.program.language.as_str(),
                                    "0.6"
                                        | "0.7"
                                        | "0.8"
                                        | "0.9"
                                        | "0.9.1"
                                        | "0.9.2"
                                        | "0.9.3"
                                        | "0.9.4"
                                        | "0.9.5"
                                        | "0.9.6"
                                        | "0.9.7"
                                        | "0.9.8"
                                        | "0.9.9"
                                        | "1.0.0"
                                        | "1.1.0"
                                        | "1.2.0"
                                        | "1.3.0"
                                        | "1.4.0"
                                        | "1.5.0"
                                        | "1.6.0"
                                        | "1.6.1"
                                        | "1.7.0"
                                        | "1.7.1"
                                        | "1.8.0"
                                        | "1.8.1"
                                        | "1.8.2"
                                        | "1.8.3"
                                        | "1.8.4"
                                        | "1.8.5"
                                        | "1.8.6"
                                        | "1.8.7"
                                        | "1.8.8"
                                        | "1.9.0"
                                        | "1.9.1"
                                        | "1.9.2"
                                        | "1.9.3"
                                        | "1.9.4"
                                        | "1.9.5"
                                        | "1.9.6"
                                        | "1.9.7"
                                        | "1.9.8"
                                        | "1.9.9"
                                        | "1.9.10"
                                        | "1.9.11"
                                        | "1.9.12"
                                        | "1.9.13"
                                        | "1.9.14"
                                        | "1.9.15"
                                        | "1.9.16"
                                        | "1.9.17"
                                        | "1.9.18"
                                        | "1.9.19"
                                        | "1.9.20"
                                        | "1.9.21"
                                        | "1.9.22"
                                        | "1.9.23"
                                        | "1.9.24"
                                        | "1.9.25"
                                        | "1.9.26"
                                        | "1.9.27"
                                        | "1.9.28"
                                        | "1.9.29"
                                        | "1.9.30"
                                        | "1.9.31"
                                        | "1.9.32"
                                        | "1.9.33"
                                        | "1.9.34"
                                        | "1.9.35"
                                        | "1.9.36"
                                        | "1.9.37"
                                        | "1.9.38"
                                        | "1.9.39"
                                        | "1.9.40"
                                        | "1.9.41"
                                        | "1.9.42"
                                        | "1.9.43"
                                        | "1.9.44"
                                        | "1.9.45"
                                        | "1.9.46"
                                        | "1.9.47"
                                        | "1.9.48"
                                        | "1.9.49"
                                        | "1.9.50"
                                        | "1.9.51"
                                        | "2.0.0"
                                ) && matches!(
                                    (n.as_str(), method.as_str()),
                                    ("In", "readSecretLine") | ("Env", "getSecret")
                                ) {
                                    return Err(diagnostic(
                                        &e.at,
                                        "secret observations require language 0.6",
                                    ));
                                }
                                if !matches!(
                                    self.program.language.as_str(),
                                    "0.9.4"
                                        | "0.9.5"
                                        | "0.9.6"
                                        | "0.9.7"
                                        | "0.9.8"
                                        | "0.9.9"
                                        | "1.0.0"
                                        | "1.1.0"
                                        | "1.2.0"
                                        | "1.3.0"
                                        | "1.4.0"
                                        | "1.5.0"
                                        | "1.6.0"
                                        | "1.6.1"
                                        | "1.7.0"
                                        | "1.7.1"
                                        | "1.8.0"
                                        | "1.8.1"
                                        | "1.8.2"
                                        | "1.8.3"
                                        | "1.8.4"
                                        | "1.8.5"
                                        | "1.8.6"
                                        | "1.8.7"
                                        | "1.8.8"
                                        | "1.9.0"
                                        | "1.9.1"
                                        | "1.9.2"
                                        | "1.9.3"
                                        | "1.9.4"
                                        | "1.9.5"
                                        | "1.9.6"
                                        | "1.9.7"
                                        | "1.9.8"
                                        | "1.9.9"
                                        | "1.9.10"
                                        | "1.9.11"
                                        | "1.9.12"
                                        | "1.9.13"
                                        | "1.9.14"
                                        | "1.9.15"
                                        | "1.9.16"
                                        | "1.9.17"
                                        | "1.9.18"
                                        | "1.9.19"
                                        | "1.9.20"
                                        | "1.9.21"
                                        | "1.9.22"
                                        | "1.9.23"
                                        | "1.9.24"
                                        | "1.9.25"
                                        | "1.9.26"
                                        | "1.9.27"
                                        | "1.9.28"
                                        | "1.9.29"
                                        | "1.9.30"
                                        | "1.9.31"
                                        | "1.9.32"
                                        | "1.9.33"
                                        | "1.9.34"
                                        | "1.9.35"
                                        | "1.9.36"
                                        | "1.9.37"
                                        | "1.9.38"
                                        | "1.9.39"
                                        | "1.9.40"
                                        | "1.9.41"
                                        | "1.9.42"
                                        | "1.9.43"
                                        | "1.9.44"
                                        | "1.9.45"
                                        | "1.9.46"
                                        | "1.9.47"
                                        | "1.9.48"
                                        | "1.9.49"
                                        | "1.9.50"
                                        | "1.9.51"
                                        | "2.0.0"
                                ) && matches!(
                                    (n.as_str(), method.as_str()),
                                    ("In", "readChunk") | ("Out", "writeBytes")
                                ) {
                                    return Err(diagnostic(
                                        &e.at,
                                        "byte I/O requires language 0.9.4",
                                    ));
                                }
                                builtin_type(n, method, &types, &e.at)?
                            }
                        } else {
                            let mut t = self.expr(base)?;
                            if let Some(inner) =
                                t.strip_prefix("Frozen<").and_then(|s| s.strip_suffix('>'))
                            {
                                if matches!(
                                    method.as_str(),
                                    "add"
                                        | "push"
                                        | "set"
                                        | "remove"
                                        | "write"
                                        | "writeBytes"
                                        | "seek"
                                        | "close"
                                ) {
                                    return Err(diagnostic(&e.at,"cannot mutate Frozen<T>; use thaw for an independent mutable copy"));
                                }
                                t = inner.into();
                            }
                            if t == "FileHandle" {
                                let expected = match (method.as_str(), types.as_slice()) {
                                    ("read" | "readBytes" | "seek", [arg])
                                        if compatible("Int", arg) =>
                                    {
                                        None
                                    }
                                    ("write", [_]) => None,
                                    ("writeBytes", [arg]) if compatible("Bytes", arg) => None,
                                    ("close", []) => None,
                                    _ => Some("invalid FileHandle method or arguments"),
                                };
                                if let Some(message) = expected {
                                    return Err(diagnostic(&e.at, message));
                                }
                            }
                            if let Some(inner) =
                                t.strip_prefix("List<").and_then(|s| s.strip_suffix('>'))
                            {
                                let value = match (method.as_str(), types.as_slice()) {
                                    ("add" | "push", [value]) | ("set", [_, value]) => Some(value),
                                    _ => None,
                                };
                                if let Some(value) = value {
                                    if !compatible(inner, value) && value != "Unknown" {
                                        return Err(diagnostic(
                                            &e.at,
                                            format!("List<{inner}> cannot contain {value}"),
                                        ));
                                    }
                                }
                            }
                            if t.starts_with("List")
                                && matches!(method.as_str(), "get" | "set")
                                && types.first().is_some_and(|index| !compatible("Int", index))
                            {
                                return Err(diagnostic(&e.at, "List index must be Int"));
                            }
                            if let Some(inner) =
                                t.strip_prefix("Map<").and_then(|s| s.strip_suffix('>'))
                            {
                                let parts = split_type_args(inner);
                                if parts.len() == 2 {
                                    let expected = match (method.as_str(), types.as_slice()) {
                                        ("set", [key, value]) => {
                                            vec![(parts[0], key), (parts[1], value)]
                                        }
                                        ("get" | "remove", [key]) => vec![(parts[0], key)],
                                        _ => Vec::new(),
                                    };
                                    for (needed, actual) in expected {
                                        if !compatible(needed, actual) && actual != "Unknown" {
                                            return Err(diagnostic(&e.at, format!("Map key/value expects {needed}, found {actual}")));
                                        }
                                    }
                                }
                            }
                            let base_type = t.split('<').next().unwrap_or("");
                            let arity_ok = (matches!(
                                self.program.language.as_str(),
                                "0.9.3"
                                    | "0.9.4"
                                    | "0.9.5"
                                    | "0.9.6"
                                    | "0.9.7"
                                    | "0.9.8"
                                    | "0.9.9"
                                    | "1.0.0"
                                    | "1.1.0"
                                    | "1.2.0"
                                    | "1.3.0"
                                    | "1.4.0"
                                    | "1.5.0"
                                    | "1.6.0"
                                    | "1.6.1"
                                    | "1.7.0"
                                    | "1.7.1"
                                    | "1.8.0"
                                    | "1.8.1"
                                    | "1.8.2"
                                    | "1.8.3"
                                    | "1.8.4"
                                    | "1.8.5"
                                    | "1.8.6"
                                    | "1.8.7"
                                    | "1.8.8"
                                    | "1.9.0"
                                    | "1.9.1"
                                    | "1.9.2"
                                    | "1.9.3"
                                    | "1.9.4"
                                    | "1.9.5"
                                    | "1.9.6"
                                    | "1.9.7"
                                    | "1.9.8"
                                    | "1.9.9"
                                    | "1.9.10"
                                    | "1.9.11"
                                    | "1.9.12"
                                    | "1.9.13"
                                    | "1.9.14"
                                    | "1.9.15"
                                    | "1.9.16"
                                    | "1.9.17"
                                    | "1.9.18"
                                    | "1.9.19"
                                    | "1.9.20"
                                    | "1.9.21"
                                    | "1.9.22"
                                    | "1.9.23"
                                    | "1.9.24"
                                    | "1.9.25"
                                    | "1.9.26"
                                    | "1.9.27"
                                    | "1.9.28"
                                    | "1.9.29"
                                    | "1.9.30"
                                    | "1.9.31"
                                    | "1.9.32"
                                    | "1.9.33"
                                    | "1.9.34"
                                    | "1.9.35"
                                    | "1.9.36"
                                    | "1.9.37"
                                    | "1.9.38"
                                    | "1.9.39"
                                    | "1.9.40"
                                    | "1.9.41"
                                    | "1.9.42"
                                    | "1.9.43"
                                    | "1.9.44"
                                    | "1.9.45"
                                    | "1.9.46"
                                    | "1.9.47"
                                    | "1.9.48"
                                    | "1.9.49"
                                    | "1.9.50"
                                    | "1.9.51"
                                    | "2.0.0"
                            ) && base_type == "List"
                                && method == "pop"
                                && types.is_empty())
                                || matches!(
                                    (base_type, method.as_str(), types.len()),
                                    ("List", "add" | "push" | "get", 1)
                                        | ("List", "set", 2)
                                        | ("List", "len", 0)
                                        | ("Map", "set", 2)
                                        | ("Map", "get" | "remove", 1)
                                        | ("Map", "keys" | "len", 0)
                                        | (
                                            "FileHandle",
                                            "read" | "readBytes" | "seek" | "write" | "writeBytes",
                                            1,
                                        )
                                        | ("FileHandle", "close", 0)
                                );
                            if !arity_ok {
                                if let Some(bound) = self.bounds.get(&t) {
                                    let mut candidates = Vec::new();
                                    for bound in bound.split('+') {
                                        let standard = standard_trait(bound);
                                        if let Some(tr) =
                                            self.program.traits.get(bound).or(standard.as_ref())
                                        {
                                            if let Some(signature) = tr.methods.get(method) {
                                                candidates.push(signature.clone());
                                            }
                                        }
                                    }
                                    if candidates.len() > 1 {
                                        return Err(diagnostic(
                                            &e.at,
                                            "ambiguous method in generic bounds",
                                        ));
                                    }
                                    if let Some((params, ret)) = candidates.first() {
                                        {
                                            if params.len() == types.len() + 1
                                                && params[1..].iter().zip(&types).all(|(e, a)| {
                                                    compatible(&e.replace("Self", &t), a)
                                                })
                                            {
                                                return Ok(ret.replace("Self", &t));
                                            }
                                        }
                                    }
                                }
                                if let Some(ret) =
                                    trait_method_return(self.program, &t, method, &types, &e.at)?
                                {
                                    return Ok(ret);
                                }
                                return Err(diagnostic(
                                    &e.at,
                                    format!("invalid method call {t}.{method}/{}", types.len()),
                                ));
                            }
                            match (t.split('<').next().unwrap_or(""), method.as_str()) {
                                ("List" | "Map", "len") => "Int".into(),
                                ("Map", "get") => t
                                    .strip_prefix("Map<")
                                    .and_then(|s| s.strip_suffix('>'))
                                    .and_then(|inner| split_type_args(inner).get(1).copied())
                                    .map(|inner| format!("Option<{inner}>"))
                                    .unwrap_or("Option<Unknown>".into()),
                                ("List", "pop")
                                    if matches!(
                                        self.program.language.as_str(),
                                        "0.9.3"
                                            | "0.9.4"
                                            | "0.9.5"
                                            | "0.9.6"
                                            | "0.9.7"
                                            | "0.9.8"
                                            | "0.9.9"
                                            | "1.0.0"
                                            | "1.1.0"
                                            | "1.2.0"
                                            | "1.3.0"
                                            | "1.4.0"
                                            | "1.5.0"
                                            | "1.6.0"
                                            | "1.6.1"
                                            | "1.7.0"
                                            | "1.7.1"
                                            | "1.8.0"
                                            | "1.8.1"
                                            | "1.8.2"
                                            | "1.8.3"
                                            | "1.8.4"
                                            | "1.8.5"
                                            | "1.8.6"
                                            | "1.8.7"
                                            | "1.8.8"
                                            | "1.9.0"
                                            | "1.9.1"
                                            | "1.9.2"
                                            | "1.9.3"
                                            | "1.9.4"
                                            | "1.9.5"
                                            | "1.9.6"
                                            | "1.9.7"
                                            | "1.9.8"
                                            | "1.9.9"
                                            | "1.9.10"
                                            | "1.9.11"
                                            | "1.9.12"
                                            | "1.9.13"
                                            | "1.9.14"
                                            | "1.9.15"
                                            | "1.9.16"
                                            | "1.9.17"
                                            | "1.9.18"
                                            | "1.9.19"
                                            | "1.9.20"
                                            | "1.9.21"
                                            | "1.9.22"
                                            | "1.9.23"
                                            | "1.9.24"
                                            | "1.9.25"
                                            | "1.9.26"
                                            | "1.9.27"
                                            | "1.9.28"
                                            | "1.9.29"
                                            | "1.9.30"
                                            | "1.9.31"
                                            | "1.9.32"
                                            | "1.9.33"
                                            | "1.9.34"
                                            | "1.9.35"
                                            | "1.9.36"
                                            | "1.9.37"
                                            | "1.9.38"
                                            | "1.9.39"
                                            | "1.9.40"
                                            | "1.9.41"
                                            | "1.9.42"
                                            | "1.9.43"
                                            | "1.9.44"
                                            | "1.9.45"
                                            | "1.9.46"
                                            | "1.9.47"
                                            | "1.9.48"
                                            | "1.9.49"
                                            | "1.9.50"
                                            | "1.9.51"
                                            | "2.0.0"
                                    ) =>
                                {
                                    format!(
                                        "Option<{}>",
                                        t.strip_prefix("List<")
                                            .and_then(|s| s.strip_suffix('>'))
                                            .unwrap_or("Unknown")
                                    )
                                }
                                ("List", "get") => t
                                    .strip_prefix("List<")
                                    .and_then(|s| s.strip_suffix('>'))
                                    .unwrap_or("Unknown")
                                    .into(),
                                ("Map", "keys") => t
                                    .strip_prefix("Map<")
                                    .and_then(|s| s.strip_suffix('>'))
                                    .and_then(|inner| split_type_args(inner).first().copied())
                                    .map(|inner| format!("List<{inner}>"))
                                    .unwrap_or("List".into()),
                                ("FileHandle", "read") => "String".into(),
                                ("FileHandle", "readBytes") => "Bytes".into(),
                                ("List", "add" | "push" | "set")
                                | ("Map", "set" | "remove")
                                | ("FileHandle", "seek" | "write" | "writeBytes" | "close") => {
                                    "Unit".into()
                                }
                                _ => {
                                    return Err(diagnostic(
                                        &e.at,
                                        format!("type {t} has no method {method}"),
                                    ))
                                }
                            }
                        }
                    } else {
                        let t = self.expr(base)?;
                        if language_at_least(&self.program.language, "1.3.0") {
                            if let Some(inner) =
                                t.strip_prefix("Map<").and_then(|s| s.strip_suffix('>'))
                            {
                                let parts = split_type_args(inner);
                                if parts.len() == 2 {
                                    let expected: Vec<&str> = match method.as_str() {
                                        "set" => vec![parts[0], parts[1]],
                                        "get" | "remove" => vec![parts[0]],
                                        "len" | "keys" => vec![],
                                        _ => {
                                            return Err(diagnostic(
                                                &e.at,
                                                format!("type {t} has no method {method}"),
                                            ))
                                        }
                                    };
                                    if expected.len() != types.len()
                                        || expected
                                            .iter()
                                            .zip(&types)
                                            .any(|(needed, actual)| !compatible(needed, actual))
                                    {
                                        return Err(diagnostic(
                                            &e.at,
                                            format!(
                                                "InvalidArguments: Map.{method} requires ({})",
                                                expected.join(",")
                                            ),
                                        ));
                                    }
                                    return Ok(match method.as_str() {
                                        "get" => format!("Option<{}>", parts[1]),
                                        "keys" => format!("List<{}>", parts[0]),
                                        "len" => "Int".into(),
                                        _ => "Unit".into(),
                                    });
                                }
                            }
                        }
                        if let Some(ret) =
                            trait_method_return(self.program, &t, method, &types, &e.at)?
                        {
                            return Ok(ret);
                        }
                        match (
                            t.split('<').next().unwrap_or(""),
                            method.as_str(),
                            types.len(),
                        ) {
                            ("List", "pop", 0)
                                if matches!(
                                    self.program.language.as_str(),
                                    "0.9.6"
                                        | "0.9.7"
                                        | "0.9.8"
                                        | "0.9.9"
                                        | "1.0.0"
                                        | "1.1.0"
                                        | "1.2.0"
                                        | "1.3.0"
                                        | "1.4.0"
                                        | "1.5.0"
                                        | "1.6.0"
                                        | "1.6.1"
                                        | "1.7.0"
                                        | "1.7.1"
                                        | "1.8.0"
                                        | "1.8.1"
                                        | "1.8.2"
                                        | "1.8.3"
                                        | "1.8.4"
                                        | "1.8.5"
                                        | "1.8.6"
                                        | "1.8.7"
                                        | "1.8.8"
                                        | "1.9.0"
                                        | "1.9.1"
                                        | "1.9.2"
                                        | "1.9.3"
                                        | "1.9.4"
                                        | "1.9.5"
                                        | "1.9.6"
                                        | "1.9.7"
                                        | "1.9.8"
                                        | "1.9.9"
                                        | "1.9.10"
                                        | "1.9.11"
                                        | "1.9.12"
                                        | "1.9.13"
                                        | "1.9.14"
                                        | "1.9.15"
                                        | "1.9.16"
                                        | "1.9.17"
                                        | "1.9.18"
                                        | "1.9.19"
                                        | "1.9.20"
                                        | "1.9.21"
                                        | "1.9.22"
                                        | "1.9.23"
                                        | "1.9.24"
                                        | "1.9.25"
                                        | "1.9.26"
                                        | "1.9.27"
                                        | "1.9.28"
                                        | "1.9.29"
                                        | "1.9.30"
                                        | "1.9.31"
                                        | "1.9.32"
                                        | "1.9.33"
                                        | "1.9.34"
                                        | "1.9.35"
                                        | "1.9.36"
                                        | "1.9.37"
                                        | "1.9.38"
                                        | "1.9.39"
                                        | "1.9.40"
                                        | "1.9.41"
                                        | "1.9.42"
                                        | "1.9.43"
                                        | "1.9.44"
                                        | "1.9.45"
                                        | "1.9.46"
                                        | "1.9.47"
                                        | "1.9.48"
                                        | "1.9.49"
                                        | "1.9.50"
                                        | "1.9.51"
                                        | "2.0.0"
                                ) =>
                            {
                                format!(
                                    "Option<{}>",
                                    t.strip_prefix("List<")
                                        .and_then(|s| s.strip_suffix('>'))
                                        .unwrap_or("Unknown")
                                )
                            }
                            ("List", "get", 1) => t
                                .strip_prefix("List<")
                                .and_then(|s| s.strip_suffix('>'))
                                .unwrap_or("Unknown")
                                .into(),
                            ("List" | "Map", "len", 0) => "Int".into(),
                            ("Map", "get", 1) => "Option<Unknown>".into(),
                            ("Map", "keys", 0) => "List".into(),
                            ("FileHandle", "read", 1) => "String".into(),
                            ("FileHandle", "readBytes", 1) => "Bytes".into(),
                            _ => {
                                return Err(diagnostic(
                                    &e.at,
                                    format!("type {t} has no method {method}"),
                                ))
                            }
                        }
                    }
                } else {
                    let ty = self.expr(target)?;
                    let Some((params, ret)) = function_signature(&ty) else {
                        return Err(diagnostic(&e.at, "invalid call target"));
                    };
                    if params.len() != types.len()
                        || params.iter().zip(&types).any(|(p, a)| !compatible(p, a))
                    {
                        return Err(diagnostic(&e.at, "invalid function arguments"));
                    }
                    ret
                }
            }
            ExprKind::Try(inner) => {
                let t = self.expr(inner)?;
                if !t.starts_with("Result<") {
                    return Err(diagnostic(&e.at, "'?' requires Result"));
                }
                let inner = t
                    .strip_prefix("Result<")
                    .and_then(|s| s.strip_suffix('>'))
                    .unwrap();
                split_type_args(inner)
                    .first()
                    .copied()
                    .unwrap_or("Unknown")
                    .into()
            }
            ExprKind::Match(value, arms) => {
                let ty = self.expr(value)?;
                self.check_match(
                    &ty,
                    &arms
                        .iter()
                        .map(|(p, g, _)| (p, g.is_some()))
                        .collect::<Vec<_>>(),
                    &e.at,
                )?;
                let mut result: Option<String> = None;
                for (pattern, guard, arm) in arms {
                    let mut bindings = BTreeMap::new();
                    self.pattern(
                        &resolve_pattern_alias(pattern, &self.program.import_aliases),
                        &ty,
                        &e.at,
                        &mut bindings,
                    )?;
                    let scoped = Checker {
                        program: self.program,
                        scopes: {
                            let mut scopes = self.scopes.clone();
                            scopes.push(bindings);
                            scopes
                        },
                        return_ty: self.return_ty.clone(),
                        loop_depth: self.loop_depth,
                        bounds: self.bounds.clone(),
                        origin: self.origin.clone(),
                    };
                    if let Some(guard) = guard {
                        if scoped.expr(guard)? != "Bool" {
                            return Err(diagnostic(&guard.at, "match guard must be Bool"));
                        }
                        check_guard_effects(self.program, guard)?;
                    }
                    let arm_ty = scoped.expr(arm)?;
                    if let Some(prior) = &result {
                        if !compatible(prior, &arm_ty) {
                            return Err(diagnostic(
                                &arm.at,
                                format!("match arm expects {prior}, found {arm_ty}"),
                            ));
                        }
                    } else {
                        result = Some(arm_ty);
                    }
                }
                result.unwrap_or("Unit".into())
            }
            ExprKind::Closure(params, ret, body) => {
                let mut scoped = Checker {
                    program: self.program,
                    scopes: self.scopes.clone(),
                    return_ty: Some(ret.clone()),
                    loop_depth: 0,
                    bounds: self.bounds.clone(),
                    origin: self.origin.clone(),
                };
                scoped.scopes.push(BTreeMap::new());
                for (name, ty) in params {
                    if scoped
                        .scopes
                        .last_mut()
                        .unwrap()
                        .insert(name.clone(), (ty.clone(), false))
                        .is_some()
                    {
                        return Err(diagnostic(&e.at, format!("duplicate parameter {name}")));
                    }
                }
                for stmt in body {
                    scoped.stmt(stmt)?;
                }
                if ret != "Unit" && !guarantees_return(body) {
                    return Err(diagnostic(
                        &e.at,
                        format!("closure returning {ret} needs return on every path"),
                    ));
                }
                if matches!(
                    self.program.language.as_str(),
                    "0.6"
                        | "0.7"
                        | "0.8"
                        | "0.9"
                        | "0.9.1"
                        | "0.9.2"
                        | "0.9.3"
                        | "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) {
                    let ty = v06::fn_type(params, ret, &v06::closure_effects(self, params, body)?);
                    v06::captures::infer(self, &ty, v06::captures::names(self, params, body))
                } else {
                    format!(
                        "fn({})->{ret}",
                        params
                            .iter()
                            .map(|(_, t)| t.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
            }
            ExprKind::NamedConstructor(name, fields) => {
                let resolved_name = resolve_alias(self.program, name);
                let (enum_name, variant) = resolved_name.split_once("::").unwrap();
                let base = enum_name.split('<').next().unwrap_or(enum_name);
                let def = self
                    .program
                    .enums
                    .get(base)
                    .ok_or_else(|| diagnostic(&e.at, format!("unknown enum {base}")))?;
                self.visible(
                    def.public,
                    &def.origin,
                    &e.at,
                    name.split_once("::").unwrap().0,
                )?;
                let expected = def
                    .variants
                    .get(variant)
                    .ok_or_else(|| diagnostic(&e.at, format!("unknown variant {variant}")))?;
                if fields.len() != expected.len() {
                    return Err(diagnostic(&e.at, "named constructor field count mismatch"));
                }
                let mut types = BTreeMap::new();
                for (field, value) in fields {
                    if types.insert(field.clone(), self.expr(value)?).is_some() {
                        return Err(diagnostic(&e.at, format!("duplicate field {field}")));
                    }
                }
                let ordered = expected
                    .iter()
                    .map(|(field, _)| {
                        types
                            .get(field)
                            .cloned()
                            .ok_or_else(|| diagnostic(&e.at, format!("missing field {field}")))
                    })
                    .collect::<Result<Vec<_>>>()?;
                enum_constructor_type(self.program, &resolved_name, &ordered, &e.at)?.unwrap()
            }
        })
    }
    fn stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match &stmt.kind {
            StmtKind::Let(name, mutable, ty, expr) => {
                let inferred = self.expr(expr)?;
                let annotation = ty
                    .as_ref()
                    .map(|ty| rename_type(ty, &self.program.import_aliases));
                if let Some(expected) = &annotation {
                    if !compatible(expected, &inferred) && inferred != "Unknown" {
                        return Err(diagnostic(
                            &stmt.at,
                            format!("type mismatch: expected {expected}, found {inferred}"),
                        ));
                    }
                }
                if self.scopes.last().unwrap().contains_key(name) {
                    return Err(diagnostic(&stmt.at, format!("duplicate binding {name}")));
                }
                if self.find(name).is_some() {
                    eprintln!(
                        "warning: {}:{}: {name} shadows an outer binding",
                        stmt.at.line, stmt.at.col
                    );
                }
                self.scopes
                    .last_mut()
                    .unwrap()
                    .insert(name.clone(), (annotation.unwrap_or(inferred), *mutable));
            }
            StmtKind::Using(name, expr) => {
                let ty = self.expr(expr)?;
                if rewind::native_resources::resource_type(&ty)
                    && matches!(expr.kind, ExprKind::Name(_) | ExprKind::Member(_, _))
                {
                    return Err(diagnostic(
                        &stmt.at,
                        "using a named native resource requires move",
                    ));
                }
                if ty != "FileHandle"
                    && ty != "TaskGroup"
                    && !rewind::native_resources::resource_type(&ty)
                {
                    return Err(diagnostic(&stmt.at, "using requires a FileHandle"));
                }
                if self.scopes.last().unwrap().contains_key(name) {
                    return Err(diagnostic(&stmt.at, format!("duplicate binding {name}")));
                }
                self.scopes
                    .last_mut()
                    .unwrap()
                    .insert(name.clone(), (ty, false));
            }
            StmtKind::Assign(lhs, _, rhs) => {
                if let ExprKind::Member(base, _) = &lhs.kind {
                    if matches!(
                        self.program.language.as_str(),
                        "0.6"
                            | "0.7"
                            | "0.8"
                            | "0.9"
                            | "0.9.1"
                            | "0.9.2"
                            | "0.9.3"
                            | "0.9.4"
                            | "0.9.5"
                            | "0.9.6"
                            | "0.9.7"
                            | "0.9.8"
                            | "0.9.9"
                            | "1.0.0"
                            | "1.1.0"
                            | "1.2.0"
                            | "1.3.0"
                            | "1.4.0"
                            | "1.5.0"
                            | "1.6.0"
                            | "1.6.1"
                            | "1.7.0"
                            | "1.7.1"
                            | "1.8.0"
                            | "1.8.1"
                            | "1.8.2"
                            | "1.8.3"
                            | "1.8.4"
                            | "1.8.5"
                            | "1.8.6"
                            | "1.8.7"
                            | "1.8.8"
                            | "1.9.0"
                            | "1.9.1"
                            | "1.9.2"
                            | "1.9.3"
                            | "1.9.4"
                            | "1.9.5"
                            | "1.9.6"
                            | "1.9.7"
                            | "1.9.8"
                            | "1.9.9"
                            | "1.9.10"
                            | "1.9.11"
                            | "1.9.12"
                            | "1.9.13"
                            | "1.9.14"
                            | "1.9.15"
                            | "1.9.16"
                            | "1.9.17"
                            | "1.9.18"
                            | "1.9.19"
                            | "1.9.20"
                            | "1.9.21"
                            | "1.9.22"
                            | "1.9.23"
                            | "1.9.24"
                            | "1.9.25"
                            | "1.9.26"
                            | "1.9.27"
                            | "1.9.28"
                            | "1.9.29"
                            | "1.9.30"
                            | "1.9.31"
                            | "1.9.32"
                            | "1.9.33"
                            | "1.9.34"
                            | "1.9.35"
                            | "1.9.36"
                            | "1.9.37"
                            | "1.9.38"
                            | "1.9.39"
                            | "1.9.40"
                            | "1.9.41"
                            | "1.9.42"
                            | "1.9.43"
                            | "1.9.44"
                            | "1.9.45"
                            | "1.9.46"
                            | "1.9.47"
                            | "1.9.48"
                            | "1.9.49"
                            | "1.9.50"
                            | "1.9.51"
                            | "2.0.0"
                    ) && self.expr(base)?.starts_with("Tuple<")
                    {
                        return Err(diagnostic(&lhs.at, "tuple fields are immutable"));
                    }
                    if self.expr(base)?.starts_with("Frozen<") {
                        return Err(diagnostic(&lhs.at, "cannot mutate Frozen<T>; use thaw"));
                    }
                }
                let right = self.expr(rhs)?;
                match &lhs.kind {
                    ExprKind::Name(name) => {
                        let (expected, mutable) = self.find(name).ok_or_else(|| {
                            diagnostic(&lhs.at, format!("unknown variable {name}"))
                        })?;
                        if !mutable {
                            return Err(diagnostic(
                                &lhs.at,
                                format!("cannot assign to let binding {name}"),
                            ));
                        }
                        if !compatible(expected, &right) && right != "Unknown" {
                            return Err(diagnostic(
                                &lhs.at,
                                format!("type mismatch: expected {expected}, found {right}"),
                            ));
                        }
                    }
                    ExprKind::Member(base, _) => {
                        let ty = self.expr(base)?;
                        if self
                            .program
                            .structs
                            .get(v06::borrowed_type(&ty).split('<').next().unwrap_or(&ty))
                            .is_some_and(|d| d.immutable)
                        {
                            return Err(diagnostic(&lhs.at, "record fields are immutable"));
                        }
                        let expected = self.expr(lhs)?;
                        if expected != "Unknown"
                            && !compatible(&expected, &right)
                            && right != "Unknown"
                        {
                            return Err(diagnostic(
                                &lhs.at,
                                format!("field expects {expected}, found {right}"),
                            ));
                        }
                    }
                    _ => return Err(diagnostic(&lhs.at, "invalid assignment target")),
                }
            }
            StmtKind::Expr(expr) => {
                self.expr(expr)?;
            }
            StmtKind::Defer(expr) => {
                if self.return_ty.is_none() {
                    return Err(diagnostic(&stmt.at, "defer requires a function"));
                }
                if cleanup_calls_publish(
                    self.program,
                    std::slice::from_ref(stmt),
                    &mut BTreeSet::new(),
                ) {
                    return Err(diagnostic(&stmt.at, "defer cleanup may not call publish"));
                }
                if !defer_safe(expr)
                    && !matches!(&expr.kind, ExprKind::Closure(_,_,body) if cleanup_safe(body) && !cleanup_calls_publish(self.program,body,&mut BTreeSet::new()))
                {
                    return Err(diagnostic(
                        &stmt.at,
                        "defer may only call virtual I/O or object methods",
                    ));
                }
                self.expr(expr)?;
            }
            StmtKind::External(_, body) | StmtKind::Block(body) | StmtKind::Branch(_, body) => {
                self.block(body)?
            }
            StmtKind::If(cond, yes, no) => {
                if self.expr(cond)? != "Bool" {
                    return Err(diagnostic(&cond.at, "condition must be Bool"));
                }
                self.block(yes)?;
                self.block(no)?;
            }
            StmtKind::While(cond, body) => {
                if self.expr(cond)? != "Bool" {
                    return Err(diagnostic(&cond.at, "condition must be Bool"));
                }
                self.loop_depth += 1;
                self.block(body)?;
                self.loop_depth -= 1;
            }
            StmtKind::For(name, a, b, body) => {
                if self.expr(a)? != "Int" || self.expr(b)? != "Int" {
                    return Err(diagnostic(&stmt.at, "range bounds must be Int"));
                }
                self.loop_depth += 1;
                self.scopes.push(BTreeMap::new());
                self.scopes
                    .last_mut()
                    .unwrap()
                    .insert(name.clone(), ("Int".into(), false));
                self.block(body)?;
                self.scopes.pop();
                self.loop_depth -= 1;
            }
            StmtKind::Break | StmtKind::Continue if self.loop_depth == 0 => {
                return Err(diagnostic(&stmt.at, "loop control outside loop"))
            }
            StmtKind::Return(expr) => {
                let expected = self
                    .return_ty
                    .as_ref()
                    .ok_or_else(|| diagnostic(&stmt.at, "return outside function"))?;
                let actual = if let Some(e) = expr {
                    self.expr(e)?
                } else {
                    "Unit".into()
                };
                if !compatible(expected, &actual) && actual != "Unknown" {
                    return Err(diagnostic(
                        &stmt.at,
                        format!(
                            "return type mismatch: expected {expected}, found {actual}{}",
                            if actual.starts_with("fn(")
                                && !v06::captures::compatible(expected, &actual)
                            {
                                "; closure cannot escape its capture contract"
                            } else {
                                ""
                            }
                        ),
                    ));
                }
            }
            StmtKind::Revert(name) | StmtKind::Resume(name)
                if name == "begin" && language_at_least(&self.program.language, "1.3.0") =>
            {
                for scope in &mut self.scopes {
                    scope.clear();
                }
            }
            StmtKind::Match(value, arms) => {
                let t = self.expr(value)?;
                self.check_match(
                    &t,
                    &arms
                        .iter()
                        .map(|(p, g, _)| (p, g.is_some()))
                        .collect::<Vec<_>>(),
                    &stmt.at,
                )?;
                for (pattern, guard, body) in arms {
                    let mut bindings = BTreeMap::new();
                    self.pattern(
                        &resolve_pattern_alias(pattern, &self.program.import_aliases),
                        &t,
                        &stmt.at,
                        &mut bindings,
                    )?;
                    self.scopes.push(BTreeMap::new());
                    self.scopes.last_mut().unwrap().extend(bindings);
                    if let Some(guard) = guard {
                        if self.expr(guard)? != "Bool" {
                            return Err(diagnostic(&guard.at, "match guard must be Bool"));
                        }
                        check_guard_effects(self.program, guard)?;
                    }
                    self.stmt(body)?;
                    self.scopes.pop();
                }
            }
            _ => {}
        }
        Ok(())
    }
}
fn defer_safe(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Call(target, args) => {
            matches!(&target.kind, ExprKind::Member(_, _))
                && defer_safe(target)
                && args.iter().all(defer_safe)
        }
        ExprKind::Member(base, _) | ExprKind::Try(base) | ExprKind::Unary(_, base) => {
            defer_safe(base)
        }
        ExprKind::Binary(_, a, b) => defer_safe(a) && defer_safe(b),
        ExprKind::Match(_, _) | ExprKind::Closure(_, _, _) | ExprKind::NamedConstructor(_, _) => {
            false
        }
        ExprKind::Value(_) | ExprKind::Name(_) => true,
    }
}
fn cleanup_safe(body: &[Stmt]) -> bool {
    body.iter().all(|stmt| match &stmt.kind {
        StmtKind::Publish(_)
        | StmtKind::Commit(_)
        | StmtKind::Revert(_)
        | StmtKind::Resume(_)
        | StmtKind::Drop(_)
        | StmtKind::Branch(_, _)
        | StmtKind::External(_, _) => false,
        StmtKind::Block(body) | StmtKind::While(_, body) | StmtKind::For(_, _, _, body) => {
            cleanup_safe(body)
        }
        StmtKind::If(_, a, b) => cleanup_safe(a) && cleanup_safe(b),
        StmtKind::Match(_, arms) => arms
            .iter()
            .all(|(_, _, s)| cleanup_safe(std::slice::from_ref(s))),
        _ => true,
    })
}
fn check_guard_effects(program: &Program, guard: &Expr) -> Result<()> {
    let statement = Stmt {
        kind: StmtKind::Expr(guard.clone()),
        at: guard.at.clone(),
    };
    if cleanup_calls_publish(program, &[statement], &mut BTreeSet::new()) {
        return Err(diagnostic(&guard.at,"match guard may only perform virtual I/O; publish, checkpoints and unresolved function values are forbidden"));
    }
    Ok(())
}
fn cleanup_calls_publish(program: &Program, body: &[Stmt], seen: &mut BTreeSet<String>) -> bool {
    fn expr(program: &Program, e: &Expr, seen: &mut BTreeSet<String>) -> bool {
        match &e.kind {
            ExprKind::Call(callee, args) => {
                let direct = match &callee.kind {
                    ExprKind::Name(name) => {
                        let resolved = resolve_alias(program, name);
                        let base = resolved.split('<').next().unwrap_or(&resolved);
                        if let Some(f) = program.functions.get(base) {
                            f.asynchronous
                                || seen.insert(base.into())
                                    && (!cleanup_safe(&f.body)
                                        || cleanup_calls_publish(program, &f.body, seen))
                        } else {
                            !matches!(
                                base,
                                "assert"
                                    | "assert_eq"
                                    | "panic"
                                    | "Ok"
                                    | "Err"
                                    | "Some"
                                    | "Bytes"
                                    | "List"
                                    | "Map"
                            ) && !program.structs.contains_key(base)
                                && !resolved.contains("::")
                        }
                    }
                    ExprKind::Member(base, method) => {
                        let alias = if let ExprKind::Name(name) = &base.kind {
                            program.import_aliases.get(&format!("{name}.{method}"))
                        } else {
                            None
                        };
                        alias.is_some_and(|symbol| {
                            program.functions.get(symbol).is_some_and(|f| {
                                f.asynchronous
                                    || (seen.insert(symbol.clone())
                                        && (!cleanup_safe(&f.body)
                                            || cleanup_calls_publish(program, &f.body, seen)))
                            })
                        }) || program
                            .impls
                            .values()
                            .filter_map(|methods| methods.get(method))
                            .any(|name| {
                                program.functions.get(name).is_some_and(|f| {
                                    seen.insert(name.clone())
                                        && (!cleanup_safe(&f.body)
                                            || cleanup_calls_publish(program, &f.body, seen))
                                })
                            })
                    }
                    _ => true,
                };
                direct
                    || expr(program, callee, seen)
                    || args.iter().any(|arg| expr(program, arg, seen))
            }
            ExprKind::Unary(op, e) => {
                matches!(op.as_str(), "await" | "spawn") || expr(program, e, seen)
            }
            ExprKind::Try(e) | ExprKind::Member(e, _) => expr(program, e, seen),
            ExprKind::Binary(_, a, b) => expr(program, a, seen) || expr(program, b, seen),
            ExprKind::Match(e, arms) => {
                expr(program, e, seen)
                    || arms.iter().any(|(_, guard, result)| {
                        guard.as_ref().is_some_and(|g| expr(program, g, seen))
                            || expr(program, result, seen)
                    })
            }
            ExprKind::Closure(_, _, body) => cleanup_calls_publish(program, body, seen),
            ExprKind::NamedConstructor(_, fields) => {
                fields.iter().any(|(_, e)| expr(program, e, seen))
            }
            _ => false,
        }
    }
    body.iter().any(|stmt| match &stmt.kind {
        StmtKind::Publish(_) => true,
        StmtKind::Let(_, _, _, e)
        | StmtKind::Using(_, e)
        | StmtKind::Expr(e)
        | StmtKind::Defer(e)
        | StmtKind::Return(Some(e)) => expr(program, e, seen),
        StmtKind::Assign(a, _, b) => expr(program, a, seen) || expr(program, b, seen),
        StmtKind::External(_, body) | StmtKind::Block(body) | StmtKind::Branch(_, body) => {
            cleanup_calls_publish(program, body, seen)
        }
        StmtKind::If(e, a, b) => {
            expr(program, e, seen)
                || cleanup_calls_publish(program, a, seen)
                || cleanup_calls_publish(program, b, seen)
        }
        StmtKind::While(e, body) => {
            expr(program, e, seen) || cleanup_calls_publish(program, body, seen)
        }
        StmtKind::For(_, a, b, body) => {
            expr(program, a, seen)
                || expr(program, b, seen)
                || cleanup_calls_publish(program, body, seen)
        }
        StmtKind::Match(e, arms) => {
            expr(program, e, seen)
                || arms.iter().any(|(_, guard, body)| {
                    guard.as_ref().is_some_and(|g| expr(program, g, seen))
                        || cleanup_calls_publish(program, std::slice::from_ref(body), seen)
                })
        }
        _ => false,
    })
}
fn builtin_type(receiver: &str, method: &str, args: &[String], at: &Tok) -> Result<String> {
    let count = args.len();
    let result = match (receiver, method, count) {
        ("Out" | "Err", "println", 1) | ("Out" | "Err", "flush", 0) => "Unit",
        ("In", "readChunk", 1) if args[0] == "Int" => "Option<Bytes>",
        ("Out", "writeBytes", 1) if args[0] == "Bytes" => "Unit",
        ("In", "readLine", 0) => "Option<String>",
        ("In", "readSecretLine", 0) => "Option<Secret<String>>",
        ("Time", "now", 0) | ("Random", "next", 0) => "Int",
        ("Args", "all", 0) | ("Directory", "entries", 1) => "List<String>",
        ("Env", "get", 1) => "Option<String>",
        ("Env", "getSecret", 1) => "Option<Secret<String>>",
        ("Locale", "current", 0) => "String",
        ("File", "readText", 1) => "Result<String,FileError>",
        ("File", "readBytes", 1) => "Result<Bytes,FileError>",
        ("File", "open" | "openSnapshot", 1) => "FileHandle",
        ("File", "writeBytes" | "writeText" | "write" | "append" | "copy" | "move", 2)
        | ("File", "create", 1 | 2)
        | ("File", "delete", 1)
        | ("File", "truncate", 2)
        | ("Directory", "create" | "delete", 1)
        | ("Directory", "move", 2) => "Unit",
        _ => {
            return Err(diagnostic(
                at,
                format!("unsupported call {receiver}.{method}/{count}"),
            ))
        }
    };
    if matches!(receiver, "File" | "Directory" | "Env") {
        for (index, actual) in args.iter().enumerate() {
            let expected = match (receiver, method, index) {
                ("File", "writeBytes", 1) => "Bytes",
                ("File", "truncate", 1) => "Int",
                _ => "String",
            };
            if !compatible(expected, actual) {
                return Err(diagnostic(
                    at,
                    format!(
                        "{receiver}.{method} argument {} expects {expected}, found {actual}",
                        index + 1
                    ),
                ));
            }
        }
    }
    Ok(result.into())
}

impl<R: BufRead> Engine<R> {
    fn new(program: Program, root: &Path, input: R) -> Result<Self> {
        let mut runtime = Runtime::new(root)?;
        if matches!(
            program.language.as_str(),
            "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) {
            runtime.enable_incremental_publish();
        }
        Ok(Self {
            program,
            runtime,
            input,
            scopes: vec![BTreeMap::new()],
            snapshots: BTreeMap::new(),
            remaining: 1_000_000,
            top_pc: 0,
            function_depth: 0,
            defers: vec![Vec::new()],
            test_mode: false,
            trace: false,
            trace_json: false,
            call_path: Vec::new(),
            next_call_id: 1,
        })
    }
}

#[derive(Clone)]
struct Binding {
    value: Value,
    mutable: bool,
    ty: String,
}
#[derive(Clone)]
struct Snapshot {
    scopes: Vec<BTreeMap<String, Binding>>,
    pc: usize,
    call_path: Vec<u64>,
}
#[allow(dead_code)]
enum Flow {
    Error(Error),
    Return(Value),
    Break,
    Continue,
    Resume(usize),
}
type Exec<T> = std::result::Result<T, Flow>;
impl From<Error> for Flow {
    fn from(value: Error) -> Self {
        Flow::Error(value)
    }
}

struct Engine<R: BufRead> {
    program: Program,
    runtime: Runtime,
    input: R,
    scopes: Vec<BTreeMap<String, Binding>>,
    snapshots: BTreeMap<String, Snapshot>,
    remaining: usize,
    top_pc: usize,
    function_depth: usize,
    defers: Vec<Vec<Expr>>,
    test_mode: bool,
    trace: bool,
    trace_json: bool,
    call_path: Vec<u64>,
    next_call_id: u64,
}
impl<R: BufRead> Engine<R> {
    fn fail(&self, at: &Tok, msg: impl AsRef<str>) -> Flow {
        Flow::Error(diagnostic(
            at,
            self.runtime.masked_value(&Value::Text(msg.as_ref().into())),
        ))
    }
    fn tick(&mut self, at: &Tok) -> Exec<()> {
        self.runtime.charge_execution_work(1).map_err(Flow::Error)?;
        if self.remaining == 0 {
            return Err(self.fail(at, "ExecutionBudgetExceeded"));
        }
        self.remaining -= 1;
        Ok(())
    }
    fn get(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
    fn define(
        &mut self,
        name: String,
        value: Value,
        mutable: bool,
        ty: Option<String>,
        at: &Tok,
    ) -> Exec<()> {
        let inferred = value_type(&value, &self.runtime);
        let ty = ty.unwrap_or(inferred.clone());
        if !compatible(&ty, &inferred) {
            return Err(self.fail(
                at,
                format!("type mismatch: expected {ty}, found {inferred}"),
            ));
        }
        let scope = self.scopes.last_mut().unwrap();
        if scope.contains_key(&name) {
            return Err(self.fail(at, format!("duplicate binding {name}; choose another name")));
        }
        let stored = if mutable {
            Value::CellRef(self.runtime.alloc(value.clone())?)
        } else {
            value.clone()
        };
        scope.insert(
            name.clone(),
            Binding {
                value: stored.clone(),
                mutable,
                ty,
            },
        );
        if self.function_depth == 0 {
            self.runtime.set_global(name, stored)?;
        } else {
            self.runtime.set_local(name, stored)?;
        }
        Ok(())
    }
    fn assign(&mut self, lhs: &Expr, op: &str, rhs: Value) -> Exec<()> {
        match &lhs.kind {
            ExprKind::Name(name) => {
                let old = self
                    .get(name)
                    .cloned()
                    .ok_or_else(|| self.fail(&lhs.at, format!("unknown variable {name}")))?;
                if !old.mutable {
                    return Err(self.fail(&lhs.at, format!("cannot assign to let binding {name}")));
                }
                let value = if op == "=" {
                    rhs
                } else {
                    let current = if let Value::CellRef(id) = old.value {
                        self.runtime.heap_get(id).cloned().unwrap_or(Value::Null)
                    } else {
                        old.value.clone()
                    };
                    binary(&op[..1], current, rhs, &lhs.at)?
                };
                let actual = value_type(&value, &self.runtime);
                if !compatible(&old.ty, &actual) {
                    return Err(self.fail(
                        &lhs.at,
                        format!("type mismatch: expected {}, found {actual}", old.ty),
                    ));
                }
                if let Value::CellRef(id) = old.value {
                    self.runtime.heap_set(id, value)?;
                    return Ok(());
                }
                for scope in self.scopes.iter_mut().rev() {
                    if let Some(binding) = scope.get_mut(name) {
                        binding.value = value.clone();
                        break;
                    }
                }
                if self.function_depth == 0 {
                    self.runtime.set_global(name.clone(), value)?;
                } else {
                    self.runtime.set_local(name.clone(), value)?;
                }
                Ok(())
            }
            ExprKind::Member(base, field) => {
                let reference = self.eval(base)?;
                let Value::HeapRef(id) = reference else {
                    return Err(self.fail(&lhs.at, "field assignment requires a struct reference"));
                };
                let mut data = self
                    .runtime
                    .heap_get(id)
                    .cloned()
                    .ok_or_else(|| self.fail(&lhs.at, "unknown object"))?;
                if let Value::Struct(name, fields) = &mut data {
                    let old = fields
                        .get(field)
                        .cloned()
                        .ok_or_else(|| self.fail(&lhs.at, "unknown struct field"))?;
                    let value = if op == "=" {
                        rhs
                    } else {
                        binary(&op[..1], old, rhs, &lhs.at)?
                    };
                    if let Some(def) = self.program.structs.get(name) {
                        if let Some((_, expected)) = def.fields.iter().find(|(key, _)| key == field)
                        {
                            let actual = value_type(&value, &self.runtime);
                            if !compatible(expected, &actual) {
                                return Err(self.fail(
                                    &lhs.at,
                                    format!("field {field} expects {expected}, found {actual}"),
                                ));
                            }
                        }
                    }
                    fields.insert(field.clone(), value);
                    self.runtime.heap_set(id, data)?;
                    Ok(())
                } else {
                    Err(self.fail(&lhs.at, "field assignment requires a struct"))
                }
            }
            _ => Err(self.fail(&lhs.at, "invalid assignment target")),
        }
    }
    fn block(&mut self, body: &[Stmt]) -> Exec<()> {
        self.scopes.push(BTreeMap::new());
        self.defers.push(Vec::new());
        let result = self.statements(body);
        if matches!(&result, Err(Flow::Resume(_))) {
            return result;
        }
        let deferred = self.defers.pop().unwrap();
        for expr in deferred.into_iter().rev() {
            self.eval(&expr)?;
        }
        self.scopes.pop();
        result
    }
    fn statements(&mut self, body: &[Stmt]) -> Exec<()> {
        for stmt in body {
            self.stmt(stmt)?;
        }
        Ok(())
    }
    fn stmt(&mut self, stmt: &Stmt) -> Exec<()> {
        self.tick(&stmt.at)?;
        self.runtime.set_program_counter(self.top_pc);
        match &stmt.kind {
            StmtKind::Let(name, mutable, ty, init) => {
                let value = self.eval(init)?;
                self.define(name.clone(), value, *mutable, ty.clone(), &stmt.at)
            }
            StmtKind::Using(name, init) => {
                let value = self.eval(init)?;
                self.define(
                    name.clone(),
                    value,
                    false,
                    Some("FileHandle".into()),
                    &stmt.at,
                )?;
                let receiver = Expr {
                    kind: ExprKind::Name(name.clone()),
                    at: stmt.at.clone(),
                };
                let method = Expr {
                    kind: ExprKind::Member(Box::new(receiver), "close".into()),
                    at: stmt.at.clone(),
                };
                self.defers.last_mut().unwrap().push(Expr {
                    kind: ExprKind::Call(Box::new(method), Vec::new()),
                    at: stmt.at.clone(),
                });
                Ok(())
            }
            StmtKind::Assign(lhs, op, rhs) => {
                let rhs = self.eval(rhs)?;
                self.assign(lhs, op, rhs)
            }
            StmtKind::Expr(e) => {
                self.eval(e)?;
                Ok(())
            }
            StmtKind::External(fresh, body) => {
                if fresh.is_live() {
                    self.runtime.enter_external_live_task(0)?;
                } else {
                    self.runtime.enter_external(fresh.is_fresh())?;
                }
                let result = self.block(body);
                self.runtime.exit_external()?;
                result
            }
            StmtKind::Block(body) => self.block(body),
            StmtKind::If(cond, yes, no) => match self.eval(cond)? {
                Value::Bool(true) => self.block(yes),
                Value::Bool(false) => self.block(no),
                _ => Err(self.fail(&cond.at, "condition must have type Bool")),
            },
            StmtKind::While(cond, body) => {
                loop {
                    self.tick(&stmt.at)?;
                    match self.eval(cond)? {
                        Value::Bool(false) => break,
                        Value::Bool(true) => {}
                        _ => return Err(self.fail(&cond.at, "condition must have type Bool")),
                    }
                    match self.block(body) {
                        Err(Flow::Break) => break,
                        Err(Flow::Continue) => continue,
                        other => other?,
                    }
                }
                Ok(())
            }
            StmtKind::For(name, start, end, body) => {
                let a = int(self.eval(start)?, &start.at)?;
                let b = int(self.eval(end)?, &end.at)?;
                for n in a..b {
                    self.tick(&stmt.at)?;
                    self.scopes.push(BTreeMap::new());
                    self.define(
                        name.clone(),
                        Value::Int(n),
                        false,
                        Some("Int".into()),
                        &stmt.at,
                    )?;
                    let result = self.block(body);
                    self.scopes.pop();
                    match result {
                        Err(Flow::Break) => break,
                        Err(Flow::Continue) => continue,
                        other => other?,
                    }
                }
                Ok(())
            }
            StmtKind::Break => Err(Flow::Break),
            StmtKind::Continue => Err(Flow::Continue),
            StmtKind::Return(expr) => Err(Flow::Return(if let Some(e) = expr {
                self.eval(e)?
            } else {
                Value::Null
            })),
            StmtKind::Defer(expr) => {
                if self.function_depth == 0 {
                    return Err(self.fail(&stmt.at, "defer requires a function"));
                }
                self.defers.last_mut().unwrap().push(expr.clone());
                Ok(())
            }
            StmtKind::Commit(name) => {
                self.runtime.set_program_counter(self.top_pc + 1);
                self.runtime.commit(name.clone())?;
                if self.trace {
                    eprintln!("{}", self.runtime.trace_checkpoint(name)?);
                }
                self.snapshots.insert(
                    name.clone(),
                    Snapshot {
                        scopes: self.scopes.clone(),
                        pc: self.top_pc + 1,
                        call_path: self.call_path.clone(),
                    },
                );
                Ok(())
            }
            StmtKind::Revert(name) => {
                let snapshot = self
                    .snapshots
                    .get(name)
                    .cloned()
                    .ok_or_else(|| self.fail(&stmt.at, format!("unknown checkpoint {name}")))?;
                if snapshot.call_path != self.call_path
                    || snapshot.scopes.len() != self.scopes.len()
                {
                    return Err(self.fail(
                        &stmt.at,
                        "InvalidContinuation: call frame or scope is no longer present",
                    ));
                }
                self.runtime.revert(name)?;
                self.scopes = snapshot.scopes;
                self.runtime.set_program_counter(self.top_pc + 1);
                Ok(())
            }
            StmtKind::Resume(name) => {
                let snapshot = self
                    .snapshots
                    .get(name)
                    .cloned()
                    .ok_or_else(|| self.fail(&stmt.at, format!("unknown checkpoint {name}")))?;
                if !snapshot.call_path.is_empty() || snapshot.scopes.len() != 1 {
                    return Err(self.fail(&stmt.at, "InvalidContinuation: resuming inside a function or block is not yet supported"));
                }
                self.runtime.revert(name)?;
                self.scopes = snapshot.scopes;
                self.defers = vec![Vec::new()];
                self.call_path.clear();
                self.function_depth = 0;
                Err(Flow::Resume(snapshot.pc))
            }
            StmtKind::Drop(name) => {
                self.runtime.drop_checkpoint(name)?;
                self.snapshots.remove(name);
                Ok(())
            }
            StmtKind::Publish(force) => {
                if self.test_mode {
                    return Err(self.fail(&stmt.at, "publish is unavailable during rewind test"));
                }
                self.runtime
                    .publish(*force, &mut io::stdout().lock(), &mut io::stderr().lock())?;
                Ok(())
            }
            StmtKind::Branch(name, body) => {
                let anchor = self.runtime.begin_branch();
                let scopes = self.scopes.clone();
                let result = self.block(body);
                if result.is_ok() {
                    self.runtime.end_branch(name.clone(), anchor)?;
                }
                if !matches!(&result, Err(Flow::Resume(_))) {
                    self.scopes = scopes;
                }
                result
            }
            StmtKind::Runtime(b, steps) => {
                self.runtime.set_budget(*b)?;
                if let Some(steps) = steps {
                    self.remaining = *steps;
                }
                Ok(())
            }
            StmtKind::Match(value, arms) => {
                let v = self.eval(value)?;
                for (pattern, guard, body) in arms {
                    let mut bindings = BTreeMap::new();
                    if !pattern_matches(pattern, &v, &self.runtime, &mut bindings) {
                        continue;
                    }
                    self.scopes.push(BTreeMap::new());
                    for (name, value) in bindings {
                        self.define(name, value, false, None, &stmt.at)?;
                    }
                    let accept = if let Some(guard) = guard {
                        self.eval(guard)? == Value::Bool(true)
                    } else {
                        true
                    };
                    let result = if accept { Some(self.stmt(body)) } else { None };
                    self.scopes.pop();
                    if let Some(result) = result {
                        return result;
                    }
                }
                Err(self.fail(&stmt.at, "non-exhaustive match"))
            }
        }
    }
    #[allow(dead_code)]
    fn run(&mut self) -> Result<()> {
        while self.top_pc < self.program.stmts.len() {
            let stmt = self.program.stmts[self.top_pc].clone();
            match self.stmt(&stmt) {
                Ok(()) => self.top_pc += 1,
                Err(Flow::Resume(pc)) => self.top_pc = pc,
                Err(Flow::Error(e)) => return Err(e),
                Err(Flow::Return(_)) => {
                    return Err(diagnostic(&stmt.at, "return outside function"))
                }
                Err(Flow::Break | Flow::Continue) => {
                    return Err(diagnostic(&stmt.at, "loop control outside loop"))
                }
            }
        }
        Ok(())
    }
    fn eval(&mut self, e: &Expr) -> Exec<Value> {
        self.tick(&e.at)?;
        match &e.kind {
            ExprKind::Value(v) => Ok(v.clone()),
            ExprKind::Name(n) => {
                if let Some(binding) = self.get(n) {
                    let value = binding.value.clone();
                    if let Value::CellRef(id) = value {
                        Ok(self.runtime.heap_get(id).cloned().unwrap_or(Value::Null))
                    } else {
                        Ok(value)
                    }
                } else if let Some(f) = self.program.functions.get(n) {
                    Ok(Value::Function(
                        n.clone(),
                        format!(
                            "fn({})->{}",
                            f.params
                                .iter()
                                .map(|(_, t)| t.as_str())
                                .collect::<Vec<_>>()
                                .join(","),
                            if f.asynchronous {
                                format!("Task<{}>", f.ret)
                            } else {
                                f.ret.clone()
                            }
                        ),
                    ))
                } else if n.contains("::") {
                    self.call(n, Vec::new(), &e.at)
                } else {
                    Err(self.fail(&e.at, format!("unknown name {n}")))
                }
            }
            ExprKind::Unary(op, inner) => {
                let v = self.eval(inner)?;
                match (op.as_str(), v) {
                    ("-", Value::Int(n)) => n
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| self.fail(&e.at, "integer overflow")),
                    ("-", Value::Float(n)) => Ok(Value::Float((-f64::from_bits(n)).to_bits())),
                    ("!", Value::Bool(n)) => Ok(Value::Bool(!n)),
                    ("move" | "borrow" | "borrowMut", v) => Ok(v),
                    _ => Err(self.fail(&e.at, "invalid unary operand")),
                }
            }
            ExprKind::Binary(op, a, b) => {
                let lhs = self.eval(a)?;
                if op == "&&" {
                    return match lhs {
                        Value::Bool(false) => Ok(Value::Bool(false)),
                        Value::Bool(true) => match self.eval(b)? {
                            Value::Bool(v) => Ok(Value::Bool(v)),
                            _ => Err(self.fail(&b.at, "Bool required")),
                        },
                        _ => Err(self.fail(&a.at, "Bool required")),
                    };
                }
                if op == "||" {
                    return match lhs {
                        Value::Bool(true) => Ok(Value::Bool(true)),
                        Value::Bool(false) => match self.eval(b)? {
                            Value::Bool(v) => Ok(Value::Bool(v)),
                            _ => Err(self.fail(&b.at, "Bool required")),
                        },
                        _ => Err(self.fail(&a.at, "Bool required")),
                    };
                }
                let rhs = self.eval(b)?;
                if matches!(
                    lhs,
                    Value::JsonStream(_)
                        | Value::CsvStream(_)
                        | Value::Regex(_)
                        | Value::Decimal(_)
                        | Value::BigInt(_)
                ) || matches!(
                    rhs,
                    Value::JsonStream(_)
                        | Value::CsvStream(_)
                        | Value::Regex(_)
                        | Value::Decimal(_)
                        | Value::BigInt(_)
                ) {
                    self.runtime
                        .charge_native_work(
                            v100::argument_work(&lhs, &self.runtime)
                                .saturating_add(v100::argument_work(&rhs, &self.runtime)),
                        )
                        .map_err(Flow::Error)?;
                }
                binary(op, lhs, rhs, &e.at)
            }
            ExprKind::Member(base, field) => {
                let value = self.eval(base)?;
                if let Some(Value::Struct(_, fields)) = v05::unfrozen(&value) {
                    let field = fields
                        .get(field)
                        .cloned()
                        .ok_or_else(|| self.fail(&e.at, "unknown frozen field"))?;
                    return Ok(v05::freeze_member(field, &self.runtime));
                }
                match value {
                    Value::FileError(error) => match field.as_str() {
                        "code" => Ok(Value::Text(error.code.into())),
                        "path" => Ok(Value::Text(error.path.into())),
                        "cause" => Ok(Value::Option(
                            error
                                .causes
                                .first()
                                .cloned()
                                .map(|s| Box::new(Value::Text(s.into()))),
                        )),
                        "causes" => Ok(Value::HeapRef(
                            self.runtime.alloc(Value::TypedList(
                                "String".into(),
                                error
                                    .causes
                                    .into_iter()
                                    .map(|text| Value::Text(text.into()))
                                    .collect(),
                            ))?,
                        )),
                        _ => Err(self.fail(&e.at, "unknown FileError field")),
                    },
                    Value::Handle(id) if field == "position" => {
                        Ok(Value::Int(self.runtime.handle(id)?.position as i64))
                    }
                    Value::HeapRef(id) => match self.runtime.heap_get(id) {
                        Some(Value::Struct(_, fields)) => fields
                            .get(field)
                            .cloned()
                            .ok_or_else(|| self.fail(&e.at, "unknown field")),
                        _ => Err(self.fail(&e.at, "unknown property")),
                    },
                    Value::Struct(_, fields) => fields
                        .get(field)
                        .cloned()
                        .ok_or_else(|| self.fail(&e.at, "unknown field")),
                    _ => Err(self.fail(&e.at, "unknown property")),
                }
            }
            ExprKind::Call(callee, args) => {
                let mut values = Vec::new();
                for arg in args {
                    values.push(self.eval(arg)?);
                }
                match &callee.kind {
                    ExprKind::Name(name) => self.call(name, values, &e.at),
                    ExprKind::Member(base, method) => {
                        if let ExprKind::Name(name) = &base.kind {
                            if self.get(name).is_none() {
                                return self.builtin(name, method, values, &e.at);
                            }
                        }
                        let target = self.eval(base)?;
                        self.method(target, method, values, &e.at)
                    }
                    _ => Err(self.fail(&e.at, "invalid call target")),
                }
            }
            ExprKind::Try(inner) => match self.eval(inner)? {
                Value::Result(Ok(v)) => Ok(*v),
                Value::Result(Err(v)) => Err(Flow::Return(Value::Result(Err(v)))),
                _ => Err(self.fail(&e.at, "'?' requires Result")),
            },
            ExprKind::Match(value, arms) => {
                let value = self.eval(value)?;
                for (pattern, guard, arm) in arms {
                    let mut bindings = BTreeMap::new();
                    if !pattern_matches(pattern, &value, &self.runtime, &mut bindings) {
                        continue;
                    }
                    self.scopes.push(BTreeMap::new());
                    for (name, value) in bindings {
                        self.define(name, value, false, None, &e.at)?;
                    }
                    let accept = if let Some(guard) = guard {
                        self.eval(guard)? == Value::Bool(true)
                    } else {
                        true
                    };
                    let result = if accept { Some(self.eval(arm)) } else { None };
                    self.scopes.pop();
                    if let Some(result) = result {
                        return result;
                    }
                }
                Err(self.fail(&e.at, "non-exhaustive match"))
            }
            ExprKind::Closure(_, _, _) => {
                Err(self.fail(&e.at, "closure cannot be evaluated in deferred expression"))
            }
            ExprKind::NamedConstructor(name, fields) => {
                let mut values = Vec::new();
                for (field, expr) in fields {
                    values.push((field.clone(), self.eval(expr)?));
                }
                self.call_named(name, values, &e.at)
            }
        }
    }
}

impl<R: BufRead> Engine<R> {
    fn call_named(&mut self, name: &str, fields: Vec<(String, Value)>, at: &Tok) -> Exec<Value> {
        let (enum_name, variant) = name
            .split_once("::")
            .ok_or_else(|| self.fail(at, "named constructor requires enum variant"))?;
        let def = self
            .program
            .enums
            .get(enum_name.split('<').next().unwrap_or(enum_name))
            .ok_or_else(|| self.fail(at, "unknown enum"))?;
        let expected = def
            .variants
            .get(variant)
            .ok_or_else(|| self.fail(at, "unknown variant"))?;
        let mut supplied = fields.into_iter().collect::<BTreeMap<_, _>>();
        let args = expected
            .iter()
            .map(|(field, _)| {
                supplied
                    .remove(field)
                    .ok_or_else(|| self.fail(at, format!("missing field {field}")))
            })
            .collect::<Exec<Vec<_>>>()?;
        if !supplied.is_empty() {
            return Err(self.fail(at, "unknown named constructor field"));
        }
        self.call(name, args, at)
    }
    fn call(&mut self, name: &str, args: Vec<Value>, at: &Tok) -> Exec<Value> {
        if matches!(
            self.program.language.as_str(),
            "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) && !self
            .program
            .functions
            .contains_key(name.split('<').next().unwrap_or(name))
        {
            self.runtime
                .charge_native_work(v100::call_work(
                    name,
                    &args,
                    &self.runtime,
                    &self.program.language,
                ))
                .map_err(Flow::Error)?;
        }
        if matches!(
            self.program.language.as_str(),
            "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) {
            if self.program.language == "0.9.9" {
                if let Some(work) = v092::work(name, &args, &self.runtime) {
                    self.runtime.charge_native_work(work).map_err(Flow::Error)?;
                }
            }
            if let Some(v) = v092::call(name, &args, &mut self.runtime).map_err(Flow::Error)? {
                return Ok(v);
            }
        }
        if matches!(
            self.program.language.as_str(),
            "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) {
            if let Some(value) = v091::call(&self.runtime, name, &args).map_err(Flow::Error)? {
                return Ok(value);
            }
        }
        if name == "$tuple"
            && matches!(
                self.program.language.as_str(),
                "0.6"
                    | "0.7"
                    | "0.8"
                    | "0.9"
                    | "0.9.1"
                    | "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "1.9.49"
                    | "1.9.50"
                    | "1.9.51"
                    | "2.0.0"
            )
        {
            return Ok(Value::Struct(
                format!(
                    "Tuple<{}>",
                    args.iter()
                        .map(|v| value_type(v, &self.runtime))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                args.into_iter()
                    .enumerate()
                    .map(|(i, v)| (format!("_{i}"), v))
                    .collect(),
            ));
        }
        if matches!(
            self.program.language.as_str(),
            "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) && matches!(name, "secret" | "reveal")
        {
            if args.len() != 1 {
                return Err(self.fail(at, "secret/reveal requires one argument"));
            }
            if matches!(
                self.program.language.as_str(),
                "0.6"
                    | "0.7"
                    | "0.8"
                    | "0.9"
                    | "0.9.1"
                    | "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "1.9.49"
                    | "1.9.50"
                    | "1.9.51"
                    | "2.0.0"
            ) {
                self.runtime.register_secret_value(&args[0]);
                self.runtime.check_sensitive_accounting()?;
            }
            return if name == "secret" {
                Ok(v05::secret(
                    args[0].clone(),
                    value_type(&args[0], &self.runtime),
                ))
            } else {
                v05::unsecret(&args[0])
                    .cloned()
                    .ok_or_else(|| self.fail(at, "reveal requires Secret<T>"))
            };
        }
        if matches!(
            self.program.language.as_str(),
            "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) && matches!(name, "freeze" | "thaw")
        {
            if args.len() != 1 {
                return Err(self.fail(at, "freeze/thaw requires one argument"));
            }
            if name == "freeze" {
                if !self.runtime.live_native_ids(&args).is_empty() {
                    return Err(self.fail(at, "native resources cannot be frozen"));
                }
                let value = self.ordered_key(&args[0], at)?;
                return Ok(v05::frozen(value, &self.runtime));
            }
            // Frozen generic reads preserve scalar values without a wrapper.
            // Their static type still requires thaw; scalars own no mutable state.
            let value = if matches!(
                self.program.language.as_str(),
                "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "1.9.49"
                    | "1.9.50"
                    | "1.9.51"
                    | "2.0.0"
            ) && matches!(
                &args[0],
                Value::Bool(_)
                    | Value::Int(_)
                    | Value::Float(_)
                    | Value::Text(_)
                    | Value::Bytes(_)
                    | Value::Null
            ) {
                &args[0]
            } else {
                v05::unfrozen(&args[0]).ok_or_else(|| self.fail(at, "thaw requires Frozen<T>"))?
            };
            return v05::thaw(&mut self.runtime, value).map_err(Flow::Error);
        }
        if name.contains("::") {
            let actual = args
                .iter()
                .map(|a| value_type(a, &self.runtime))
                .collect::<Vec<_>>();
            if let Some(ty) =
                enum_constructor_type(&self.program, name, &actual, at).map_err(Flow::Error)?
            {
                let (enum_name, variant) = name.split_once("::").unwrap();
                let fields = &self.program.enums[enum_name.split('<').next().unwrap_or(enum_name)]
                    .variants[variant];
                return Ok(Value::Enum(
                    ty,
                    variant.into(),
                    fields
                        .iter()
                        .map(|(name, _)| name.clone())
                        .zip(args)
                        .collect(),
                ));
            }
        }
        if let Some(inner) = name.strip_prefix("List<").and_then(|s| s.strip_suffix('>')) {
            if !args.is_empty() {
                return Err(self.fail(at, "List constructor takes no arguments"));
            }
            return Ok(Value::HeapRef(
                self.runtime
                    .alloc(Value::TypedList(inner.into(), Vec::new().into()))?,
            ));
        }
        if let Some(inner) = name.strip_prefix("Map<").and_then(|s| s.strip_suffix('>')) {
            if !args.is_empty() {
                return Err(self.fail(at, "Map constructor takes no arguments"));
            }
            let types = split_type_args(inner);
            if types.len() != 2 {
                return Err(self.fail(at, "Map requires key and value types"));
            }
            if !trait_satisfied(&self.program, "Ord", types[0]) {
                return Err(self.fail(at, "Map key type must implement Ord"));
            }
            if !builtin_key_type(&self.program, types[0]) {
                return Ok(Value::HeapRef(self.runtime.alloc(Value::OrderedMap(
                    types[0].into(),
                    types[1].into(),
                    Vec::new(),
                ))?));
            }
            return Ok(Value::HeapRef(self.runtime.alloc(Value::TypedMap(
                types[0].into(),
                types[1].into(),
                BTreeMap::new().into(),
            ))?));
        }
        match name {
            "List" if args.is_empty() => {
                return Ok(Value::HeapRef(self.runtime.alloc(Value::List(Vec::new()))?))
            }
            "Map" if args.is_empty() => {
                return Ok(Value::HeapRef(
                    self.runtime.alloc(Value::Map(BTreeMap::new().into()))?,
                ))
            }
            "Bytes" if args.len() == 1 => match &args[0] {
                Value::Text(s) => return Ok(Value::Bytes(s.as_bytes().to_vec().into())),
                _ => return Err(self.fail(at, "Bytes expects a String")),
            },
            "Ok" if args.len() == 1 => return Ok(Value::Result(Ok(Box::new(args[0].clone())))),
            "Err" if args.len() == 1 => return Ok(Value::Result(Err(Box::new(args[0].clone())))),
            "Some" if args.len() == 1 => return Ok(Value::Option(Some(Box::new(args[0].clone())))),
            "assert" if args == [Value::Bool(true)] => return Ok(Value::Null),
            "assert" if args == [Value::Bool(false)] => {
                return Err(self.fail(at, "assertion failed"))
            }
            "assert_eq" if args.len() == 2 => {
                if args[0] == args[1] {
                    return Ok(Value::Null);
                }
                return Err(self.fail(
                    at,
                    format!(
                        "assert_eq failed: left={} right={}",
                        self.runtime.display_value(&args[0]),
                        self.runtime.display_value(&args[1])
                    ),
                ));
            }
            "panic" if args.len() == 1 => return Err(self.fail(at, format!("panic: {}", args[0]))),
            _ => {}
        }
        if let Some(def) = self
            .program
            .structs
            .get(name.split('<').next().unwrap_or(name))
            .cloned()
        {
            if args.len() != def.fields.len() {
                return Err(self.fail(at, format!("{name} expects {} fields", def.fields.len())));
            }
            let mut fields = BTreeMap::new();
            let actuals = args
                .iter()
                .map(|arg| value_type(arg, &self.runtime))
                .collect::<Vec<_>>();
            let substitutions = infer_arguments(&def.fields, &def.type_params, &actuals, at)
                .map_err(Flow::Error)?;
            for ((field, ty), arg) in def.fields.iter().zip(args) {
                let actual = value_type(&arg, &self.runtime);
                let expected = substitute_type(ty, &substitutions);
                if !compatible(&expected, &actual) {
                    return Err(self.fail(
                        at,
                        format!("field {field} expects {expected}, found {actual}"),
                    ));
                }
                fields.insert(field.clone(), arg);
            }
            let concrete = if def.type_params.is_empty() {
                name.into()
            } else {
                format!(
                    "{}<{}>",
                    name.split('<').next().unwrap(),
                    def.type_params
                        .iter()
                        .map(|p| substitutions[p].as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };
            return if def.immutable {
                Ok(Value::Struct(concrete, fields))
            } else {
                Ok(Value::HeapRef(
                    self.runtime.alloc(Value::Struct(concrete, fields))?,
                ))
            };
        }
        let mut def = self
            .program
            .functions
            .get(name.split('<').next().unwrap_or(name))
            .cloned()
            .ok_or_else(|| self.fail(at, format!("unknown function {name}")))?;
        let types = args
            .iter()
            .map(|v| value_type(v, &self.runtime))
            .collect::<Vec<_>>();
        let parameters = def
            .type_params
            .iter()
            .map(|(p, _)| p.clone())
            .collect::<Vec<_>>();
        let substitutions = infer_call_arguments(name, &def.params, &parameters, &types, at)
            .map_err(Flow::Error)?;
        def.ret = substitute_type(&def.ret, &substitutions);
        for (_, ty) in &mut def.params {
            *ty = substitute_type(ty, &substitutions);
        }
        for stmt in &mut def.body {
            rename_stmt(stmt, &substitutions);
        }
        if args.len() != def.params.len() {
            return Err(self.fail(at, format!("{name} expects {} arguments", def.params.len())));
        }
        if self.function_depth >= 1024 {
            return Err(self.fail(at, "ExecutionBudgetExceeded: call depth"));
        }
        self.runtime.push_frame(self.top_pc)?;
        self.function_depth += 1;
        self.call_path.push(self.next_call_id);
        self.next_call_id += 1;
        self.scopes.push(BTreeMap::new());
        self.defers.push(Vec::new());
        for ((param, ty), arg) in def.params.iter().zip(args) {
            let _ = ty;
            self.define(param.clone(), arg, false, None, at)?;
        }
        let body = self.statements(&def.body);
        if matches!(&body, Err(Flow::Resume(_))) {
            return body.map(|_| Value::Null);
        }
        let deferred = self.defers.pop().unwrap();
        let mut defer_error = None;
        for expr in deferred.into_iter().rev() {
            if let Err(flow) = self.eval(&expr) {
                defer_error = Some(flow);
                break;
            }
        }
        self.scopes.pop();
        self.function_depth -= 1;
        self.call_path.pop();
        self.runtime.pop_frame()?;
        if let Some(err) = defer_error {
            return Err(err);
        }
        let value = match body {
            Ok(()) => Value::Null,
            Err(Flow::Return(v)) => v,
            Err(other) => return Err(other),
        };
        let actual = value_type(&value, &self.runtime);
        if !compatible(&def.ret, &actual) {
            return Err(self.fail(at, format!("{name} returns {actual}, expected {}", def.ret)));
        }
        Ok(value)
    }
    fn ordered_key(&self, value: &Value, at: &Tok) -> Exec<Value> {
        fn freeze<R: BufRead>(
            engine: &Engine<R>,
            value: &Value,
            at: &Tok,
            path: &mut BTreeSet<u64>,
            depth: usize,
            remaining: &mut usize,
        ) -> Exec<Value> {
            if depth > 64 || *remaining == 0 {
                return Err(engine.fail(at, "Map key snapshot budget exceeded"));
            }
            *remaining -= 1;
            let mut child = |v: &Value| freeze(engine, v, at, path, depth + 1, remaining);
            Ok(match value {
                Value::HeapRef(id) | Value::CellRef(id) => {
                    if !path.insert(*id) {
                        return Err(engine.fail(at, "cyclic Map key"));
                    }
                    let value = engine
                        .runtime
                        .heap_get(*id)
                        .ok_or_else(|| engine.fail(at, "missing Map key object"))?;
                    let frozen = freeze(engine, value, at, path, depth + 1, remaining)?;
                    path.remove(id);
                    frozen
                }
                Value::Struct(ty, fields) => Value::Struct(
                    ty.clone(),
                    fields
                        .iter()
                        .map(|(n, v)| Ok((n.clone(), child(v)?)))
                        .collect::<Exec<_>>()?,
                ),
                Value::Enum(ty, variant, fields) => Value::Enum(
                    ty.clone(),
                    variant.clone(),
                    fields
                        .iter()
                        .map(|(n, v)| Ok((n.clone(), child(v)?)))
                        .collect::<Exec<_>>()?,
                ),
                Value::List(values) => {
                    Value::List(values.iter().map(&mut child).collect::<Exec<_>>()?)
                }
                Value::TypedList(ty, values) => Value::TypedList(
                    ty.clone(),
                    values.iter().map(&mut child).collect::<Exec<_>>()?,
                ),
                Value::Map(values) => Value::Map(
                    values
                        .iter()
                        .map(|(k, v)| Ok((k.clone(), child(v)?)))
                        .collect::<Exec<_>>()?,
                ),
                Value::TypedMap(k, v, values) => Value::TypedMap(
                    k.clone(),
                    v.clone(),
                    values
                        .iter()
                        .map(|(k, v)| Ok((k.clone(), child(v)?)))
                        .collect::<Exec<_>>()?,
                ),
                Value::OrderedMap(k, v, values) => Value::OrderedMap(
                    k.clone(),
                    v.clone(),
                    values
                        .iter()
                        .map(|(k, v)| Ok((child(k)?, child(v)?)))
                        .collect::<Exec<_>>()?,
                ),
                Value::Option(value) => {
                    Value::Option(value.as_ref().map(|v| child(v).map(Box::new)).transpose()?)
                }
                Value::Result(Ok(v)) => Value::Result(Ok(Box::new(child(v)?))),
                Value::Result(Err(v)) => Value::Result(Err(Box::new(child(v)?))),
                Value::Handle(_) | Value::Closure(_, _, _) | Value::Function(_, _) => {
                    return Err(engine.fail(at, "Map keys cannot capture resources or functions"))
                }
                other => other.clone(),
            })
        }
        freeze(self, value, at, &mut BTreeSet::new(), 0, &mut 10_000)
    }
    fn ordered_position(
        &mut self,
        ty: &str,
        entries: &[(Value, Value)],
        key: &Value,
        at: &Tok,
    ) -> Exec<std::result::Result<usize, usize>> {
        let symbol = self
            .program
            .impls
            .get(&("Ord".into(), ty.into()))
            .and_then(|m| m.get("cmp"))
            .cloned()
            .ok_or_else(|| self.fail(at, format!("{ty} has no Ord.cmp implementation")))?;
        let mut position = entries.len();
        for (i, (existing, _)) in entries.iter().enumerate() {
            let forward = self.call(&symbol, vec![key.clone(), existing.clone()], at)?;
            let reverse = self.call(&symbol, vec![existing.clone(), key.clone()], at)?;
            let (Value::Int(forward), Value::Int(reverse)) = (forward, reverse) else {
                return Err(self.fail(at, "Ord.cmp must return Int"));
            };
            if forward.signum() != -reverse.signum() {
                return Err(self.fail(at, "Ord.cmp is not antisymmetric"));
            }
            if forward == 0 {
                return Ok(Ok(i));
            }
            if forward < 0 && position == entries.len() {
                position = i;
            }
            if forward > 0 && position != entries.len() {
                return Err(self.fail(at, "Ord.cmp is not a total order"));
            }
        }
        Ok(Err(position))
    }
    fn method(&mut self, target: Value, method: &str, args: Vec<Value>, at: &Tok) -> Exec<Value> {
        if matches!(
            self.program.language.as_str(),
            "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) {
            self.runtime
                .charge_native_work(v100::method_work(&target, method, &args, &self.runtime))
                .map_err(Flow::Error)?;
        }
        self.method_inner(target, method, args, at)
    }
    fn method_inner(
        &mut self,
        target: Value,
        method: &str,
        args: Vec<Value>,
        at: &Tok,
    ) -> Exec<Value> {
        if matches!(
            self.program.language.as_str(),
            "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) {
            if let Some(inner) = v05::unsecret(&target) {
                let value = self.method(inner.clone(), method, args, at)?;
                return Ok(v05::secret(
                    value.clone(),
                    value_type(&value, &self.runtime),
                ));
            }
            if let Some(value) = v06::primitive_method(&target, method, &args) {
                return Ok(value);
            }
        }
        if let Some(value) = v05::unfrozen(&target) {
            let result = self.method(value.clone(), method, args, at)?;
            return Ok(if method == "iter" {
                result
            } else {
                v05::freeze_member(result, &self.runtime)
            });
        }
        if matches!(
            self.program.language.as_str(),
            "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        ) && args.is_empty()
        {
            if method == "iter" {
                let value = self.ordered_key(&target, at)?;
                if let Some(iterator) =
                    v05::new_iterator(&mut self.runtime, &value).map_err(Flow::Error)?
                {
                    return Ok(iterator);
                }
            }
            if method == "next" {
                if let Some(value) =
                    v05::iterator_next(&mut self.runtime, &target).map_err(Flow::Error)?
                {
                    return Ok(
                        if matches!(
                            self.program.language.as_str(),
                            "0.6"
                                | "0.7"
                                | "0.8"
                                | "0.9"
                                | "0.9.1"
                                | "0.9.2"
                                | "0.9.3"
                                | "0.9.4"
                                | "0.9.5"
                                | "0.9.6"
                                | "0.9.7"
                                | "0.9.8"
                                | "0.9.9"
                                | "1.0.0"
                                | "1.1.0"
                                | "1.2.0"
                                | "1.3.0"
                                | "1.4.0"
                                | "1.5.0"
                                | "1.6.0"
                                | "1.6.1"
                                | "1.7.0"
                                | "1.7.1"
                                | "1.8.0"
                                | "1.8.1"
                                | "1.8.2"
                                | "1.8.3"
                                | "1.8.4"
                                | "1.8.5"
                                | "1.8.6"
                                | "1.8.7"
                                | "1.8.8"
                                | "1.9.0"
                                | "1.9.1"
                                | "1.9.2"
                                | "1.9.3"
                                | "1.9.4"
                                | "1.9.5"
                                | "1.9.6"
                                | "1.9.7"
                                | "1.9.8"
                                | "1.9.9"
                                | "1.9.10"
                                | "1.9.11"
                                | "1.9.12"
                                | "1.9.13"
                                | "1.9.14"
                                | "1.9.15"
                                | "1.9.16"
                                | "1.9.17"
                                | "1.9.18"
                                | "1.9.19"
                                | "1.9.20"
                                | "1.9.21"
                                | "1.9.22"
                                | "1.9.23"
                                | "1.9.24"
                                | "1.9.25"
                                | "1.9.26"
                                | "1.9.27"
                                | "1.9.28"
                                | "1.9.29"
                                | "1.9.30"
                                | "1.9.31"
                                | "1.9.32"
                                | "1.9.33"
                                | "1.9.34"
                                | "1.9.35"
                                | "1.9.36"
                                | "1.9.37"
                                | "1.9.38"
                                | "1.9.39"
                                | "1.9.40"
                                | "1.9.41"
                                | "1.9.42"
                                | "1.9.43"
                                | "1.9.44"
                                | "1.9.45"
                                | "1.9.46"
                                | "1.9.47"
                                | "1.9.48"
                                | "1.9.49"
                                | "1.9.50"
                                | "1.9.51"
                                | "2.0.0"
                        ) {
                            v06::immutable_tuple(value, &self.runtime)
                        } else {
                            value
                        },
                    );
                }
            }
        }
        let unit = Value::Null;
        if let Some(key) = MapKey::from_value(&target) {
            match (method, args.as_slice()) {
                ("eq", [other]) => {
                    return Ok(Value::Bool(
                        MapKey::from_value(other).as_ref() == Some(&key),
                    ))
                }
                ("cmp", [other]) => {
                    let other = MapKey::from_value(other)
                        .ok_or_else(|| self.fail(at, "comparison requires a primitive value"))?;
                    return Ok(Value::Int(match key.cmp(&other) {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    }));
                }
                ("display", []) => return Ok(Value::Text(target.to_string().into())),
                ("hash", []) => {
                    if let Value::Decimal(value) = &target {
                        return Ok(Value::Int(value.numeric_hash() as i64));
                    }
                    let bytes = format!("{key:?}");
                    let mut hash = 0xcbf29ce484222325u64;
                    for byte in bytes.bytes() {
                        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                    }
                    return Ok(Value::Int(hash as i64));
                }
                _ => {}
            }
        }
        match target {
            Value::List(list) => match (method, args.as_slice()) {
                ("get", [Value::Int(n)]) if *n >= 0 => list
                    .get(*n as usize)
                    .cloned()
                    .ok_or_else(|| self.fail(at, "list index out of bounds")),
                ("len", []) => Ok(Value::Int(list.len() as i64)),
                _ => Err(self.fail(at, "unsupported immutable List method")),
            },
            Value::TypedList(_, list) => match (method, args.as_slice()) {
                ("get", [Value::Int(n)]) if *n >= 0 => list
                    .get(*n as usize)
                    .cloned()
                    .ok_or_else(|| self.fail(at, "list index out of bounds")),
                ("len", []) => Ok(Value::Int(list.len() as i64)),
                _ => Err(self.fail(at, "unsupported immutable List method")),
            },
            Value::Map(map) => match (method, args.as_slice()) {
                ("get", [key]) => {
                    let key = MapKey::from_value(key)
                        .ok_or_else(|| self.fail(at, "unsupported Map key type"))?;
                    Ok(Value::Option(map.get(&key).cloned().map(Box::new)))
                }
                ("keys", []) => Ok(Value::List(map.keys().map(MapKey::value).collect())),
                ("len", []) => Ok(Value::Int(map.len() as i64)),
                _ => Err(self.fail(at, "unsupported immutable Map method")),
            },
            Value::TypedMap(_, _, map) => match (method, args.as_slice()) {
                ("get", [key]) => {
                    let key = MapKey::from_value(key)
                        .ok_or_else(|| self.fail(at, "unsupported Map key type"))?;
                    Ok(Value::Option(map.get(&key).cloned().map(Box::new)))
                }
                ("keys", []) => Ok(Value::List(map.keys().map(MapKey::value).collect())),
                ("len", []) => Ok(Value::Int(map.len() as i64)),
                _ => Err(self.fail(at, "unsupported immutable Map method")),
            },
            Value::OrderedMap(key_ty, _, entries) => match (method, args.as_slice()) {
                ("get", [key]) => {
                    let key = self.ordered_key(key, at)?;
                    let position = self.ordered_position(&key_ty, &entries, &key, at)?;
                    Ok(Value::Option(
                        position.ok().map(|i| Box::new(entries[i].1.clone())),
                    ))
                }
                ("keys", []) => Ok(Value::HeapRef(self.runtime.alloc(Value::TypedList(
                    key_ty,
                    entries.into_iter().map(|(key, _)| key).collect(),
                ))?)),
                ("len", []) => Ok(Value::Int(entries.len() as i64)),
                _ => Err(self.fail(at, "unsupported immutable Map method")),
            },
            Value::HeapRef(id) => {
                // Read only the requested element; never clone the collection root's payload.
                if method == "len" && args.is_empty() {
                    let length = match self.runtime.heap_get(id) {
                        Some(Value::List(xs)) => Some(xs.len()),
                        Some(Value::TypedList(_, xs)) => Some(xs.len()),
                        Some(Value::Map(xs)) => Some(xs.len()),
                        Some(Value::TypedMap(_, _, xs)) => Some(xs.len()),
                        Some(Value::OrderedMap(_, _, xs)) => Some(xs.len()),
                        _ => None,
                    };
                    if let Some(n) = length {
                        return Ok(Value::Int(n as i64));
                    }
                }
                if method == "get" && args.len() == 1 {
                    match self.runtime.heap_get(id) {
                        Some(Value::TypedList(_, xs)) => {
                            let Value::Int(i) = args[0] else {
                                return Err(self.fail(at, "list index must be Int"));
                            };
                            return xs
                                .get(i as usize)
                                .cloned()
                                .ok_or_else(|| self.fail(at, "list index out of bounds"));
                        }
                        Some(Value::TypedMap(_, _, xs)) => {
                            let key = MapKey::from_value(&args[0])
                                .ok_or_else(|| self.fail(at, "unsupported Map key type"))?;
                            return Ok(Value::Option(xs.get(&key).cloned().map(Box::new)));
                        }
                        Some(Value::OrderedMap(ty, _, xs)) => {
                            let ty = ty.clone();
                            let length = xs.len();
                            let key = self.ordered_key(&args[0], at)?;
                            for i in 0..length {
                                let (k, v) = match self.runtime.heap_get(id) {
                                    Some(Value::OrderedMap(_, _, xs)) => xs[i].clone(),
                                    _ => unreachable!(),
                                };
                                match self.ordered_position(&ty, &[(k, Value::Null)], &key, at)? {
                                    Ok(_) => return Ok(Value::Option(Some(Box::new(v)))),
                                    Err(0) => return Ok(Value::Option(None)),
                                    Err(_) => {}
                                }
                            }
                            return Ok(Value::Option(None));
                        }
                        _ => {}
                    }
                }
                let old = self
                    .runtime
                    .heap_get(id)
                    .cloned()
                    .ok_or_else(|| self.fail(at, "unknown object"))?;
                match old {
                    Value::OrderedMap(key_ty, value_ty, mut entries) => {
                        match (method, args.as_slice()) {
                            ("set", [key, value]) => {
                                let actual_key = value_type(key, &self.runtime);
                                let actual_value = value_type(value, &self.runtime);
                                if !compatible(&key_ty, &actual_key)
                                    || !compatible(&value_ty, &actual_value)
                                {
                                    return Err(self.fail(at,format!("Map<{key_ty},{value_ty}> cannot contain {actual_key},{actual_value}")));
                                }
                                let key = self.ordered_key(key, at)?;
                                match self.ordered_position(&key_ty, &entries, &key, at)? {
                                    Ok(i) => entries[i] = (key, value.clone()),
                                    Err(i) => entries.insert(i, (key, value.clone())),
                                }
                                self.runtime
                                    .heap_set(id, Value::OrderedMap(key_ty, value_ty, entries))?;
                                Ok(unit)
                            }
                            ("get", [key]) => {
                                let key = self.ordered_key(key, at)?;
                                let position =
                                    self.ordered_position(&key_ty, &entries, &key, at)?;
                                Ok(Value::Option(
                                    position.ok().map(|i| Box::new(entries[i].1.clone())),
                                ))
                            }
                            ("remove", [key]) => {
                                let key = self.ordered_key(key, at)?;
                                if let Ok(i) = self.ordered_position(&key_ty, &entries, &key, at)? {
                                    entries.remove(i);
                                }
                                self.runtime
                                    .heap_set(id, Value::OrderedMap(key_ty, value_ty, entries))?;
                                Ok(unit)
                            }
                            ("keys", []) => {
                                Ok(Value::HeapRef(self.runtime.alloc(Value::TypedList(
                                    key_ty,
                                    entries.into_iter().map(|(key, _)| key).collect(),
                                ))?))
                            }
                            ("len", []) => Ok(Value::Int(entries.len() as i64)),
                            _ => Err(self.fail(at, "unsupported Map method")),
                        }
                    }
                    Value::TypedList(ty, mut list) => {
                        match (method, args.as_slice()) {
                            ("pop", [])
                                if matches!(
                                    self.program.language.as_str(),
                                    "0.9.3"
                                        | "0.9.4"
                                        | "0.9.5"
                                        | "0.9.6"
                                        | "0.9.7"
                                        | "0.9.8"
                                        | "0.9.9"
                                        | "1.0.0"
                                        | "1.1.0"
                                        | "1.2.0"
                                        | "1.3.0"
                                        | "1.4.0"
                                        | "1.5.0"
                                        | "1.6.0"
                                        | "1.6.1"
                                        | "1.7.0"
                                        | "1.7.1"
                                        | "1.8.0"
                                        | "1.8.1"
                                        | "1.8.2"
                                        | "1.8.3"
                                        | "1.8.4"
                                        | "1.8.5"
                                        | "1.8.6"
                                        | "1.8.7"
                                        | "1.8.8"
                                        | "1.9.0"
                                        | "1.9.1"
                                        | "1.9.2"
                                        | "1.9.3"
                                        | "1.9.4"
                                        | "1.9.5"
                                        | "1.9.6"
                                        | "1.9.7"
                                        | "1.9.8"
                                        | "1.9.9"
                                        | "1.9.10"
                                        | "1.9.11"
                                        | "1.9.12"
                                        | "1.9.13"
                                        | "1.9.14"
                                        | "1.9.15"
                                        | "1.9.16"
                                        | "1.9.17"
                                        | "1.9.18"
                                        | "1.9.19"
                                        | "1.9.20"
                                        | "1.9.21"
                                        | "1.9.22"
                                        | "1.9.23"
                                        | "1.9.24"
                                        | "1.9.25"
                                        | "1.9.26"
                                        | "1.9.27"
                                        | "1.9.28"
                                        | "1.9.29"
                                        | "1.9.30"
                                        | "1.9.31"
                                        | "1.9.32"
                                        | "1.9.33"
                                        | "1.9.34"
                                        | "1.9.35"
                                        | "1.9.36"
                                        | "1.9.37"
                                        | "1.9.38"
                                        | "1.9.39"
                                        | "1.9.40"
                                        | "1.9.41"
                                        | "1.9.42"
                                        | "1.9.43"
                                        | "1.9.44"
                                        | "1.9.45"
                                        | "1.9.46"
                                        | "1.9.47"
                                        | "1.9.48"
                                        | "1.9.49"
                                        | "1.9.50"
                                        | "1.9.51"
                                        | "2.0.0"
                                ) =>
                            {
                                let value = list.pop();
                                self.runtime.heap_set(id, Value::TypedList(ty, list))?;
                                Ok(Value::Option(value.map(Box::new)))
                            }
                            ("add", [value]) | ("push", [value]) => {
                                let actual = value_type(value, &self.runtime);
                                if !compatible(&ty, &actual) {
                                    return Err(self
                                        .fail(at, format!("List<{ty}> cannot contain {actual}")));
                                }
                                list.push(value.clone());
                                self.runtime.heap_set(id, Value::TypedList(ty, list))?;
                                Ok(unit)
                            }
                            ("get", [Value::Int(n)]) if *n >= 0 => list
                                .get(*n as usize)
                                .cloned()
                                .ok_or_else(|| self.fail(at, "list index out of bounds")),
                            ("len", []) => Ok(Value::Int(list.len() as i64)),
                            ("set", [Value::Int(n), value])
                                if *n >= 0 && (*n as usize) < list.len() =>
                            {
                                let actual = value_type(value, &self.runtime);
                                if !compatible(&ty, &actual) {
                                    return Err(self
                                        .fail(at, format!("List<{ty}> cannot contain {actual}")));
                                }
                                list.set(*n as usize, value.clone());
                                self.runtime.heap_set(id, Value::TypedList(ty, list))?;
                                Ok(unit)
                            }
                            _ => Err(self.fail(at, "unsupported List method")),
                        }
                    }
                    Value::TypedMap(key_ty, value_ty, mut map) => match (method, args.as_slice()) {
                        ("set", [key, value]) => {
                            let key_actual = value_type(key, &self.runtime);
                            let value_actual = value_type(value, &self.runtime);
                            if !compatible(&key_ty, &key_actual)
                                || !compatible(&value_ty, &value_actual)
                            {
                                return Err(self.fail(at, format!("Map<{key_ty},{value_ty}> cannot contain {key_actual},{value_actual}")));
                            }
                            let key = MapKey::from_value(key)
                                .ok_or_else(|| self.fail(at, "unsupported Map key type"))?;
                            map.set(key, value.clone());
                            self.runtime
                                .heap_set(id, Value::TypedMap(key_ty, value_ty, map))?;
                            Ok(unit)
                        }
                        ("get", [key]) => {
                            let key = MapKey::from_value(key)
                                .ok_or_else(|| self.fail(at, "unsupported Map key type"))?;
                            Ok(Value::Option(map.get(&key).cloned().map(Box::new)))
                        }
                        ("remove", [key]) => {
                            let key = MapKey::from_value(key)
                                .ok_or_else(|| self.fail(at, "unsupported Map key type"))?;
                            map.remove(&key);
                            self.runtime
                                .heap_set(id, Value::TypedMap(key_ty, value_ty, map))?;
                            Ok(unit)
                        }
                        ("keys", []) => Ok(Value::List(map.keys().map(MapKey::value).collect())),
                        ("len", []) => Ok(Value::Int(map.len() as i64)),
                        _ => Err(self.fail(at, "unsupported Map method")),
                    },
                    Value::List(mut list) => match (method, args.as_slice()) {
                        ("add", [value]) | ("push", [value]) => {
                            list.push(value.clone());
                            self.runtime.heap_set(id, Value::List(list))?;
                            Ok(unit)
                        }
                        ("get", [Value::Int(n)]) if *n >= 0 => list
                            .get(*n as usize)
                            .cloned()
                            .ok_or_else(|| self.fail(at, "list index out of bounds")),
                        ("len", []) => Ok(Value::Int(list.len() as i64)),
                        ("set", [Value::Int(n), value])
                            if *n >= 0 && (*n as usize) < list.len() =>
                        {
                            list[*n as usize] = value.clone();
                            self.runtime.heap_set(id, Value::List(list))?;
                            Ok(unit)
                        }
                        _ => Err(self.fail(at, "unsupported List method")),
                    },
                    Value::Map(mut map) => match (method, args.as_slice()) {
                        ("set", [key, v]) => {
                            let key = MapKey::from_value(key)
                                .ok_or_else(|| self.fail(at, "unsupported Map key type"))?;
                            map.set(key, v.clone());
                            self.runtime.heap_set(id, Value::Map(map))?;
                            Ok(unit)
                        }
                        ("get", [key]) => {
                            let key = MapKey::from_value(key)
                                .ok_or_else(|| self.fail(at, "unsupported Map key type"))?;
                            Ok(Value::Option(map.get(&key).cloned().map(Box::new)))
                        }
                        ("remove", [key]) => {
                            let key = MapKey::from_value(key)
                                .ok_or_else(|| self.fail(at, "unsupported Map key type"))?;
                            map.remove(&key);
                            self.runtime.heap_set(id, Value::Map(map))?;
                            Ok(unit)
                        }
                        ("keys", []) => Ok(Value::List(map.keys().map(MapKey::value).collect())),
                        ("len", []) => Ok(Value::Int(map.len() as i64)),
                        _ => Err(self.fail(at, "unsupported Map method")),
                    },
                    _ => Err(self.fail(at, "unsupported object method")),
                }
            }
            Value::Handle(id) => match (method, args.as_slice()) {
                ("seek", [Value::Int(n)]) if *n >= 0 => {
                    self.runtime.seek(id, *n as usize)?;
                    Ok(unit)
                }
                ("read", [Value::Int(n)]) if *n >= 0 => {
                    let data = self.runtime.read_handle(id, *n as usize)?;
                    String::from_utf8(data)
                        .map(|text| Value::Text(text.into()))
                        .map_err(|e| self.fail(at, e.to_string()))
                }
                ("readBytes", [Value::Int(n)]) if *n >= 0 => Ok(Value::Bytes(
                    self.runtime.read_handle(id, *n as usize)?.into(),
                )),
                ("write", [v]) => {
                    match v {
                        Value::Bytes(bytes) => self.runtime.write_handle(id, bytes)?,
                        other => self
                            .runtime
                            .write_handle(id, other.to_string().as_bytes())?,
                    }
                    Ok(unit)
                }
                ("writeBytes", [Value::Bytes(bytes)]) => {
                    self.runtime.write_handle(id, bytes)?;
                    Ok(unit)
                }
                ("close", []) => {
                    self.runtime.close_handle(id)?;
                    Ok(unit)
                }
                _ => Err(self.fail(at, "unsupported handle method")),
            },
            _ => Err(self.fail(at, "method receiver is not an object")),
        }
    }
    fn builtin(&mut self, receiver: &str, method: &str, args: Vec<Value>, at: &Tok) -> Exec<Value> {
        let strings = || args.iter().map(ToString::to_string).collect::<Vec<_>>();
        let unit = Value::Null;
        match (receiver, method, args.len()) {
            ("Out", "println", 1) => {
                let s = self.runtime.display_value(&args[0]);
                self.runtime.print_out(&(s + "\n"))?;
                Ok(unit)
            }
            ("Err", "println", 1) => {
                let s = self.runtime.display_value(&args[0]);
                self.runtime.print_err(&(s + "\n"))?;
                Ok(unit)
            }
            ("Out", "flush", 0) | ("Err", "flush", 0) => Ok(unit),
            ("In", "readSecretLine", 0)
                if matches!(
                    self.program.language.as_str(),
                    "0.6"
                        | "0.7"
                        | "0.8"
                        | "0.9"
                        | "0.9.1"
                        | "0.9.2"
                        | "0.9.3"
                        | "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) =>
            {
                Ok(Value::Option(
                    self.runtime
                        .secret_input_line(&mut self.input)?
                        .map(|s| Box::new(v05::secret(Value::Text(s.into()), "String".into()))),
                ))
            }
            ("Env", "getSecret", 1)
                if matches!(
                    self.program.language.as_str(),
                    "0.6"
                        | "0.7"
                        | "0.8"
                        | "0.9"
                        | "0.9.1"
                        | "0.9.2"
                        | "0.9.3"
                        | "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) =>
            {
                Ok(Value::Option(
                    self.runtime
                        .secret_environment_value(&strings()[0])?
                        .map(|s| Box::new(v05::secret(Value::Text(s.into()), "String".into()))),
                ))
            }
            ("In", "readChunk", 1)
                if matches!(
                    self.program.language.as_str(),
                    "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) =>
            {
                let Value::Int(limit) = args[0] else {
                    return Err(self.fail(at, "readChunk expects Int"));
                };
                if !(1..=65536).contains(&limit) {
                    return Err(self.fail(at, "readChunk size must be 1..65536"));
                }
                Ok(Value::Option(
                    self.runtime
                        .input_chunk(&mut self.input, limit as usize)?
                        .map(|bytes| Box::new(Value::Bytes(bytes.into()))),
                ))
            }
            ("Out", "writeBytes", 1)
                if matches!(
                    self.program.language.as_str(),
                    "0.9.4"
                        | "0.9.5"
                        | "0.9.6"
                        | "0.9.7"
                        | "0.9.8"
                        | "0.9.9"
                        | "1.0.0"
                        | "1.1.0"
                        | "1.2.0"
                        | "1.3.0"
                        | "1.4.0"
                        | "1.5.0"
                        | "1.6.0"
                        | "1.6.1"
                        | "1.7.0"
                        | "1.7.1"
                        | "1.8.0"
                        | "1.8.1"
                        | "1.8.2"
                        | "1.8.3"
                        | "1.8.4"
                        | "1.8.5"
                        | "1.8.6"
                        | "1.8.7"
                        | "1.8.8"
                        | "1.9.0"
                        | "1.9.1"
                        | "1.9.2"
                        | "1.9.3"
                        | "1.9.4"
                        | "1.9.5"
                        | "1.9.6"
                        | "1.9.7"
                        | "1.9.8"
                        | "1.9.9"
                        | "1.9.10"
                        | "1.9.11"
                        | "1.9.12"
                        | "1.9.13"
                        | "1.9.14"
                        | "1.9.15"
                        | "1.9.16"
                        | "1.9.17"
                        | "1.9.18"
                        | "1.9.19"
                        | "1.9.20"
                        | "1.9.21"
                        | "1.9.22"
                        | "1.9.23"
                        | "1.9.24"
                        | "1.9.25"
                        | "1.9.26"
                        | "1.9.27"
                        | "1.9.28"
                        | "1.9.29"
                        | "1.9.30"
                        | "1.9.31"
                        | "1.9.32"
                        | "1.9.33"
                        | "1.9.34"
                        | "1.9.35"
                        | "1.9.36"
                        | "1.9.37"
                        | "1.9.38"
                        | "1.9.39"
                        | "1.9.40"
                        | "1.9.41"
                        | "1.9.42"
                        | "1.9.43"
                        | "1.9.44"
                        | "1.9.45"
                        | "1.9.46"
                        | "1.9.47"
                        | "1.9.48"
                        | "1.9.49"
                        | "1.9.50"
                        | "1.9.51"
                        | "2.0.0"
                ) =>
            {
                let Value::Bytes(bytes) = &args[0] else {
                    return Err(self.fail(at, "writeBytes expects Bytes"));
                };
                self.runtime.write_output(bytes)?;
                Ok(unit)
            }
            ("In", "readLine", 0) => Ok(Value::Option(
                self.runtime
                    .input_line(&mut self.input)?
                    .map(|line| Box::new(Value::Text(line.into()))),
            )),
            ("Time", "now", 0) => Ok(Value::Int(self.runtime.now_millis()? as i64)),
            ("Random", "next", 0) => Ok(Value::Int(self.runtime.random_u64() as i64)),
            ("Args", "all", 0) => {
                let values: Vec<_> = self
                    .runtime
                    .arguments()
                    .into_iter()
                    .map(|text| Value::Text(text.into()))
                    .collect();
                Ok(Value::HeapRef(
                    self.runtime
                        .alloc(Value::TypedList("String".into(), values.into()))?,
                ))
            }
            ("Env", "get", 1) => Ok(Value::Option(
                self.runtime
                    .environment_value(&strings()[0])?
                    .map(|s| Box::new(Value::Text(s.into()))),
            )),
            ("Locale", "current", 0) => Ok(Value::Text(self.runtime.locale().into())),
            ("Directory", "entries", 1) => {
                let values: Vec<_> = self
                    .runtime
                    .directory_entries(&strings()[0])?
                    .into_iter()
                    .map(|text| Value::Text(text.into()))
                    .collect();
                Ok(Value::HeapRef(
                    self.runtime
                        .alloc(Value::TypedList("String".into(), values.into()))?,
                ))
            }
            ("File", "readText", 1) => {
                let result = self.runtime.read_file(&strings()[0]).and_then(|v| {
                    String::from_utf8(v).map_err(|e| Error::InvalidOperation(e.to_string()))
                });
                Ok(match result {
                    Ok(s) => Value::Result(Ok(Box::new(Value::Text(s.into())))),
                    Err(e) => Value::Result(Err(Box::new(Value::FileError(
                        FileFailure::from_error(&e, &strings()[0]),
                    )))),
                })
            }
            ("File", "readBytes", 1) => Ok(match self.runtime.read_file(&strings()[0]) {
                Ok(v) => Value::Result(Ok(Box::new(Value::Bytes(v.into())))),
                Err(e) => Value::Result(Err(Box::new(Value::FileError(FileFailure::from_error(
                    &e,
                    &strings()[0],
                ))))),
            }),
            ("File", "writeText", 2) | ("File", "write", 2) | ("File", "create", 2) => {
                let a = strings();
                self.runtime.write_file(&a[0], a[1].as_bytes())?;
                Ok(unit)
            }
            ("File", "writeBytes", 2) => {
                let Value::Bytes(bytes) = &args[1] else {
                    return Err(self.fail(at, "writeBytes expects Bytes"));
                };
                self.runtime.write_file(&strings()[0], bytes.as_slice())?;
                Ok(unit)
            }
            ("File", "create", 1) => {
                self.runtime.write_file(&strings()[0], [])?;
                Ok(unit)
            }
            ("File", "append", 2) => {
                let a = strings();
                self.runtime.append_file(&a[0], a[1].as_bytes())?;
                Ok(unit)
            }
            ("File", "open", 1) => Ok(Value::Handle(self.runtime.open_file(&strings()[0])?)),
            ("File", "openSnapshot", 1) => {
                Ok(Value::Handle(self.runtime.open_snapshot(&strings()[0])?))
            }
            ("File", "delete", 1) => {
                self.runtime.delete_file(&strings()[0])?;
                Ok(unit)
            }
            ("File", "copy", 2) => {
                let a = strings();
                self.runtime.copy_file(&a[0], &a[1])?;
                Ok(unit)
            }
            ("File", "move", 2) => {
                let a = strings();
                self.runtime.move_file(&a[0], &a[1])?;
                Ok(unit)
            }
            ("File", "truncate", 2) => {
                let n = int(args[1].clone(), at)?;
                if n < 0 {
                    return Err(self.fail(at, "length must be nonnegative"));
                }
                self.runtime.truncate_file(&strings()[0], n as usize)?;
                Ok(unit)
            }
            ("Directory", "create", 1) => {
                self.runtime.create_directory(&strings()[0])?;
                Ok(unit)
            }
            ("Directory", "delete", 1) => {
                self.runtime.delete_directory(&strings()[0])?;
                Ok(unit)
            }
            ("Directory", "move", 2) => {
                let a = strings();
                self.runtime.move_directory(&a[0], &a[1])?;
                Ok(unit)
            }
            _ => Err(self.fail(
                at,
                format!("unsupported call {receiver}.{method}/{}", args.len()),
            )),
        }
    }
}

fn int(v: Value, at: &Tok) -> Exec<i64> {
    match v {
        Value::Int(n) => Ok(n),
        _ => Err(Flow::Error(diagnostic(at, "expected Int"))),
    }
}
fn value_type(v: &Value, rt: &Runtime) -> String {
    if let Value::HeapRef(id) | Value::CellRef(id) = v {
        return rt
            .heap_get(*id)
            .map(|v| value_type(v, rt))
            .unwrap_or_else(|| "Unknown".into());
    }
    match v {
        Value::Bool(_) => "Bool",
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::Text(_) => "String",
        Value::Bytes(_) => "Bytes",
        Value::BigInt(_) => "BigInt",
        Value::Decimal(_) => "Decimal",
        Value::Regex(_) => "Regex",
        Value::CsvStream(_) => "CsvStreamState",
        Value::JsonStream(_) => "JsonStreamState",
        Value::Instant(_) => "Instant",
        Value::Duration(_) => "Duration",
        Value::NumericArray(a) => match a.dtype() {
            rewind::numeric::DType::Float64 => "FloatArray",
            rewind::numeric::DType::Int64 => "IntArray",
        },
        Value::FileError(_) => "FileError",
        Value::Null => "Unit",
        Value::List(_) => "List",
        Value::TypedList(ty, _) => return format!("List<{ty}>"),
        Value::Map(_) => "Map",
        Value::TypedMap(key, value, _) => return format!("Map<{key},{value}>"),
        Value::OrderedMap(key, value, _) => return format!("Map<{key},{value}>"),
        Value::Struct(name, _) => name,
        Value::Enum(name, _, _) => name,
        Value::Function(_, ty) | Value::Closure(_, ty, _) => ty,
        Value::Option(Some(v)) => return format!("Option<{}>", value_type(v, rt)),
        Value::Option(None) => return "Option<Unknown>".into(),
        Value::Result(Ok(v)) => return format!("Result<{},Unknown>", value_type(v, rt)),
        Value::Result(Err(v)) => return format!("Result<Unknown,{}>", value_type(v, rt)),
        Value::Handle(_) => "FileHandle",
        Value::HeapRef(_) | Value::CellRef(_) => unreachable!(),
    }
    .into()
}
fn compatible(expected: &str, actual: &str) -> bool {
    if expected.starts_with('&') || actual.starts_with('&') {
        let expected = v06::borrowed_type(expected);
        let actual = v06::borrowed_type(actual);
        let expected = if expected.starts_with("fn(") {
            v06::captures::with_flags(expected, false, false)
        } else {
            expected.into()
        };
        return compatible(&expected, actual);
    }
    if let (Some((ea, er)), Some((aa, ar))) =
        (function_signature(expected), function_signature(actual))
    {
        return ea.len() == aa.len()
            && ea.iter().zip(aa).all(|(e, a)| compatible(e, &a))
            && compatible(&er, &ar)
            && v06::effects_compatible(expected, actual)
            && v06::captures::compatible(expected, actual);
    }
    if expected == actual || expected == "Unknown" || actual == "Unknown" || actual == "Never" {
        return true;
    }
    let (Some((expected_head, expected_inner)), Some((actual_head, actual_inner))) =
        (expected.split_once('<'), actual.split_once('<'))
    else {
        return false;
    };
    if expected_head != actual_head {
        return false;
    }
    let expected_parts = split_type_args(outer_type_end(expected_inner));
    let actual_parts = split_type_args(outer_type_end(actual_inner));
    expected_parts.len() == actual_parts.len()
        && expected_parts
            .iter()
            .zip(actual_parts)
            .all(|(a, b)| compatible(a, b))
}
fn unify_type(
    expected: &str,
    actual: &str,
    params: &[String],
    substitutions: &mut BTreeMap<String, String>,
) -> bool {
    if expected.starts_with('&') || actual.starts_with('&') {
        let base = v06::borrowed_type(expected);
        let expected = if base.starts_with("fn(") {
            v06::captures::with_flags(base, false, false)
        } else {
            base.into()
        };
        return unify_type(&expected, v06::borrowed_type(actual), params, substitutions);
    }
    if let (Some((ea, er)), Some((aa, ar))) =
        (function_signature(expected), function_signature(actual))
    {
        if ea.len() != aa.len()
            || !ea
                .iter()
                .zip(aa)
                .all(|(e, a)| unify_type(e, &a, params, substitutions))
            || !unify_type(&er, &ar, params, substitutions)
        {
            return false;
        }
        return v06::captures::compatible(expected, actual)
            && v06::unify_effects(expected, actual, params, substitutions);
    }
    if params.iter().any(|p| p == expected) {
        return match substitutions.get(expected) {
            Some(previous) if previous == "Unknown" && actual != "Unknown" => {
                substitutions.insert(expected.into(), actual.into());
                true
            }
            Some(previous) => compatible(previous, actual),
            None => {
                substitutions.insert(expected.into(), actual.into());
                true
            }
        };
    }
    if let (Some((eh, ei)), Some((ah, ai))) = (expected.split_once('<'), actual.split_once('<')) {
        if eh != ah {
            return false;
        }
        let ep = split_type_args(outer_type_end(ei));
        let ap = split_type_args(outer_type_end(ai));
        return ep.len() == ap.len()
            && ep
                .iter()
                .zip(ap)
                .all(|(e, a)| unify_type(e, a, params, substitutions));
    }
    compatible(expected, actual)
}
fn substitute_type(ty: &str, substitutions: &BTreeMap<String, String>) -> String {
    rename_type(ty, substitutions)
}
fn infer_arguments(
    params: &[(String, String)],
    type_params: &[String],
    actual: &[String],
    at: &Tok,
) -> Result<BTreeMap<String, String>> {
    if params.len() != actual.len() {
        return Err(diagnostic(
            at,
            format!(
                "expected {} arguments, found {}",
                params.len(),
                actual.len()
            ),
        ));
    }
    let mut substitutions = BTreeMap::new();
    for ((_, expected), found) in params.iter().zip(actual) {
        if !unify_type(expected, found, type_params, &mut substitutions) {
            return Err(diagnostic(
                at,
                format!("expected {expected}, found {found}"),
            ));
        }
    }
    for ((_, expected), found) in params.iter().zip(actual) {
        if !v06::solve_effects(expected, found, type_params, &mut substitutions) {
            return Err(diagnostic(
                at,
                format!("effect inclusion constraint failed: {expected} <- {found}"),
            ));
        }
    }
    for name in type_params {
        if !substitutions.contains_key(name) || substitutions[name] == "Unknown" {
            return Err(diagnostic(
                at,
                format!("cannot infer type parameter {name}"),
            ));
        }
    }
    Ok(substitutions)
}
fn bound_provided(bounds: &BTreeMap<String, String>, ty: &str, required: &str) -> bool {
    bounds.get(ty).is_some_and(|available| {
        required.split('+').all(|r| {
            available
                .split('+')
                .any(|b| b == r || (r == "Send" && b == "Share"))
        })
    })
}
fn builtin_key_type(program: &Program, ty: &str) -> bool {
    matches!(ty, "Bool" | "Int" | "Float" | "String" | "Bytes")
        || (ty == "BigInt" && language_at_least(&program.language, "1.8.2"))
        || (ty == "Decimal" && language_at_least(&program.language, "1.8.3"))
        || (ty == "Regex" && language_at_least(&program.language, "1.8.6"))
        || (matches!(ty, "Instant" | "Duration") && language_at_least(&program.language, "1.8.4"))
}
fn trait_satisfied(program: &Program, bound: &str, ty: &str) -> bool {
    if bound.contains('+') {
        return bound.split('+').all(|b| trait_satisfied(program, b, ty));
    }
    if bound == "Effect" {
        return ty.starts_with('@');
    }
    if matches!(
        program.language.as_str(),
        "0.5"
            | "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
            | "1.1.0"
            | "1.2.0"
            | "1.3.0"
            | "1.4.0"
            | "1.5.0"
            | "1.6.0"
            | "1.6.1"
            | "1.7.0"
            | "1.7.1"
            | "1.8.0"
            | "1.8.1"
            | "1.8.2"
            | "1.8.3"
            | "1.8.4"
            | "1.8.5"
            | "1.8.6"
            | "1.8.7"
            | "1.8.8"
            | "1.9.0"
            | "1.9.1"
            | "1.9.2"
            | "1.9.3"
            | "1.9.4"
            | "1.9.5"
            | "1.9.6"
            | "1.9.7"
            | "1.9.8"
            | "1.9.9"
            | "1.9.10"
            | "1.9.11"
            | "1.9.12"
            | "1.9.13"
            | "1.9.14"
            | "1.9.15"
            | "1.9.16"
            | "1.9.17"
            | "1.9.18"
            | "1.9.19"
            | "1.9.20"
            | "1.9.21"
            | "1.9.22"
            | "1.9.23"
            | "1.9.24"
            | "1.9.25"
            | "1.9.26"
            | "1.9.27"
            | "1.9.28"
            | "1.9.29"
            | "1.9.30"
            | "1.9.31"
            | "1.9.32"
            | "1.9.33"
            | "1.9.34"
            | "1.9.35"
            | "1.9.36"
            | "1.9.37"
            | "1.9.38"
            | "1.9.39"
            | "1.9.40"
            | "1.9.41"
            | "1.9.42"
            | "1.9.43"
            | "1.9.44"
            | "1.9.45"
            | "1.9.46"
            | "1.9.47"
            | "1.9.48"
            | "1.9.49"
            | "1.9.50"
            | "1.9.51"
            | "2.0.0"
    ) && matches!(bound, "Send" | "Share")
    {
        return v05::transfer_type(program, ty, bound == "Share", &mut BTreeSet::new());
    }
    if matches!(bound, "Eq" | "Ord" | "Hash" | "Display") && builtin_key_type(program, ty) {
        return true;
    }
    program
        .impls
        .iter()
        .any(|((tr, target), methods)| tr == bound && impl_matches(program, target, ty, methods))
}
fn impl_matches(
    program: &Program,
    target: &str,
    ty: &str,
    methods: &BTreeMap<String, String>,
) -> bool {
    if target == ty {
        return true;
    }
    if !matches!(
        program.language.as_str(),
        "0.5"
            | "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
            | "1.1.0"
            | "1.2.0"
            | "1.3.0"
            | "1.4.0"
            | "1.5.0"
            | "1.6.0"
            | "1.6.1"
            | "1.7.0"
            | "1.7.1"
            | "1.8.0"
            | "1.8.1"
            | "1.8.2"
            | "1.8.3"
            | "1.8.4"
            | "1.8.5"
            | "1.8.6"
            | "1.8.7"
            | "1.8.8"
            | "1.9.0"
            | "1.9.1"
            | "1.9.2"
            | "1.9.3"
            | "1.9.4"
            | "1.9.5"
            | "1.9.6"
            | "1.9.7"
            | "1.9.8"
            | "1.9.9"
            | "1.9.10"
            | "1.9.11"
            | "1.9.12"
            | "1.9.13"
            | "1.9.14"
            | "1.9.15"
            | "1.9.16"
            | "1.9.17"
            | "1.9.18"
            | "1.9.19"
            | "1.9.20"
            | "1.9.21"
            | "1.9.22"
            | "1.9.23"
            | "1.9.24"
            | "1.9.25"
            | "1.9.26"
            | "1.9.27"
            | "1.9.28"
            | "1.9.29"
            | "1.9.30"
            | "1.9.31"
            | "1.9.32"
            | "1.9.33"
            | "1.9.34"
            | "1.9.35"
            | "1.9.36"
            | "1.9.37"
            | "1.9.38"
            | "1.9.39"
            | "1.9.40"
            | "1.9.41"
            | "1.9.42"
            | "1.9.43"
            | "1.9.44"
            | "1.9.45"
            | "1.9.46"
            | "1.9.47"
            | "1.9.48"
            | "1.9.49"
            | "1.9.50"
            | "1.9.51"
            | "2.0.0"
    ) {
        return false;
    }
    let Some(f) = methods
        .values()
        .next()
        .and_then(|n| program.functions.get(n))
    else {
        return false;
    };
    let params = f
        .type_params
        .iter()
        .map(|(n, _)| n.clone())
        .collect::<Vec<_>>();
    let mut substitutions = BTreeMap::new();
    unify_type(target, ty, &params, &mut substitutions)
        && f.type_params.iter().all(|(n, b)| {
            substitutions
                .get(n)
                .is_some_and(|t| b.as_ref().is_none_or(|b| trait_satisfied(program, b, t)))
        })
}
fn matching_methods(program: &Program, ty: &str, method: &str) -> Vec<String> {
    program
        .impls
        .iter()
        .filter_map(|((_, target), methods)| {
            if impl_matches(program, target, ty, methods) {
                methods.get(method).cloned()
            } else {
                None
            }
        })
        .collect()
}
fn infer_call_arguments(
    name: &str,
    parameters: &[(String, String)],
    type_params: &[String],
    actuals: &[String],
    at: &Tok,
) -> Result<BTreeMap<String, String>> {
    if let Some((_, explicit)) = name.split_once('<') {
        let types = split_type_args(outer_type_end(explicit));
        if types.len() != type_params.len() || parameters.len() != actuals.len() {
            return Err(diagnostic(at, "wrong number of type or value arguments"));
        }
        let substitutions = type_params
            .iter()
            .cloned()
            .zip(types.into_iter().map(str::to_string))
            .collect::<BTreeMap<_, _>>();
        for ((_, expected), actual) in parameters.iter().zip(actuals) {
            let expected = substitute_type(expected, &substitutions);
            if !compatible(&expected, actual) {
                return Err(diagnostic(
                    at,
                    format!("expected {expected}, found {actual}"),
                ));
            }
        }
        Ok(substitutions)
    } else {
        infer_arguments(parameters, type_params, actuals, at)
    }
}
fn standard_trait(name: &str) -> Option<TraitDef> {
    if matches!(name, "Iterator" | "IntoIterator") {
        return Some(TraitDef {
            defaults: BTreeMap::new(),
            method_effects: BTreeMap::new(),
            associated: BTreeSet::from(["Item".into()]),
            methods: BTreeMap::from([(
                if name == "Iterator" { "next" } else { "iter" }.into(),
                (
                    vec!["Self".into()],
                    if name == "Iterator" {
                        "Option<Self::Item>"
                    } else {
                        "Iterator<Self::Item>"
                    }
                    .into(),
                ),
            )]),
            public: true,
            origin: PathBuf::new(),
        });
    }
    let (method, params, ret): (&str, Vec<&str>, &str) = match name {
        "Eq" => ("eq", vec!["Self", "Self"], "Bool"),
        "Ord" => ("cmp", vec!["Self", "Self"], "Int"),
        "Hash" => ("hash", vec!["Self"], "Int"),
        "Display" => ("display", vec!["Self"], "String"),
        _ => return None,
    };
    let mut methods = BTreeMap::new();
    methods.insert(
        method.into(),
        (params.into_iter().map(str::to_string).collect(), ret.into()),
    );
    Some(TraitDef {
        defaults: BTreeMap::new(),
        method_effects: BTreeMap::new(),
        associated: BTreeSet::new(),
        methods,
        public: true,
        origin: PathBuf::new(),
    })
}
fn function_signature(ty: &str) -> Option<(Vec<String>, String)> {
    let ty = v06::borrowed_type(ty);
    let tail = ty.strip_prefix("fn(")?;
    let mut depth = 1usize;
    let close = tail.char_indices().find_map(|(i, c)| {
        if c == '(' {
            depth += 1;
        }
        if c == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        None
    })?;
    let args = &tail[..close];
    let ret = tail[close + 1..].strip_prefix("->")?;
    let args = if args.is_empty() {
        Vec::new()
    } else {
        split_type_args(args)
            .iter()
            .map(|a| a.to_string())
            .collect()
    };
    Some((args, v06::strip_effects(ret).into()))
}
fn trait_method_return(
    program: &Program,
    ty: &str,
    method: &str,
    args: &[String],
    at: &Tok,
) -> Result<Option<String>> {
    let matching = matching_methods(program, ty, method);
    if matching.len() > 1 {
        return Err(diagnostic(
            at,
            format!("ambiguous method {method} for {ty}"),
        ));
    }
    let Some(symbol) = matching.first() else {
        if builtin_key_type(program, ty) {
            let expected = match method {
                "eq" | "cmp" => vec![ty.to_string()],
                "hash" | "display" => Vec::new(),
                _ => return Ok(None),
            };
            if expected != args {
                return Err(diagnostic(
                    at,
                    format!("invalid arguments for {ty}.{method}"),
                ));
            }
            return Ok(Some(
                match method {
                    "eq" => "Bool",
                    "display" => "String",
                    _ => "Int",
                }
                .into(),
            ));
        }
        return Ok(None);
    };
    let f = &program.functions[symbol];
    if !f.type_params.is_empty() {
        let mut actuals = vec![ty.to_string()];
        actuals.extend(args.iter().cloned());
        let params = f
            .type_params
            .iter()
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        let substitutions = infer_arguments(&f.params, &params, &actuals, at)?;
        return Ok(Some(substitute_type(&f.ret, &substitutions)));
    }
    if f.params.len() != args.len() + 1
        || f.params.first().is_none_or(|(_, t)| {
            if matches!(
                program.language.as_str(),
                "0.6"
                    | "0.7"
                    | "0.8"
                    | "0.9"
                    | "0.9.1"
                    | "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "1.9.49"
                    | "1.9.50"
                    | "1.9.51"
                    | "2.0.0"
            ) {
                !compatible(t, ty)
            } else {
                t != ty
            }
        })
        || f.params
            .iter()
            .skip(1)
            .zip(args)
            .any(|((_, e), a)| !compatible(e, a))
    {
        return Err(diagnostic(
            at,
            format!("invalid arguments for {ty}.{method}"),
        ));
    }
    Ok(Some(f.ret.clone()))
}
fn enum_constructor_type(
    program: &Program,
    name: &str,
    args: &[String],
    at: &Tok,
) -> Result<Option<String>> {
    let Some((enum_ty, variant)) = name.split_once("::") else {
        return Ok(None);
    };
    let base = enum_ty.split('<').next().unwrap_or(enum_ty);
    let def = program
        .enums
        .get(base)
        .ok_or_else(|| diagnostic(at, format!("unknown enum {base}")))?;
    let fields = def
        .variants
        .get(variant)
        .ok_or_else(|| diagnostic(at, format!("unknown variant {variant}")))?;
    if fields.len() != args.len() {
        return Err(diagnostic(
            at,
            format!("{name} expects {} fields", fields.len()),
        ));
    }
    let mut substitutions = BTreeMap::new();
    for ((_, expected), actual) in fields.iter().zip(args) {
        if !unify_type(expected, actual, &def.type_params, &mut substitutions) {
            return Err(diagnostic(
                at,
                format!("expected {expected}, found {actual}"),
            ));
        }
    }
    if let Some((_, explicit)) = enum_ty.split_once('<') {
        let explicit = split_type_args(outer_type_end(explicit));
        if explicit.len() != def.type_params.len() {
            return Err(diagnostic(at, "wrong number of enum type arguments"));
        }
        for (p, actual) in def.type_params.iter().zip(explicit) {
            if let Some(inferred) = substitutions.get(p) {
                if inferred != actual {
                    return Err(diagnostic(at, "enum type arguments disagree with fields"));
                }
            } else {
                substitutions.insert(p.clone(), actual.into());
            }
        }
    }
    if def
        .type_params
        .iter()
        .any(|p| !substitutions.contains_key(p))
    {
        return Err(diagnostic(at, "cannot infer enum type arguments"));
    }
    Ok(Some(if def.type_params.is_empty() {
        base.into()
    } else {
        format!(
            "{base}<{}>",
            def.type_params
                .iter()
                .map(|p| substitutions[p].as_str())
                .collect::<Vec<_>>()
                .join(",")
        )
    }))
}
fn resolve_alias(program: &Program, name: &str) -> String {
    if let Some((head, tail)) = name.split_once("::") {
        let base = head.split('<').next().unwrap_or(head);
        if let Some(alias) = program.import_aliases.get(base) {
            return format!("{}{}::{tail}", alias, &head[base.len()..]);
        }
    }
    if let Some((head, tail)) = name.split_once('<') {
        if let Some(alias) = program.import_aliases.get(head) {
            return format!("{alias}<{tail}");
        }
    }
    program
        .import_aliases
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.into())
}
fn resolve_pattern_alias(pattern: &Pattern, aliases: &BTreeMap<String, String>) -> Pattern {
    match pattern {
        Pattern::Variant(name, parts) => {
            let name = if let Some((head, tail)) = name.split_once("::") {
                format!(
                    "{}::{tail}",
                    aliases.get(head).map(String::as_str).unwrap_or(head)
                )
            } else {
                name.clone()
            };
            Pattern::Variant(
                name,
                parts
                    .iter()
                    .map(|p| resolve_pattern_alias(p, aliases))
                    .collect(),
            )
        }
        Pattern::Struct(name, fields) => {
            let name = if let Some((head, tail)) = name.split_once("::") {
                format!(
                    "{}::{tail}",
                    aliases.get(head).map(String::as_str).unwrap_or(head)
                )
            } else {
                aliases.get(name).cloned().unwrap_or_else(|| name.clone())
            };
            Pattern::Struct(
                name,
                fields
                    .iter()
                    .map(|(n, p)| (n.clone(), resolve_pattern_alias(p, aliases)))
                    .collect(),
            )
        }
        Pattern::List(parts) => Pattern::List(
            parts
                .iter()
                .map(|p| resolve_pattern_alias(p, aliases))
                .collect(),
        ),
        other => other.clone(),
    }
}
fn pattern_matches(
    pattern: &Pattern,
    value: &Value,
    runtime: &Runtime,
    bindings: &mut BTreeMap<String, Value>,
) -> bool {
    let resolved = if let Value::HeapRef(id) = value {
        runtime.heap_get(*id).unwrap_or(value)
    } else {
        value
    };
    match pattern {
        Pattern::Wildcard => true,
        Pattern::Bind(name) => bindings.insert(name.clone(), value.clone()).is_none(),
        Pattern::Literal(expected) => expected == resolved,
        Pattern::Range(start, end) => matches!(resolved, Value::Int(n) if n >= start && n < end),
        Pattern::List(parts) => {
            let values = match resolved {
                Value::List(v) => v.iter().collect::<Vec<_>>(),
                Value::TypedList(_, v) => v.iter().collect::<Vec<_>>(),
                _ => return false,
            };
            parts.len() == values.len()
                && parts
                    .iter()
                    .zip(values)
                    .all(|(p, v)| pattern_matches(p, v, runtime, bindings))
        }
        Pattern::Struct(name, fields) => {
            if name == "FileError" {
                let Value::FileError(error) = resolved else {
                    return false;
                };
                return fields.iter().all(|(field, p)| {
                    let value = match field.as_str() {
                        "code" => Value::Text(error.code.clone().into()),
                        "path" => Value::Text(error.path.clone().into()),
                        "cause" => Value::Option(
                            error
                                .causes
                                .first()
                                .cloned()
                                .map(|s| Box::new(Value::Text(s.into()))),
                        ),
                        "causes" => Value::TypedList(
                            "String".into(),
                            error
                                .causes
                                .iter()
                                .cloned()
                                .map(|text| Value::Text(text.into()))
                                .collect(),
                        ),
                        _ => return false,
                    };
                    pattern_matches(p, &value, runtime, bindings)
                });
            }
            if let Some((expected_ty, expected_variant)) = name.split_once("::") {
                let Value::Enum(actual_ty, actual_variant, values) = resolved else {
                    return false;
                };
                return expected_ty.split('<').next() == actual_ty.split('<').next()
                    && expected_variant == actual_variant
                    && fields.iter().all(|(field, p)| {
                        values
                            .iter()
                            .find(|(n, _)| n == field)
                            .is_some_and(|(_, v)| pattern_matches(p, v, runtime, bindings))
                    });
            }
            let Value::Struct(actual, values) = resolved else {
                return false;
            };
            actual.split('<').next() == Some(name.as_str())
                && fields.iter().all(|(key, p)| {
                    values
                        .get(key)
                        .is_some_and(|v| pattern_matches(p, v, runtime, bindings))
                })
        }
        Pattern::Variant(name, parts) => {
            let (variant, values): (&str, Vec<Value>) = match resolved {
                Value::Enum(ty, v, fields)
                    if name == v
                        || name.split_once("::").is_some_and(|(head, variant)| {
                            head.split('<').next() == ty.split('<').next() && variant == v
                        }) =>
                {
                    (v, fields.iter().map(|(_, value)| value.clone()).collect())
                }
                Value::Struct(ty, fields) if name == "$tuple" && ty.starts_with("Tuple<") => (
                    "$tuple",
                    (0..fields.len())
                        .filter_map(|i| fields.get(&format!("_{i}")).cloned())
                        .collect(),
                ),
                Value::Option(Some(v)) => ("Some", vec![(**v).clone()]),
                Value::Option(None) => ("None", Vec::new()),
                Value::Result(Ok(v)) => ("Ok", vec![(**v).clone()]),
                Value::Result(Err(v)) => ("Err", vec![(**v).clone()]),
                _ => return false,
            };
            (name == variant || name.ends_with(&format!("::{variant}")))
                && parts.len() == values.len()
                && parts
                    .iter()
                    .zip(&values)
                    .all(|(p, v)| pattern_matches(p, v, runtime, bindings))
        }
    }
}
fn split_type_args(input: &str) -> Vec<&str> {
    let mut depth = 0usize;
    let mut parens = 0usize;
    let mut start = 0usize;
    let mut out = Vec::new();
    for (i, c) in input.char_indices() {
        match c {
            '<' => depth += 1,
            '>' if i == 0 || input.as_bytes()[i - 1] != b'-' => depth = depth.saturating_sub(1),
            '(' => parens += 1,
            ')' => parens = parens.saturating_sub(1),
            ',' if depth == 0 && parens == 0 => {
                out.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&input[start..]);
    out
}
fn outer_type_end(input: &str) -> &str {
    input.strip_suffix('>').unwrap_or(input)
}
fn binary(op: &str, a: Value, b: Value, at: &Tok) -> Exec<Value> {
    if v05::unsecret(&a).is_some() || v05::unsecret(&b).is_some() {
        let a = v05::unsecret(&a).cloned().unwrap_or(a);
        let b = v05::unsecret(&b).cloned().unwrap_or(b);
        let value = binary(op, a, b, at)?;
        let ty = match &value {
            Value::Bool(_) => "Bool",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Text(_) => "String",
            _ => "Unit",
        };
        return Ok(v05::secret(value, ty.into()));
    }
    let bad = || Flow::Error(diagnostic(at, format!("invalid operands for '{op}'")));
    match (op, a, b) {
        ("+", Value::Text(a), Value::Text(b)) => Ok(Value::Text(a + &b)),
        ("+", Value::Text(a), b) => Ok(Value::Text(a + &b.to_string())),
        ("+", a, Value::Text(b)) => Ok(Value::Text((a.to_string() + &b).into())),
        ("+", Value::Int(a), Value::Int(b)) => a
            .checked_add(b)
            .map(Value::Int)
            .ok_or_else(|| Flow::Error(diagnostic(at, "integer overflow"))),
        ("-", Value::Int(a), Value::Int(b)) => a
            .checked_sub(b)
            .map(Value::Int)
            .ok_or_else(|| Flow::Error(diagnostic(at, "integer overflow"))),
        ("*", Value::Int(a), Value::Int(b)) => a
            .checked_mul(b)
            .map(Value::Int)
            .ok_or_else(|| Flow::Error(diagnostic(at, "integer overflow"))),
        ("/", Value::Int(a), Value::Int(b)) => a.checked_div(b).map(Value::Int).ok_or_else(|| {
            Flow::Error(diagnostic(
                at,
                if b == 0 {
                    "division by zero"
                } else {
                    "integer overflow"
                },
            ))
        }),
        ("%", Value::Int(a), Value::Int(b)) => a.checked_rem(b).map(Value::Int).ok_or_else(|| {
            Flow::Error(diagnostic(
                at,
                if b == 0 {
                    "division by zero"
                } else {
                    "integer overflow"
                },
            ))
        }),
        ("+", Value::Float(a), Value::Float(b)) => Ok(Value::Float(
            (f64::from_bits(a) + f64::from_bits(b)).to_bits(),
        )),
        ("-", Value::Float(a), Value::Float(b)) => Ok(Value::Float(
            (f64::from_bits(a) - f64::from_bits(b)).to_bits(),
        )),
        ("*", Value::Float(a), Value::Float(b)) => Ok(Value::Float(
            (f64::from_bits(a) * f64::from_bits(b)).to_bits(),
        )),
        ("/", Value::Float(a), Value::Float(b)) => Ok(Value::Float(
            (f64::from_bits(a) / f64::from_bits(b)).to_bits(),
        )),
        ("%", Value::Float(a), Value::Float(b)) => Ok(Value::Float(
            (f64::from_bits(a) % f64::from_bits(b)).to_bits(),
        )),
        ("==", Value::Float(a), Value::Float(b)) => {
            Ok(Value::Bool(f64::from_bits(a) == f64::from_bits(b)))
        }
        ("!=", Value::Float(a), Value::Float(b)) => {
            Ok(Value::Bool(f64::from_bits(a) != f64::from_bits(b)))
        }
        (op, Value::Regex(a), Value::Regex(b)) if matches!(op, "<" | "<=" | ">" | ">=") => {
            Ok(Value::Bool(match op {
                "<" => a < b,
                "<=" => a <= b,
                ">" => a > b,
                _ => a >= b,
            }))
        }
        (op, Value::Instant(a), Value::Instant(b)) if matches!(op, "<" | "<=" | ">" | ">=") => {
            Ok(Value::Bool(match op {
                "<" => a < b,
                "<=" => a <= b,
                ">" => a > b,
                _ => a >= b,
            }))
        }
        (op, Value::Duration(a), Value::Duration(b)) if matches!(op, "<" | "<=" | ">" | ">=") => {
            Ok(Value::Bool(match op {
                "<" => a < b,
                "<=" => a <= b,
                ">" => a > b,
                _ => a >= b,
            }))
        }
        (op, Value::Decimal(a), Value::Decimal(b))
            if matches!(op, "==" | "!=" | "<" | "<=" | ">" | ">=") =>
        {
            let order = a.0.cmp(&b.0);
            Ok(Value::Bool(match op {
                "==" => order.is_eq(),
                "!=" => !order.is_eq(),
                "<" => order.is_lt(),
                "<=" => !order.is_gt(),
                ">" => order.is_gt(),
                _ => !order.is_lt(),
            }))
        }
        ("==", a, b) => Ok(Value::Bool(a == b)),
        ("!=", a, b) => Ok(Value::Bool(a != b)),
        ("<", Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
        ("<=", Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
        (">", Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
        (">=", Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
        ("<", Value::Float(a), Value::Float(b)) => {
            Ok(Value::Bool(f64::from_bits(a) < f64::from_bits(b)))
        }
        ("<=", Value::Float(a), Value::Float(b)) => {
            Ok(Value::Bool(f64::from_bits(a) <= f64::from_bits(b)))
        }
        (">", Value::Float(a), Value::Float(b)) => {
            Ok(Value::Bool(f64::from_bits(a) > f64::from_bits(b)))
        }
        (">=", Value::Float(a), Value::Float(b)) => {
            Ok(Value::Bool(f64::from_bits(a) >= f64::from_bits(b)))
        }
        _ => Err(bad()),
    }
}

pub fn api_snapshot(root: &Path, output: Option<&Path>) -> Result<()> {
    v07::api_snapshot(root, output)
}
pub fn api_diff(before: &Path, after: &Path, deny: bool) -> Result<()> {
    v07::api_diff(before, after, deny)
}
pub fn doctest(root: &Path, path: &Path) -> Result<()> {
    v07::doctest(root, path)
}
pub fn timeline(
    path: &Path,
    start: usize,
    count: usize,
    task: Option<u64>,
    key: Option<&str>,
) -> Result<()> {
    if fs::metadata(path)?.len() > 128 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "timeline trace exceeds budget".into(),
        ));
    }
    if let Some(key) = key {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(path)?)
            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
        let kind = if value["kind"] == "rewind-inspection" {
            "inspection"
        } else {
            "trace"
        };
        packages::verify_file(
            path,
            &PathBuf::from(format!("{}.signature", path.display())),
            key,
            kind,
        )?;
    }
    v07::timeline(path, start, count, task)
}

pub fn repl(root: &Path, record: Option<&Path>) -> Result<()> {
    let config = project::ProjectConfig::load(root)?;
    if config.as_ref().is_some_and(|c| {
        matches!(
            c.language.as_str(),
            "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "2.0.0"
        )
    }) {
        v09::repl(root, record)
    } else if record.is_some() {
        Err(Error::InvalidOperation(
            "session recording requires language 0.9".into(),
        ))
    } else {
        v08::repl(root)
    }
}
pub fn session_replay(root: &Path, path: &Path) -> Result<()> {
    v09::session_replay(root, path)
}
pub fn install_production(root: &Path, output: &Path) -> Result<()> {
    v09::install_production(root, output)
}
pub fn trace_export(path: &Path, output: &Path, key: Option<&str>) -> Result<()> {
    v08::trace_export(path, output, key)
}

pub fn api_convert(root: &Path, input: &Path, output: &Path) -> Result<()> {
    v07::api_convert(root, input, output)
}

pub fn verify_release(path: &Path) -> Result<()> {
    v091::verify_release(path)
}

pub fn sdk_build(out: &Path, key: &Path) -> Result<()> {
    v092::sdk_build(out, key)
}
pub fn sdk_install(root: &Path, sdk: &Path, public: &str) -> Result<()> {
    v092::sdk_install(root, sdk, public)
}
pub fn sdk_verify(sdk: &Path, public: &str) -> Result<()> {
    v092::sdk_verify(sdk, public)
}

pub fn render_diagnostic(error: &Error) -> String {
    v11::render(error)
}
pub fn enrich_diagnostic(error: &mut Error) {
    if let Error::Diagnostic(d) = error {
        v11::enrich(d);
    }
}
