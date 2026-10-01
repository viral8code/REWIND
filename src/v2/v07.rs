use super::*;
use serde_json::{json, Value as Json};
mod api;
mod docs;
pub(super) use api::{convert as api_convert, diff as api_diff, snapshot as api_snapshot};
pub(super) use docs::doctest;

pub(super) fn validate_records(program: &Program) -> Result<()> {
    for (name, record) in &program.structs {
        if !record.immutable {
            continue;
        }
        if !program_v07(program) {
            return Err(Error::InvalidOperation(
                "record requires language 0.7".into(),
            ));
        }
        if !record.type_params.is_empty() && !program_v09(program) {
            return Err(Error::InvalidOperation(
                "record generic parameters require explicit field contracts (not supported)".into(),
            ));
        }
        if program_v09(program)
            && record
                .type_params
                .iter()
                .any(|p| record.bounds.get(p).map(String::as_str) != Some("Share"))
        {
            return Err(Error::InvalidOperation(
                "generic record parameters require Share bounds".into(),
            ));
        }
        for (field, ty) in &record.fields {
            if !v05::transfer_bounded(program, ty, true, &record.bounds) {
                return Err(Error::InvalidOperation(format!(
                    "record {name}.{field} requires a Share field; freeze mutable values"
                )));
            }
        }
    }
    Ok(())
}
pub(super) fn validate_constants(program: &Program) -> Result<()> {
    constant_values(program).map(|_| ())
}
pub(in crate::v2) fn constant_values(program: &Program) -> Result<BTreeMap<String, Json>> {
    if !program_v07(program) {
        return Ok(BTreeMap::new());
    }
    let root = program.root_origin.parent().unwrap_or(Path::new("."));
    let mut engine = Engine::new(program.clone(), root, io::Cursor::new(Vec::<u8>::new()))?;
    engine.remaining = 100_000;
    engine.runtime.enable_virtual_publish();
    engine.runtime.set_budget(ResourceBudget {
        history_memory: 4 * 1024 * 1024,
        history_storage: 4 * 1024 * 1024,
        spill_threshold: 4 * 1024 * 1024,
    })?;
    let mut checker = Checker {
        program,
        scopes: vec![BTreeMap::new()],
        return_ty: None,
        loop_depth: 0,
        bounds: BTreeMap::new(),
        origin: program.root_origin.clone(),
    };
    let mut values = BTreeMap::new();
    for (stmt, origin) in program.stmts.iter().zip(&program.stmt_origins) {
        let StmtKind::Let(name, _, annotation, expr) = &stmt.kind else {
            continue;
        };
        if !program.const_origins.contains_key(name) {
            continue;
        }
        checker.origin = origin.clone();
        let effects = v06::closure_effects(&checker, &[], std::slice::from_ref(stmt))?;
        if !effects.is_empty() {
            return Err(diagnostic(
                &stmt.at,
                format!("const requires a pure expression; effects {effects:?}"),
            ));
        }
        let ty = checker.expr(expr)?;
        if !v05::transfer_type(program, &ty, true, &mut BTreeSet::new()) {
            return Err(diagnostic(&stmt.at, "const requires a Share value"));
        }
        // Checkpoint, mutation, and cleanup are boundary operations even in pure functions.
        let mut forbidden = false;
        let mut pending = Vec::new();
        let mut seen = BTreeSet::new();
        let inspect = |body: &[Stmt], pending: &mut Vec<String>, forbidden: &mut bool| {
            v05::expressions(body, &mut |e| {
                if let ExprKind::Name(n) = &e.kind {
                    let resolved = resolve_alias(program, n);
                    let name = resolved.split('<').next().unwrap_or(&resolved);
                    if program.functions.contains_key(name) {
                        pending.push(name.into());
                    }
                }
                if let ExprKind::Call(t, _) = &e.kind {
                    if let ExprKind::Name(n) = &t.kind {
                        let n = resolve_alias(program, n);
                        let n = n.split('<').next().unwrap_or(&n);
                        if program.functions.contains_key(n) {
                            pending.push(n.into());
                        }
                    } else if let ExprKind::Member(base, method) = &t.kind {
                        if let ExprKind::Name(n) = &base.kind {
                            if let Some(symbol) =
                                program.import_aliases.get(&format!("{n}.{method}"))
                            {
                                pending.push(symbol.clone());
                            }
                        }
                        for methods in program.impls.values() {
                            if let Some(symbol) = methods.get(method) {
                                pending.push(symbol.clone());
                            }
                        }
                    }
                }
            });
            fn boundary(body: &[Stmt]) -> bool {
                body.iter().any(|s| match &s.kind {
                    StmtKind::Commit(_)
                    | StmtKind::Revert(_)
                    | StmtKind::Resume(_)
                    | StmtKind::Drop(_)
                    | StmtKind::Publish(_)
                    | StmtKind::Defer(_)
                    | StmtKind::Using(_, _)
                    | StmtKind::Runtime(_, _)
                    | StmtKind::Branch(_, _) => true,
                    StmtKind::External(_, b)
                    | StmtKind::Block(b)
                    | StmtKind::While(_, b)
                    | StmtKind::For(_, _, _, b) => boundary(b),
                    StmtKind::If(_, a, b) => boundary(a) || boundary(b),
                    StmtKind::Match(_, arms) => arms
                        .iter()
                        .any(|(_, _, s)| boundary(std::slice::from_ref(s))),
                    _ => false,
                })
            }
            *forbidden |= boundary(body);
            v05::expressions(body, &mut |e| {
                if let ExprKind::Closure(_, _, b) = &e.kind {
                    *forbidden |= boundary(b);
                }
            });
        };
        inspect(std::slice::from_ref(stmt), &mut pending, &mut forbidden);
        while let Some(n) = pending.pop() {
            if seen.insert(n.clone()) {
                inspect(&program.functions[&n].body, &mut pending, &mut forbidden);
            }
        }
        if forbidden {
            return Err(diagnostic(
                &stmt.at,
                "const cannot use checkpoint, cleanup, or runtime boundaries",
            ));
        }
        match engine
            .eval(expr)
            .and_then(|v| engine.define(name.clone(), v, false, annotation.clone(), &stmt.at))
        {
            Ok(()) => {}
            Err(Flow::Error(e)) => {
                return Err(diagnostic(&stmt.at, format!("ConstEvaluationFailed: {e}")))
            }
            Err(_) => {
                return Err(diagnostic(
                    &stmt.at,
                    "ConstEvaluationFailed: invalid control flow",
                ))
            }
        }
        if program_v09(program) {
            values.insert(
                name.clone(),
                v09::constant_digest(&engine, &engine.get(name).unwrap().value)?,
            );
        }
        checker.scopes[0].insert(name.clone(), (ty, false));
    }
    Ok(values)
}

