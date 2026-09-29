use rewind::{Error, ResourceBudget, Result, Runtime, Value};
use std::env;
use std::fs;
use std::io::{self, BufRead};
mod v2;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Ident(String),
    Number(i64),
    String(String),
    Symbol(char),
    End,
}

fn lex(source: &str) -> std::result::Result<Vec<Token>, String> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            for next in chars.by_ref() {
                if next == '\n' {
                    break;
                }
            }
            continue;
        }
        if c == '"' {
            let mut value = String::new();
            let mut closed = false;
            while let Some(next) = chars.next() {
                match next {
                    '"' => {
                        closed = true;
                        break;
                    }
                    '\\' => match chars.next() {
                        Some('n') => value.push('\n'),
                        Some('r') => value.push('\r'),
                        Some('t') => value.push('\t'),
                        Some('"') => value.push('"'),
                        Some('\\') => value.push('\\'),
                        Some(other) => return Err(format!("unknown escape: \\{other}")),
                        None => return Err("unterminated escape".into()),
                    },
                    other => value.push(other),
                }
            }
            if !closed {
                return Err("unterminated string".into());
            }
            tokens.push(Token::String(value));
            continue;
        }
        if c.is_ascii_digit() {
            let mut number = c.to_string();
            while chars.peek().is_some_and(|x| x.is_ascii_digit()) {
                number.push(chars.next().unwrap());
            }
            tokens.push(Token::Number(
                number.parse().map_err(|_| "integer overflow")?,
            ));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut ident = c.to_string();
            while chars
                .peek()
                .is_some_and(|x| x.is_alphanumeric() || *x == '_')
            {
                ident.push(chars.next().unwrap());
            }
            tokens.push(Token::Ident(ident));
            continue;
        }
        if "=+-*(),;{}.".contains(c) {
            tokens.push(Token::Symbol(c));
            continue;
        }
        return Err(format!("unexpected character: {c}"));
    }
    tokens.push(Token::End);
    Ok(tokens)
}

struct Runner<R: BufRead> {
    tokens: Vec<Token>,
    pos: usize,
    runtime: Runtime,
    input: R,
}

