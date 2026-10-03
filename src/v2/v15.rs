use super::*;
fn walk_expr(e: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(e);
    match &e.kind {
        ExprKind::Unary(_, e) | ExprKind::Try(e) | ExprKind::Member(e, _) => walk_expr(e, visit),
        ExprKind::Binary(_, a, b) => {
            walk_expr(a, visit);
            walk_expr(b, visit);
        }
        ExprKind::Call(f, args) => {
            walk_expr(f, visit);
            for a in args {
                walk_expr(a, visit);
            }
        }
        ExprKind::Match(e, arms) => {
            walk_expr(e, visit);
            for (_, g, e) in arms {
                if let Some(g) = g {
                    walk_expr(g, visit);
                }
                walk_expr(e, visit);
            }
        }
        ExprKind::NamedConstructor(_, fields) => {
            for (_, e) in fields {
                walk_expr(e, visit);
            }
        }
        // A closure's return does not escape its enclosing lexical region.
        ExprKind::Closure(_, _, _) => {}
        _ => {}
    }
}
fn direct_exprs(s: &Stmt, visit: &mut impl FnMut(&Expr)) {
    match &s.kind {
        StmtKind::Let(_, _, _, e)
        | StmtKind::Using(_, e)
        | StmtKind::Expr(e)
        | StmtKind::Defer(e)
        | StmtKind::Return(Some(e))
        | StmtKind::If(e, _, _)
        | StmtKind::While(e, _)
        | StmtKind::Match(e, _) => walk_expr(e, visit),
        StmtKind::Assign(a, _, b) | StmtKind::For(_, a, b, _) => {
            walk_expr(a, visit);
            walk_expr(b, visit);
        }
        _ => {}
    }
}
fn calls(p: &Program, e: &Expr, needs: &BTreeSet<String>) -> bool {
    if let ExprKind::Call(callee, _) = &e.kind {
        let name = match &callee.kind {
            ExprKind::Name(n) => Some(resolve_alias(p, n)),
            ExprKind::Member(b, m) => {
                if let ExprKind::Name(n) = &b.kind {
                    p.import_aliases.get(&format!("{n}.{m}")).cloned()
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(n) = name {
            let n = n.split('<').next().unwrap_or(&n);
            return n.starts_with("stdExternal") || needs.contains(n);
        }
    }
    false
}
fn requires(p: &Program, b: &[Stmt], needs: &BTreeSet<String>) -> bool {
    b.iter().any(|s| {
        let mut found = false;
        direct_exprs(s, &mut |e| found |= calls(p, e, needs));
        found
            || match &s.kind {
                StmtKind::External(_, _) => false,
                StmtKind::Block(b)
                | StmtKind::Branch(_, b)
                | StmtKind::While(_, b)
                | StmtKind::For(_, _, _, b) => requires(p, b, needs),
                StmtKind::If(_, a, b) => requires(p, a, needs) || requires(p, b, needs),
                StmtKind::Match(_, arms) => arms
                    .iter()
                    .any(|(_, _, s)| requires(p, std::slice::from_ref(s), needs)),
                _ => false,
            }
    })
}
fn task_switch(p: &Program, e: &Expr, seen: &mut BTreeSet<String>) -> bool {
    let ExprKind::Call(callee, _) = &e.kind else {
        return false;
    };
    let name = match &callee.kind {
        ExprKind::Name(n) => Some(resolve_alias(p, n)),
        ExprKind::Member(base, method) => {
            if let ExprKind::Name(n) = &base.kind {
                p.import_aliases.get(&format!("{n}.{method}")).cloned()
            } else {
                None
            }
        }
        _ => None,
    };
    let Some(name) = name else {
        return false;
    };
    let base = name.split('<').next().unwrap_or(&name);
    if base == "stdTaskYieldNow" {
        return true;
    }
    if !seen.insert(base.into()) {
        return false;
    }
    let Some(f) = p.functions.get(base) else {
        return false;
    };
    let mut found = false;
    v05::expressions(&f.body, &mut |e| found |= task_switch(p, e, seen));
    found
}
pub(super) fn validate(p: &Program) -> Result<()> {
    let mut needs = BTreeSet::new();
    loop {
        let before = needs.len();
        for (n, f) in &p.functions {
            if requires(p, &f.body, &needs) {
                needs.insert(n.clone());
            }
        }
        if needs.len() == before {
            break;
        }
    }
    fn body(
        p: &Program,
        b: &[Stmt],
        region: bool,
        permitted: bool,
        needs: &BTreeSet<String>,
    ) -> Result<()> {
        for s in b {
            let mut error = None;
            direct_exprs(s, &mut |e| {
                if let ExprKind::Closure(_, _, closure) = &e.kind {
                    if let Err(e) = body(
                        p,
                        closure,
                        false,
                        permitted || requires(p, closure, needs),
                        needs,
                    ) {
                        error = Some(e);
                    }
                }
                if !permitted && calls(p, e, needs) {
                    error = Some(diagnostic(
                        &e.at,
                        "ExternalBoundary: call requires external { ... }",
                    ));
                }
                if region
                    && (task_switch(p, e, &mut BTreeSet::new())
                        || matches!(&e.kind, ExprKind::Try(_))
                        || matches!(&e.kind, ExprKind::Unary(op,_) if op=="await" || op=="spawn"))
                {
                    error=Some(diagnostic(&e.at,"ExternalBoundary: propagation and task switching are forbidden in this region"));
                }
            });
            if let Some(e) = error {
                return Err(e);
            }
            match &s.kind {
                StmtKind::External(_,b)=>{
                    if !language_at_least(&p.language,"1.5.0"){return Err(diagnostic(&s.at,"external requires language 1.5.0"));}
                    if region {return Err(diagnostic(&s.at,"ExternalBoundary: nested external region"));}
                    body(p,b,true,true,needs)?;
                }
                StmtKind::Commit(_) | StmtKind::Revert(_) | StmtKind::Resume(_) | StmtKind::Drop(_) | StmtKind::Publish(_) | StmtKind::Branch(_,_) | StmtKind::Return(_) | StmtKind::Break | StmtKind::Continue | StmtKind::Defer(_) if region=>return Err(diagnostic(&s.at,"ExternalBoundary: checkpoints and escaping control flow are forbidden in this region")),
                StmtKind::Block(b)|StmtKind::Branch(_,b)|StmtKind::While(_,b)|StmtKind::For(_,_,_,b)=>body(p,b,region,permitted,needs)?,
                StmtKind::If(_,a,b)=>{body(p,a,region,permitted,needs)?;body(p,b,region,permitted,needs)?;}
                StmtKind::Match(_,arms)=>{for (_,_,s) in arms{body(p,std::slice::from_ref(s),region,permitted,needs)?;}}
                _=>{}
            }
        }
        Ok(())
    }
    body(p, &p.stmts, false, false, &needs)?;
    for (n, f) in &p.functions {
        body(p, &f.body, false, n != "main" && needs.contains(n), &needs)?;
    }
    Ok(())
}