pub(super) fn timeline(path: &Path, start: usize, count: usize, task: Option<u64>) -> Result<()> {
    println!("{}", timeline_data(path, start, count, task)?);
    Ok(())
}
pub(in crate::v2) fn timeline_data(
    path: &Path,
    start: usize,
    count: usize,
    task: Option<u64>,
) -> Result<Json> {
    if count > 1000 || fs::metadata(path)?.len() > 128 * 1024 * 1024 {
        return Err(Error::InvalidOperation("timeline budget exceeded".into()));
    }
    let trace: Json = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    let trace = v08::inspection_trace(trace)?;
    if !v08::readable_trace(&trace) {
        return Err(Error::InvalidOperation(
            "unsupported timeline trace format/compiler".into(),
        ));
    }
    let events = trace["events"]
        .as_array()
        .ok_or_else(|| Error::InvalidOperation("missing timeline events".into()))?;
    let mut rows = Vec::new();
    for (index, event) in events
        .iter()
        .enumerate()
        .skip(start)
        .filter(|(_, e)| task.is_none_or(|id| e["task"] == id))
        .take(count)
    {
        let before = v06::debug::view(&trace, index)?
            .ok_or_else(|| Error::InvalidOperation("timeline requires an indexed trace".into()))?;
        let after = v06::debug::view(&trace, index + 1)?
            .ok_or_else(|| Error::InvalidOperation("timeline requires an indexed trace".into()))?;
        rows.push(json!({"event":index,"instruction":event,"tasks":after["scheduler"]["tasks"],"files":v06::debug::delta(&before["runtime"]["file_deltas"],&after["runtime"]["file_deltas"])}));
    }
    Ok(
        json!({"format":1,"schema":"rewind-timeline/v1","events":rows,"result":trace["result"],"audit":trace["audit"]}),
    )
}
