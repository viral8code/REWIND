use super::*;

pub(in crate::v2) fn destructure(
    p: &Pattern,
    value: Expr,
    mutable: bool,
    at: &Tok,
    out: &mut Vec<Stmt>,
) -> Result<()> {
    fn shape(p: &Pattern) -> Pattern {
        match p {
            Pattern::Variant(n, parts) if n == "$tuple" => {
                Pattern::Variant(n.clone(), parts.iter().map(shape).collect())
            }
            _ => Pattern::Wildcard,
        }
    }
    fn bind(
        p: &Pattern,
        value: Expr,
        mutable: bool,
        at: &Tok,
        out: &mut Vec<Stmt>,
        names: &mut BTreeSet<String>,
    ) -> Result<()> {
        match p {
            Pattern::Wildcard => {}
            Pattern::Bind(n) => {
                if !names.insert(n.clone()) {
                    return Err(diagnostic(at, format!("duplicate pattern binding {n}")));
                }
                out.push(Stmt {
                    kind: StmtKind::Let(
                        n.clone(),
                        mutable,
                        None,
                        Expr {
                            kind: ExprKind::Unary("move".into(), Box::new(value)),
                            at: at.clone(),
                        },
                    ),
                    at: at.clone(),
                });
            }
            Pattern::Variant(n, parts) if n == "$tuple" => {
                for (i, p) in parts.iter().enumerate() {
                    bind(
                        p,
                        Expr {
                            kind: ExprKind::Member(Box::new(value.clone()), format!("_{i}")),
                            at: at.clone(),
                        },
                        mutable,
                        at,
                        out,
                        names,
                    )?;
                }
            }
            _ => {
                return Err(diagnostic(
                    at,
                    "let tuple pattern must contain bindings, tuples, or _",
                ))
            }
        }
        Ok(())
    }
    out.push(Stmt {
        kind: StmtKind::Expr(Expr {
            kind: ExprKind::Match(
                Box::new(value.clone()),
                vec![(
                    shape(p),
                    None,
                    Expr {
                        kind: ExprKind::Value(Value::Null),
                        at: at.clone(),
                    },
                )],
            ),
            at: at.clone(),
        }),
        at: at.clone(),
    });
    bind(p, value, mutable, at, out, &mut BTreeSet::new())
}

pub(in crate::v2) fn rename_function(f: &mut Function, names: &BTreeMap<String, String>) {
    for (_, ty) in &mut f.params {
        *ty = rename_type(ty, names);
    }
    f.ret = rename_type(&f.ret, names);
    for (_, bound) in &mut f.type_params {
        if let Some(b) = bound {
            *b = rename_type(b, names);
        }
    }
    let mut locals = f
        .params
        .iter()
        .map(|(n, _)| n.clone())
        .collect::<BTreeSet<_>>();
    collect_local_bindings(&f.body, &mut locals);
    let names = names
        .iter()
        .filter(|(n, _)| !locals.contains(*n))
        .map(|(n, t)| (n.clone(), t.clone()))
        .collect();
    for stmt in &mut f.body {
        rename_stmt(stmt, &names);
    }
}

