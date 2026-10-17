//! Named and default call arguments (LANG-01, language 3.6.0).
//!
//! The parser turns `label: value` inside a call into a marker call to `NAMED`; this pass runs
//! once the module's imports are merged, resolves the callee to a declared function and rewrites
//! the call into plain positional form, so the type checker, ownership checker and VM never see
//! the sugar. Arguments are then evaluated in parameter order, not in the order written.
use super::*;

pub(super) const NAMED: &str = "$named";

struct Signature {
    names: Vec<String>,
    defaults: BTreeMap<String, Expr>,
}

struct Resolver {
    signatures: BTreeMap<String, Signature>,
    aliases: BTreeMap<String, String>,
}

impl Resolver {
    fn signature(&self, callee: &Expr) -> Option<&Signature> {
        let symbol = match &callee.kind {
            ExprKind::Name(name) => self
                .aliases
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.clone()),
            ExprKind::Member(base, method) => match &base.kind {
                ExprKind::Name(alias) => self.aliases.get(&format!("{alias}.{method}"))?.clone(),
                _ => return None,
            },
            _ => return None,
        };
        self.signatures.get(&symbol)
    }

    fn expr(&self, e: &mut Expr) -> Result<()> {
        match &mut e.kind {
            ExprKind::Value(_) | ExprKind::Name(_) => {}
            ExprKind::Unary(_, a) | ExprKind::Try(a) | ExprKind::Member(a, _) => self.expr(a)?,
            ExprKind::Binary(_, a, b) => {
                self.expr(a)?;
                self.expr(b)?;
            }
            ExprKind::Call(callee, args) => {
                self.expr(callee)?;
                for arg in args.iter_mut() {
                    self.expr(arg)?;
                }
                self.call(callee, args, &e.at)?;
            }
            ExprKind::Match(value, arms) => {
                self.expr(value)?;
                for (_, guard, arm) in arms.iter_mut() {
                    if let Some(guard) = guard {
                        self.expr(guard)?;
                    }
                    self.expr(arm)?;
                }
            }
            ExprKind::Block(body, tail) => {
                self.stmts(body)?;
                if let Some(tail) = tail {
                    self.expr(tail)?;
                }
            }
            ExprKind::Closure(_, _, body) => self.stmts(body)?,
            ExprKind::NamedConstructor(_, fields) => {
                for (_, value) in fields.iter_mut() {
                    self.expr(value)?;
                }
            }
        }
        Ok(())
    }

    fn call(&self, callee: &Expr, args: &mut Vec<Expr>, at: &Tok) -> Result<()> {
        let is_named = |a: &Expr| {
            matches!(&a.kind, ExprKind::Call(c, _)
                if matches!(&c.kind, ExprKind::Name(n) if n == NAMED))
        };
        let named_count = args.iter().filter(|a| is_named(a)).count();
        let signature = self.signature(callee);
        let Some(signature) = signature else {
            if named_count > 0 {
                return Err(diagnostic(
                    at,
                    "named arguments need a call to a declared function or module function",
                ));
            }
            return Ok(());
        };
        if named_count == 0
            && (args.len() >= signature.names.len() || signature.defaults.is_empty())
        {
            return Ok(());
        }
        let positional = args.iter().take_while(|a| !is_named(a)).count();
        if args[positional..].iter().any(|a| !is_named(a)) {
            return Err(diagnostic(
                at,
                "a positional argument cannot follow a named argument",
            ));
        }
        if positional > signature.names.len() {
            return Ok(());
        }
        let mut slots: Vec<Option<Expr>> = vec![None; signature.names.len()];
        let mut written: Vec<Expr> = std::mem::take(args);
        let named: Vec<Expr> = written.split_off(positional);
        for (slot, value) in slots.iter_mut().zip(written) {
            *slot = Some(value);
        }
        for item in named {
            let ExprKind::Call(_, mut parts) = item.kind else {
                unreachable!()
            };
            let value = parts.pop().unwrap();
            let label = parts.pop().unwrap();
            let ExprKind::Name(label_name) = &label.kind else {
                unreachable!()
            };
            let Some(index) = signature.names.iter().position(|n| n == label_name) else {
                return Err(diagnostic(
                    &label.at,
                    format!("unknown parameter '{label_name}'"),
                ));
            };
            if slots[index].is_some() {
                return Err(diagnostic(
                    &label.at,
                    format!("parameter '{label_name}' is given twice"),
                ));
            }
            slots[index] = Some(value);
        }
        for (index, slot) in slots.iter_mut().enumerate() {
            if slot.is_none() {
                match signature.defaults.get(&signature.names[index]) {
                    Some(default) => *slot = Some(default.clone()),
                    None if named_count > 0 => {
                        return Err(diagnostic(
                            at,
                            format!("missing argument '{}'", signature.names[index]),
                        ));
                    }
                    None => {}
                }
            }
        }
        // Positional call with too few arguments and no default for the gap: leave it for the
        // ordinary arity diagnostic.
        let filled = slots.iter().take_while(|s| s.is_some()).count();
        if slots[filled..].iter().any(|s| s.is_some()) {
            return Err(diagnostic(at, "an argument is missing before a given one"));
        }
        *args = slots.into_iter().flatten().collect();
        Ok(())
    }

    fn stmts(&self, stmts: &mut [Stmt]) -> Result<()> {
        for stmt in stmts {
            self.stmt(stmt)?;
        }
        Ok(())
    }

    fn stmt(&self, stmt: &mut Stmt) -> Result<()> {
        match &mut stmt.kind {
            StmtKind::Let(_, _, _, e)
            | StmtKind::Using(_, e)
            | StmtKind::Expr(e)
            | StmtKind::Defer(e) => self.expr(e)?,
            StmtKind::Assign(target, _, value) => {
                self.expr(target)?;
                self.expr(value)?;
            }
            StmtKind::Block(body) | StmtKind::External(_, body) | StmtKind::Branch(_, body) => {
                self.stmts(body)?
            }
            StmtKind::If(cond, then, otherwise) => {
                self.expr(cond)?;
                self.stmts(then)?;
                self.stmts(otherwise)?;
            }
            StmtKind::While(cond, body) => {
                self.expr(cond)?;
                self.stmts(body)?;
            }
            StmtKind::For(_, from, to, body) => {
                self.expr(from)?;
                self.expr(to)?;
                self.stmts(body)?;
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(value)?;
                }
            }
            StmtKind::Match(value, arms) => {
                self.expr(value)?;
                for (_, guard, arm) in arms.iter_mut() {
                    if let Some(guard) = guard {
                        self.expr(guard)?;
                    }
                    self.stmt(arm)?;
                }
            }
            StmtKind::Break
            | StmtKind::Continue
            | StmtKind::Commit(_)
            | StmtKind::Revert(_)
            | StmtKind::Resume(_)
            | StmtKind::Drop(_)
            | StmtKind::Publish(_)
            | StmtKind::Runtime(_, _) => {}
        }
        Ok(())
    }
}

/// Rewrite named and defaulted calls in the code that `origin` contributed to `program`.
pub(super) fn resolve(program: &mut Program, origin: &Path) -> Result<()> {
    let resolver = Resolver {
        signatures: program
            .functions
            .iter()
            .map(|(name, f)| {
                (
                    name.clone(),
                    Signature {
                        names: f.params.iter().map(|(n, _)| n.clone()).collect(),
                        defaults: f.defaults.clone(),
                    },
                )
            })
            .collect(),
        aliases: program.import_aliases.clone(),
    };
    for f in program.functions.values_mut() {
        if f.origin == origin {
            resolver.stmts(&mut f.body)?;
        }
    }
    for def in program.traits.values_mut() {
        if def.origin == origin {
            for f in def.defaults.values_mut() {
                resolver.stmts(&mut f.body)?;
            }
        }
    }
    for (stmt, stmt_origin) in program.stmts.iter_mut().zip(&program.stmt_origins) {
        if stmt_origin == origin {
            resolver.stmt(stmt)?;
        }
    }
    Ok(())
}
