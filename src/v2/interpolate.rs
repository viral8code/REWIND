//! Language 3.0.0 string interpolation: `"total: ${count}"`.
//!
//! The literal is split at `${ ... }`, each embedded expression is parsed and
//! converted with `.format()`, and the pieces are joined with `+`. `$${` writes
//! a literal `${`.
use super::*;

pub(super) fn expand(program: &mut Program) -> Result<()> {
    for function in program.functions.values_mut() {
        stmts(&mut function.body)?;
    }
    Ok(())
}

fn stmts(body: &mut [Stmt]) -> Result<()> {
    for stmt in body {
        stmt_kind(&mut stmt.kind)?;
    }
    Ok(())
}

fn stmt_kind(kind: &mut StmtKind) -> Result<()> {
    match kind {
        StmtKind::Let(_, _, _, e)
        | StmtKind::Using(_, e)
        | StmtKind::Expr(e)
        | StmtKind::Defer(e)
        | StmtKind::Return(Some(e)) => expr(e)?,
        StmtKind::Assign(a, _, b) => {
            expr(a)?;
            expr(b)?;
        }
        StmtKind::Block(b) | StmtKind::External(_, b) | StmtKind::Branch(_, b) => stmts(b)?,
        StmtKind::If(e, a, b) => {
            expr(e)?;
            stmts(a)?;
            stmts(b)?;
        }
        StmtKind::While(e, b) => {
            expr(e)?;
            stmts(b)?;
        }
        StmtKind::For(_, a, b, body) => {
            expr(a)?;
            expr(b)?;
            stmts(body)?;
        }
        StmtKind::Match(e, arms) => {
            expr(e)?;
            for (_, guard, arm) in arms {
                if let Some(g) = guard {
                    expr(g)?;
                }
                stmt_kind(&mut arm.kind)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn expr(e: &mut Expr) -> Result<()> {
    match &mut e.kind {
        ExprKind::Value(Value::Text(text)) if text.contains("${") || text.contains("$${") => {
            let at = e.at.clone();
            let replacement = interpolate(text, &at)?;
            *e = replacement;
        }
        ExprKind::Unary(_, v) | ExprKind::Try(v) | ExprKind::Member(v, _) => expr(v)?,
        ExprKind::Binary(_, a, b) => {
            expr(a)?;
            expr(b)?;
        }
        ExprKind::Call(f, args) => {
            expr(f)?;
            for a in args {
                expr(a)?;
            }
        }
        ExprKind::Match(v, arms) => {
            expr(v)?;
            for (_, guard, arm) in arms {
                if let Some(g) = guard {
                    expr(g)?;
                }
                expr(arm)?;
            }
        }
        ExprKind::Block(body, tail) => {
            stmts(body)?;
            if let Some(tail) = tail {
                expr(tail)?;
            }
        }
        ExprKind::Closure(_, _, body) => stmts(body)?,
        ExprKind::NamedConstructor(_, fields) => {
            for (_, v) in fields {
                expr(v)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn literal(text: String, at: &Tok) -> Expr {
    Expr {
        kind: ExprKind::Value(Value::Text(text.into())),
        at: at.clone(),
    }
}

fn interpolate(text: &str, at: &Tok) -> Result<Expr> {
    let mut parts: Vec<Expr> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && chars.get(i + 1) == Some(&'$') && chars.get(i + 2) == Some(&'{') {
            current.push_str("${");
            i += 3;
        } else if chars[i] == '$' && chars.get(i + 1) == Some(&'{') {
            let start = i + 2;
            let mut depth = 1;
            let mut j = start;
            let mut quoted = false;
            while j < chars.len() {
                match chars[j] {
                    '\\' if quoted => j += 1,
                    '"' => quoted = !quoted,
                    '{' if !quoted => depth += 1,
                    '}' if !quoted => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            if j >= chars.len() {
                return Err(diagnostic(at, "unclosed ${ in string; add '}'"));
            }
            let source: String = chars[start..j].iter().collect();
            if source.trim().is_empty() {
                return Err(diagnostic(at, "empty ${} in string"));
            }
            if !current.is_empty() {
                parts.push(literal(std::mem::take(&mut current), at));
            }
            let mut tokens = lex(&source)?;
            for token in &mut tokens {
                token.source = at.source.clone();
                token.line = at.line;
                token.col = at.col;
            }
            let mut parser = Parser {
                toks: tokens,
                pos: 0,
                type_depth: 0,
            };
            let inner = parser.expr(0)?;
            if !parser.is("<eof>") {
                return Err(diagnostic(
                    at,
                    "unexpected text after the expression in ${...}",
                ));
            }
            parts.push(Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Member(Box::new(inner), "format".into()),
                        at: at.clone(),
                    }),
                    Vec::new(),
                ),
                at: at.clone(),
            });
            i = j + 1;
        } else {
            current.push(chars[i]);
            i += 1;
        }
    }
    if !current.is_empty() {
        parts.push(literal(current, at));
    }
    // Start from a String so every `+` is a text concatenation.
    let mut result = if matches!(
        parts.first().map(|p| &p.kind),
        Some(ExprKind::Value(Value::Text(_)))
    ) {
        parts.remove(0)
    } else {
        literal(String::new(), at)
    };
    for part in parts {
        result = Expr {
            kind: ExprKind::Binary("+".into(), Box::new(result), Box::new(part)),
            at: at.clone(),
        };
    }
    Ok(result)
}
