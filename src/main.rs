use rewind::{Error, ResourceBudget, Result, Runtime, Value};
use std::env;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
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

fn exit_status(error: &Error) -> i32 {
    if let Error::Diagnostic(d) = error {
        fn partially_applied(d: &rewind::DiagnosticRecord) -> bool {
            d.code == "PublishPartiallyApplied" || d.causes.iter().any(partially_applied)
        }
        if partially_applied(d) {
            return 73;
        }
        if !d.causes.is_empty() {
            return 70;
        }
        if let Some(n) = d
            .code
            .strip_prefix("ApplicationExit")
            .and_then(|s| s.parse::<i32>().ok())
            .filter(|n| (1..=63).contains(n))
        {
            return n;
        }
        if d.code.contains("Budget")
            || d.code.starts_with("History")
            || matches!(
                d.code.as_str(),
                "TaskSteps"
                    | "SchedulerStorage"
                    | "SchedulerObjects"
                    | "TransferDepth"
                    | "IteratorItems"
                    | "EffectInference"
                    | "DependencyResolution"
            )
        {
            return 71;
        }
        if d.code == "ExternalStateConflict" {
            return 72;
        }
        if d.code == "PublishPartiallyApplied" {
            return 73;
        }
        if d.code == "TaskCancelled" {
            return 74;
        }
        if d.message.contains("panic:") {
            return 70;
        }
        return 65;
    }
    match error {
        Error::ExternalStateConflict(_) => 72,
        Error::PublishPartiallyApplied(_) => 73,
        Error::HistoryBudgetExceeded => 71,
        _ => 64,
    }
}
/// Address-space admission applies to compiler and runtime, including native allocations.
fn process_memory_limit(mib: usize) -> Result<()> {
    if mib < 64 || mib > 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "ProcessMemoryBudget: --memory-mib must be 64..1048576".into(),
        ));
    }
    #[cfg(target_os = "linux")]
    {
        let bytes = mib
            .checked_mul(1024 * 1024)
            .ok_or_else(|| Error::InvalidOperation("ProcessMemoryBudget: overflow".into()))?;
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if unsafe { libc::getrlimit(libc::RLIMIT_AS, &mut limit) } != 0 {
            return Err(io::Error::last_os_error().into());
        }
        limit.rlim_cur = (bytes as libc::rlim_t)
            .min(limit.rlim_max)
            .min(limit.rlim_cur);
        if unsafe { libc::setrlimit(libc::RLIMIT_AS, &limit) } != 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(Error::InvalidOperation(
            "ProcessMemoryBudget: --memory-mib requires Linux".into(),
        ))
    }
}

