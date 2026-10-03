//! Module qualification respects lexical value bindings and generic type parameters.
use super::*;
fn value_name(name: &str, names: &BTreeMap<String, String>, bound: &BTreeSet<String>) -> String {
    let head = name.split(['<', ':']).next().unwrap_or(name);
    if bound.contains(head) {
        name.into()
    } else {
        rename_symbol(name, names)
    }
}
fn expression(
    e: &mut Expr,
    names: &BTreeMap<String, String>,
    types: &BTreeMap<String, String>,
    bound: &BTreeSet<String>,
) {
    match &mut e.kind {
        ExprKind::Name(n) => *n = value_name(n, names, bound),
        ExprKind::Unary(_, v) | ExprKind::Try(v) | ExprKind::Member(v, _) => {
            expression(v, names, types, bound)
        }
        ExprKind::Binary(_, a, b) => {
            expression(a, names, types, bound);
            expression(b, names, types, bound);
        }
        ExprKind::Call(f, args) => {
            expression(f, names, types, bound);
            for a in args {
                expression(a, names, types, bound);
            }
        }
        ExprKind::NamedConstructor(n, fields) => {
            *n = rename_symbol(n, types);
            for (_, e) in fields {
                expression(e, names, types, bound);
            }
        }
        ExprKind::Match(value, arms) => {
            expression(value, names, types, bound);
            for (pat, guard, result) in arms {
                rename_pattern(pat, types);
                let mut arm = bound.clone();
                collect_pattern_bindings(pat, &mut arm);
                if let Some(g) = guard {
                    expression(g, names, types, &arm);
                }
                expression(result, names, types, &arm);
            }
        }
        ExprKind::Closure(params, ret, body) => {
            for (_, t) in params.iter_mut() {
                *t = rename_type(t, types);
            }
            *ret = rename_type(ret, types);
            let mut local = bound.clone();
            local.extend(params.iter().map(|(n, _)| n.clone()));
            block(body, names, types, &local);
        }
        ExprKind::Value(_) => {}
    }
}
fn statement(
    s: &mut Stmt,
    names: &BTreeMap<String, String>,
    types: &BTreeMap<String, String>,
    bound: &mut BTreeSet<String>,
) {
    match &mut s.kind {
        StmtKind::Let(n, _, ty, e) => {
            if let Some(t) = ty {
                *t = rename_type(t, types);
            }
            expression(e, names, types, bound);
            bound.insert(n.clone());
        }
        StmtKind::Using(n, e) => {
            expression(e, names, types, bound);
            bound.insert(n.clone());
        }
        StmtKind::Expr(e) | StmtKind::Defer(e) => expression(e, names, types, bound),
        StmtKind::Assign(a, _, b) => {
            expression(a, names, types, bound);
            expression(b, names, types, bound);
        }
        StmtKind::Block(body) | StmtKind::External(_, body) | StmtKind::Branch(_, body) => {
            block(body, names, types, bound)
        }
        StmtKind::If(e, a, b) => {
            expression(e, names, types, bound);
            block(a, names, types, bound);
            block(b, names, types, bound);
        }
        StmtKind::While(e, body) => {
            expression(e, names, types, bound);
            block(body, names, types, bound);
        }
        StmtKind::For(n, a, b, body) => {
            expression(a, names, types, bound);
            expression(b, names, types, bound);
            let mut local = bound.clone();
            local.insert(n.clone());
            block(body, names, types, &local);
        }
        StmtKind::Match(e, arms) => {
            expression(e, names, types, bound);
            for (pat, guard, body) in arms {
                rename_pattern(pat, types);
                let mut arm = bound.clone();
                collect_pattern_bindings(pat, &mut arm);
                if let Some(g) = guard {
                    expression(g, names, types, &arm);
                }
                statement(body, names, types, &mut arm);
            }
        }
        StmtKind::Return(Some(e)) => expression(e, names, types, bound),
        _ => {}
    }
}
fn block(
    body: &mut [Stmt],
    names: &BTreeMap<String, String>,
    types: &BTreeMap<String, String>,
    outer: &BTreeSet<String>,
) {
    let mut local = outer.clone();
    for s in body {
        statement(s, names, types, &mut local);
    }
}
pub(super) fn function(f: &mut Function, names: &BTreeMap<String, String>) {
    let generic = f
        .type_params
        .iter()
        .map(|(n, _)| n.clone())
        .collect::<BTreeSet<_>>();
    let types = names
        .iter()
        .filter(|(n, _)| !generic.contains(*n))
        .map(|(n, t)| (n.clone(), t.clone()))
        .collect::<BTreeMap<_, _>>();
    for (_, ty) in &mut f.params {
        *ty = rename_type(ty, &types);
    }
    f.ret = rename_type(&f.ret, &types);
    for (_, bound) in &mut f.type_params {
        if let Some(b) = bound {
            *b = rename_type(b, &types);
        }
    }
    let mut bound = f
        .params
        .iter()
        .map(|(n, _)| n.clone())
        .collect::<BTreeSet<_>>();
    bound.extend(generic);
    block(&mut f.body, names, &types, &bound);
}
pub(super) fn top_statement(
    s: &mut Stmt,
    names: &BTreeMap<String, String>,
    bound: &mut BTreeSet<String>,
) {
    statement(s, names, names, bound);
    if let StmtKind::Let(n, _, _, _) | StmtKind::Using(n, _) = &mut s.kind {
        if let Some(qualified) = names.get(n) {
            bound.remove(n);
            *n = qualified.clone();
            bound.insert(n.clone());
        }
    }
}