struct Expand<'a> {
    program: &'a Program,
    origin: &'a Path,
    at: &'a Tok,
}
impl Expand<'_> {
    fn ty(&self, ty: &str, depth: usize) -> Result<String> {
        if depth >= 64 {
            return Err(diagnostic(self.at, "TypeExpansionBudgetExceeded: aliases"));
        }
        for prefix in ["&mut ", "&"] {
            if let Some(t) = ty.strip_prefix(prefix) {
                return Ok(format!("{prefix}{}", self.ty(t, depth + 1)?));
            }
        }
        if let Some((args, ret)) = function_signature(ty) {
            let args = args
                .iter()
                .map(|t| self.ty(t, depth + 1))
                .collect::<Result<Vec<_>>>()?;
            let suffix = &ty[strip_effects(ty).len()..];
            return Ok(format!(
                "fn({})->{}{suffix}",
                args.join(","),
                self.ty(&ret, depth + 1)?
            ));
        }
        let (base, args) = if let Some((base, inner)) = ty.split_once('<') {
            (
                base,
                split_type_args(outer_type_end(inner))
                    .iter()
                    .map(|t| self.ty(t, depth + 1))
                    .collect::<Result<Vec<_>>>()?,
            )
        } else {
            (ty, Vec::new())
        };
        let base = resolve_alias(self.program, base);
        if let Some(alias) = self.program.aliases.get(&base) {
            let checker = Checker {
                program: self.program,
                scopes: Vec::new(),
                return_ty: None,
                loop_depth: 0,
                bounds: BTreeMap::new(),
                origin: self.origin.into(),
            };
            checker.visible(alias.public, &alias.origin, self.at, &base)?;
            if args.len() != alias.params.len() {
                return Err(diagnostic(
                    self.at,
                    format!("type alias {base} expects {} arguments", alias.params.len()),
                ));
            }
            let map = alias.params.iter().cloned().zip(args).collect();
            return Expand {
                program: self.program,
                origin: &alias.origin,
                at: &alias.at,
            }
            .ty(&substitute_type(&alias.ty, &map), depth + 1);
        }
        Ok(if args.is_empty() {
            base
        } else {
            format!("{base}<{}>", args.join(","))
        })
    }
    fn expr(&self, e: &mut Expr) -> Result<()> {
        match &mut e.kind {
            ExprKind::Unary(_, e) | ExprKind::Member(e, _) | ExprKind::Try(e) => self.expr(e)?,
            ExprKind::Binary(_, a, b) => {
                self.expr(a)?;
                self.expr(b)?;
            }
            ExprKind::Call(target, args) => {
                self.expr(target)?;
                for a in args {
                    self.expr(a)?;
                }
            }
            ExprKind::Name(n) if n.contains('<') => {
                *n = self.ty(n, 0)?;
            }
            ExprKind::NamedConstructor(n, fields) => {
                *n = self.ty(n, 0)?;
                for (_, e) in fields {
                    self.expr(e)?;
                }
            }
            ExprKind::Closure(params, ret, body) => {
                for (_, t) in params {
                    *t = self.ty(t, 0)?;
                }
                *ret = self.ty(ret, 0)?;
                self.body(body)?;
            }
            ExprKind::Match(e, arms) => {
                self.expr(e)?;
                for (_, guard, e) in arms {
                    if let Some(g) = guard {
                        self.expr(g)?;
                    }
                    self.expr(e)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn body(&self, body: &mut [Stmt]) -> Result<()> {
        for s in body {
            match &mut s.kind {
                StmtKind::Let(_, _, ty, e) => {
                    if let Some(t) = ty {
                        *t = self.ty(t, 0)?;
                    }
                    self.expr(e)?;
                }
                StmtKind::Expr(e)
                | StmtKind::Using(_, e)
                | StmtKind::Defer(e)
                | StmtKind::Return(Some(e)) => self.expr(e)?,
                StmtKind::Assign(a, _, b) => {
                    self.expr(a)?;
                    self.expr(b)?;
                }
                StmtKind::External(_, b) | StmtKind::Block(b) | StmtKind::Branch(_, b) => {
                    self.body(b)?
                }
                StmtKind::If(e, a, b) => {
                    self.expr(e)?;
                    self.body(a)?;
                    self.body(b)?;
                }
                StmtKind::While(e, b) => {
                    self.expr(e)?;
                    self.body(b)?;
                }
                StmtKind::For(_, a, b, body) => {
                    self.expr(a)?;
                    self.expr(b)?;
                    self.body(body)?;
                }
                StmtKind::Match(e, arms) => {
                    self.expr(e)?;
                    for (_, guard, b) in arms {
                        if let Some(g) = guard {
                            self.expr(g)?;
                        }
                        self.body(std::slice::from_mut(b))?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn function(&self, f: &mut Function) -> Result<()> {
        for (_, t) in &mut f.params {
            *t = self.ty(t, 0)?;
        }
        f.ret = self.ty(&f.ret, 0)?;
        self.body(&mut f.body)
    }
}

pub(in crate::v2) fn prepare(program: &mut Program) -> Result<()> {
    let uses_defaults = program.traits.values().any(|tr| !tr.defaults.is_empty());
    if !matches!(
        program.language.as_str(),
        "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
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
            | "1.9.60"
            | "1.9.61"
            | "1.9.62"
            | "1.9.63"
            | "2.0.0"
    ) {
        if !program.aliases.is_empty() || uses_defaults {
            return Err(Error::InvalidOperation(
                "type aliases and default trait methods require language 0.6".into(),
            ));
        }
        return Ok(());
    }
    let snapshot = program.clone();
    for (n, alias) in &snapshot.aliases {
        if snapshot.structs.contains_key(n)
            || snapshot.enums.contains_key(n)
            || snapshot.traits.contains_key(n)
            || matches!(
                n.as_str(),
                "Bool"
                    | "Int"
                    | "Float"
                    | "String"
                    | "Bytes"
                    | "Unit"
                    | "Tuple"
                    | "Frozen"
                    | "Secret"
                    | "Task"
                    | "Channel"
                    | "Iterator"
                    | "Option"
                    | "Result"
            )
        {
            return Err(diagnostic(
                &alias.at,
                format!("type alias conflicts with type {n}"),
            ));
        }
        Expand {
            program: &snapshot,
            origin: &alias.origin,
            at: &alias.at,
        }
        .ty(&alias.ty, 0)?;
    }
    for f in program.functions.values_mut() {
        Expand {
            program: &snapshot,
            origin: &f.origin.clone(),
            at: &f.at.clone(),
        }
        .function(f)?;
    }
    for (stmt, origin) in program.stmts.iter_mut().zip(&program.stmt_origins) {
        Expand {
            program: &snapshot,
            origin,
            at: &stmt.at.clone(),
        }
        .body(std::slice::from_mut(stmt))?;
    }
    for def in program.structs.values_mut() {
        let at = Tok {
            source: String::new(),
            text: String::new(),
            line: 1,
            col: 1,
        };
        let ex = Expand {
            program: &snapshot,
            origin: &def.origin,
            at: &at,
        };
        for (_, t) in &mut def.fields {
            *t = ex.ty(t, 0)?;
        }
    }
    for def in program.enums.values_mut() {
        let at = Tok {
            source: String::new(),
            text: String::new(),
            line: 1,
            col: 1,
        };
        let ex = Expand {
            program: &snapshot,
            origin: &def.origin,
            at: &at,
        };
        for fields in def.variants.values_mut() {
            for (_, t) in fields {
                *t = ex.ty(t, 0)?;
            }
        }
    }
    for tr in program.traits.values_mut() {
        let at = Tok {
            source: String::new(),
            text: String::new(),
            line: 1,
            col: 1,
        };
        let ex = Expand {
            program: &snapshot,
            origin: &tr.origin,
            at: &at,
        };
        for (args, ret) in tr.methods.values_mut() {
            for t in args {
                *t = ex.ty(t, 0)?;
            }
            *ret = ex.ty(ret, 0)?;
        }
        for f in tr.defaults.values_mut() {
            ex.function(f)?;
        }
    }
    let mut impls = BTreeMap::new();
    let mut associated = BTreeMap::new();
    let mut generics = BTreeMap::new();
    let mut origins = BTreeMap::new();
    for (key, methods) in std::mem::take(&mut program.impls) {
        let at = methods
            .values()
            .find_map(|n| program.functions.get(n))
            .map(|f| f.at.clone())
            .unwrap_or(Tok {
                source: String::new(),
                text: String::new(),
                line: 1,
                col: 1,
            });
        let origin = methods
            .values()
            .find_map(|n| program.functions.get(n))
            .map(|f| f.origin.clone())
            .unwrap_or_else(|| program.root_origin.clone());
        let ex = Expand {
            program: &snapshot,
            origin: &origin,
            at: &at,
        };
        let resolved = (ex.ty(&key.0, 0)?, ex.ty(&key.1, 0)?);
        if impls.insert(resolved.clone(), methods).is_some() {
            return Err(diagnostic(&at, "duplicate impl after type alias expansion"));
        }
        if let Some(values) = program.impl_associated.remove(&key) {
            associated.insert(
                resolved.clone(),
                values
                    .into_iter()
                    .map(|(n, t)| Ok((n, ex.ty(&t, 0)?)))
                    .collect::<Result<BTreeMap<_, _>>>()?,
            );
        }
        if let Some(origin) = program.impl_origins.remove(&key) {
            origins.insert(resolved.clone(), origin);
        }
        if let Some(params) = program.impl_generics.remove(&key) {
            generics.insert(resolved, params);
        }
    }
    program.impls = impls;
    program.impl_associated = associated;
    program.impl_generics = generics;
    program.impl_origins = origins;
    for (name, tr) in &program.traits {
        for (method, template) in &tr.defaults {
            let mut f = template.clone();
            f.type_params = vec![("Self".into(), Some(name.clone()))];
            program
                .functions
                .insert(format!("$traitDefault${name}${method}"), f);
        }
    }
    for (key, methods) in &mut program.impls {
        let Some(tr) = program.traits.get(&key.0) else {
            continue;
        };
        let mut substitutions = BTreeMap::from([("Self".into(), key.1.clone())]);
        if let Some(associated) = program.impl_associated.get(key) {
            for (n, t) in associated {
                substitutions.insert(format!("Self::{n}"), t.clone());
            }
        }
        for (method, template) in &tr.defaults {
            if methods.contains_key(method) {
                continue;
            }
            let mut f = template.clone();
            f.type_params = program.impl_generics.get(key).cloned().unwrap_or_default();
            rename_function(&mut f, &substitutions);
            let symbol = format!("$default${}${}${method}", key.0, key.1);
            methods.insert(method.clone(), symbol.clone());
            program.functions.insert(symbol, f);
        }
    }
    Ok(())
}
