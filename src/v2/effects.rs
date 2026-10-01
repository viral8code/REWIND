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
    "gui",
    "external",
    "network",
];
fn task_type(program: &Program, ty: &str, seen: &mut BTreeSet<String>) -> bool {
    if ty.contains("Task<") || ty.contains("TaskGroup") || ty.contains("Channel<") {
        return true;
    }
    let base = ty.split('<').next().unwrap_or(ty);
    if !seen.insert(base.into()) {
        return false;
    }
    program
        .structs
        .get(base)
        .is_some_and(|s| s.fields.iter().any(|(_, t)| task_type(program, t, seen)))
        || program.enums.get(base).is_some_and(|e| {
            e.variants
                .values()
                .any(|fields| fields.iter().any(|(_, t)| task_type(program, t, seen)))
        })
}
fn expr(program: &Program, e: &Expr, seen: &mut BTreeSet<String>, required: &mut BTreeSet<String>) {
    match &e.kind {
        ExprKind::Call(target, args) => {
            let name = match &target.kind {
                ExprKind::Name(name) => Some(resolve_alias(program, name)),
                ExprKind::Member(base, method) => {
                    if let ExprKind::Name(name) = &base.kind {
                        program
                            .import_aliases
                            .get(&format!("{name}.{method}"))
                            .cloned()
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(name) = name {
                let base = name.split('<').next().unwrap_or(&name);
                if base.starts_with("stdExternal") {
                    required.insert("external".into());
                    if base == "stdExternalClock" {
                        required.insert("clock".into());
                    }
                    if base.starts_with("stdExternalHttp") {
                        required.insert("network".into());
                        required.insert("tasks".into());
                    }
                }
                if base.starts_with("stdGui") && base != "stdGuiEdit" {
                    required.insert("gui".into());
                }
                if base == "Channel" || base == "TaskGroup" {
                    required.insert("tasks".into());
                }
                if let Some(f) = program.functions.get(base) {
                    if f.asynchronous {
                        required.insert("tasks".into());
                    }
                    if seen.insert(base.into()) {
                        body(program, &f.body, seen, required);
                    }
                }
            }
            if let ExprKind::Member(base, method) = &target.kind {
                if let ExprKind::Name(receiver) = &base.kind {
                    let effect = match receiver.as_str() {
                        "Out" | "Err" => Some("output"),
                        "In" => Some("input"),
                        "Time" => Some("clock"),
                        "Random" => Some("random"),
                        "Env" => Some("env"),
                        "Args" => Some("args"),
                        "Locale" => Some("locale"),
                        "File" => Some(
                            if matches!(
                                method.as_str(),
                                "readText" | "readBytes" | "open" | "openSnapshot"
                            ) {
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
                    };
                    if let Some(effect) = effect {
                        required.insert(effect.into());
                    }
                }
                match method.as_str() {
                    "send" | "receive" | "cancel" | "setPriority" | "join" => {
                        required.insert("tasks".into());
                    }
                    "read" | "readBytes" => {
                        required.insert("fileRead".into());
                    }
                    "write" | "writeBytes" => {
                        required.insert("fileWrite".into());
                    }
                    _ => {}
                }
                for methods in program.impls.values() {
                    if let Some(symbol) = methods.get(method) {
                        if seen.insert(symbol.clone()) {
                            body(program, &program.functions[symbol].body, seen, required);
                        }
                    }
                }
            }
            expr(program, target, seen, required);
            for a in args {
                expr(program, a, seen, required);
            }
        }
        ExprKind::Unary(op, e) => {
            if matches!(op.as_str(), "spawn" | "await") {
                required.insert("tasks".into());
            }
            expr(program, e, seen, required);
        }
        ExprKind::Member(e, _) | ExprKind::Try(e) => expr(program, e, seen, required),
        ExprKind::Binary(_, a, b) => {
            expr(program, a, seen, required);
            expr(program, b, seen, required);
        }
        ExprKind::Closure(_, _, stmts) => body(program, stmts, seen, required),
        ExprKind::Match(e, arms) => {
            expr(program, e, seen, required);
            for (_, guard, value) in arms {
                if let Some(g) = guard {
                    expr(program, g, seen, required);
                }
                expr(program, value, seen, required);
            }
        }
        ExprKind::NamedConstructor(_, fields) => {
            for (_, value) in fields {
                expr(program, value, seen, required);
            }
        }
        _ => {}
    }
}
fn body(
    program: &Program,
    stmts: &[Stmt],
    seen: &mut BTreeSet<String>,
    required: &mut BTreeSet<String>,
) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Publish(_) => {
                required.insert("publish".into());
            }
            StmtKind::Let(_, _, _, e)
            | StmtKind::Using(_, e)
            | StmtKind::Expr(e)
            | StmtKind::Defer(e)
            | StmtKind::Return(Some(e)) => expr(program, e, seen, required),
            StmtKind::Assign(a, _, b) => {
                expr(program, a, seen, required);
                expr(program, b, seen, required);
            }
            StmtKind::If(e, a, b) => {
                expr(program, e, seen, required);
                body(program, a, seen, required);
                body(program, b, seen, required);
            }
            StmtKind::External(_, b) | StmtKind::Block(b) | StmtKind::Branch(_, b) => {
                body(program, b, seen, required)
            }
            StmtKind::While(e, b) => {
                expr(program, e, seen, required);
                body(program, b, seen, required);
            }
            StmtKind::For(_, a, b, stmts) => {
                expr(program, a, seen, required);
                expr(program, b, seen, required);
                body(program, stmts, seen, required);
            }
            StmtKind::Match(e, arms) => {
                expr(program, e, seen, required);
                for (_, guard, s) in arms {
                    if let Some(g) = guard {
                        expr(program, g, seen, required);
                    }
                    body(program, std::slice::from_ref(s), seen, required);
                }
            }
            _ => {}
        }
    }
}
pub(super) fn validate(program: &Program, config: &project::ProjectConfig) -> Result<()> {
    for effect in config
        .effects
        .iter()
        .chain(program.module_effects.values().flatten())
    {
        if !KNOWN.contains(&effect.as_str()) {
            return Err(Error::InvalidOperation(format!("unknown effect {effect}")));
        }
    }
    let mut required_by_module: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for (stmt, origin) in program.stmts.iter().zip(&program.stmt_origins) {
        body(
            program,
            std::slice::from_ref(stmt),
            &mut BTreeSet::new(),
            required_by_module.entry(origin.clone()).or_default(),
        );
    }
    for f in program.functions.values() {
        let mut required = BTreeSet::new();
        if f.asynchronous
            || f.params
                .iter()
                .map(|(_, ty)| ty)
                .chain(std::iter::once(&f.ret))
                .any(|ty| task_type(program, ty, &mut BTreeSet::new()))
        {
            required.insert("tasks".into());
        }
        body(program, &f.body, &mut BTreeSet::new(), &mut required);
        if f.asynchronous && required.contains("publish") {
            return Err(diagnostic(&f.at, "async function cannot publish"));
        }
        required_by_module
            .entry(f.origin.clone())
            .or_default()
            .extend(required);
    }
    for (origin, mut required) in required_by_module {
        if origin == program.root_origin {
            required.remove("publish");
        } else {
            if required.contains("publish") {
                return Err(Error::InvalidOperation(format!(
                    "library {} cannot publish",
                    origin.display()
                )));
            }
            let declared = program
                .module_effects
                .get(&origin)
                .cloned()
                .unwrap_or_default();
            if let Some(missing) = required.difference(&declared).next() {
                return Err(Error::InvalidOperation(format!(
                    "{}: undeclared effect {missing}",
                    origin.display()
                )));
            }
            for (name, path) in &config.imports {
                if !name.is_empty() && origin.starts_with(path) {
                    let metadata: serde_json::Value =
                        serde_json::from_slice(&fs::read(path.join("rewind.package.json"))?)
                            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
                    let package = metadata["effects"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|e| e.as_str().map(str::to_string))
                        .collect::<BTreeSet<_>>();
                    if let Some(missing) = declared.difference(&package).next() {
                        return Err(Error::InvalidOperation(format!(
                            "package {name}: module requests undeclared package effect {missing}"
                        )));
                    }
                }
            }
        }
        if let Some(missing) = required.difference(&config.effects).next() {
            return Err(Error::InvalidOperation(format!(
                "{}: effect {missing} is not allowed",
                origin.display()
            )));
        }
    }
    Ok(())
}