impl<R: BufRead> Runner<R> {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn advance(&mut self) -> Token {
        let t = self.peek().clone();
        self.pos += 1;
        t
    }
    fn symbol(&mut self, c: char) -> bool {
        if self.peek() == &Token::Symbol(c) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: char) -> Result<()> {
        if self.symbol(c) {
            Ok(())
        } else {
            Err(self.syntax(format!("expected '{c}'")))
        }
    }
    fn ident(&mut self) -> Result<String> {
        match self.advance() {
            Token::Ident(s) => Ok(s),
            _ => Err(self.syntax("expected name".into())),
        }
    }
    fn syntax(&self, message: String) -> Error {
        Error::InvalidOperation(format!("token {}: {message}", self.pos))
    }
    fn run(&mut self) -> Result<()> {
        while self.peek() != &Token::End {
            self.statement()?;
        }
        Ok(())
    }
    fn statement(&mut self) -> Result<()> {
        self.runtime.set_program_counter(self.pos);
        let name = self.ident()?;
        match name.as_str() {
            "runtime" => {
                self.expect('{')?;
                let mut budget = ResourceBudget::default();
                while !self.symbol('}') {
                    let setting = self.ident()?;
                    self.expect('=')?;
                    let number = match self.advance() {
                        Token::Number(n) if n >= 0 => n as usize,
                        _ => return Err(self.syntax("expected nonnegative size".into())),
                    };
                    let scale = match self.peek() {
                        Token::Ident(unit) => {
                            let scale = match unit.as_str() {
                                "KB" | "KiB" => 1024,
                                "MB" | "MiB" => 1024 * 1024,
                                "GB" | "GiB" => 1024 * 1024 * 1024,
                                _ => return Err(self.syntax(format!("unknown size unit: {unit}"))),
                            };
                            self.advance();
                            scale
                        }
                        _ => 1,
                    };
                    let size = number
                        .checked_mul(scale)
                        .ok_or_else(|| self.syntax("size overflow".into()))?;
                    self.expect(';')?;
                    match setting.as_str() {
                        "historyMemory" => budget.history_memory = size,
                        "historyStorage" => budget.history_storage = size,
                        "spillThreshold" => budget.spill_threshold = size,
                        _ => return Err(self.syntax(format!("unknown runtime setting: {setting}"))),
                    }
                }
                self.runtime.set_budget(budget)
            }
            "var" => {
                let key = self.ident()?;
                self.expect('=')?;
                let value = self.expression(0)?;
                self.expect(';')?;
                self.runtime.set_global(key, value)
            }
            "commit" => {
                let key = self.ident()?;
                self.expect(';')?;
                self.runtime.commit(key)
            }
            "revert" => {
                let key = self.ident()?;
                self.expect(';')?;
                self.runtime.revert(&key)
            }
            "drop" => {
                let key = self.ident()?;
                self.expect(';')?;
                self.runtime.drop_checkpoint(&key)
            }
            "publish" => {
                let force = if self.peek() == &Token::Ident("force".into()) {
                    self.advance();
                    true
                } else {
                    false
                };
                self.expect(';')?;
                self.runtime
                    .publish(force, &mut io::stdout().lock(), &mut io::stderr().lock())
            }
            "branch" => {
                let key = self.ident()?;
                self.expect('{')?;
                let anchor = self.runtime.begin_branch();
                while !self.symbol('}') {
                    if self.peek() == &Token::End {
                        return Err(self.syntax("unclosed branch".into()));
                    }
                    self.statement()?;
                }
                self.runtime.end_branch(key, anchor)
            }
            "Out" | "Err" | "File" | "Directory" => self.method_statement(&name),
            key => {
                if self.symbol('.') {
                    let method = self.ident()?;
                    self.expect('(')?;
                    let args = self.arguments()?;
                    self.expect(';')?;
                    return self.invoke(key, &method, args).map(|_| ());
                }
                let operator = if self.symbol('=') {
                    '='
                } else if self.symbol('+') {
                    self.expect('=')?;
                    '+'
                } else if self.symbol('-') {
                    self.expect('=')?;
                    '-'
                } else if self.symbol('*') {
                    self.expect('=')?;
                    '*'
                } else {
                    return Err(self.syntax("expected assignment".into()));
                };
                let rhs = self.expression(0)?;
                self.expect(';')?;
                let value = if operator == '=' {
                    rhs
                } else {
                    let lhs = self
                        .runtime
                        .global(key)
                        .cloned()
                        .ok_or_else(|| self.syntax(format!("unknown variable: {key}")))?;
                    binary(operator, lhs, rhs)?
                };
                self.runtime.set_global(key, value)
            }
        }
    }
    fn method_statement(&mut self, receiver: &str) -> Result<()> {
        self.expect('.')?;
        let method = self.ident()?;
        self.expect('(')?;
        let args = self.arguments()?;
        self.expect(';')?;
        self.invoke(receiver, &method, args).map(|_| ())
    }
    fn arguments(&mut self) -> Result<Vec<Value>> {
        let mut args = Vec::new();
        if self.symbol(')') {
            return Ok(args);
        }
        loop {
            args.push(self.expression(0)?);
            if self.symbol(')') {
                break;
            }
            self.expect(',')?;
        }
        Ok(args)
    }
    fn expression(&mut self, min_prec: u8) -> Result<Value> {
        let mut lhs = match self.advance() {
            Token::Number(n) => Value::Int(n),
            Token::String(s) => Value::Text(s),
            Token::Symbol('(') => {
                let v = self.expression(0)?;
                self.expect(')')?;
                v
            }
            Token::Symbol('-') => match self.expression(3)? {
                Value::Int(n) => Value::Int(
                    n.checked_neg()
                        .ok_or_else(|| self.syntax("integer overflow".into()))?,
                ),
                _ => return Err(self.syntax("expected integer".into())),
            },
            Token::Ident(name) => {
                if self.symbol('.') {
                    let method = self.ident()?;
                    if self.symbol('(') {
                        let args = self.arguments()?;
                        self.invoke(&name, &method, args)?
                    } else {
                        match (self.runtime.global(&name), method.as_str()) {
                            (Some(Value::Handle(id)), "position") => {
                                Value::Int(self.runtime.handle(*id)?.position as i64)
                            }
                            _ => {
                                return Err(
                                    self.syntax(format!("unknown property: {name}.{method}"))
                                )
                            }
                        }
                    }
                } else if self.symbol('(') {
                    let args = self.arguments()?;
                    match (name.as_str(), args.len()) {
                        ("List", 0) => Value::HeapRef(self.runtime.alloc(Value::List(Vec::new()))?),
                        _ => return Err(self.syntax(format!("unsupported constructor: {name}"))),
                    }
                } else {
                    self.runtime
                        .global(&name)
                        .cloned()
                        .ok_or_else(|| self.syntax(format!("unknown variable: {name}")))?
                }
            }
            _ => return Err(self.syntax("expected expression".into())),
        };
        loop {
            let (operator, prec) = match self.peek() {
                Token::Symbol('+') => ('+', 1),
                Token::Symbol('-') => ('-', 1),
                Token::Symbol('*') => ('*', 2),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            self.advance();
            let rhs = self.expression(prec + 1)?;
            lhs = binary(operator, lhs, rhs)?;
        }
        Ok(lhs)
    }
    fn invoke(&mut self, receiver: &str, method: &str, args: Vec<Value>) -> Result<Value> {
        if let Some(value) = self.runtime.global(receiver).cloned() {
            return match (value, method, args.len()) {
                (Value::HeapRef(id), "add", 1) => {
                    let mut list = match self.runtime.heap_get(id) {
                        Some(Value::List(list)) => list.clone(),
                        _ => return Err(self.syntax("invalid list reference".into())),
                    };
                    list.push(args[0].clone());
                    self.runtime.heap_set(id, Value::List(list))?;
                    Ok(Value::Null)
                }
                (Value::Handle(id), "seek", 1) => match args[0] {
                    Value::Int(n) if n >= 0 => {
                        self.runtime.seek(id, n as usize)?;
                        Ok(Value::Null)
                    }
                    _ => Err(self.syntax("seek requires a nonnegative integer".into())),
                },
                (Value::Handle(id), "read", 1) => match args[0] {
                    Value::Int(n) if n >= 0 => {
                        let bytes = self.runtime.read_handle(id, n as usize)?;
                        String::from_utf8(bytes)
                            .map(Value::Text)
                            .map_err(|e| Error::InvalidOperation(e.to_string()))
                    }
                    _ => Err(self.syntax("read requires a nonnegative integer".into())),
                },
                (Value::Handle(id), "write", 1) => {
                    self.runtime
                        .write_handle(id, args[0].to_string().as_bytes())?;
                    Ok(Value::Null)
                }
                _ => Err(self.syntax(format!("unsupported object call: {receiver}.{method}"))),
            };
        }
        let strings = || args.iter().map(ToString::to_string).collect::<Vec<_>>();
        match (receiver, method, args.len()) {
            ("Out", "println", 1) => {
                let text = self.runtime.display_value(&args[0]);
                self.runtime.print_out(&(text + "\n"))?;
                Ok(Value::Null)
            }
            ("Err", "println", 1) => {
                let text = self.runtime.display_value(&args[0]);
                self.runtime.print_err(&(text + "\n"))?;
                Ok(Value::Null)
            }
            ("Out", "flush", 0) | ("Err", "flush", 0) => Ok(Value::Null),
            ("In", "readLine", 0) => Ok(self
                .runtime
                .input_line(&mut self.input)?
                .map(Value::Text)
                .unwrap_or(Value::Null)),
            ("Time", "now", 0) => Ok(Value::Text(self.runtime.now_millis()?.to_string())),
            ("Random", "next", 0) => Ok(Value::Int(self.runtime.random_u64() as i64)),
            ("File", "readText", 1) => {
                let text = String::from_utf8(self.runtime.read_file(&strings()[0])?)
                    .map_err(|e| Error::InvalidOperation(e.to_string()))?;
                Ok(Value::Text(text))
            }
            ("File", "open", 1) => Ok(Value::Handle(self.runtime.open_file(&strings()[0])?)),
            ("File", "openSnapshot", 1) => {
                Ok(Value::Handle(self.runtime.open_snapshot(&strings()[0])?))
            }
            ("File", "create", 1) => {
                self.runtime.write_file(&strings()[0], [])?;
                Ok(Value::Null)
            }
            ("File", "write", 2) | ("File", "create", 2) => {
                let a = strings();
                self.runtime.write_file(&a[0], a[1].as_bytes())?;
                Ok(Value::Null)
            }
            ("File", "append", 2) => {
                let a = strings();
                self.runtime.append_file(&a[0], a[1].as_bytes())?;
                Ok(Value::Null)
            }
            ("File", "truncate", 2) => {
                let len = match args[1] {
                    Value::Int(n) if n >= 0 => n as usize,
                    _ => return Err(self.syntax("truncate requires a nonnegative integer".into())),
                };
                self.runtime.truncate_file(&strings()[0], len)?;
                Ok(Value::Null)
            }
            ("File", "delete", 1) => {
                self.runtime.delete_file(&strings()[0])?;
                Ok(Value::Null)
            }
            ("File", "copy", 2) => {
                let a = strings();
                self.runtime.copy_file(&a[0], &a[1])?;
                Ok(Value::Null)
            }
            ("File", "move", 2) => {
                let a = strings();
                self.runtime.move_file(&a[0], &a[1])?;
                Ok(Value::Null)
            }
            ("Directory", "create", 1) => {
                self.runtime.create_directory(&strings()[0])?;
                Ok(Value::Null)
            }
            ("Directory", "delete", 1) => {
                self.runtime.delete_directory(&strings()[0])?;
                Ok(Value::Null)
            }
            ("Directory", "move", 2) => {
                let a = strings();
                self.runtime.move_directory(&a[0], &a[1])?;
                Ok(Value::Null)
            }
            _ => Err(self.syntax(format!(
                "unsupported call: {receiver}.{method}/{}",
                args.len()
            ))),
        }
    }
}

fn binary(operator: char, lhs: Value, rhs: Value) -> Result<Value> {
    match (operator, lhs, rhs) {
        ('+', Value::Text(a), b) => Ok(Value::Text(a + &b.to_string())),
        ('+', a, Value::Text(b)) => Ok(Value::Text(a.to_string() + &b)),
        ('+', Value::Int(a), Value::Int(b)) => a
            .checked_add(b)
            .map(Value::Int)
            .ok_or_else(|| Error::InvalidOperation("integer overflow".into())),
        ('-', Value::Int(a), Value::Int(b)) => a
            .checked_sub(b)
            .map(Value::Int)
            .ok_or_else(|| Error::InvalidOperation("integer overflow".into())),
        ('*', Value::Int(a), Value::Int(b)) => a
            .checked_mul(b)
            .map(Value::Int)
            .ok_or_else(|| Error::InvalidOperation("integer overflow".into())),
        _ => Err(Error::InvalidOperation(format!(
            "invalid operands for '{operator}'"
        ))),
    }
}

fn main() {
    if let Err(error) = run_cli() {
        eprintln!("rewind: {error}");
        std::process::exit(1);
    }
}

fn run_cli() -> Result<()> {
    let mut args = env::args().skip(1);
    let script = args
        .next()
        .ok_or_else(|| Error::InvalidOperation("usage: rewind <script.rw> [--root DIR]".into()))?;
    if script == "fmt" {
        let file = args
            .next()
            .ok_or_else(|| Error::InvalidOperation("usage: rewind fmt FILE [--check]".into()))?;
        let check = match args.next().as_deref() {
            None => false,
            Some("--check") => true,
            Some(other) => return Err(Error::InvalidOperation(format!("unknown option: {other}"))),
        };
        if args.next().is_some() {
            return Err(Error::InvalidOperation("too many arguments".into()));
        }
        let source = fs::read_to_string(&file)?;
        let formatted = v2::format_source(&source)?;
        if check {
            if source != formatted {
                return Err(Error::InvalidOperation(format!("format differs: {file}")));
            }
        } else if source != formatted {
            fs::write(file, formatted)?;
        }
        return Ok(());
    }
    if script == "doc" {
        let file = args.next().ok_or_else(|| {
            Error::InvalidOperation("usage: rewind doc FILE [--root DIR] [--output FILE]".into())
        })?;
        let mut root = env::current_dir()?;
        let mut output: Option<String> = None;
        while let Some(option) = args.next() {
            match option.as_str() {
                "--root" => {
                    root = args
                        .next()
                        .ok_or_else(|| Error::InvalidOperation("missing root directory".into()))?
                        .into()
                }
                "--output" => {
                    output = Some(
                        args.next()
                            .ok_or_else(|| Error::InvalidOperation("missing output path".into()))?,
                    )
                }
                _ => return Err(Error::InvalidOperation(format!("unknown option: {option}"))),
            }
        }
        let document = v2::documentation(&file, &root)?;
        if let Some(path) = output {
            fs::write(path, document)?;
        } else {
            print!("{document}");
        }
        return Ok(());
    }
    if script == "lock" {
        let mut root = env::current_dir()?;
        if let Some(option) = args.next() {
            if option != "--root" {
                return Err(Error::InvalidOperation(format!("unknown option: {option}")));
            }
            root = args
                .next()
                .ok_or_else(|| Error::InvalidOperation("missing root directory".into()))?
                .into();
        }
        if args.next().is_some() {
            return Err(Error::InvalidOperation("too many arguments".into()));
        }
        return v2::lock_project(&root);
    }
    if matches!(script.as_str(), "check" | "run" | "test" | "trace") {
        let remaining = args.collect::<Vec<_>>();
        let mut index = 0;
        let default_file = remaining.first().is_none_or(|s| s.starts_with("--"));
        let file = if default_file {
            "main.rw".to_string()
        } else {
            let file = remaining.first().ok_or_else(|| {
                Error::InvalidOperation(format!("usage: rewind {script} <script.rw> [--root DIR]"))
            })?;
            index += 1;
            file.clone()
        };
        let mut root = env::current_dir()?;
        let mut trace = script == "trace";
        let mut options = v2::RunOptions::default();
        let mut files = vec![if default_file {
            String::new()
        } else {
            file.clone()
        }];
        while index < remaining.len() {
            match remaining[index].as_str() {
                "--" => {
                    options
                        .arguments
                        .extend(remaining[index + 1..].iter().cloned());
                    break;
                }
                "--root" => {
                    index += 1;
                    root = remaining
                        .get(index)
                        .ok_or_else(|| Error::InvalidOperation("missing root directory".into()))?
                        .into();
                }
                "--trace" => trace = true,
                "--trace-json" => {
                    trace = true;
                    options.trace_json = true;
                }
                "--allow-env" | "--secret-env" => {
                    let secret = remaining[index] == "--secret-env";
                    index += 1;
                    let name = remaining
                        .get(index)
                        .ok_or_else(|| Error::InvalidOperation("missing environment name".into()))?
                        .clone();
                    options.allowed_env.insert(name.clone());
                    if secret {
                        options.secret_env.insert(name);
                    }
                }
                "--locale" => {
                    index += 1;
                    options.locale = Some(
                        remaining
                            .get(index)
                            .ok_or_else(|| Error::InvalidOperation("missing locale".into()))?
                            .clone(),
                    );
                }
                "--filter" => {
                    index += 1;
                    options.test_filter = Some(
                        remaining
                            .get(index)
                            .ok_or_else(|| Error::InvalidOperation("missing test filter".into()))?
                            .clone(),
                    );
                }
                value if script == "check" && !value.starts_with("--") => files.push(value.into()),
                flag => return Err(Error::InvalidOperation(format!("unknown option: {flag}"))),
            }
            index += 1;
        }
        let file = if default_file { String::new() } else { file };
        if script == "check" {
            let mut errors = Vec::new();
            for path in files {
                if let Err(error) = v2::cli("check", &path, &root, false, options.clone()) {
                    errors.push(format!("{path}: {error}"));
                }
            }
            if !errors.is_empty() {
                return Err(Error::InvalidOperation(errors.join("\n")));
            }
            return Ok(());
        }
        return v2::cli(&script, &file, &root, trace, options);
    }
    let mut root = env::current_dir()?;
    if let Some(flag) = args.next() {
        if flag != "--root" {
            return Err(Error::InvalidOperation("expected --root".into()));
        }
        root = args
            .next()
            .ok_or_else(|| Error::InvalidOperation("missing root directory".into()))?
            .into();
    }
    if args.next().is_some() {
        return Err(Error::InvalidOperation("too many arguments".into()));
    }
    let source = fs::read_to_string(script)?;
    let tokens = lex(&source).map_err(Error::InvalidOperation)?;
    Runner {
        tokens,
        pos: 0,
        runtime: Runtime::new(root)?,
        input: io::stdin().lock(),
    }
    .run()
}
