use super::*;
#[derive(Clone)]
struct Owner {
    ty: String,
    transferable: bool,
    shareable: bool,
    mutable: bool,
    moved: bool,
    borrow: Option<(String, bool)>,
}
#[derive(Clone)]
struct Flow<'a> {
    program: &'a Program,
    bounds: BTreeMap<String, String>,
    origin: PathBuf,
    vars: BTreeMap<String, Owner>,
    checkpoints: BTreeMap<String, BTreeMap<String, Owner>>,
}
impl Flow<'_> {
    fn transfer_ty(&self, ty: &str, shared: bool) -> bool {
        transfer_bounded(self.program, ty, shared, &self.bounds)
    }
    fn shareable(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Name(n) => self.vars.get(n).map(|v| v.shareable).unwrap_or_else(|| {
                needed_globals(self.program, n).iter().all(|n| {
                    self.vars
                        .get(n)
                        .is_none_or(|v| v.shareable && !v.mutable && v.borrow.is_none())
                })
            }),
            ExprKind::Unary(op, v) if op == "move" => self.shareable(v),
            ExprKind::Closure(params, _, body) => free_names(body, params).iter().all(|n| {
                self.vars
                    .get(n)
                    .is_none_or(|v| v.shareable && !v.mutable && v.borrow.is_none())
            }),
            _ => self
                .checker()
                .expr(e)
                .is_ok_and(|t| self.transfer_ty(&t, true)),
        }
    }
    fn sendable(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Name(n) => self.vars.get(n).map(|v| v.transferable).unwrap_or_else(|| {
                needed_globals(self.program, n).iter().all(|n| {
                    self.vars
                        .get(n)
                        .is_none_or(|v| v.transferable && v.borrow.is_none())
                })
            }),
            ExprKind::Unary(op, v) if op == "move" => self.sendable(v),
            ExprKind::Closure(params, _, body) => free_names(body, params).iter().all(|n| {
                self.vars
                    .get(n)
                    .map(|v| v.transferable && v.borrow.is_none())
                    .unwrap_or_else(|| {
                        needed_globals(self.program, n).iter().all(|n| {
                            self.vars
                                .get(n)
                                .is_none_or(|v| v.transferable && v.borrow.is_none())
                        })
                    })
            }),
            ExprKind::NamedConstructor(_, fields) => fields.iter().all(|(_, v)| self.sendable(v)),
            ExprKind::Call(target, args) => {
                let ty = self.checker().expr(e).unwrap_or_default();
                self.transfer_ty(&ty, false)
                    && args.iter().all(|v| self.sendable(v))
                    && match &target.kind {
                        ExprKind::Member(base, _) => self.sendable(base),
                        _ => true,
                    }
            }
            _ => self
                .checker()
                .expr(e)
                .is_ok_and(|t| self.transfer_ty(&t, false)),
        }
    }
    fn checker(&self) -> Checker<'_> {
        Checker {
            program: self.program,
            scopes: vec![self
                .vars
                .iter()
                .map(|(n, v)| (n.clone(), (v.ty.clone(), true)))
                .collect()],
            return_ty: None,
            loop_depth: 0,
            bounds: self.bounds.clone(),
            origin: self.origin.clone(),
        }
    }
    fn borrowed(&self, n: &str) -> bool {
        self.vars
            .values()
            .any(|v| v.borrow.as_ref().is_some_and(|(target, _)| target == n))
    }
    fn use_name(&self, n: &str, at: &Tok) -> Result<()> {
        if self.vars.get(n).is_some_and(|v| v.moved) {
            return Err(diagnostic(
                at,
                format!("use after move: {n}; freeze or borrow before transferring ownership"),
            ));
        }
        if self.vars.values().any(|v| {
            v.borrow
                .as_ref()
                .is_some_and(|(target, m)| target == n && *m)
        }) {
            return Err(diagnostic(
                at,
                format!("{n} is exclusively borrowed; use the borrower within its scope"),
            ));
        }
        Ok(())
    }
    fn expr(&mut self, e: &Expr) -> Result<()> {
        match &e.kind {
            ExprKind::Name(n) => self.use_name(n, &e.at)?,
            ExprKind::Unary(op, v) if op == "move" => {
                let ExprKind::Name(n) = &v.kind else {
                    return Err(diagnostic(&e.at, "move requires a named owner"));
                };
                self.use_name(n, &e.at)?;
                if self.borrowed(n) {
                    return Err(diagnostic(&e.at, "cannot move a borrowed owner"));
                }
                if let Some(v) = self.vars.get_mut(n) {
                    if v.borrow.is_some() {
                        return Err(diagnostic(&e.at, "cannot move a borrowed value"));
                    }
                    v.moved = true;
                }
            }
            ExprKind::Unary(op, v) if matches!(op.as_str(), "borrow" | "borrowMut") => {
                let ExprKind::Name(n) = &v.kind else {
                    return Err(diagnostic(&e.at, "borrow requires a named owner"));
                };
                self.use_name(n, &e.at)?;
                if self.borrowed(n) && op == "borrowMut" {
                    return Err(diagnostic(
                        &e.at,
                        "mutable borrow conflicts with an existing borrow",
                    ));
                }
            }
            ExprKind::Unary(op, v) => {
                if op == "await"
                    && self
                        .vars
                        .values()
                        .any(|v| v.borrow.as_ref().is_some_and(|(_, m)| *m))
                {
                    return Err(diagnostic(
                        &e.at,
                        "mutable borrow cannot cross await; finish the borrow scope first",
                    ));
                }
                self.expr(v)?;
            }
            ExprKind::Member(v, _) | ExprKind::Try(v) => self.expr(v)?,
            ExprKind::Binary(_, a, b) => {
                self.expr(a)?;
                self.expr(b)?;
            }
            ExprKind::Call(target, args) => {
                if let ExprKind::Member(base, method) = &target.kind {
                    if let ExprKind::Name(n) = &base.kind {
                        if matches!(
                            method.as_str(),
                            "add"
                                | "push"
                                | "set"
                                | "remove"
                                | "write"
                                | "writeBytes"
                                | "seek"
                                | "close"
                                | "next"
                        ) {
                            if self.borrowed(n) {
                                return Err(diagnostic(
                                    &e.at,
                                    "cannot mutate an owner while borrowed",
                                ));
                            }
                            if self
                                .vars
                                .get(n)
                                .and_then(|v| v.borrow.as_ref())
                                .is_some_and(|(_, m)| !*m)
                            {
                                return Err(diagnostic(
                                    &e.at,
                                    "cannot mutate through a shared borrow",
                                ));
                            }
                        }
                    }
                }
                let function = if let ExprKind::Name(n) = &target.kind {
                    self.program.functions.get(&resolve_alias(self.program, n))
                } else {
                    None
                };
                let asynchronous = function.is_some_and(|f| f.asynchronous)
                    || self
                        .checker()
                        .expr(target)
                        .ok()
                        .and_then(|t| function_signature(&t))
                        .is_some_and(|(_, ret)| ret.starts_with("Task<"));
                let send = matches!(&target.kind,ExprKind::Member(_,m) if m=="send");
                for (index, arg) in args.iter().enumerate() {
                    if let Some(f) = function {
                        if let Some((_, expected)) = f.params.get(index) {
                            for (parameter, bound) in &f.type_params {
                                if bound.as_deref() == Some("Send")
                                    && substitute_type(
                                        expected,
                                        &BTreeMap::from([(parameter.clone(), "$owner".into())]),
                                    ) != *expected
                                    && !self.sendable(arg)
                                {
                                    return Err(diagnostic(&arg.at,"generic Send bound excludes resources and resource captures"));
                                }
                            }
                        }
                    }
                    if matches!(&arg.kind,ExprKind::Unary(op,_) if op=="borrow"||op=="borrowMut") {
                        return Err(diagnostic(
                            &arg.at,
                            "a lexical borrow cannot escape through a call; use freeze",
                        ));
                    }
                    if let ExprKind::Name(n) = &arg.kind {
                        if self.vars.get(n).is_some_and(|v| v.borrow.is_some()) {
                            return Err(diagnostic(
                                &arg.at,
                                "a lexical borrow cannot escape through a call; use freeze",
                            ));
                        }
                    }
                    if asynchronous || send {
                        let ty = self.checker().expr(arg)?;
                        if !self.sendable(arg) {
                            return Err(diagnostic(
                                &arg.at,
                                format!("{ty} is not Send; resources cannot cross task boundaries"),
                            ));
                        }
                        if !self.shareable(arg)
                            && !matches!(&arg.kind,ExprKind::Unary(op,_) if op=="move")
                            && !matches!(
                                &arg.kind,
                                ExprKind::Call(_, _) | ExprKind::NamedConstructor(_, _)
                            )
                        {
                            return Err(diagnostic(
                                &arg.at,
                                "task transfer requires move or Frozen<T>; use freeze(value)",
                            ));
                        }
                    } else if function.is_some()
                        && !self.shareable(arg)
                        && matches!(&arg.kind, ExprKind::Name(_) | ExprKind::Member(_, _))
                    {
                        return Err(diagnostic(
                            &arg.at,
                            "passing a mutable owner requires move or freeze",
                        ));
                    }
                    self.expr(arg)?;
                }
                self.expr(target)?;
                if let ExprKind::Member(base, method) = &target.kind {
                    if matches!(method.as_str(), "add" | "push" | "set")
                        && args.iter().any(|v| !self.sendable(v))
                    {
                        if let ExprKind::Name(n) = &base.kind {
                            if let Some(v) = self.vars.get_mut(n) {
                                v.transferable = false;
                            }
                        }
                    }
                }
            }
            ExprKind::Closure(params, _, body) => {
                let mut nested = self.clone();
                for (n, t) in params {
                    nested.vars.insert(
                        n.clone(),
                        Owner {
                            ty: t.clone(),
                            transferable: self.transfer_ty(t, false),
                            shareable: self.transfer_ty(t, true),
                            mutable: false,
                            moved: false,
                            borrow: None,
                        },
                    );
                }
                nested.body(body)?;
                for n in super::names(body) {
                    if self.vars.get(&n).is_some_and(|v| v.borrow.is_some()) {
                        return Err(diagnostic(&e.at, "closure cannot capture a lexical borrow"));
                    }
                }
            }
            ExprKind::Match(v, arms) => {
                let ty = self.checker().expr(v)?;
                self.expr(v)?;
                for (pattern, g, a) in arms {
                    let mut branch = self.clone();
                    let mut bindings = BTreeMap::new();
                    self.checker().pattern(pattern, &ty, &e.at, &mut bindings)?;
                    for (n, (ty, _)) in bindings {
                        branch.vars.insert(
                            n,
                            Owner {
                                transferable: self.transfer_ty(&ty, false),
                                shareable: self.transfer_ty(&ty, true),
                                mutable: false,
                                ty,
                                moved: false,
                                borrow: None,
                            },
                        );
                    }
                    if let Some(g) = g {
                        branch.expr(g)?;
                    }
                    branch.expr(a)?;
                    for (n, owner) in &mut self.vars {
                        if let Some(v) = branch.vars.get(n) {
                            owner.moved |= v.moved;
                            owner.transferable &= v.transferable;
                            owner.shareable &= v.shareable;
                        }
                    }
                }
            }
            ExprKind::NamedConstructor(_, fields) => {
                for (_, v) in fields {
                    self.expr(v)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn scope(&mut self, body: &[Stmt]) -> Result<()> {
        let before = self.vars.clone();
        let shadowed = body
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Let(n, _, _, _) | StmtKind::Using(n, _) => Some(n.clone()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let names = self.vars.keys().cloned().collect::<BTreeSet<_>>();
        self.body(body)?;
        self.vars.retain(|n, _| names.contains(n));
        for n in shadowed {
            if let Some(v) = before.get(&n) {
                self.vars.insert(n, v.clone());
            }
        }
        Ok(())
    }
    fn body(&mut self, body: &[Stmt]) -> Result<()> {
        for s in body {
            match &s.kind {
                StmtKind::Let(n, _, _, e) | StmtKind::Using(n, e) => {
                    let ty = self.checker().expr(e)?;
                    let transferable = self.sendable(e);
                    let shareable = self.shareable(e);
                    if let ExprKind::Name(name) = &e.kind {
                        if self.vars.get(name).is_some_and(|v| !v.shareable) {
                            return Err(diagnostic(
                                &e.at,
                                "mutable owner alias requires move, & / &mut, or freeze",
                            ));
                        }
                    }
                    self.expr(e)?;
                    let borrow = if let ExprKind::Unary(op, v) = &e.kind {
                        if matches!(op.as_str(), "borrow" | "borrowMut") {
                            if let ExprKind::Name(n) = &v.kind {
                                Some((n.clone(), op == "borrowMut"))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    self.vars.insert(
                        n.clone(),
                        Owner {
                            ty,
                            transferable,
                            shareable,
                            mutable: matches!(&s.kind, StmtKind::Let(_, true, _, _)),
                            moved: false,
                            borrow,
                        },
                    );
                }
                StmtKind::Assign(a, _, b) => {
                    let transferable = self.sendable(b);
                    let shareable = self.shareable(b);
                    if let ExprKind::Member(base, _) = &a.kind {
                        if let ExprKind::Name(n) = &base.kind {
                            if self.borrowed(n)
                                || self
                                    .vars
                                    .get(n)
                                    .and_then(|v| v.borrow.as_ref())
                                    .is_some_and(|(_, m)| !*m)
                            {
                                return Err(diagnostic(
                                    &a.at,
                                    "cannot mutate a field while its owner is shared borrowed",
                                ));
                            }
                        }
                    }
                    if let ExprKind::Name(n) = &a.kind {
                        if self.borrowed(n) {
                            return Err(diagnostic(&a.at, "cannot assign an owner while borrowed"));
                        }
                    }
                    if let ExprKind::Name(n) = &b.kind {
                        if self.vars.get(n).is_some_and(|v| !v.shareable) {
                            return Err(diagnostic(
                                &b.at,
                                "assignment of a mutable owner requires move or freeze",
                            ));
                        }
                    }
                    self.expr(a)?;
                    self.expr(b)?;
                    if let ExprKind::Name(n) = &a.kind {
                        if let Some(owner) = self.vars.get_mut(n) {
                            owner.transferable = transferable;
                            owner.shareable = shareable;
                            owner.moved = false;
                        }
                    }
                }
                StmtKind::Expr(e) | StmtKind::Defer(e) => self.expr(e)?,
                StmtKind::Return(Some(e)) => {
                    if self.checker().expr(e).is_ok_and(|t| t.starts_with("fn("))
                        && !self.sendable(e)
                    {
                        return Err(diagnostic(&e.at,"closure containing resources or borrows cannot escape; capture a transferable snapshot"));
                    }
                    if let ExprKind::Name(n) = &e.kind {
                        if self.vars.get(n).is_some_and(|v| v.borrow.is_some()) {
                            return Err(diagnostic(
                                &e.at,
                                "borrow cannot escape its lexical scope",
                            ));
                        }
                    }
                    self.expr(e)?;
                }
                StmtKind::Block(b) => self.scope(b)?,
                StmtKind::Branch(_, b) => {
                    let old = self.clone();
                    self.scope(b)?;
                    *self = old;
                }
                StmtKind::If(e, a, b) => {
                    self.expr(e)?;
                    let mut left = self.clone();
                    left.scope(a)?;
                    let mut right = self.clone();
                    right.scope(b)?;
                    for (n, v) in &mut self.vars {
                        v.moved = left.vars.get(n).is_some_and(|v| v.moved)
                            || right.vars.get(n).is_some_and(|v| v.moved);
                        v.transferable &= left.vars.get(n).is_none_or(|v| v.transferable)
                            && right.vars.get(n).is_none_or(|v| v.transferable);
                        v.shareable &= left.vars.get(n).is_none_or(|v| v.shareable)
                            && right.vars.get(n).is_none_or(|v| v.shareable);
                    }
                }
                StmtKind::While(e, b) => {
                    self.expr(e)?;
                    let old = self.vars.clone();
                    self.scope(b)?;
                    if self
                        .vars
                        .iter()
                        .any(|(n, v)| v.moved && !old.get(n).is_some_and(|v| v.moved))
                    {
                        return Err(diagnostic(
                            &s.at,
                            "moving an outer owner inside a repeated loop is not allowed",
                        ));
                    }
                }
                StmtKind::For(n, a, b, stmts) => {
                    let old = self.vars.clone();
                    self.expr(a)?;
                    self.expr(b)?;
                    self.vars.insert(
                        n.clone(),
                        Owner {
                            ty: "Int".into(),
                            transferable: true,
                            shareable: true,
                            mutable: false,
                            moved: false,
                            borrow: None,
                        },
                    );
                    self.scope(stmts)?;
                    self.vars.remove(n);
                    if self
                        .vars
                        .iter()
                        .any(|(n, v)| v.moved && !old.get(n).is_some_and(|v| v.moved))
                    {
                        return Err(diagnostic(
                            &s.at,
                            "moving an outer owner inside a repeated loop is not allowed",
                        ));
                    }
                    if let Some(v) = old.get(n) {
                        self.vars.insert(n.clone(), v.clone());
                    }
                }
                StmtKind::Match(e, arms) => {
                    let ty = self.checker().expr(e)?;
                    self.expr(e)?;
                    for (p, g, b) in arms {
                        let mut branch = self.clone();
                        let mut bindings = BTreeMap::new();
                        self.checker().pattern(p, &ty, &s.at, &mut bindings)?;
                        for (n, (t, _)) in bindings {
                            branch.vars.insert(
                                n,
                                Owner {
                                    transferable: self.transfer_ty(&t, false),
                                    shareable: self.transfer_ty(&t, true),
                                    mutable: false,
                                    ty: t,
                                    moved: false,
                                    borrow: None,
                                },
                            );
                        }
                        if let Some(g) = g {
                            branch.expr(g)?;
                        }
                        branch.scope(std::slice::from_ref(b))?;
                        for (n, v) in &branch.vars {
                            if let Some(owner) = self.vars.get_mut(n) {
                                owner.transferable &= v.transferable;
                                owner.shareable &= v.shareable;
                            }
                            if v.moved {
                                if let Some(v) = self.vars.get_mut(n) {
                                    v.moved = true;
                                }
                            }
                        }
                    }
                }
                StmtKind::Commit(n) => {
                    self.checkpoints.insert(n.clone(), self.vars.clone());
                }
                StmtKind::Revert(n) | StmtKind::Resume(n) => {
                    if let Some(vars) = self.checkpoints.get(n) {
                        self.vars = vars.clone();
                    }
                }
                StmtKind::Drop(n) => {
                    self.checkpoints.remove(n);
                }
                _ => {}
            }
        }
        Ok(())
    }
}
pub(super) fn validate(program: &Program) -> Result<()> {
    let mut flow = Flow {
        program,
        bounds: BTreeMap::new(),
        origin: program.root_origin.clone(),
        vars: BTreeMap::new(),
        checkpoints: BTreeMap::new(),
    };
    flow.body(&program.stmts)?;
    for (function_name, f) in &program.functions {
        let mut flow = flow.clone();
        flow.bounds = f
            .type_params
            .iter()
            .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
            .collect();
        flow.origin = f.origin.clone();
        for (n, ty) in &f.params {
            flow.vars.insert(
                n.clone(),
                Owner {
                    ty: ty.clone(),
                    transferable: flow.transfer_ty(ty, false),
                    shareable: flow.transfer_ty(ty, true),
                    mutable: false,
                    moved: false,
                    borrow: None,
                },
            );
        }
        flow.body(&f.body)?;
        if f.asynchronous {
            if !flow.transfer_ty(&f.ret, false) {
                return Err(diagnostic(
                    &f.at,
                    format!("async return {} is not Send", f.ret),
                ));
            }
            for n in needed_globals(program, function_name) {
                if let Some(v) = flow.vars.get(&n) {
                    if !v.transferable || v.borrow.is_some() {
                        return Err(diagnostic(
                            &f.at,
                            format!(
                                "async capture {n}: {} is not Send; pass a transferable snapshot",
                                v.ty
                            ),
                        ));
                    }
                }
            }
            for (_, t) in &f.params {
                if !flow.transfer_ty(t, false) {
                    return Err(diagnostic(
                        &f.at,
                        format!("async parameter {t} is not Send"),
                    ));
                }
            }
        }
    }
    Ok(())
}
