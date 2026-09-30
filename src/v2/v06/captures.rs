use super::*;

pub(in crate::v2) fn flags(ty: &str) -> BTreeSet<String> {
    ty.rsplit_once("~{")
        .and_then(|(_, s)| s.strip_suffix('}'))
        .map(|s| {
            s.split('+')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_else(|| BTreeSet::from(["Send".into()]))
}
pub(in crate::v2) fn with_flags(ty: &str, send: bool, share: bool) -> String {
    let base = ty.rsplit_once("~{").map(|(s, _)| s).unwrap_or(ty);
    let flags = [(send, "Send"), (share, "Share")]
        .into_iter()
        .filter_map(|(yes, name)| yes.then_some(name))
        .collect::<Vec<_>>()
        .join("+");
    format!("{base}~{{{flags}}}")
}
pub(in crate::v2) fn compatible(expected: &str, actual: &str) -> bool {
    // Legacy runtime function values have no serialized capture/effect metadata.
    if !actual.contains("~{") {
        return true;
    }
    flags(expected).is_subset(&flags(actual))
}
pub(in crate::v2) fn infer(checker: &Checker<'_>, ty: &str, mut names: BTreeSet<String>) -> String {
    for n in names.clone() {
        names.extend(v05::needed_globals(checker.program, &n));
    }
    let transferable = |shared: bool| {
        names.iter().all(|n| {
            checker.find(n).is_none_or(|(t, mutable)| {
                !t.starts_with('&')
                    && (!shared || !mutable)
                    && v05::transfer_bounded(checker.program, t, shared, &checker.bounds)
            })
        })
    };
    with_flags(ty, transferable(false), transferable(true))
}
pub(in crate::v2) fn value_flags(
    program: &Program,
    rt: &Runtime,
    v: &Value,
    shared: bool,
    seen: &mut BTreeSet<u64>,
) -> bool {
    match v {
        Value::CellRef(_) if shared => false,
        Value::HeapRef(id) | Value::CellRef(id) => {
            if !seen.insert(*id) {
                return false;
            }
            let result = rt
                .heap_get(*id)
                .is_some_and(|v| value_flags(program, rt, v, shared, seen));
            seen.remove(id);
            result
        }
        Value::Closure(_, _, captures) => captures
            .values()
            .all(|v| value_flags(program, rt, v, shared, seen)),
        Value::Function(_, ty) => flags(ty).contains(if shared { "Share" } else { "Send" }),
        _ => v05::transfer_type(program, &value_type(v, rt), shared, &mut BTreeSet::new()),
    }
}

pub(in crate::v2) fn method_dependencies(
    checker: &Checker<'_>,
    params: &[(String, String)],
    body: &[Stmt],
) -> BTreeSet<String> {
    let mut scoped = Checker {
        program: checker.program,
        scopes: checker.scopes.clone(),
        return_ty: None,
        loop_depth: 0,
        bounds: checker.bounds.clone(),
        origin: checker.origin.clone(),
    };
    scoped.scopes.push(
        params
            .iter()
            .map(|(n, t)| (n.clone(), (t.clone(), false)))
            .collect(),
    );
    for stmt in body {
        if let StmtKind::Let(n, mutable, annotation, e) = &stmt.kind {
            let ty = annotation
                .clone()
                .or_else(|| scoped.expr(e).ok())
                .unwrap_or_else(|| "Unknown".into());
            scoped
                .scopes
                .last_mut()
                .unwrap()
                .insert(n.clone(), (ty, *mutable));
        }
    }
    let mut symbols = BTreeSet::new();
    v05::expressions(body, &mut |e| {
        if let ExprKind::Call(target, _) = &e.kind {
            if let ExprKind::Member(base, method) = &target.kind {
                if matches!(&base.kind,ExprKind::Name(n) if ["Out","Err","File","Directory","Env","In","Time","Random","Args","Locale"].contains(&n.as_str()))
                {
                    return;
                }
                if let Ok(ty) = scoped.expr(base) {
                    symbols.extend(matching_methods(checker.program, &ty, method));
                } else {
                    // Unknown outer captures are resolved by their caller. Keep every possible
                    // method dependency here so a resource hidden in a method is never omitted.
                    for methods in checker.program.impls.values() {
                        if let Some(symbol) = methods.get(method) {
                            symbols.insert(symbol.clone());
                        }
                    }
                }
            }
        }
    });
    symbols
}
pub(in crate::v2) fn names(
    checker: &Checker<'_>,
    params: &[(String, String)],
    body: &[Stmt],
) -> BTreeSet<String> {
    let mut names = v05::free_names(body, params);
    for symbol in method_dependencies(checker, params, body) {
        names.insert(symbol.clone());
        names.extend(v05::needed_globals(checker.program, &symbol));
    }
    names
}
