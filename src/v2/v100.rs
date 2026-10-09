//! Stable-language elaboration. Expected factory types become explicit arguments
//! before ownership checking and artifact generation, so execution needs no inference.
use super::*;

pub(super) fn prepare(program: &mut Program) -> Result<()> {
    if !matches!(
        program.language.as_str(),
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
            | "1.9.52"
            | "1.9.53"
            | "1.9.54"
            | "1.9.55"
            | "1.9.56"
            | "1.9.57"
            | "1.9.58"
            | "1.9.59"
            | "2.0.0"
    ) {
        return Ok(());
    }
    fn expected(p: &Program, expr: &mut Expr, ty: &str) -> Result<()> {
        let ExprKind::Call(target, _) = &mut expr.kind else {
            return Ok(());
        };
        let name = match &target.kind {
            ExprKind::Name(n) => n.clone(),
            ExprKind::Member(base, field) => match &base.kind {
                ExprKind::Name(n) => format!("{n}.{field}"),
                _ => return Ok(()),
            },
            _ => return Ok(()),
        };
        if name.contains('<') {
            return Ok(());
        }
        let symbol = resolve_alias(p, &name);
        let Some(f) = p.functions.get(&symbol) else {
            return Ok(());
        };
        if f.type_params.is_empty() {
            return Ok(());
        }
        let params = f
            .type_params
            .iter()
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        let mut sub = BTreeMap::new();
        let ty = rename_type(ty, &p.import_aliases);
        let ret = if f.asynchronous {
            format!("Task<{}>", f.ret)
        } else {
            f.ret.clone()
        };
        if unify_type(&ret, &ty, &params, &mut sub)
            && params
                .iter()
                .all(|n| sub.get(n).is_some_and(|t| t != "Unknown"))
        {
            target.kind = ExprKind::Name(format!(
                "{name}<{}>",
                params
                    .iter()
                    .map(|n| sub[n].as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        Ok(())
    }
    fn body(p: &Program, stmts: &mut [Stmt], ret: Option<&str>) -> Result<()> {
        for s in stmts {
            match &mut s.kind {
                StmtKind::Let(_, _, Some(ty), expr) => expected(p, expr, ty)?,
                StmtKind::Return(Some(expr)) => {
                    if let Some(ty) = ret {
                        expected(p, expr, ty)?;
                    }
                }
                StmtKind::External(_, b)
                | StmtKind::Block(b)
                | StmtKind::Branch(_, b)
                | StmtKind::While(_, b)
                | StmtKind::For(_, _, _, b) => body(p, b, ret)?,
                StmtKind::If(_, a, b) => {
                    body(p, a, ret)?;
                    body(p, b, ret)?;
                }
                StmtKind::Match(_, arms) => {
                    for (_, _, s) in arms {
                        body(p, std::slice::from_mut(s), ret)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    let p = program.clone();
    body(&p, &mut program.stmts, None)?;
    for f in program.functions.values_mut() {
        body(&p, &mut f.body, Some(&f.ret))?;
    }
    Ok(())
}

pub(super) fn argument_work(value: &Value, runtime: &Runtime) -> usize {
    let value = match value {
        Value::HeapRef(id) | Value::CellRef(id) => runtime.heap_get(*id).unwrap_or(value),
        _ => value,
    };
    match value {
        Value::Regex(p) => p.source().len().saturating_add(128),
        Value::Text(s) => s.len().saturating_add(1),
        Value::Bytes(s) => s.len().saturating_add(1),
        Value::List(v) => v.len().saturating_add(Runtime::value_bytes(value)),
        Value::TypedList(_, v) => v.len().saturating_add(v.logical_bytes()),
        Value::Map(v) | Value::TypedMap(_, _, v) => v.len().saturating_add(v.logical_bytes()),
        _ => Runtime::value_bytes(value).saturating_add(1),
    }
}
pub(super) fn call_work(name: &str, args: &[Value], runtime: &Runtime, language: &str) -> usize {
    // Older artifacts retain their published fixed identity fee. New source
    // accounts for cold hashing without depending on mutable cache warmth.
    if name == "stdNumericTensorKey" && !language_at_least(language, "1.9.39") {
        return 1024;
    }
    v092::work(name, args, runtime).unwrap_or_else(|| {
        args.iter()
            .fold(1usize, |n, v| n.saturating_add(argument_work(v, runtime)))
    })
}
pub(super) fn method_work(
    target: &Value,
    method: &str,
    args: &[Value],
    runtime: &Runtime,
) -> usize {
    let value = match target {
        Value::HeapRef(id) | Value::CellRef(id) => runtime.heap_get(*id).unwrap_or(target),
        _ => target,
    };
    let base = args
        .iter()
        .fold(1usize, |n, v| n.saturating_add(argument_work(v, runtime)));
    match value {
        Value::Text(s) if method == "byteLen" => base.saturating_add(s.clone_work()),
        Value::Text(s) => base.saturating_add(s.len()),
        Value::Bytes(_) => base,
        Value::TypedList(_, v) => {
            let payload = if matches!(method, "get" | "set" | "pop") {
                let index = match args.first() {
                    Some(Value::Int(n)) if *n >= 0 => Some(*n as usize),
                    _ if method == "pop" => v.len().checked_sub(1),
                    _ => None,
                };
                index.and_then(|i| v.get(i)).map_or(0, Runtime::value_bytes)
            } else {
                0
            };
            base.saturating_add(payload)
                .saturating_add((usize::BITS - v.len().max(1).leading_zeros()) as usize)
        }
        Value::Map(v) | Value::TypedMap(_, _, v) => {
            if method == "keys" {
                base.saturating_add(v.logical_bytes())
                    .saturating_add(v.len())
            } else {
                base.saturating_add((usize::BITS - v.len().max(1).leading_zeros()) as usize)
            }
        }
        Value::OrderedMap(_, _, v) => base.saturating_add(v.len()),
        _ => base.saturating_add(argument_work(target, runtime)),
    }
}

pub(super) fn syntax_budget(program: &Program) -> Result<()> {
    enum Item<'a> {
        Stmt(&'a Stmt),
        Expr(&'a Expr),
        Pattern(&'a Pattern),
    }
    let mut pending = program
        .stmts
        .iter()
        .map(|s| (Item::Stmt(s), 0))
        .collect::<Vec<_>>();
    pending.extend(
        program
            .functions
            .values()
            .flat_map(|f| f.body.iter())
            .map(|s| (Item::Stmt(s), 0)),
    );
    while let Some((item, depth)) = pending.pop() {
        let at = match &item {
            Item::Stmt(s) => Some(&s.at),
            Item::Expr(e) => Some(&e.at),
            _ => None,
        };
        if depth > 128 {
            return Err(at.map_or_else(
                || Error::InvalidOperation("CompilerBudgetExceeded: AST nesting (128)".into()),
                |at| diagnostic(at, "CompilerBudgetExceeded: AST nesting (128)"),
            ));
        }
        let mut add = |i| pending.push((i, depth + 1));
        match item {
            Item::Stmt(s) => match &s.kind {
                StmtKind::Let(_, _, _, e)
                | StmtKind::Using(_, e)
                | StmtKind::Expr(e)
                | StmtKind::Defer(e) => add(Item::Expr(e)),
                StmtKind::Assign(a, _, b) => {
                    add(Item::Expr(a));
                    add(Item::Expr(b));
                }
                StmtKind::External(_, b) | StmtKind::Block(b) | StmtKind::Branch(_, b) => {
                    for s in b {
                        add(Item::Stmt(s));
                    }
                }
                StmtKind::While(e, b) => {
                    add(Item::Expr(e));
                    for s in b {
                        add(Item::Stmt(s));
                    }
                }
                StmtKind::If(e, a, b) => {
                    add(Item::Expr(e));
                    for s in a.iter().chain(b) {
                        add(Item::Stmt(s));
                    }
                }
                StmtKind::For(_, a, b, body) => {
                    add(Item::Expr(a));
                    add(Item::Expr(b));
                    for s in body {
                        add(Item::Stmt(s));
                    }
                }
                StmtKind::Return(Some(e)) => add(Item::Expr(e)),
                StmtKind::Match(e, arms) => {
                    add(Item::Expr(e));
                    for (p, g, s) in arms {
                        add(Item::Pattern(p));
                        if let Some(e) = g {
                            add(Item::Expr(e));
                        }
                        add(Item::Stmt(s));
                    }
                }
                _ => {}
            },
            Item::Expr(e) => match &e.kind {
                ExprKind::Unary(_, e) | ExprKind::Try(e) | ExprKind::Member(e, _) => {
                    add(Item::Expr(e))
                }
                ExprKind::Binary(_, a, b) => {
                    add(Item::Expr(a));
                    add(Item::Expr(b));
                }
                ExprKind::Call(e, args) => {
                    add(Item::Expr(e));
                    for a in args {
                        add(Item::Expr(a));
                    }
                }
                ExprKind::Match(e, arms) => {
                    add(Item::Expr(e));
                    for (p, g, e) in arms {
                        add(Item::Pattern(p));
                        if let Some(e) = g {
                            add(Item::Expr(e));
                        }
                        add(Item::Expr(e));
                    }
                }
                ExprKind::Closure(_, _, body) => {
                    for s in body {
                        add(Item::Stmt(s));
                    }
                }
                ExprKind::NamedConstructor(_, fields) => {
                    for (_, e) in fields {
                        add(Item::Expr(e));
                    }
                }
                _ => {}
            },
            Item::Pattern(p) => match p {
                Pattern::Variant(_, ps) | Pattern::List(ps) => {
                    for p in ps {
                        add(Item::Pattern(p));
                    }
                }
                Pattern::Struct(_, fields) => {
                    for (_, p) in fields {
                        add(Item::Pattern(p));
                    }
                }
                _ => {}
            },
        }
    }
    Ok(())
}
