use rewind::{Error, MapKey, ResourceBudget, Result, Runtime, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
mod vm;

#[derive(Clone, Debug)]
struct Tok {
    text: String,
    line: usize,
    col: usize,
}

fn diagnostic(t: &Tok, message: impl AsRef<str>) -> Error {
    Error::InvalidOperation(format!("{}:{}: {}", t.line, t.col, message.as_ref()))
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
                        col += 1;
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
                        text,
                        line: start_line,
                        col: start_col,
                    },
                    "unterminated string; add a closing quote",
                ));
            }
        } else if c.is_ascii_digit() {
            while it.peek().is_some_and(|n| n.is_ascii_digit()) {
                text.push(it.next().unwrap());
                col += 1;
            }
            if it.peek() == Some(&'.') {
                let mut look = it.clone();
                look.next();
                if look.peek().is_some_and(|n| n.is_ascii_digit()) {
                    text.push(it.next().unwrap());
                    col += 1;
                    while it.peek().is_some_and(|n| n.is_ascii_digit()) {
                        text.push(it.next().unwrap());
                        col += 1;
                    }
                }
            }
            if matches!(it.peek(), Some('e' | 'E')) {
                let mut look = it.clone();
                look.next();
                if matches!(look.peek(), Some('+' | '-')) {
                    look.next();
                }
                if look.peek().is_some_and(|n| n.is_ascii_digit()) {
                    text.push(it.next().unwrap());
                    col += 1;
                    if matches!(it.peek(), Some('+' | '-')) {
                        text.push(it.next().unwrap());
                        col += 1;
                    }
                    while it.peek().is_some_and(|n| n.is_ascii_digit()) {
                        text.push(it.next().unwrap());
                        col += 1;
                    }
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
                        | ('.', '.')
                ) {
                    text.push(it.next().unwrap());
                    col += 1;
                }
            }
            if text.len() == 1 && !"=+-*/%(),;{}.!<>?:[]".contains(c) {
                return Err(diagnostic(
                    &Tok {
                        text,
                        line: start_line,
                        col: start_col,
                    },
                    "unexpected character",
                ));
            }
        }
        out.push(Tok {
            text,
            line: start_line,
            col: start_col,
        });
    }
    out.push(Tok {
        text: "<eof>".into(),
        line,
        col,
    });
    Ok(out)
}