fn main() {
    let mut arguments = Vec::new();
    let mut format = "text".to_string();
    #[cfg(target_os = "linux")]
    let mut memory_limit = Ok(Some(2048));
    #[cfg(not(target_os = "linux"))]
    let mut memory_limit = Ok(None);
    let mut raw = env::args().skip(1);
    let mut application = false;
    while let Some(arg) = raw.next() {
        if arg == "--" {
            application = true;
        }
        if !application && arg == "--diagnostic-format" {
            format = raw.next().unwrap_or_default();
        } else if !application && arg == "--memory-mib" {
            memory_limit = raw
                .next()
                .and_then(|s| s.parse::<usize>().ok())
                .map(Some)
                .ok_or_else(|| {
                    Error::InvalidOperation(
                        "ProcessMemoryBudget: --memory-mib requires an integer".into(),
                    )
                });
        } else {
            arguments.push(arg);
        }
    }
    let compiler_entry = env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .is_some_and(|s| s == "rewindc");
    if arguments
        .first()
        .is_some_and(|a| a == "--help" || a == "-h")
    {
        println!("REWIND {}\nrewind run FILE.rw [--allow-effects EFFECTS]\nrewind compile FILE.rw [--output FILE.rwc]\nrewind run FILE.rwc [--allow-effects EFFECTS]\nrewind FILE.rwc\nrewindc FILE.rw\nProject: rewind check|test|run|build --root DIR\nTools: update, doc, fmt, replay, sdk-build, sdk-install, sdk-verify\nBudgets: --steps N, --native-work N, --task-steps N, --memory-mib N (Linux).\nStandalone defaults: input, output, args, locale, random, tasks.\nFile, environment and clock access require explicit permission.", env!("CARGO_PKG_VERSION"));
        return;
    }
    if arguments.first().is_some_and(|a| a == "--version") {
        println!("rewind {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if compiler_entry && arguments.first().is_none_or(|a| a != "compile") {
        arguments.insert(0, "compile".into());
    }
    if !compiler_entry && arguments.first().is_some_and(|a| a.ends_with(".rwc")) {
        arguments.insert(0, "run".into());
    }
    let result = if matches!(format.as_str(), "text" | "json") {
        memory_limit.and_then(|limit| {
            if let Some(mib) = limit {
                process_memory_limit(mib)?;
            }
            run_cli(arguments)
        })
    } else {
        Err(Error::InvalidOperation(
            "diagnostic format must be text or json".into(),
        ))
    };
    if let Err(error) = result {
        let status = exit_status(&error);
        if format == "json" {
            let diagnostic = if let Error::Diagnostic(d) = &error {
                serde_json::to_value(d).unwrap_or_default()
            } else {
                serde_json::Value::Null
            };
            eprintln!(
                "{}",
                serde_json::json!({"exit_status":status,"message":error.to_string(),"diagnostic":diagnostic,"publish_failure":error.publish_report(),"retryable":false,"retry_hint":if status==72 {"reload_and_decide"}else{"none"}})
            );
        } else {
            eprintln!("rewind: {error}");
        }
        std::process::exit(status);
    }
}

fn run_cli(arguments: Vec<String>) -> Result<()> {
    let mut args = arguments.into_iter();
    let script = args
        .next()
        .ok_or_else(|| Error::InvalidOperation("usage: rewind <script.rw> [--root DIR]".into()))?;
    if matches!(script.as_str(), "sdk-build" | "sdk-install" | "sdk-verify") {
        let mut root = env::current_dir()?;
        let mut sdk = None;
        let mut output = None;
        let mut key = None;
        let mut public = None;
        while let Some(option) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| Error::InvalidOperation("missing SDK option value".into()))?;
            match option.as_str() {
                "--root" => root = PathBuf::from(value),
                "--sdk" => sdk = Some(PathBuf::from(value)),
                "--output" => output = Some(PathBuf::from(value)),
                "--key" => key = Some(PathBuf::from(value)),
                "--public-key" => public = Some(value),
                _ => return Err(Error::InvalidOperation("unknown SDK option".into())),
            }
        }
        return match script.as_str() {
            "sdk-build" => v2::sdk_build(
                &output.ok_or_else(|| Error::InvalidOperation("--output required".into()))?,
                &key.ok_or_else(|| Error::InvalidOperation("--key required".into()))?,
            ),
            "sdk-install" => v2::sdk_install(
                &root,
                &sdk.ok_or_else(|| Error::InvalidOperation("--sdk required".into()))?,
                &public.ok_or_else(|| Error::InvalidOperation("--public-key required".into()))?,
            ),
            _ => v2::sdk_verify(
                &sdk.ok_or_else(|| Error::InvalidOperation("--sdk required".into()))?,
                &public.ok_or_else(|| Error::InvalidOperation("--public-key required".into()))?,
            ),
        };
    }
    if script == "lsp" {
        let mut root = env::current_dir()?;
        if let Some(option) = args.next() {
            if option != "--root" {
                return Err(Error::InvalidOperation(
                    "usage: rewind lsp [--root DIR]".into(),
                ));
            }
            root = PathBuf::from(
                args.next()
                    .ok_or_else(|| Error::InvalidOperation("missing root".into()))?,
            );
        }
        if args.next().is_some() {
            return Err(Error::InvalidOperation("too many arguments".into()));
        }
        return v2::lsp(&root);
    }
    if script == "debug-session" {
        let path = args.next().ok_or_else(|| {
            Error::InvalidOperation("usage: rewind debug-session TRACE [--root DIR]".into())
        })?;
        let mut root = env::current_dir()?;
        let mut options = v2::RunOptions::default();
        while let Some(option) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| Error::InvalidOperation("missing option value".into()))?;
            match option.as_str() {
                "--root" => root = PathBuf::from(value),
                "--verify-key" => options.verify_key = Some(value),
                "--secret-env" => {
                    options.allowed_env.insert(value.clone());
                    options.secret_env.insert(value);
                }
                "--secret-input" => options.secret_input = Some(PathBuf::from(value)),
                _ => return Err(Error::InvalidOperation(format!("unknown option {option}"))),
            }
        }
        return v2::debug_session(Path::new(&path), &root, options);
    }
    if script == "keygen" {
        let path = args
            .next()
            .ok_or_else(|| Error::InvalidOperation("usage: rewind keygen NEW_SEED_FILE".into()))?;
        if args.next().is_some() {
            return Err(Error::InvalidOperation("too many arguments".into()));
        }
        return v2::keygen(Path::new(&path));
    }
    if matches!(
        script.as_str(),
        "sign-artifact"
            | "sign-trace"
            | "verify-artifact"
            | "verify-trace"
            | "sign-inspection"
            | "verify-inspection"
            | "sign-session"
            | "verify-session"
            | "sign-release"
            | "verify-release"
    ) {
        let path = PathBuf::from(
            args.next()
                .ok_or_else(|| Error::InvalidOperation("missing file".into()))?,
        );
        let kind = if script.ends_with("artifact") {
            "artifact"
        } else if script.ends_with("inspection") {
            "inspection"
        } else if script.ends_with("release") {
            "release"
        } else if script.ends_with("session") {
            "session"
        } else {
            "trace"
        };
        let mut key = None;
        let mut public = None;
        let mut signature = PathBuf::from(format!("{}.signature", path.display()));
        while let Some(option) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| Error::InvalidOperation("missing option value".into()))?;
            match option.as_str() {
                "--key" => key = Some(PathBuf::from(value)),
                "--public-key" => public = Some(value),
                "--signature" | "--output" => signature = PathBuf::from(value),
                _ => return Err(Error::InvalidOperation(format!("unknown option {option}"))),
            }
        }
        return if script.starts_with("verify") {
            v2::verify_file(
                &path,
                &signature,
                &public.ok_or_else(|| Error::InvalidOperation("--public-key required".into()))?,
                kind,
            )?;
            if kind == "release" {
                v2::verify_release(&path)?;
            }
            Ok(())
        } else {
            v2::sign_file(
                &path,
                &key.ok_or_else(|| Error::InvalidOperation("--key required".into()))?,
                &signature,
                kind,
            )
        };
    }
    if script == "sign" {
        let package = PathBuf::from(args.next().ok_or_else(|| {
            Error::InvalidOperation(
                "usage: rewind sign PACKAGE --key SEED [--output SIGNATURE]".into(),
            )
        })?);
        let mut key = None;
        let mut output = package.join("rewind.signature");
        while let Some(option) = args.next() {
            match option.as_str() {
                "--key" => {
                    key = Some(PathBuf::from(args.next().ok_or_else(|| {
                        Error::InvalidOperation("missing signing seed".into())
                    })?))
                }
                "--output" => {
                    output = args
                        .next()
                        .ok_or_else(|| Error::InvalidOperation("missing signature path".into()))?
                        .into()
                }
                _ => return Err(Error::InvalidOperation(format!("unknown option {option}"))),
            }
        }
        return v2::sign_package(
            &package,
            &key.ok_or_else(|| Error::InvalidOperation("--key is required".into()))?,
            &output,
        );
    }
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
    if script == "migrate" {
        let mut root = env::current_dir()?;
        let mut write = false;
        while let Some(option) = args.next() {
            match option.as_str() {
                "--root" => {
                    root = args
                        .next()
                        .ok_or_else(|| Error::InvalidOperation("missing root".into()))?
                        .into()
                }
                "--write" => write = true,
                _ => return Err(Error::InvalidOperation(format!("unknown option {option}"))),
            }
        }
        return v2::migrate_project(&root, write);
    }
    if script == "repl" || script == "session-replay" || script == "install" {
        let path = if script == "session-replay" {
            Some(
                args.next()
                    .ok_or_else(|| Error::InvalidOperation("missing session transcript".into()))?,
            )
        } else {
            None
        };
        let mut root = env::current_dir()?;
        let mut output = None;
        let mut production = false;
        while let Some(option) = args.next() {
            if option == "--production" {
                production = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| Error::InvalidOperation("missing option value".into()))?;
            match option.as_str() {
                "--root" => root = PathBuf::from(value),
                "--record" | "--output" => output = Some(PathBuf::from(value)),
                _ => {
                    return Err(Error::InvalidOperation(
                        "unknown session/install option".into(),
                    ))
                }
            }
        }
        return match script.as_str() {
            "repl" => v2::repl(&root, output.as_deref()),
            "session-replay" => v2::session_replay(&root, Path::new(&path.unwrap())),
            _ if production => v2::install_production(
                &root,
                &output.ok_or_else(|| {
                    Error::InvalidOperation("install --production requires --output".into())
                })?,
            ),
            _ => Err(Error::InvalidOperation(
                "install requires --production".into(),
            )),
        };
    }
    if script == "trace-export" {
        let file = args.next().ok_or_else(|| {
            Error::InvalidOperation("trace-export TRACE --output FILE [--public-key KEY]".into())
        })?;
        let mut output = None;
        let mut key = None;
        while let Some(option) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| Error::InvalidOperation("missing export option value".into()))?;
            match option.as_str() {
                "--output" => output = Some(PathBuf::from(value)),
                "--public-key" => key = Some(value),
                _ => return Err(Error::InvalidOperation("unknown export option".into())),
            }
        }
        return v2::trace_export(
            Path::new(&file),
            &output.ok_or_else(|| Error::InvalidOperation("missing --output".into()))?,
            key.as_deref(),
        );
    }
    if script == "api-snapshot" {
        let mut root = env::current_dir()?;
        let mut output = None;
        while let Some(option) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| Error::InvalidOperation("missing API option value".into()))?;
            match option.as_str() {
                "--root" => root = PathBuf::from(value),
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(Error::InvalidOperation("unknown API option".into())),
            }
        }
        return v2::api_snapshot(&root, output.as_deref());
    }
    if script == "api-convert" {
        let input = args.next().ok_or_else(|| {
            Error::InvalidOperation("api-convert OLD --root DIR --output FILE".into())
        })?;
        let mut root = env::current_dir()?;
        let mut output = None;
        while let Some(option) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| Error::InvalidOperation("missing conversion option".into()))?;
            match option.as_str() {
                "--root" => root = PathBuf::from(value),
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(Error::InvalidOperation("unknown conversion option".into())),
            }
        }
        return v2::api_convert(
            &root,
            Path::new(&input),
            &output.ok_or_else(|| Error::InvalidOperation("missing --output".into()))?,
        );
    }
    if script == "api-diff" {
        let before = args.next().ok_or_else(|| {
            Error::InvalidOperation("api-diff BEFORE AFTER [--deny-breaking]".into())
        })?;
        let after = args.next().ok_or_else(|| {
            Error::InvalidOperation("api-diff BEFORE AFTER [--deny-breaking]".into())
        })?;
        let deny = match args.next().as_deref() {
            None => false,
            Some("--deny-breaking") => true,
            _ => return Err(Error::InvalidOperation("unknown API diff option".into())),
        };
        if args.next().is_some() {
            return Err(Error::InvalidOperation(
                "too many API diff arguments".into(),
            ));
        }
        return v2::api_diff(Path::new(&before), Path::new(&after), deny);
    }
    if script == "doctest" {
        let file = args
            .next()
            .ok_or_else(|| Error::InvalidOperation("doctest FILE.md [--root DIR]".into()))?;
        let mut root = env::current_dir()?;
        if let Some(flag) = args.next() {
            if flag != "--root" {
                return Err(Error::InvalidOperation("unknown doctest option".into()));
            }
            root = PathBuf::from(
                args.next()
                    .ok_or_else(|| Error::InvalidOperation("missing root".into()))?,
            );
        }
        if args.next().is_some() {
            return Err(Error::InvalidOperation("too many doctest arguments".into()));
        }
        return v2::doctest(&root, Path::new(&file));
    }
    if script == "timeline" {
        let file = args.next().ok_or_else(|| {
            Error::InvalidOperation("timeline TRACE [--from N] [--count N] [--task ID]".into())
        })?;
        let mut start = 0;
        let mut count = 100;
        let mut task = None;
        let mut key = None;
        while let Some(option) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| Error::InvalidOperation("missing timeline option value".into()))?;
            match option.as_str() {
                "--public-key" => key = Some(value),
                "--from" => {
                    start = value
                        .parse()
                        .map_err(|_| Error::InvalidOperation("invalid event".into()))?
                }
                "--count" => {
                    count = value
                        .parse()
                        .map_err(|_| Error::InvalidOperation("invalid count".into()))?
                }
                "--task" => {
                    task = Some(
                        value
                            .parse()
                            .map_err(|_| Error::InvalidOperation("invalid task".into()))?,
                    )
                }
                _ => return Err(Error::InvalidOperation("unknown timeline option".into())),
            }
        }
        return v2::timeline(Path::new(&file), start, count, task, key.as_deref());
    }
    if script == "compatibility" {
        let path = args
            .next()
            .ok_or_else(|| Error::InvalidOperation("usage: rewind compatibility FILE".into()))?;
        if args.next().is_some() {
            return Err(Error::InvalidOperation("too many arguments".into()));
        }
        return v2::compatibility(Path::new(&path));
    }
    if script == "lock" || script == "update" {
        let mut root = env::current_dir()?;
        let mut preview = false;
        let mut apply = None;
        let mut output = None;
        while let Some(option) = args.next() {
            match option.as_str() {
                "--root" => {
                    root =
                        PathBuf::from(args.next().ok_or_else(|| {
                            Error::InvalidOperation("missing root directory".into())
                        })?)
                }
                "--preview" if script == "update" => preview = true,
                "--apply" if script == "update" => {
                    apply = Some(PathBuf::from(args.next().ok_or_else(|| {
                        Error::InvalidOperation("missing proposal file".into())
                    })?))
                }
                "--output" if script == "update" => {
                    output = Some(PathBuf::from(args.next().ok_or_else(|| {
                        Error::InvalidOperation("missing output file".into())
                    })?))
                }
                _ => return Err(Error::InvalidOperation(format!("unknown option: {option}"))),
            }
        }
        if preview && apply.is_some() || output.is_some() && !preview {
            return Err(Error::InvalidOperation(
                "use --preview [--output FILE] or --apply FILE".into(),
            ));
        }
        return if preview {
            v2::update_preview(&root, output.as_deref())
        } else if let Some(path) = apply {
            v2::update_apply(&root, &path)
        } else if script == "update" {
            v2::update_project(&root)
        } else {
            v2::lock_project(&root)
        };
    }
    if matches!(
        script.as_str(),
        "check"
            | "run"
            | "test"
            | "trace"
            | "build"
            | "compile"
            | "debug"
            | "profile"
            | "replay"
            | "run-artifact"
    ) {
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
        let mut explicit_root = false;
        let mut trace = script == "trace";
        let mut options = v2::RunOptions {
            inspect: script == "debug",
            profile: script == "profile",
            ..v2::RunOptions::default()
        };
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
                    explicit_root = true;
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
                "--output" | "--record" => {
                    let option = remaining[index].clone();
                    index += 1;
                    let path =
                        PathBuf::from(remaining.get(index).ok_or_else(|| {
                            Error::InvalidOperation("missing output path".into())
                        })?);
                    if option == "--record" {
                        options.record = Some(path);
                    } else {
                        options.output = Some(path);
                    }
                }
                "--allow-effects" => {
                    index += 1;
                    let effects = remaining
                        .get(index)
                        .ok_or_else(|| Error::InvalidOperation("missing allowed effects".into()))?;
                    options.allowed_effects.extend(
                        effects
                            .split(',')
                            .filter(|s| !s.is_empty())
                            .map(str::to_string),
                    );
                }
                "--verify-key" => {
                    index += 1;
                    options.verify_key = Some(
                        remaining
                            .get(index)
                            .ok_or_else(|| Error::InvalidOperation("missing public key".into()))?
                            .clone(),
                    );
                }
                "--explore" => {
                    index += 1;
                    let limit = remaining
                        .get(index)
                        .ok_or_else(|| {
                            Error::InvalidOperation("missing exploration budget".into())
                        })?
                        .parse::<usize>()
                        .map_err(|_| {
                            Error::InvalidOperation("invalid exploration budget".into())
                        })?;
                    if script != "test" || !(1..=10000).contains(&limit) {
                        return Err(Error::InvalidOperation(
                            "--explore requires test and budget 1..10000".into(),
                        ));
                    }
                    options.explore = limit;
                }
                "--native-work" | "--steps" => {
                    let option = remaining[index].clone();
                    index += 1;
                    let limit = remaining
                        .get(index)
                        .ok_or_else(|| Error::InvalidOperation(format!("missing {option} budget")))?
                        .parse::<usize>()
                        .map_err(|_| Error::InvalidOperation(format!("invalid {option} budget")))?;
                    if limit == 0 {
                        return Err(Error::InvalidOperation("budget must be positive".into()));
                    }
                    if option == "--native-work" {
                        options.native_work = Some(limit);
                    } else {
                        options.execution_steps = Some(limit);
                    }
                }
                "--task-steps" => {
                    index += 1;
                    let limit = remaining
                        .get(index)
                        .ok_or_else(|| {
                            Error::InvalidOperation("missing task instruction budget".into())
                        })?
                        .parse::<usize>()
                        .map_err(|_| {
                            Error::InvalidOperation("invalid task instruction budget".into())
                        })?;
                    if limit == 0 {
                        return Err(Error::InvalidOperation(
                            "task instruction budget must be positive".into(),
                        ));
                    }
                    options.task_steps = Some(limit);
                }
                "--secret-input" => {
                    index += 1;
                    options.secret_input =
                        Some(PathBuf::from(remaining.get(index).ok_or_else(|| {
                            Error::InvalidOperation("missing secret input file".into())
                        })?));
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
        if !explicit_root && !file.is_empty() {
            let full = std::fs::canonicalize(&file)?;
            let mut ancestor = full.parent();
            let mut project_root = None;
            while let Some(dir) = ancestor {
                if dir.join("rewind.toml").exists() {
                    project_root = Some(dir.to_path_buf());
                    break;
                }
                ancestor = dir.parent();
            }
            root = project_root.unwrap_or_else(|| full.parent().unwrap().to_path_buf());
            options.standalone = !root.join("rewind.toml").exists();
        }
        if script == "compile" && options.output.is_none() {
            let source = if file.is_empty() {
                root.join("main.rw")
            } else {
                PathBuf::from(&file)
            };
            options.output = Some(source.with_extension("rwc"));
        }
        if script == "run" && file.ends_with(".rwc") {
            return v2::run_compiled(Path::new(&file), &root, options);
        }
        if script == "run-artifact" {
            return v2::run_artifact(Path::new(&file), &root, options);
        }
        if script == "debug" && Path::new(&file).extension().is_some_and(|e| e == "json") {
            return v2::debug_trace(Path::new(&file));
        }
        if script == "replay" {
            return v2::replay_trace(Path::new(&file), &root, options);
        }
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
        return v2::cli(
            if script == "compile" {
                "build"
            } else {
                &script
            },
            &file,
            &root,
            trace,
            options,
        );
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
