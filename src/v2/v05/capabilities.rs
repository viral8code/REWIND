use super::*;
const KNOWN: &[&str] = &[
    "fileRead",
    "fileWrite",
    "output",
    "input",
    "clock",
    "random",
    "env",
    "args",
    "locale",
    "tasks",
];
#[derive(Default, Clone)]
struct Needs {
    direct: BTreeSet<String>,
    calls: BTreeSet<String>,
}
fn analyze(
    program: &Program,
    body: &[Stmt],
    params: &[(String, String)],
    globals: &BTreeMap<String, (String, bool)>,
    bounds: &BTreeMap<String, String>,
) -> Needs {
    fn merge(out: &mut Needs, n: Needs) {
        out.direct.extend(n.direct);
        out.calls.extend(n.calls);
    }
    fn walk(
        program: &Program,
        body: &[Stmt],
        variables: &mut BTreeMap<String, (String, bool)>,
        aliases: &mut BTreeMap<String, String>,
        bounds: &BTreeMap<String, String>,
        out: &mut Needs,
    ) {
        for s in body {
            let mut head = s.clone();
            match &mut head.kind {
                StmtKind::Block(b)
                | StmtKind::Branch(_, b)
                | StmtKind::While(_, b)
                | StmtKind::For(_, _, _, b) => b.clear(),
                StmtKind::If(_, a, b) => {
                    a.clear();
                    b.clear();
                }
                StmtKind::Match(_, arms) => {
                    for (_, _, s) in arms {
                        s.kind = StmtKind::Block(vec![]);
                    }
                }
                _ => {}
            }
            merge(
                out,
                analyze_flat(
                    program,
                    std::slice::from_ref(&head),
                    &[],
                    variables,
                    bounds,
                    aliases,
                ),
            );
            let checker = Checker {
                program,
                scopes: vec![variables.clone()],
                return_ty: None,
                loop_depth: 0,
                bounds: bounds.clone(),
                origin: program.root_origin.clone(),
            };
            match &s.kind {
                StmtKind::Let(n, m, annotation, e) => {
                    if let ExprKind::Name(v) = &e.kind {
                        let target = aliases.get(v).unwrap_or(v).clone();
                        aliases.insert(n.clone(), target);
                    } else {
                        aliases.remove(n);
                    }
                    if let Ok(t) = checker.expr(e) {
                        variables.insert(n.clone(), (annotation.clone().unwrap_or(t), *m));
                    }
                }
                StmtKind::Using(n, e) => {
                    if let Ok(t) = checker.expr(e) {
                        variables.insert(n.clone(), (t, false));
                    }
                }
                StmtKind::Block(b) | StmtKind::Branch(_, b) | StmtKind::While(_, b) => walk(
                    program,
                    b,
                    &mut variables.clone(),
                    &mut aliases.clone(),
                    bounds,
                    out,
                ),
                StmtKind::For(n, _, _, b) => {
                    let mut nested = variables.clone();
                    nested.insert(n.clone(), ("Int".into(), false));
                    walk(program, b, &mut nested, &mut aliases.clone(), bounds, out);
                }
                StmtKind::If(_, a, b) => {
                    walk(
                        program,
                        a,
                        &mut variables.clone(),
                        &mut aliases.clone(),
                        bounds,
                        out,
                    );
                    walk(
                        program,
                        b,
                        &mut variables.clone(),
                        &mut aliases.clone(),
                        bounds,
                        out,
                    );
                }
                StmtKind::Match(e, arms) => {
                    if let Ok(ty) = checker.expr(e) {
                        for (pattern, _, s) in arms {
                            let mut nested = variables.clone();
                            let _ = checker.pattern(pattern, &ty, &s.at, &mut nested);
                            let mut local_aliases = aliases.clone();
                            for n in nested.keys() {
                                if !variables.contains_key(n) {
                                    local_aliases.remove(n);
                                }
                            }
                            walk(
                                program,
                                std::slice::from_ref(s),
                                &mut nested,
                                &mut local_aliases,
                                bounds,
                                out,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let mut variables = globals.clone();
    let mut aliases = BTreeMap::new();
    for s in &program.stmts {
        if let StmtKind::Let(
            n,
            _,
            _,
            Expr {
                kind: ExprKind::Name(v),
                ..
            },
        ) = &s.kind
        {
            aliases.insert(n.clone(), v.clone());
        }
    }
    for (n, _) in params {
        aliases.remove(n);
    }
    variables.extend(params.iter().map(|(n, t)| (n.clone(), (t.clone(), false))));
    let mut needs = Needs::default();
    walk(
        program,
        body,
        &mut variables,
        &mut aliases,
        bounds,
        &mut needs,
    );
    needs
}
fn analyze_flat(
    program: &Program,
    body: &[Stmt],
    params: &[(String, String)],
    globals: &BTreeMap<String, (String, bool)>,
    bounds: &BTreeMap<String, String>,
    scoped_aliases: &BTreeMap<String, String>,
) -> Needs {
    let mut needs = Needs::default();
    let mut variables = globals.clone();
    variables.extend(params.iter().map(|(n, t)| (n.clone(), (t.clone(), false))));
    let mut aliases = BTreeMap::new();
    fn declarations(body: &[Stmt], aliases: &mut BTreeMap<String, String>) {
        for s in body {
            match &s.kind {
                StmtKind::Let(
                    n,
                    _,
                    _,
                    Expr {
                        kind: ExprKind::Name(v),
                        ..
                    },
                ) => {
                    aliases.insert(n.clone(), v.clone());
                }
                StmtKind::Block(b)
                | StmtKind::Branch(_, b)
                | StmtKind::While(_, b)
                | StmtKind::For(_, _, _, b) => declarations(b, aliases),
                StmtKind::If(_, a, b) => {
                    declarations(a, aliases);
                    declarations(b, aliases);
                }
                _ => {}
            }
        }
    }
    aliases.extend(scoped_aliases.clone());
    declarations(body, &mut aliases);
    for (n, _) in params {
        aliases.remove(n);
    }
    // Reassignment and shadowing make a function value opaque. Requiring the
    // full upper bound prevents a later assignment from hiding effects.
    let mut opaque = BTreeSet::new();
    fn assigned(body: &[Stmt], out: &mut BTreeSet<String>) {
        for s in body {
            match &s.kind {
                StmtKind::Assign(
                    Expr {
                        kind: ExprKind::Name(n),
                        ..
                    },
                    _,
                    _,
                ) => {
                    out.insert(n.clone());
                }
                StmtKind::Block(b)
                | StmtKind::Branch(_, b)
                | StmtKind::While(_, b)
                | StmtKind::For(_, _, _, b) => assigned(b, out),
                StmtKind::If(_, a, b) => {
                    assigned(a, out);
                    assigned(b, out);
                }
                StmtKind::Match(_, arms) => {
                    for (_, _, s) in arms {
                        assigned(std::slice::from_ref(s), out);
                    }
                }
                _ => {}
            }
        }
    }
    assigned(&program.stmts, &mut opaque);
    assigned(body, &mut opaque);
    fn locals(program: &Program, body: &[Stmt], variables: &mut BTreeMap<String, (String, bool)>) {
        for s in body {
            match &s.kind {
                StmtKind::Let(n, _, _, e) | StmtKind::Using(n, e) => {
                    let checker = Checker {
                        program,
                        scopes: vec![variables.clone()],
                        return_ty: None,
                        loop_depth: 0,
                        bounds: BTreeMap::new(),
                        origin: program.root_origin.clone(),
                    };
                    if let Ok(ty) = checker.expr(e) {
                        variables.insert(
                            n.clone(),
                            (ty, matches!(&s.kind, StmtKind::Let(_, true, _, _))),
                        );
                    }
                }
                StmtKind::Block(b)
                | StmtKind::Branch(_, b)
                | StmtKind::While(_, b)
                | StmtKind::For(_, _, _, b) => locals(program, b, variables),
                StmtKind::If(_, a, b) => {
                    locals(program, a, variables);
                    locals(program, b, variables);
                }
                StmtKind::Match(_, arms) => {
                    for (_, _, s) in arms {
                        locals(program, std::slice::from_ref(s), variables);
                    }
                }
                _ => {}
            }
        }
    }
    locals(program, body, &mut variables);
    let checker = Checker {
        program,
        scopes: vec![variables],
        return_ty: None,
        loop_depth: 0,
        bounds: bounds.clone(),
        origin: program.root_origin.clone(),
    };
    expressions(body, &mut |e| match &e.kind {
        ExprKind::Unary(op, _) if matches!(op.as_str(), "await" | "spawn") => {
            needs.direct.insert("tasks".into());
        }
        ExprKind::Call(target, _) => {
            if !matches!(&target.kind, ExprKind::Name(_))
                && checker.expr(target).is_ok_and(|t| t.starts_with("fn("))
            {
                needs.direct.extend(KNOWN.iter().map(|s| s.to_string()));
            }
            let name = match &target.kind {
                ExprKind::Name(n) => {
                    if checker.find(n).is_some_and(|(t, _)| t.starts_with("fn("))
                        && (!aliases.contains_key(n)
                            || globals.get(n).is_some_and(|(_, mutable)| *mutable))
                    {
                        needs.direct.extend(KNOWN.iter().map(|s| s.to_string()));
                        return;
                    }
                    if opaque.contains(n)
                        && checker.find(n).is_some_and(|(t, _)| t.starts_with("fn("))
                    {
                        needs.direct.extend(KNOWN.iter().map(|s| s.to_string()));
                    }
                    let mut resolved = n.clone();
                    let mut seen = BTreeSet::new();
                    while seen.insert(resolved.clone()) {
                        if opaque.contains(&resolved) {
                            needs.direct.extend(KNOWN.iter().map(|s| s.to_string()));
                        }
                        if let Some(next) = aliases.get(&resolved) {
                            resolved = next.clone();
                        } else {
                            break;
                        }
                    }
                    Some(resolve_alias(program, &resolved))
                }
                ExprKind::Member(base, m) => {
                    if let ExprKind::Name(n) = &base.kind {
                        program.import_aliases.get(&format!("{n}.{m}")).cloned()
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(n) = name {
                let base = n.split('<').next().unwrap_or(&n);
                if program.functions.contains_key(base) {
                    needs.calls.insert(base.into());
                    if program.functions[base].asynchronous {
                        needs.direct.insert("tasks".into());
                    }
                } else if matches!(base, "Channel" | "TaskGroup") {
                    needs.direct.insert("tasks".into());
                } else if checker
                    .find(base)
                    .is_some_and(|(t, _)| t.starts_with("fn("))
                {
                    needs.direct.extend(KNOWN.iter().map(|s| s.to_string()));
                }
            }
            if let ExprKind::Member(base, m) = &target.kind {
                if let ExprKind::Name(n) = &base.kind {
                    let effect = match n.as_str() {
                        "Out" | "Err" => Some("output"),
                        "In" => Some("input"),
                        "Time" => Some("clock"),
                        "Random" => Some("random"),
                        "Env" => Some("env"),
                        "Args" => Some("args"),
                        "Locale" => Some("locale"),
                        "File" => Some(
                            if matches!(
                                m.as_str(),
                                "readText" | "readBytes" | "open" | "openSnapshot"
                            ) {
                                "fileRead"
                            } else {
                                "fileWrite"
                            },
                        ),
                        "Directory" => Some(if m == "entries" {
                            "fileRead"
                        } else {
                            "fileWrite"
                        }),
                        _ => None,
                    };
                    if let Some(effect) = effect {
                        needs.direct.insert(effect.into());
                    }
                }
                if let Ok(ty) = checker.expr(base) {
                    if ty.starts_with("Channel<") || ty.starts_with("Task<") || ty == "TaskGroup" {
                        needs.direct.insert("tasks".into());
                    }
                    if ty == "FileHandle" {
                        if matches!(m.as_str(), "read" | "readBytes") {
                            needs.direct.insert("fileRead".into());
                        } else if matches!(m.as_str(), "write" | "writeBytes") {
                            needs.direct.insert("fileWrite".into());
                        }
                    }
                    needs.calls.extend(matching_methods(program, &ty, m));
                    if let Some(bound) = bounds.get(&ty) {
                        for ((trait_name, _), methods) in &program.impls {
                            if bound.split('+').any(|b| b == trait_name) {
                                if let Some(symbol) = methods.get(m) {
                                    needs.calls.insert(symbol.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    });
    fn publish(body: &[Stmt]) -> bool {
        body.iter().any(|s| match &s.kind {
            StmtKind::Publish(_) => true,
            StmtKind::Block(b)
            | StmtKind::Branch(_, b)
            | StmtKind::While(_, b)
            | StmtKind::For(_, _, _, b) => publish(b),
            StmtKind::If(_, a, b) => publish(a) || publish(b),
            StmtKind::Match(_, arms) => arms
                .iter()
                .any(|(_, _, s)| publish(std::slice::from_ref(s))),
            _ => false,
        })
    }
    if publish(body) {
        needs.direct.insert("publish".into());
    }
    expressions(body, &mut |e| {
        if let ExprKind::Closure(_, _, b) = &e.kind {
            if publish(b) {
                needs.direct.insert("publish".into());
            }
        }
    });
    needs
}
pub(super) fn validate(program: &Program, config: &project::ProjectConfig) -> Result<()> {
    config.validate_module_effects(program)?;
    for e in config
        .effects
        .iter()
        .chain(program.module_effects.values().flatten())
        .chain(
            program
                .functions
                .values()
                .filter_map(|f| f.effects.as_ref())
                .flatten(),
        )
    {
        if !KNOWN.contains(&e.as_str()) {
            return Err(Error::InvalidOperation(format!("unknown effect {e}")));
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
    for s in &program.stmts {
        checker.stmt(s)?;
    }
    let globals = &checker.scopes[0];
    let graph = program
        .functions
        .iter()
        .map(|(n, f)| {
            (
                n.clone(),
                analyze(
                    program,
                    &f.body,
                    &f.params,
                    globals,
                    &f.type_params
                        .iter()
                        .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                        .collect(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut requirements = graph
        .iter()
        .map(|(n, g)| (n.clone(), g.direct.clone()))
        .collect::<BTreeMap<_, _>>();
    loop {
        let old = requirements.clone();
        for (n, g) in &graph {
            for call in &g.calls {
                if let Some(r) = old.get(call) {
                    requirements.get_mut(n).unwrap().extend(r.clone());
                }
            }
        }
        if old == requirements {
            break;
        }
    }
    for (n, f) in &program.functions {
        let required = &requirements[n];
        if required.contains("publish") && (f.asynchronous || f.origin != program.root_origin) {
            return Err(diagnostic(&f.at, "library/async function cannot publish"));
        }
        if f.public && f.effects.is_none() {
            return Err(diagnostic(
                &f.at,
                format!("public function {n} requires effects {{...}}"),
            ));
        }
        if let Some(declared) = &f.effects {
            let mut required = required.clone();
            required.remove("publish");
            if let Some(e) = required.difference(declared).next() {
                return Err(diagnostic(
                    &f.at,
                    format!("{n} -> operation: undeclared effect {e}"),
                ));
            }
        }
        if f.origin != program.root_origin {
            let declared = program
                .module_effects
                .get(&f.origin)
                .cloned()
                .unwrap_or_default();
            if let Some(e) = required.difference(&declared).next() {
                return Err(diagnostic(
                    &f.at,
                    format!("module -> {n}: undeclared effect {e}"),
                ));
            }
        }
    }
    let mut top = analyze(program, &program.stmts, &[], globals, &BTreeMap::new());
    for (n, f) in &program.functions {
        if f.test {
            top.calls.insert(n.clone());
        }
    }
    for call in top.calls {
        if let Some(r) = requirements.get(&call) {
            top.direct.extend(r.clone());
        }
    }
    top.direct.remove("publish");
    if let Some(e) = top.direct.difference(&config.effects).next() {
        return Err(Error::InvalidOperation(format!(
            "entry -> reachable function -> operation: effect {e} is not allowed"
        )));
    }
    Ok(())
}