#[derive(Clone, Debug)]
struct Expr {
    kind: ExprKind,
    at: Tok,
}
#[derive(Clone, Debug)]
enum ExprKind {
    Value(Value),
    Name(String),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Member(Box<Expr>, String),
    Try(Box<Expr>),
}
#[derive(Clone, Debug)]
struct Stmt {
    kind: StmtKind,
    at: Tok,
}
#[derive(Clone, Debug)]
enum StmtKind {
    Let(String, bool, Option<String>, Expr),
    Assign(Expr, String, Expr),
    Expr(Expr),
    Block(Vec<Stmt>),
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
    Match(Expr, Vec<(String, String, Expr)>),
}
#[derive(Clone, Debug)]
struct Function {
    params: Vec<(String, String)>,
    ret: String,
    body: Vec<Stmt>,
    at: Tok,
    test: bool,
}
#[derive(Clone, Debug)]
struct StructDef {
    fields: Vec<(String, String)>,
}
#[derive(Clone, Debug, Default)]
struct Program {
    stmts: Vec<Stmt>,
    functions: BTreeMap<String, Function>,
    structs: BTreeMap<String, StructDef>,
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}
impl Parser {
    fn current(&self) -> &Tok {
        &self.toks[self.pos]
    }
    fn is(&self, s: &str) -> bool {
        self.current().text == s
    }
    fn eat(&mut self, s: &str) -> bool {
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
        let mut name = self.name()?;
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
    fn program(&mut self) -> Result<Program> {
        let mut p = Program::default();
        while !self.is("<eof>") {
            if self.eat("import") {
                let mut path = self.name()?;
                while self.eat(".") {
                    path.push('/');
                    path.push_str(&self.name()?);
                }
                self.need(";")?;
                let at = self.toks[self.pos - 1].clone();
                p.stmts.push(Stmt {
                    kind: StmtKind::Expr(Expr {
                        kind: ExprKind::Name(format!("@import:{path}")),
                        at: at.clone(),
                    }),
                    at,
                });
            } else if self.eat("struct") {
                let name = self.name()?;
                self.need("{")?;
                let mut fields = Vec::new();
                while !self.eat("}") {
                    let field = self.name()?;
                    self.need(":")?;
                    let ty = self.ty()?;
                    fields.push((field, ty));
                    if !self.is("}") {
                        self.need(",")?;
                    }
                }
                if p.structs
                    .insert(name.clone(), StructDef { fields })
                    .is_some()
                {
                    return Err(diagnostic(
                        self.current(),
                        format!("duplicate struct {name}"),
                    ));
                }
            } else if self.is("fn")
                || self.is("pub")
                || (self.is("test") && self.toks.get(self.pos + 1).is_some_and(|t| t.text == "fn"))
            {
                let test = self.eat("test");
                let public = self.eat("pub");
                let at = self.current().clone();
                self.need("fn")?;
                let name = self.name()?;
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
                let body = self.block()?;
                if p.functions
                    .insert(
                        name.clone(),
                        Function {
                            params,
                            ret,
                            body,
                            at,
                            test,
                        },
                    )
                    .is_some()
                {
                    return Err(diagnostic(
                        self.current(),
                        format!("duplicate function {name}"),
                    ));
                }
            } else {
                p.stmts.push(self.stmt()?);
            }
        }
        Ok(p)
    }
    fn block(&mut self) -> Result<Vec<Stmt>> {
        self.need("{")?;
        let mut body = Vec::new();
        while !self.eat("}") {
            if self.is("<eof>") {
                return Err(diagnostic(self.current(), "unclosed block; add '}'"));
            }
            body.push(self.stmt()?);
        }
        Ok(body)
    }
    fn stmt(&mut self) -> Result<Stmt> {
        let at = self.current().clone();
        let kind = if self.eat("let") || self.eat("var") {
            let mutable = at.text == "var";
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
            self.need("..")?;
            let end = self.expr(0)?;
            StmtKind::For(name, start, end, self.block()?)
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
        } else if self.eat("commit") {
            let n = self.name()?;
            self.need(";")?;
            StmtKind::Commit(n)
        } else if self.eat("revert") {
            let n = self.name()?;
            self.need(";")?;
            StmtKind::Revert(n)
        } else if self.eat("resume") {
            let n = self.name()?;
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
                let variant = self.name()?;
                self.need("(")?;
                let binding = self.name()?;
                self.need(")")?;
                self.need("=>")?;
                let expr = self.expr(0)?;
                arms.push((variant, binding, expr));
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
                    self.need(")")?;
                    e
                }
            }
            "-" if self.is("9223372036854775808") => {
                self.pos += 1;
                Expr {
                    kind: ExprKind::Value(Value::Int(i64::MIN)),
                    at: at.clone(),
                }
            }
            "-" | "!" => Expr {
                kind: ExprKind::Unary(at.text.clone(), Box::new(self.expr(13)?)),
                at: at.clone(),
            },
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
                            _ => return Err(diagnostic(&at, "invalid escape")),
                        }
                    } else {
                        s.push(c);
                    }
                }
                Expr {
                    kind: ExprKind::Value(Value::Text(s)),
                    at: at.clone(),
                }
            }
            _ if at.text.chars().next().is_some_and(|c| c.is_ascii_digit()) => {
                let v = if at.text.contains('.') || at.text.contains('e') || at.text.contains('E') {
                    Value::Float(
                        at.text
                            .parse::<f64>()
                            .map_err(|_| diagnostic(&at, "invalid float"))?
                            .to_bits(),
                    )
                } else {
                    Value::Int(
                        at.text
                            .parse::<i64>()
                            .map_err(|_| diagnostic(&at, "integer literal overflow"))?,
                    )
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
                if matches!(at.text.as_str(), "List" | "Map") && self.eat("<") {
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
                Expr {
                    kind: ExprKind::Name(name),
                    at: at.clone(),
                }
            }
            _ => return Err(diagnostic(&at, "expected an expression")),
        };
        loop {
            if self.eat(".") {
                let field = self.name()?;
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

fn load_program(
    path: &Path,
    root: &Path,
    visiting: &mut BTreeSet<PathBuf>,
    loaded: &mut BTreeSet<PathBuf>,
) -> Result<Program> {
    let full = fs::canonicalize(path)?;
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
    if !loaded.insert(full.clone()) {
        return Ok(Program::default());
    }
    visiting.insert(full.clone());
    let source = fs::read_to_string(&full)?;
    let mut parser = Parser {
        toks: lex(&source)?,
        pos: 0,
    };
    let mut own = parser.program()?;
    let mut program = Program::default();
    for stmt in &own.stmts {
        if let StmtKind::Expr(Expr {
            kind: ExprKind::Name(name),
            ..
        }) = &stmt.kind
        {
            if let Some(module) = name.strip_prefix("@import:") {
                let imported = load_program(
                    &project_root.join(format!("{module}.rw")),
                    root,
                    visiting,
                    loaded,
                )?;
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
                program.stmts.extend(imported.stmts);
            }
        }
    }
    own.stmts.retain(|s| !matches!(&s.kind, StmtKind::Expr(Expr { kind: ExprKind::Name(n), .. }) if n.starts_with("@import:")));
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
    program.stmts.extend(own.stmts);
    visiting.remove(&full);
    Ok(program)
}

pub fn cli(mode: &str, file: &str, root: &Path, trace: bool) -> Result<()> {
    let program = load_program(
        Path::new(file),
        root,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
    )?;
    check_program(&program)?;
    vm::validate(&program)?;
    if mode == "check" {
        return Ok(());
    }
    vm::execute(program, root, trace, mode)
}

fn check_program(program: &Program) -> Result<()> {
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
    };
    for stmt in &program.stmts {
        checker.stmt(stmt)?;
    }
    let globals = checker.scopes[0].clone();
    for f in program.functions.values() {
        let mut checker = Checker {
            program,
            scopes: vec![globals.clone(), BTreeMap::new()],
            return_ty: Some(f.ret.clone()),
            loop_depth: 0,
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
    }
    Ok(())
}
fn guarantees_return(body: &[Stmt]) -> bool {
    body.iter().any(|stmt| match &stmt.kind {
        StmtKind::Return(_) => true,
        StmtKind::Block(body) => guarantees_return(body),
        StmtKind::If(_, yes, no) => guarantees_return(yes) && guarantees_return(no),
        _ => false,
    })
}

struct Checker<'a> {
    program: &'a Program,
    scopes: Vec<BTreeMap<String, (String, bool)>>,
    return_ty: Option<String>,
    loop_depth: usize,
}
impl Checker<'_> {
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
                Value::FileError(_) => "FileError",
                Value::Null => "Unit",
                Value::Option(_) => "Option<Unknown>",
                Value::Result(_) => "Result<Unknown,Unknown>",
                _ => "Unknown",
            }
            .into(),
            ExprKind::Name(n) => self
                .find(n)
                .map(|b| b.0.clone())
                .ok_or_else(|| diagnostic(&e.at, format!("unknown name {n}")))?,
            ExprKind::Unary(op, inner) => {
                let t = self.expr(inner)?;
                if op == "!" && t != "Bool" || op == "-" && !matches!(t.as_str(), "Int" | "Float") {
                    return Err(diagnostic(&e.at, format!("invalid operand {t} for {op}")));
                }
                t
            }
            ExprKind::Binary(op, a, b) => {
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
                    if matches!(
                        n.as_str(),
                        "Out" | "Err" | "File" | "Directory" | "Time" | "Random" | "In"
                    ) {
                        return Ok("Builtin".into());
                    }
                }
                let t = self.expr(base)?;
                if let Some(def) = self.program.structs.get(&t) {
                    def.fields
                        .iter()
                        .find(|(n, _)| n == field)
                        .map(|(_, t)| t.clone())
                        .ok_or_else(|| diagnostic(&e.at, format!("unknown field {field}")))?
                } else if t == "FileHandle" && field == "position" {
                    "Int".into()
                } else {
                    return Err(diagnostic(&e.at, format!("type {t} has no field {field}")));
                }
            }
            ExprKind::Call(target, args) => {
                let types = args
                    .iter()
                    .map(|a| self.expr(a))
                    .collect::<Result<Vec<_>>>()?;
                if let ExprKind::Name(name) = &target.kind {
                    if let Some(f) = self.program.functions.get(name) {
                        if f.params.len() != types.len() {
                            return Err(diagnostic(
                                &e.at,
                                format!("{name} expects {} arguments", f.params.len()),
                            ));
                        }
                        for ((_, expected), actual) in f.params.iter().zip(&types) {
                            if !compatible(expected, actual) && actual != "Unknown" {
                                return Err(diagnostic(
                                    &e.at,
                                    format!("{name} expects {expected}, found {actual}"),
                                ));
                            }
                        }
                        f.ret.clone()
                    } else if let Some(s) = self.program.structs.get(name) {
                        if s.fields.len() != types.len() {
                            return Err(diagnostic(
                                &e.at,
                                format!("{name} expects {} fields", s.fields.len()),
                            ));
                        }
                        for ((field, expected), actual) in s.fields.iter().zip(&types) {
                            if !compatible(expected, actual) {
                                return Err(diagnostic(
                                    &e.at,
                                    format!("field {field} expects {expected}, found {actual}"),
                                ));
                            }
                        }
                        name.clone()
                    } else {
                        match name.as_str() {
                            "List" => "List".into(),
                            "Map" => "Map".into(),
                            "Bytes" => "Bytes".into(),
                            "Ok" if types.len() == 1 => format!("Result<{},Unknown>", types[0]),
                            "Err" if types.len() == 1 => format!("Result<Unknown,{}>", types[0]),
                            "Some" if types.len() == 1 => format!("Option<{}>", types[0]),
                            "assert" if types == ["Bool"] => "Unit".into(),
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
                                        || !matches!(
                                            parts[0],
                                            "Bool" | "Int" | "Float" | "String" | "Bytes"
                                        )
                                    {
                                        return Err(diagnostic(&e.at, "Map key type must be Bool, Int, Float, String, or Bytes"));
                                    }
                                }
                                name.clone()
                            }
                            _ => return Err(diagnostic(&e.at, format!("unknown function {name}"))),
                        }
                    }
                } else if let ExprKind::Member(base, method) = &target.kind {
                    if let ExprKind::Name(n) = &base.kind {
                        if matches!(
                            n.as_str(),
                            "Out" | "Err" | "File" | "Directory" | "Time" | "Random" | "In"
                        ) {
                            builtin_type(n, method, &types, &e.at)?
                        } else {
                            let t = self.expr(base)?;
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
                            let arity_ok = matches!(
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
                        match (
                            t.split('<').next().unwrap_or(""),
                            method.as_str(),
                            types.len(),
                        ) {
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
                    return Err(diagnostic(&e.at, "invalid call target"));
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
        })
    }
    fn stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match &stmt.kind {
            StmtKind::Let(name, mutable, ty, expr) => {
                let inferred = self.expr(expr)?;
                if let Some(expected) = ty {
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
                    .insert(name.clone(), (ty.clone().unwrap_or(inferred), *mutable));
            }
            StmtKind::Assign(lhs, _, rhs) => {
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
                    ExprKind::Member(_, _) => {
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
                if !defer_safe(expr) {
                    return Err(diagnostic(
                        &stmt.at,
                        "defer may only call virtual I/O or object methods",
                    ));
                }
                self.expr(expr)?;
            }
            StmtKind::Block(body) | StmtKind::Branch(_, body) => self.block(body)?,
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
                        format!("return type mismatch: expected {expected}, found {actual}"),
                    ));
                }
            }
            StmtKind::Match(value, arms) => {
                let t = self.expr(value)?;
                if !t.starts_with("Result") && !t.starts_with("Option") {
                    return Err(diagnostic(&value.at, "match expects Result or Option"));
                }
                let required = if t.starts_with("Result") {
                    ["Ok", "Err"]
                } else {
                    ["Some", "None"]
                };
                let present = arms
                    .iter()
                    .map(|(variant, _, _)| variant.as_str())
                    .collect::<BTreeSet<_>>();
                if present.len() != arms.len()
                    || required.iter().any(|variant| !present.contains(variant))
                    || present.len() != 2
                {
                    return Err(diagnostic(
                        &stmt.at,
                        "match must have exactly one arm for each variant",
                    ));
                }
                let parts = t
                    .split_once('<')
                    .and_then(|(_, tail)| tail.strip_suffix('>'))
                    .map(split_type_args)
                    .unwrap_or_default();
                for (variant, binding, expr) in arms {
                    let inner_ty = match variant.as_str() {
                        "Ok" | "Some" => parts.first().copied().unwrap_or("Unknown"),
                        "Err" => parts.get(1).copied().unwrap_or("Unknown"),
                        _ => "Unit",
                    };
                    self.scopes.push(BTreeMap::new());
                    self.scopes
                        .last_mut()
                        .unwrap()
                        .insert(binding.clone(), (inner_ty.into(), false));
                    self.expr(expr)?;
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
        ExprKind::Value(_) | ExprKind::Name(_) => true,
    }
}
fn builtin_type(receiver: &str, method: &str, args: &[String], at: &Tok) -> Result<String> {
    let count = args.len();
    let result = match (receiver, method, count) {
        ("Out" | "Err", "println", 1) | ("Out" | "Err", "flush", 0) => "Unit",
        ("In", "readLine", 0) => "Option<String>",
        ("Time", "now", 0) | ("Random", "next", 0) => "Int",
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
    if matches!(receiver, "File" | "Directory") {
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
        Ok(Self {
            program,
            runtime: Runtime::new(root)?,
            input,
            scopes: vec![BTreeMap::new()],
            snapshots: BTreeMap::new(),
            remaining: 1_000_000,
            top_pc: 0,
            function_depth: 0,
            defers: vec![Vec::new()],
            test_mode: false,
            trace: false,
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
    call_path: Vec<u64>,
    next_call_id: u64,
}
impl<R: BufRead> Engine<R> {
    fn fail(&self, at: &Tok, msg: impl AsRef<str>) -> Flow {
        Flow::Error(diagnostic(at, msg))
    }
    fn tick(&mut self, at: &Tok) -> Exec<()> {
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
        scope.insert(
            name.clone(),
            Binding {
                value: value.clone(),
                mutable,
                ty,
            },
        );
        if self.function_depth == 0 {
            self.runtime.set_global(name, value)?;
        } else {
            self.runtime.set_local(name, value)?;
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
                    binary(&op[..1], old.value, rhs, &lhs.at)?
                };
                let actual = value_type(&value, &self.runtime);
                if !compatible(&old.ty, &actual) {
                    return Err(self.fail(
                        &lhs.at,
                        format!("type mismatch: expected {}, found {actual}", old.ty),
                    ));
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
            StmtKind::Assign(lhs, op, rhs) => {
                let rhs = self.eval(rhs)?;
                self.assign(lhs, op, rhs)
            }
            StmtKind::Expr(e) => {
                self.eval(e)?;
                Ok(())
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
                let (variant, inner) = match v {
                    Value::Result(Ok(v)) => ("Ok", *v),
                    Value::Result(Err(v)) => ("Err", *v),
                    Value::Option(Some(v)) => ("Some", *v),
                    Value::Option(None) => ("None", Value::Null),
                    _ => return Err(self.fail(&value.at, "match expects Result or Option")),
                };
                let (_, binding, arm) = arms
                    .iter()
                    .find(|(v, _, _)| v == variant)
                    .ok_or_else(|| self.fail(&stmt.at, "non-exhaustive match"))?;
                self.scopes.push(BTreeMap::new());
                self.define(binding.clone(), inner, false, None, &stmt.at)?;
                let result = self.eval(arm).map(|_| ());
                self.scopes.pop();
                result
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
            ExprKind::Name(n) => self
                .get(n)
                .map(|b| b.value.clone())
                .ok_or_else(|| self.fail(&e.at, format!("unknown name {n}"))),
            ExprKind::Unary(op, inner) => {
                let v = self.eval(inner)?;
                match (op.as_str(), v) {
                    ("-", Value::Int(n)) => n
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| self.fail(&e.at, "integer overflow")),
                    ("-", Value::Float(n)) => Ok(Value::Float((-f64::from_bits(n)).to_bits())),
                    ("!", Value::Bool(n)) => Ok(Value::Bool(!n)),
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
                binary(op, lhs, rhs, &e.at)
            }
            ExprKind::Member(base, field) => {
                let value = self.eval(base)?;
                match value {
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
        }
    }
}

impl<R: BufRead> Engine<R> {
    fn call(&mut self, name: &str, args: Vec<Value>, at: &Tok) -> Exec<Value> {
        if let Some(inner) = name.strip_prefix("List<").and_then(|s| s.strip_suffix('>')) {
            if !args.is_empty() {
                return Err(self.fail(at, "List constructor takes no arguments"));
            }
            return Ok(Value::HeapRef(
                self.runtime
                    .alloc(Value::TypedList(inner.into(), Vec::new()))?,
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
            return Ok(Value::HeapRef(self.runtime.alloc(Value::TypedMap(
                types[0].into(),
                types[1].into(),
                BTreeMap::new(),
            ))?));
        }
        match name {
            "List" if args.is_empty() => {
                return Ok(Value::HeapRef(self.runtime.alloc(Value::List(Vec::new()))?))
            }
            "Map" if args.is_empty() => {
                return Ok(Value::HeapRef(
                    self.runtime.alloc(Value::Map(BTreeMap::new()))?,
                ))
            }
            "Bytes" if args.len() == 1 => match &args[0] {
                Value::Text(s) => return Ok(Value::Bytes(s.as_bytes().to_vec())),
                _ => return Err(self.fail(at, "Bytes expects a String")),
            },
            "Ok" if args.len() == 1 => return Ok(Value::Result(Ok(Box::new(args[0].clone())))),
            "Err" if args.len() == 1 => return Ok(Value::Result(Err(Box::new(args[0].clone())))),
            "Some" if args.len() == 1 => return Ok(Value::Option(Some(Box::new(args[0].clone())))),
            "assert" if args == [Value::Bool(true)] => return Ok(Value::Null),
            "assert" if args == [Value::Bool(false)] => {
                return Err(self.fail(at, "assertion failed"))
            }
            _ => {}
        }
        if let Some(def) = self.program.structs.get(name).cloned() {
            if args.len() != def.fields.len() {
                return Err(self.fail(at, format!("{name} expects {} fields", def.fields.len())));
            }
            let mut fields = BTreeMap::new();
            for ((field, ty), arg) in def.fields.iter().zip(args) {
                let actual = value_type(&arg, &self.runtime);
                if !compatible(ty, &actual) {
                    return Err(
                        self.fail(at, format!("field {field} expects {ty}, found {actual}"))
                    );
                }
                fields.insert(field.clone(), arg);
            }
            return Ok(Value::HeapRef(
                self.runtime.alloc(Value::Struct(name.into(), fields))?,
            ));
        }
        let def = self
            .program
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| self.fail(at, format!("unknown function {name}")))?;
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
            self.define(param.clone(), arg, false, Some(ty.clone()), at)?;
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
    fn method(&mut self, target: Value, method: &str, args: Vec<Value>, at: &Tok) -> Exec<Value> {
        let unit = Value::Null;
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
            Value::HeapRef(id) => {
                let old = self
                    .runtime
                    .heap_get(id)
                    .cloned()
                    .ok_or_else(|| self.fail(at, "unknown object"))?;
                match old {
                    Value::TypedList(ty, mut list) => {
                        match (method, args.as_slice()) {
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
                                list[*n as usize] = value.clone();
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
                            map.insert(key, value.clone());
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
                            map.insert(key, v.clone());
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
                        .map(Value::Text)
                        .map_err(|e| self.fail(at, e.to_string()))
                }
                ("readBytes", [Value::Int(n)]) if *n >= 0 => {
                    Ok(Value::Bytes(self.runtime.read_handle(id, *n as usize)?))
                }
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
            ("In", "readLine", 0) => Ok(Value::Option(
                self.runtime
                    .input_line(&mut self.input)?
                    .map(|line| Box::new(Value::Text(line))),
            )),
            ("Time", "now", 0) => Ok(Value::Int(self.runtime.now_millis()? as i64)),
            ("Random", "next", 0) => Ok(Value::Int(self.runtime.random_u64() as i64)),
            ("File", "readText", 1) => {
                let result = self.runtime.read_file(&strings()[0]).and_then(|v| {
                    String::from_utf8(v).map_err(|e| Error::InvalidOperation(e.to_string()))
                });
                Ok(match result {
                    Ok(s) => Value::Result(Ok(Box::new(Value::Text(s)))),
                    Err(e) => Value::Result(Err(Box::new(Value::FileError(e.to_string())))),
                })
            }
            ("File", "readBytes", 1) => Ok(match self.runtime.read_file(&strings()[0]) {
                Ok(v) => Value::Result(Ok(Box::new(Value::Bytes(v)))),
                Err(e) => Value::Result(Err(Box::new(Value::FileError(e.to_string())))),
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
                self.runtime.write_file(&strings()[0], bytes)?;
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
    if let Value::HeapRef(id) = v {
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
        Value::FileError(_) => "FileError",
        Value::Null => "Unit",
        Value::List(_) => "List",
        Value::TypedList(ty, _) => return format!("List<{ty}>"),
        Value::Map(_) => "Map",
        Value::TypedMap(key, value, _) => return format!("Map<{key},{value}>"),
        Value::Struct(name, _) => name,
        Value::Option(Some(v)) => return format!("Option<{}>", value_type(v, rt)),
        Value::Option(None) => return "Option<Unknown>".into(),
        Value::Result(Ok(v)) => return format!("Result<{},Unknown>", value_type(v, rt)),
        Value::Result(Err(v)) => return format!("Result<Unknown,{}>", value_type(v, rt)),
        Value::Handle(_) => "FileHandle",
        Value::HeapRef(_) => unreachable!(),
    }
    .into()
}
fn compatible(expected: &str, actual: &str) -> bool {
    if expected == actual || expected == "Unknown" || actual == "Unknown" {
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
    let expected_parts = split_type_args(expected_inner.trim_end_matches('>'));
    let actual_parts = split_type_args(actual_inner.trim_end_matches('>'));
    expected_parts.len() == actual_parts.len()
        && expected_parts
            .iter()
            .zip(actual_parts)
            .all(|(a, b)| compatible(a, b))
}
fn split_type_args(input: &str) -> Vec<&str> {
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut out = Vec::new();
    for (i, c) in input.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&input[start..]);
    out
}
fn binary(op: &str, a: Value, b: Value, at: &Tok) -> Exec<Value> {
    let bad = || Flow::Error(diagnostic(at, format!("invalid operands for '{op}'")));
    match (op, a, b) {
        ("+", Value::Text(a), Value::Text(b)) => Ok(Value::Text(a + &b)),
        ("+", Value::Text(a), b) => Ok(Value::Text(a + &b.to_string())),
        ("+", a, Value::Text(b)) => Ok(Value::Text(a.to_string() + &b)),
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
        ("/", Value::Int(a), Value::Int(b)) => a
            .checked_div(b)
            .map(Value::Int)
            .ok_or_else(|| Flow::Error(diagnostic(at, "division by zero or integer overflow"))),
        ("%", Value::Int(a), Value::Int(b)) => a
            .checked_rem(b)
            .map(Value::Int)
            .ok_or_else(|| Flow::Error(diagnostic(at, "division by zero or integer overflow"))),
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
