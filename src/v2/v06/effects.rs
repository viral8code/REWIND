use super::*;
fn built(base: &str, method: &str) -> Option<&'static str> {
    match base {
        "Out" | "Err" => Some("output"),
        "In" => Some("input"),
        "Time" => Some("clock"),
        "Random" => Some("random"),
        "Env" => Some("env"),
        "Args" => Some("args"),
        "Locale" => Some("locale"),
        "File" => Some(
            if matches!(method, "readText" | "readBytes" | "open" | "openSnapshot") {
                "fileRead"
            } else {
                "fileWrite"
            },
        ),
        "Directory" => Some(if method == "entries" {
            "fileRead"
        } else {
            "fileWrite"
        }),
        _ => None,
    }
}
fn syntactic(program: &Program, body: &[Stmt], seen: &mut BTreeSet<String>) -> BTreeSet<String> {
    let mut needs = BTreeSet::new();
    v05::expressions(body, &mut |e| match &e.kind {
        ExprKind::Call(t, _) => match &t.kind {
            ExprKind::Member(b, m) => {
                if let ExprKind::Name(n) = &b.kind {
                    if let Some(effect) = built(n, m) {
                        needs.insert(effect.into());
                    }
                    if let Some(symbol) = program.import_aliases.get(&format!("{n}.{m}")) {
                        needs.extend(summary(program, symbol, seen));
                    }
                }
            }
            ExprKind::Name(n) => {
                let n = resolve_alias(program, n);
                if n.starts_with("stdExternal") {
                    needs.insert("external".into());
                    if n == "stdExternalClock" {
                        needs.insert("clock".into());
                    }
                    if n.starts_with("stdExternalHttp") {
                        needs.insert("network".into());
                        needs.insert("tasks".into());
                    }
                }
                if n.starts_with("stdGui") && n != "stdGuiEdit" {
                    needs.insert("gui".into());
                }
                if program.functions.contains_key(&n) {
                    needs.extend(summary(program, &n, seen));
                }
                if matches!(n.as_str(), "TaskGroup") || n.starts_with("Channel<") {
                    needs.insert("tasks".into());
                }
            }
            _ => {}
        },
        ExprKind::Unary(op, _) if op == "await" || op == "spawn" => {
            needs.insert("tasks".into());
        }
        _ => {}
    });
    needs
}
fn summary(program: &Program, n: &str, seen: &mut BTreeSet<String>) -> BTreeSet<String> {
    let Some(f) = program.functions.get(n) else {
        return BTreeSet::new();
    };
    if let Some(e) = &f.effects {
        return e.clone();
    }
    if seen.len() > 128 {
        return KNOWN.iter().map(|s| s.to_string()).collect();
    }
    if !seen.insert(n.into()) {
        return BTreeSet::new();
    }
    let e = syntactic(program, &f.body, seen);
    seen.remove(n);
    e
}
pub(in crate::v2) fn function_effects(program: &Program, n: &str) -> BTreeSet<String> {
    if let Some(f) = program.functions.get(n) {
        if let Some(e) = &f.effects {
            return e.clone();
        }
    }
    if let Some(e) = program.inferred_effects.get(n) {
        return e.clone();
    }
    summary(program, n, &mut BTreeSet::new())
}
struct Scan<'a> {
    checker: Checker<'a>,
    needs: BTreeSet<String>,
    depth: usize,
    instances: BTreeSet<String>,
}
impl Scan<'_> {
    fn expr(&mut self, e: &Expr) -> Result<()> {
        if self.depth > 128 {
            return Err(diagnostic(&e.at, "EffectInferenceBudgetExceeded"));
        }
        self.depth += 1;
        match &e.kind {
            ExprKind::Unary(op, v) => {
                if op == "await" || op == "spawn" {
                    self.needs.insert("tasks".into());
                }
                self.expr(v)?;
            }
            ExprKind::Member(v, _) | ExprKind::Try(v) => self.expr(v)?,
            ExprKind::Binary(_, a, b) => {
                self.expr(a)?;
                self.expr(b)?;
            }
            ExprKind::Call(target, args) => {
                self.expr(target)?;
                for arg in args {
                    if !matches!(&arg.kind, ExprKind::Closure(..)) {
                        self.expr(arg)?;
                    }
                }
                let name = match &target.kind {
                    ExprKind::Name(n) if self.checker.find(n).is_none() => {
                        Some(resolve_alias(self.checker.program, n))
                    }
                    ExprKind::Member(b, m) => {
                        if let ExprKind::Name(n) = &b.kind {
                            self.checker
                                .program
                                .import_aliases
                                .get(&format!("{n}.{m}"))
                                .cloned()
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(n) = &name {
                    let base = n.split('<').next().unwrap_or(n);
                    if base.starts_with("stdExternal") {
                        self.needs.insert("external".into());
                        if base == "stdExternalClock" {
                            self.needs.insert("clock".into());
                        }
                        if base.starts_with("stdExternalHttp") {
                            self.needs.insert("network".into());
                            self.needs.insert("tasks".into());
                        }
                    }
                    if base.starts_with("stdGui") && base != "stdGuiEdit" {
                        self.needs.insert("gui".into());
                    }
                    if let Some(f) = self.checker.program.functions.get(base) {
                        let types = args
                            .iter()
                            .map(|a| self.checker.expr(a))
                            .collect::<Result<Vec<_>>>()?;
                        let params = f
                            .type_params
                            .iter()
                            .map(|(n, _)| n.clone())
                            .collect::<Vec<_>>();
                        let substitutions =
                            infer_call_arguments(n, &f.params, &params, &types, &e.at)?;
                        let mut needs = function_effects(self.checker.program, base);
                        if !f.type_params.is_empty() && self.depth < 128 {
                            let key = format!("{base}:{substitutions:?}");
                            if !self.instances.contains(&key) {
                                let mut specialized = f.clone();
                                super::language::rename_function(&mut specialized, &substitutions);
                                let mut scopes = self
                                    .checker
                                    .scopes
                                    .first()
                                    .cloned()
                                    .into_iter()
                                    .collect::<Vec<_>>();
                                scopes.push(
                                    specialized
                                        .params
                                        .iter()
                                        .map(|(n, t)| (n.clone(), (t.clone(), false)))
                                        .collect(),
                                );
                                let mut instances = self.instances.clone();
                                instances.insert(key);
                                let mut scan = Scan {
                                    checker: Checker {
                                        program: self.checker.program,
                                        scopes,
                                        return_ty: Some(specialized.ret.clone()),
                                        loop_depth: 0,
                                        bounds: BTreeMap::new(),
                                        origin: f.origin.clone(),
                                    },
                                    needs: BTreeSet::new(),
                                    depth: self.depth + 1,
                                    instances,
                                };
                                scan.body(&specialized.body)?;
                                needs = scan.needs;
                            }
                        }
                        for effect in needs {
                            if let Some(effect) = substitutions.get(&effect) {
                                self.needs.extend(
                                    effect
                                        .trim_start_matches('@')
                                        .split('+')
                                        .filter(|s| !s.is_empty())
                                        .map(str::to_string),
                                );
                            } else {
                                self.needs.insert(effect);
                            }
                        }
                        if f.asynchronous {
                            self.needs.insert("tasks".into());
                        }
                    } else if base == "TaskGroup" || base.starts_with("Channel<") {
                        self.needs.insert("tasks".into());
                    } else if let Ok(t) = self.checker.expr(target) {
                        if t.starts_with("fn(") {
                            self.needs.extend(type_effects(&t));
                        }
                    }
                } else if let Ok(t) = self.checker.expr(target) {
                    if t.starts_with("fn(") {
                        self.needs.extend(type_effects(&t));
                    }
                }
                if let ExprKind::Member(base, m) = &target.kind {
                    if let ExprKind::Name(n) = &base.kind {
                        if let Some(effect) = built(n, m) {
                            self.needs.insert(effect.into());
                        }
                    }
                    if let Ok(ty) = self.checker.expr(base) {
                        if ty == "FileHandle" && matches!(m.as_str(), "read" | "readBytes") {
                            self.needs.insert("fileRead".into());
                        }
                        if ty == "FileHandle" && matches!(m.as_str(), "write" | "writeBytes") {
                            self.needs.insert("fileWrite".into());
                        }
                        if ty.starts_with("Task<")
                            || ty.starts_with("Channel<")
                            || ty == "TaskGroup"
                        {
                            self.needs.insert("tasks".into());
                        }
                        for symbol in matching_methods(self.checker.program, &ty, m) {
                            self.needs
                                .extend(function_effects(self.checker.program, &symbol));
                        }
                        if matches!(
                            m.as_str(),
                            "map" | "filter" | "take" | "fold" | "collect" | "enumerate" | "zip"
                        ) {
                            for symbol in matching_methods(self.checker.program, &ty, "next") {
                                self.needs
                                    .extend(function_effects(self.checker.program, &symbol));
                            }
                            if m == "zip" {
                                for arg in args {
                                    let ty = self.checker.expr(arg)?;
                                    for symbol in
                                        matching_methods(self.checker.program, &ty, "next")
                                    {
                                        self.needs.extend(function_effects(
                                            self.checker.program,
                                            &symbol,
                                        ));
                                    }
                                }
                            }
                        }
                        if let Some(bound) = self.checker.bounds.get(&ty) {
                            if let Some(tr) = self.checker.program.traits.get(bound) {
                                if let Some(e) = tr.method_effects.get(m) {
                                    self.needs.extend(e.clone());
                                }
                            }
                        }
                    }
                    for arg in args {
                        if let ExprKind::Closure(params, _, body) = &arg.kind {
                            let e = closure_effects(&self.checker, params, body)?;
                            self.needs.extend(e);
                        } else if matches!(
                            m.as_str(),
                            "map" | "mapErr" | "andThen" | "filter" | "fold"
                        ) {
                            if let Ok(t) = self.checker.expr(arg) {
                                if t.starts_with("fn(") {
                                    self.needs.extend(type_effects(&t));
                                }
                            }
                        }
                    }
                }
            }
            ExprKind::Closure(_, _, _) => {}
            ExprKind::NamedConstructor(_, fields) => {
                for (_, e) in fields {
                    self.expr(e)?;
                }
            }
            ExprKind::Match(v, arms) => {
                let ty = self.checker.expr(v)?;
                self.expr(v)?;
                for (p, g, e) in arms {
                    let mut bindings = BTreeMap::new();
                    self.checker.pattern(p, &ty, &e.at, &mut bindings)?;
                    self.checker.scopes.push(bindings);
                    if let Some(g) = g {
                        self.expr(g)?;
                    }
                    self.expr(e)?;
                    self.checker.scopes.pop();
                }
            }
            _ => {}
        }
        self.depth -= 1;
        Ok(())
    }
    fn body(&mut self, body: &[Stmt]) -> Result<()> {
        for s in body {
            match &s.kind {
                StmtKind::Let(n, m, annotation, e) => {
                    self.expr(e)?;
                    let ty = self.checker.expr(e)?;
                    self.checker
                        .scopes
                        .last_mut()
                        .unwrap()
                        .insert(n.clone(), (annotation.clone().unwrap_or(ty), *m));
                }
                StmtKind::Using(n, e) => {
                    self.expr(e)?;
                    let ty = self.checker.expr(e)?;
                    self.checker
                        .scopes
                        .last_mut()
                        .unwrap()
                        .insert(n.clone(), (ty, false));
                }
                StmtKind::Assign(a, _, b) => {
                    self.expr(a)?;
                    self.expr(b)?;
                    if let ExprKind::Name(n) = &a.kind {
                        let ty = self.checker.expr(b)?;
                        if ty.starts_with("fn(") {
                            for scope in self.checker.scopes.iter_mut().rev() {
                                if let Some((old, _)) = scope.get_mut(n) {
                                    let mut e = type_effects(old);
                                    e.extend(type_effects(&ty));
                                    let (params, ret) = function_signature(old).unwrap();
                                    *old = fn_type(
                                        &params
                                            .into_iter()
                                            .map(|t| (String::new(), t))
                                            .collect::<Vec<_>>(),
                                        &ret,
                                        &e,
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
                StmtKind::Expr(e) | StmtKind::Return(Some(e)) => self.expr(e)?,
                StmtKind::Defer(e) => {
                    if let ExprKind::Closure(params, _, b) = &e.kind {
                        self.needs
                            .extend(closure_effects(&self.checker, params, b)?);
                    } else {
                        self.expr(e)?;
                    }
                }
                StmtKind::External(_, b) | StmtKind::Block(b) | StmtKind::Branch(_, b) => {
                    self.scope(b)?
                }
                StmtKind::If(e, a, b) => {
                    self.expr(e)?;
                    self.scope(a)?;
                    self.scope(b)?;
                }
                StmtKind::While(e, b) => {
                    self.expr(e)?;
                    self.scope(b)?;
                }
                StmtKind::For(n, a, b, body) => {
                    self.expr(a)?;
                    self.expr(b)?;
                    self.checker
                        .scopes
                        .push(BTreeMap::from([(n.clone(), ("Int".into(), false))]));
                    self.scope(body)?;
                    self.checker.scopes.pop();
                }
                StmtKind::Match(e, arms) => {
                    let ty = self.checker.expr(e)?;
                    self.expr(e)?;
                    for (p, g, b) in arms {
                        let mut vars = BTreeMap::new();
                        self.checker.pattern(p, &ty, &s.at, &mut vars)?;
                        self.checker.scopes.push(vars);
                        if let Some(g) = g {
                            self.expr(g)?;
                        }
                        self.body(std::slice::from_ref(b))?;
                        self.checker.scopes.pop();
                    }
                }
                StmtKind::Publish(_) => {
                    self.needs.insert("publish".into());
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn scope(&mut self, b: &[Stmt]) -> Result<()> {
        self.checker.scopes.push(BTreeMap::new());
        self.body(b)?;
        self.checker.scopes.pop();
        Ok(())
    }
}
pub(in crate::v2) fn closure_effects(
    checker: &Checker<'_>,
    params: &[(String, String)],
    body: &[Stmt],
) -> Result<BTreeSet<String>> {
    let checker = Checker {
        program: checker.program,
        scopes: checker.scopes.clone(),
        return_ty: None,
        loop_depth: 0,
        bounds: checker.bounds.clone(),
        origin: checker.origin.clone(),
    };
    let mut scan = Scan {
        checker,
        needs: BTreeSet::new(),
        depth: 0,
        instances: BTreeSet::new(),
    };
    scan.checker.scopes.push(
        params
            .iter()
            .map(|(n, t)| (n.clone(), (t.clone(), false)))
            .collect(),
    );
    scan.body(body)?;
    Ok(scan.needs)
}
pub(in crate::v2) fn infer(program: &mut Program) -> Result<()> {
    program.inferred_effects = program
        .functions
        .keys()
        .map(|n| (n.clone(), BTreeSet::new()))
        .collect();
    for _ in 0..128 {
        let checker = Checker {
            program,
            scopes: vec![BTreeMap::new()],
            return_ty: None,
            loop_depth: 0,
            bounds: BTreeMap::new(),
            origin: program.root_origin.clone(),
        };
        let mut top = Scan {
            checker,
            needs: BTreeSet::new(),
            depth: 0,
            instances: BTreeSet::new(),
        };
        top.body(&program.stmts)?;
        let mut next = program.inferred_effects.clone();
        for (n, f) in &program.functions {
            if f.effects.is_some() {
                continue;
            }
            let checker = Checker {
                program,
                scopes: top.checker.scopes.clone(),
                return_ty: Some(f.ret.clone()),
                loop_depth: 0,
                bounds: f
                    .type_params
                    .iter()
                    .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                    .collect(),
                origin: f.origin.clone(),
            };
            let mut scan = Scan {
                checker,
                needs: BTreeSet::new(),
                depth: 0,
                instances: BTreeSet::new(),
            };
            scan.checker.scopes.push(
                f.params
                    .iter()
                    .map(|(n, t)| (n.clone(), (t.clone(), false)))
                    .collect(),
            );
            scan.body(&f.body)?;
            next.get_mut(n).unwrap().extend(scan.needs);
        }
        if next == program.inferred_effects {
            return Ok(());
        }
        program.inferred_effects = next;
    }
    Err(Error::InvalidOperation(
        "EffectInferenceBudgetExceeded: 128 iterations".into(),
    ))
}
// Build a bounded source route independently of inference, preserving typed causes.
fn effect_route(
    program: &Program,
    body: &[Stmt],
    effect: &str,
    seen: &mut BTreeSet<String>,
) -> Option<rewind::DiagnosticRecord> {
    let mut route = None;
    v05::expressions(body, &mut |expr| {
        if route.is_some() {
            return;
        }
        let operation = match &expr.kind {
            ExprKind::Call(target, _) => {
                if let ExprKind::Member(base, method) = &target.kind {
                    if let ExprKind::Name(n) = &base.kind {
                        built(n, method) == Some(effect)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            ExprKind::Unary(op, _) => effect == "tasks" && matches!(op.as_str(), "spawn" | "await"),
            _ => false,
        };
        let mut record = rewind::DiagnosticRecord {
            code: "MissingEffect".into(),
            message: format!("operation requires effect {effect}"),
            source: expr.at.source.clone(),
            line: expr.at.line,
            column: expr.at.col,
            task_id: None,
            frames: vec![],
            hints: vec![],
            frames_truncated: false,
            causes: Vec::new(),
            wait_edges: Vec::new(),
        };
        if operation {
            route = Some(record);
            return;
        }
        if let ExprKind::Call(target, _) = &expr.kind {
            let name = match &target.kind {
                ExprKind::Name(n) => Some(resolve_alias(program, n)),
                ExprKind::Member(base, m) => {
                    if let ExprKind::Name(n) = &base.kind {
                        program.import_aliases.get(&format!("{n}.{m}")).cloned()
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(name) = name {
                let name = name.split('<').next().unwrap_or(&name).to_string();
                if seen.len() < 128 && seen.insert(name.clone()) {
                    if let Some(f) = program.functions.get(&name) {
                        if let Some(cause) = effect_route(program, &f.body, effect, seen) {
                            record.message = format!("{name} requires effect {effect}");
                            record.causes.push(cause);
                            route = Some(record);
                        } else if function_effects(program, &name).contains(effect) {
                            record.message = format!("{name} declares effect {effect}");
                            route = Some(record);
                        }
                    }
                    seen.remove(&name);
                }
            }
        }
    });
    route
}
fn missing_effect(
    program: &Program,
    body: &[Stmt],
    at: &Tok,
    message: String,
    effect: &str,
) -> Error {
    let mut record = match diagnostic(at, message) {
        Error::Diagnostic(d) => *d,
        _ => unreachable!(),
    };
    record.code = "MissingEffect".into();
    if let Some(cause) = effect_route(program, body, effect, &mut BTreeSet::new()) {
        record.causes.push(cause);
    }
    Error::Diagnostic(Box::new(record))
}
pub(in crate::v2) fn validate(program: &Program, config: &project::ProjectConfig) -> Result<()> {
    if let Some(e) = config.effects.iter().find(|e| !KNOWN.contains(&e.as_str())) {
        return Err(Error::InvalidOperation(format!("unknown effect {e}")));
    }
    config.validate_module_effects(program)?;
    let globals = Checker {
        program,
        scopes: vec![BTreeMap::new()],
        return_ty: None,
        loop_depth: 0,
        bounds: BTreeMap::new(),
        origin: program.root_origin.clone(),
    };
    let mut top = Scan {
        checker: globals,
        needs: BTreeSet::new(),
        depth: 0,
        instances: BTreeSet::new(),
    };
    top.body(&program.stmts)?;
    let vars = top.checker.scopes.clone();
    for (name, f) in &program.functions {
        let mut scan = Scan {
            checker: Checker {
                program,
                scopes: vars.clone(),
                return_ty: Some(f.ret.clone()),
                loop_depth: 0,
                bounds: f
                    .type_params
                    .iter()
                    .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                    .collect(),
                origin: f.origin.clone(),
            },
            needs: BTreeSet::new(),
            depth: 0,
            instances: BTreeSet::new(),
        };
        scan.checker.scopes.push(
            f.params
                .iter()
                .map(|(n, t)| (n.clone(), (t.clone(), false)))
                .collect(),
        );
        scan.body(&f.body)?;
        let declared = function_effects(program, name);
        if f.asynchronous && (declared.contains("gui") || scan.needs.contains("gui")) {
            return Err(diagnostic(
                &f.at,
                "GuiMainTaskOnly: GUI input and staging require the application task",
            ));
        }
        let effect_vars = f
            .type_params
            .iter()
            .filter(|(_, b)| b.as_deref() == Some("Effect"))
            .map(|(n, _)| n.clone())
            .collect::<BTreeSet<_>>();
        for e in declared.iter().chain(scan.needs.iter()) {
            if !KNOWN.contains(&e.as_str()) && e != "publish" && !effect_vars.contains(e) {
                return Err(diagnostic(&f.at, format!("unknown effect {e}")));
            }
        }
        if f.public && f.effects.is_none() {
            return Err(diagnostic(&f.at, "public function requires effects"));
        }
        if let Some(e) = scan
            .needs
            .difference(&declared)
            .find(|e| e.as_str() != "publish")
        {
            return Err(missing_effect(
                program,
                &f.body,
                &f.at,
                format!("{name}: undeclared effect {e}"),
                e,
            ));
        }
        if scan.needs.contains("publish") && (f.asynchronous || f.origin != program.root_origin) {
            return Err(diagnostic(&f.at, "library/async cannot publish"));
        }
        if f.origin != program.root_origin {
            let upper = program
                .module_effects
                .get(&f.origin)
                .cloned()
                .unwrap_or_default();
            if let Some(e) = scan
                .needs
                .iter()
                .find(|e| !upper.contains(*e) && !effect_vars.contains(*e))
            {
                return Err(diagnostic(
                    &f.at,
                    format!("module -> {name}: undeclared effect {e}"),
                ));
            }
        }
        if f.test {
            top.needs.extend(declared);
        }
    }
    for ((trait_name, _), methods) in &program.impls {
        if let Some(tr) = program.traits.get(trait_name) {
            for (method, symbol) in methods {
                let allowed = tr.method_effects.get(method).cloned().unwrap_or_default();
                if let Some(effect) = function_effects(program, symbol)
                    .difference(&allowed)
                    .next()
                {
                    return Err(diagnostic(
                        &program.functions[symbol].at,
                        format!("trait {trait_name}.{method}: effect {effect} exceeds contract"),
                    ));
                }
            }
        }
    }
    top.needs.remove("publish");
    if let Some(e) = top.needs.difference(&config.effects).next() {
        let at = program.stmts.first().map(|s| s.at.clone()).unwrap_or(Tok {
            source: String::new(),
            text: String::new(),
            line: 1,
            col: 1,
        });
        return Err(missing_effect(
            program,
            &program.stmts,
            &at,
            format!("entry: effect {e} is not allowed"),
            e,
        ));
    }
    Ok(())
}
