use super::*;
use rewind::BranchAnchor;
mod extensions;
mod scheduler;
use scheduler::Scheduler;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
enum Op {
    ApplicationEntry(usize),
    Push(Value),
    Load(String),
    Declare(String, bool, Option<String>),
    Store(String, String),
    StoreField(String, String),
    Unary(String),
    Await,
    Spawn,
    Binary(String),
    Property(String),
    Call(String, usize),
    CallValue(usize),
    CallNamed(String, Vec<String>),
    MakeClosure(String, String),
    Method(String, usize),
    Builtin(String, String, usize),
    Try,
    Pop,
    Enter,
    EnterExternal(bool),
    ExitExternal,
    Exit,
    Jump(usize),
    JumpFalse(usize),
    JumpTrue(usize),
    Return,
    Defer(Expr),
    DeferValue,
    Using(String),
    Commit(String),
    Revert(String),
    Resume(String),
    Drop(String),
    Publish(bool),
    BeginBranch,
    EndBranch(String),
    Runtime(ResourceBudget, Option<usize>),
    PatternTest(Pattern, usize),
    BorrowPatternTest(Pattern, usize),
    FailMatch,
    Halt,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Inst {
    op: Op,
    at: Tok,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct FunctionChunk {
    code: Vec<Inst>,
    closures: BTreeMap<String, Function>,
    hidden: usize,
    closure_typing: BTreeMap<String, Vec<BTreeMap<String, (String, bool)>>>,
}
fn relocate(code: &mut [Inst], base: usize, relative: bool) {
    for inst in code {
        match &mut inst.op {
            Op::ApplicationEntry(to)
            | Op::Jump(to)
            | Op::JumpFalse(to)
            | Op::JumpTrue(to)
            | Op::PatternTest(_, to)
            | Op::BorrowPatternTest(_, to) => {
                *to = if relative {
                    to.saturating_sub(base)
                } else {
                    to.saturating_add(base)
                };
            }
            _ => {}
        }
    }
}
struct LoopPatch {
    breaks: Vec<usize>,
    continues: Vec<usize>,
    depth: usize,
    continue_target: Option<usize>,
}
struct Compiler {
    code: Vec<Inst>,
    functions: BTreeMap<String, usize>,
    loops: Vec<LoopPatch>,
    depth: usize,
    hidden: usize,
    closure_defs: BTreeMap<String, Function>,
    aliases: BTreeMap<String, String>,
    type_params: Vec<(String, Option<String>)>,
    typing_program: Program,
    typing_scopes: Vec<BTreeMap<String, (String, bool)>>,
    typing_origin: PathBuf,
    typing_key: String,
    closure_typing: BTreeMap<String, Vec<BTreeMap<String, (String, bool)>>>,
}
impl Compiler {
    fn expression_type(&self, e: &Expr) -> Option<String> {
        if !matches!(
            self.typing_program.language.as_str(),
            "0.9.2"
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
                | "1.9.0"
                | "2.0.0"
        ) {
            return None;
        }
        let checker = Checker {
            program: &self.typing_program,
            scopes: self.typing_scopes.clone(),
            return_ty: None,
            loop_depth: 0,
            bounds: self
                .type_params
                .iter()
                .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                .collect(),
            origin: self.typing_origin.clone(),
        };
        checker.expr(e).ok()
    }
    fn call_name(&self, name: &str, args: &[Expr], at: &Tok) -> String {
        if !matches!(
            self.typing_program.language.as_str(),
            "0.9.2"
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
                | "1.9.0"
                | "2.0.0"
        ) || name.contains('<')
        {
            return name.into();
        }
        let Some(f) = self.typing_program.functions.get(name) else {
            return name.into();
        };
        if f.type_params.is_empty() {
            return name.into();
        }
        let Some(types) = args
            .iter()
            .map(|e| self.expression_type(e))
            .collect::<Option<Vec<_>>>()
        else {
            return name.into();
        };
        let params = f
            .type_params
            .iter()
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        let Ok(sub) = infer_call_arguments(name, &f.params, &params, &types, at) else {
            return name.into();
        };
        format!(
            "{name}<{}>",
            params
                .iter()
                .map(|p| sub[p].clone())
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    fn pattern_types(&mut self, pattern: &Pattern, ty: Option<&str>, at: &Tok) {
        let Some(ty) = ty else {
            return;
        };
        let checker = Checker {
            program: &self.typing_program,
            scopes: self.typing_scopes.clone(),
            return_ty: None,
            loop_depth: 0,
            bounds: self
                .type_params
                .iter()
                .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                .collect(),
            origin: self.typing_origin.clone(),
        };
        let mut bindings = BTreeMap::new();
        if checker
            .pattern(
                &resolve_pattern_alias(pattern, &self.aliases),
                ty,
                at,
                &mut bindings,
            )
            .is_ok()
        {
            self.typing_scopes.last_mut().unwrap().extend(bindings);
        }
    }
    fn aliased(&self, name: &str) -> String {
        if let Some((head, tail)) = name.split_once("::") {
            let base = head.split('<').next().unwrap_or(head);
            if let Some(alias) = self.aliases.get(base) {
                return format!("{}{}::{tail}", alias, &head[base.len()..]);
            }
        }
        if let Some((head, tail)) = name.split_once('<') {
            if let Some(alias) = self.aliases.get(head) {
                return format!("{alias}<{tail}");
            }
        }
        self.aliases
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.into())
    }
    fn new() -> Self {
        Self {
            code: Vec::new(),
            functions: BTreeMap::new(),
            loops: Vec::new(),
            depth: 0,
            hidden: 0,
            closure_defs: BTreeMap::new(),
            aliases: BTreeMap::new(),
            typing_program: Program::default(),
            typing_scopes: vec![BTreeMap::new()],
            typing_origin: PathBuf::new(),
            typing_key: String::new(),
            closure_typing: BTreeMap::new(),
            type_params: Vec::new(),
        }
    }
    fn emit(&mut self, op: Op, at: &Tok) -> usize {
        let pos = self.code.len();
        self.code.push(Inst { op, at: at.clone() });
        pos
    }
    fn patch(&mut self, index: usize, target: usize) {
        match &mut self.code[index].op {
            Op::ApplicationEntry(to)
            | Op::Jump(to)
            | Op::JumpFalse(to)
            | Op::JumpTrue(to)
            | Op::PatternTest(_, to)
            | Op::BorrowPatternTest(_, to) => *to = target,
            _ => unreachable!(),
        }
    }
    fn enter(&mut self, at: &Tok) {
        self.typing_scopes.push(BTreeMap::new());
        self.emit(Op::Enter, at);
        self.depth += 1;
    }
    fn exit(&mut self, at: &Tok) {
        self.typing_scopes.pop();
        self.emit(Op::Exit, at);
        self.depth -= 1;
    }
    fn block(&mut self, body: &[Stmt], at: &Tok) -> Result<()> {
        self.enter(at);
        for stmt in body {
            self.stmt(stmt)?;
        }
        self.exit(at);
        Ok(())
    }
    fn compile(program: &Program) -> Result<Self> {
        let mut c = Self::new();
        c.aliases = program.import_aliases.clone();
        c.typing_program = program.clone();
        c.typing_origin = program.root_origin.clone();
        for stmt in &program.stmts {
            let guard = if matches!(
                program.language.as_str(),
                "0.9.1"
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
                    | "1.9.0"
                    | "2.0.0"
            ) && stmt.at.text == "@application-entry"
            {
                Some(c.emit(Op::ApplicationEntry(0), &stmt.at))
            } else {
                None
            };
            c.stmt(stmt)?;
            if let Some(guard) = guard {
                c.patch(guard, c.code.len());
            }
        }
        let end = program.stmts.last().map(|s| s.at.clone()).unwrap_or(Tok {
            source: String::new(),
            text: "<eof>".into(),
            line: 1,
            col: 1,
        });
        c.emit(Op::Halt, &end);
        if matches!(
            program.language.as_str(),
            "0.9.3"
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
                | "1.9.0"
                | "2.0.0"
        ) {
            let signatures = program
                .functions
                .iter()
                .map(|(name, f)| {
                    (
                        name,
                        &f.params,
                        &f.type_params,
                        &f.ret,
                        &f.effects,
                        f.asynchronous,
                    )
                })
                .collect::<Vec<_>>();
            c.typing_key = v06::cache::key(&(
                &program.structs,
                &program.enums,
                &program.aliases,
                &program.traits,
                &program.impls,
                &program.consts,
                &c.typing_scopes,
                signatures,
            ))
            .unwrap_or_default();
        }
        for (name, f) in &program.functions {
            c.functions.insert(name.clone(), c.code.len());
            c.compile_function(name, f, program)?;
        }
        let mut compiled = BTreeSet::new();
        loop {
            let pending = c
                .closure_defs
                .iter()
                .filter(|(name, _)| !compiled.contains(*name))
                .map(|(name, f)| (name.clone(), f.clone()))
                .collect::<Vec<_>>();
            if pending.is_empty() {
                break;
            }
            for (name, f) in pending {
                compiled.insert(name.clone());
                c.functions.insert(name.clone(), c.code.len());
                c.compile_function(&name, &f, program)?;
            }
        }
        Ok(c)
    }
    fn compile_function(&mut self, name: &str, f: &Function, program: &Program) -> Result<()> {
        let root = program.root_origin.parent().unwrap_or(Path::new("."));
        let key = v06::cache::key(&(
            f,
            &self.aliases,
            self.hidden,
            &program.language,
            (program.language == "0.9.2").then_some(program),
            &self.typing_key,
            self.closure_typing.get(name),
        ))
        .unwrap_or_default();
        let base = self.code.len();
        if matches!(
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
                | "1.9.0"
                | "2.0.0"
        ) {
            if let Some(mut chunk) = v06::cache::get::<FunctionChunk>(root, "compiled", &key) {
                relocate(&mut chunk.code, base, false);
                self.code.extend(chunk.code);
                self.closure_defs.extend(chunk.closures);
                self.closure_typing.extend(chunk.closure_typing);
                self.hidden = chunk.hidden;
                return Ok(());
            }
        }
        let before = self.closure_defs.keys().cloned().collect::<BTreeSet<_>>();
        let typing_scopes = self.typing_scopes.clone();
        let typing_origin = self.typing_origin.clone();
        if let Some(context) = self.closure_typing.get(name) {
            self.typing_scopes = context.clone();
        } else {
            self.typing_scopes.truncate(1);
        }
        self.typing_scopes.push(
            f.params
                .iter()
                .map(|(n, t)| (n.clone(), (t.clone(), false)))
                .collect(),
        );
        self.typing_origin = f.origin.clone();
        self.type_params = f.type_params.clone();
        self.depth = 1;
        for stmt in &f.body {
            self.stmt(stmt)?;
        }
        self.emit(Op::Push(Value::Null), &f.at);
        self.emit(Op::Return, &f.at);
        self.depth = 0;
        self.typing_scopes = typing_scopes;
        self.typing_origin = typing_origin;
        if matches!(
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
                | "1.9.0"
                | "2.0.0"
        ) {
            let mut code = self.code[base..].to_vec();
            relocate(&mut code, base, true);
            let closures = self
                .closure_defs
                .iter()
                .filter(|(n, _)| !before.contains(*n))
                .map(|(n, f)| (n.clone(), f.clone()))
                .collect();
            v06::cache::put(
                root,
                "compiled",
                &key,
                &FunctionChunk {
                    code,
                    closures,
                    hidden: self.hidden,
                    closure_typing: self.closure_typing.clone(),
                },
            );
        }
        Ok(())
    }
    fn stmt(&mut self, stmt: &Stmt) -> Result<()> {
        let at = &stmt.at;
        match &stmt.kind {
            StmtKind::Let(name, mutable, ty, value) => {
                let inferred = ty
                    .as_ref()
                    .map(|ty| rename_type(ty, &self.aliases))
                    .or_else(|| self.expression_type(value));
                self.expr(value)?;
                if let Some(ty) = inferred {
                    self.typing_scopes
                        .last_mut()
                        .unwrap()
                        .insert(name.clone(), (ty, *mutable));
                }
                self.emit(
                    Op::Declare(
                        name.clone(),
                        *mutable,
                        ty.as_ref().map(|ty| rename_type(ty, &self.aliases)),
                    ),
                    at,
                );
            }
            StmtKind::Using(name, expr) => {
                self.expr(expr)?;
                self.emit(Op::Declare(name.clone(), false, None), at);
                self.emit(Op::Using(name.clone()), at);
            }
            StmtKind::Assign(lhs, op, rhs) => match &lhs.kind {
                ExprKind::Name(name) => {
                    self.expr(rhs)?;
                    self.emit(Op::Store(name.clone(), op.clone()), at);
                }
                ExprKind::Member(base, field) => {
                    self.expr(base)?;
                    self.expr(rhs)?;
                    self.emit(Op::StoreField(field.clone(), op.clone()), at);
                }
                _ => return Err(diagnostic(at, "invalid assignment target")),
            },
            StmtKind::Expr(e) => {
                self.expr(e)?;
                self.emit(Op::Pop, at);
            }
            StmtKind::External(fresh, body) => {
                self.emit(Op::EnterExternal(*fresh), at);
                self.block(body, at)?;
                self.emit(Op::ExitExternal, at);
            }
            StmtKind::Block(body) => self.block(body, at)?,
            StmtKind::If(cond, yes, no) => {
                self.expr(cond)?;
                let false_jump = self.emit(Op::JumpFalse(0), at);
                self.block(yes, at)?;
                let end_jump = self.emit(Op::Jump(0), at);
                self.patch(false_jump, self.code.len());
                self.block(no, at)?;
                self.patch(end_jump, self.code.len());
            }
            StmtKind::While(cond, body) => {
                let start = self.code.len();
                self.expr(cond)?;
                let end_jump = self.emit(Op::JumpFalse(0), at);
                self.loops.push(LoopPatch {
                    breaks: Vec::new(),
                    continues: Vec::new(),
                    depth: self.depth,
                    continue_target: Some(start),
                });
                self.block(body, at)?;
                self.emit(Op::Jump(start), at);
                let end = self.code.len();
                self.patch(end_jump, end);
                let loop_ctx = self.loops.pop().unwrap();
                for jump in loop_ctx.breaks {
                    self.patch(jump, end);
                }
                for jump in loop_ctx.continues {
                    self.patch(jump, start);
                }
            }
            StmtKind::For(name, start, end, body) => {
                self.hidden += 1;
                let current = format!("$for{}current", self.hidden);
                let limit = format!("$for{}limit", self.hidden);
                self.enter(at);
                self.expr(start)?;
                self.emit(Op::Declare(current.clone(), true, Some("Int".into())), at);
                self.expr(end)?;
                self.emit(Op::Declare(limit.clone(), false, Some("Int".into())), at);
                let cond = self.code.len();
                self.emit(Op::Load(current.clone()), at);
                self.emit(Op::Load(limit), at);
                self.emit(Op::Binary("<".into()), at);
                let end_jump = self.emit(Op::JumpFalse(0), at);
                self.loops.push(LoopPatch {
                    breaks: Vec::new(),
                    continues: Vec::new(),
                    depth: self.depth,
                    continue_target: None,
                });
                self.enter(at);
                self.emit(Op::Load(current.clone()), at);
                self.emit(Op::Declare(name.clone(), false, Some("Int".into())), at);
                self.typing_scopes
                    .last_mut()
                    .unwrap()
                    .insert(name.clone(), ("Int".into(), false));
                for s in body {
                    self.stmt(s)?;
                }
                self.exit(at);
                let increment = self.code.len();
                self.emit(Op::Load(current.clone()), at);
                self.emit(Op::Push(Value::Int(1)), at);
                self.emit(Op::Binary("+".into()), at);
                self.emit(Op::Store(current, "=".into()), at);
                self.emit(Op::Jump(cond), at);
                let exit = self.code.len();
                self.patch(end_jump, exit);
                let loop_ctx = self.loops.pop().unwrap();
                for jump in loop_ctx.breaks {
                    self.patch(jump, exit);
                }
                for jump in loop_ctx.continues {
                    self.patch(jump, increment);
                }
                self.exit(at);
            }
            StmtKind::Break | StmtKind::Continue => {
                let loop_ctx = self
                    .loops
                    .last()
                    .ok_or_else(|| diagnostic(at, "loop control outside loop"))?;
                let exits = self.depth - loop_ctx.depth;
                for _ in 0..exits {
                    self.emit(Op::Exit, at);
                }
                let jump = self.emit(Op::Jump(0), at);
                let loop_ctx = self.loops.last_mut().unwrap();
                if matches!(stmt.kind, StmtKind::Break) {
                    loop_ctx.breaks.push(jump);
                } else if let Some(to) = loop_ctx.continue_target {
                    self.patch(jump, to);
                } else {
                    loop_ctx.continues.push(jump);
                }
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(value)?;
                } else {
                    self.emit(Op::Push(Value::Null), at);
                }
                self.emit(Op::Return, at);
            }
            StmtKind::Defer(expr) => {
                if matches!(expr.kind, ExprKind::Closure(_, _, _)) {
                    self.expr(expr)?;
                    self.emit(Op::DeferValue, at);
                } else {
                    self.emit(Op::Defer(expr.clone()), at);
                }
            }
            StmtKind::Commit(name) => {
                self.emit(Op::Commit(name.clone()), at);
            }
            StmtKind::Revert(name) => {
                self.emit(Op::Revert(name.clone()), at);
            }
            StmtKind::Resume(name) => {
                self.emit(Op::Resume(name.clone()), at);
            }
            StmtKind::Drop(name) => {
                self.emit(Op::Drop(name.clone()), at);
            }
            StmtKind::Publish(force) => {
                self.emit(Op::Publish(*force), at);
            }
            StmtKind::Branch(name, body) => {
                self.emit(Op::BeginBranch, at);
                self.block(body, at)?;
                self.emit(Op::EndBranch(name.clone()), at);
            }
            StmtKind::Runtime(budget, steps) => {
                self.emit(Op::Runtime(*budget, *steps), at);
            }
            StmtKind::Match(value, arms) => {
                let value_type = self.expression_type(value);
                self.hidden += 1;
                let hidden = format!("$match{}", self.hidden);
                self.enter(at);
                self.expr(value)?;
                self.emit(Op::Declare(hidden.clone(), false, None), at);
                let mut ends = Vec::new();
                for (pattern, guard, body) in arms {
                    self.enter(at);
                    self.pattern_types(pattern, value_type.as_deref(), at);
                    self.emit(Op::Load(hidden.clone()), at);
                    let miss = self.emit(
                        if language_at_least(&self.typing_program.language, "1.6.1")
                            && matches!(value.kind, ExprKind::Name(_) | ExprKind::Member(_, _))
                        {
                            Op::BorrowPatternTest(resolve_pattern_alias(pattern, &self.aliases), 0)
                        } else {
                            Op::PatternTest(resolve_pattern_alias(pattern, &self.aliases), 0)
                        },
                        at,
                    );
                    let guarded_miss = if let Some(guard) = guard {
                        self.expr(guard)?;
                        Some(self.emit(Op::JumpFalse(0), at))
                    } else {
                        None
                    };
                    self.stmt(body)?;
                    self.exit(at);
                    ends.push(self.emit(Op::Jump(0), at));
                    let next = self.code.len();
                    self.patch(miss, next);
                    if let Some(jump) = guarded_miss {
                        self.patch(jump, next);
                    }
                    self.emit(Op::Exit, at);
                }
                self.emit(Op::FailMatch, at);
                let end = self.code.len();
                for jump in ends {
                    self.patch(jump, end);
                }
                self.exit(at);
            }
        }
        Ok(())
    }
    fn expr(&mut self, e: &Expr) -> Result<()> {
        match &e.kind {
            ExprKind::Value(v) => {
                self.emit(Op::Push(v.clone()), &e.at);
            }
            ExprKind::Name(name) => {
                self.emit(Op::Load(self.aliased(name)), &e.at);
            }
            ExprKind::Unary(op, inner) => {
                self.expr(inner)?;
                self.emit(
                    match op.as_str() {
                        "await" => Op::Await,
                        "spawn" => Op::Spawn,
                        _ => Op::Unary(op.clone()),
                    },
                    &e.at,
                );
            }
            ExprKind::Binary(op, a, b) if op == "&&" => {
                self.expr(a)?;
                let jump = self.emit(Op::JumpFalse(0), &e.at);
                self.expr(b)?;
                let end = self.emit(Op::Jump(0), &e.at);
                self.patch(jump, self.code.len());
                self.emit(Op::Push(Value::Bool(false)), &e.at);
                self.patch(end, self.code.len());
            }
            ExprKind::Binary(op, a, b) if op == "||" => {
                self.expr(a)?;
                let jump = self.emit(Op::JumpTrue(0), &e.at);
                self.expr(b)?;
                let end = self.emit(Op::Jump(0), &e.at);
                self.patch(jump, self.code.len());
                self.emit(Op::Push(Value::Bool(true)), &e.at);
                self.patch(end, self.code.len());
            }
            ExprKind::Binary(op, a, b) => {
                self.expr(a)?;
                self.expr(b)?;
                self.emit(Op::Binary(op.clone()), &e.at);
            }
            ExprKind::Member(base, field) => {
                if let ExprKind::Name(name) = &base.kind {
                    if let Some(symbol) = self
                        .aliases
                        .get(&format!("{name}.{field}"))
                        .filter(|_| !self.typing_scopes.iter().any(|s| s.contains_key(name)))
                        .cloned()
                    {
                        self.emit(Op::Load(symbol), &e.at);
                        return Ok(());
                    }
                }
                self.expr(base)?;
                self.emit(Op::Property(field.clone()), &e.at);
            }
            ExprKind::Call(target, args) => {
                if let ExprKind::Member(base, method) = &target.kind {
                    if let ExprKind::Name(receiver) = &base.kind {
                        let qualified = format!("{receiver}.{method}");
                        if let Some(symbol) = self
                            .aliases
                            .get(&qualified)
                            .filter(|_| {
                                !self.typing_scopes.iter().any(|s| s.contains_key(receiver))
                            })
                            .cloned()
                        {
                            for arg in args {
                                self.expr(arg)?;
                            }
                            let call = self.call_name(&symbol, args, &e.at);
                            self.emit(Op::Call(call, args.len()), &e.at);
                            return Ok(());
                        }
                        if matches!(
                            receiver.as_str(),
                            "Out"
                                | "Err"
                                | "File"
                                | "Directory"
                                | "In"
                                | "Time"
                                | "Random"
                                | "Args"
                                | "Env"
                                | "Locale"
                        ) {
                            for arg in args {
                                self.expr(arg)?;
                            }
                            self.emit(
                                Op::Builtin(receiver.clone(), method.clone(), args.len()),
                                &e.at,
                            );
                            return Ok(());
                        }
                    }
                    self.expr(base)?;
                    for arg in args {
                        self.expr(arg)?;
                    }
                    self.emit(Op::Method(method.clone(), args.len()), &e.at);
                } else if let ExprKind::Name(name) = &target.kind {
                    for arg in args {
                        self.expr(arg)?;
                    }
                    let call = self.call_name(&self.aliased(name), args, &e.at);
                    self.emit(Op::Call(call, args.len()), &e.at);
                } else {
                    self.expr(target)?;
                    for arg in args {
                        self.expr(arg)?;
                    }
                    self.emit(Op::CallValue(args.len()), &e.at);
                }
            }
            ExprKind::Try(inner) => {
                self.expr(inner)?;
                self.emit(Op::Try, &e.at);
            }
            ExprKind::Match(value, arms) => {
                let value_type = self.expression_type(value);
                self.hidden += 1;
                let hidden = format!("$match{}", self.hidden);
                self.enter(&e.at);
                self.expr(value)?;
                self.emit(Op::Declare(hidden.clone(), false, None), &e.at);
                let mut ends = Vec::new();
                for (pattern, guard, arm) in arms {
                    self.enter(&e.at);
                    self.pattern_types(pattern, value_type.as_deref(), &e.at);
                    self.emit(Op::Load(hidden.clone()), &e.at);
                    let miss = self.emit(
                        if language_at_least(&self.typing_program.language, "1.6.1")
                            && matches!(value.kind, ExprKind::Name(_) | ExprKind::Member(_, _))
                        {
                            Op::BorrowPatternTest(resolve_pattern_alias(pattern, &self.aliases), 0)
                        } else {
                            Op::PatternTest(resolve_pattern_alias(pattern, &self.aliases), 0)
                        },
                        &e.at,
                    );
                    let guarded_miss = if let Some(guard) = guard {
                        self.expr(guard)?;
                        Some(self.emit(Op::JumpFalse(0), &e.at))
                    } else {
                        None
                    };
                    self.expr(arm)?;
                    self.exit(&e.at);
                    ends.push(self.emit(Op::Jump(0), &e.at));
                    let next = self.code.len();
                    self.patch(miss, next);
                    if let Some(jump) = guarded_miss {
                        self.patch(jump, next);
                    }
                    self.emit(Op::Exit, &e.at);
                }
                self.emit(Op::FailMatch, &e.at);
                let end = self.code.len();
                for jump in ends {
                    self.patch(jump, end);
                }
                self.exit(&e.at);
            }
            ExprKind::Closure(params, ret, body) => {
                self.hidden += 1;
                let name = format!("$closure{}", self.hidden);
                self.closure_typing
                    .insert(name.clone(), self.typing_scopes.clone());
                let ty = format!(
                    "fn({})->{ret}",
                    params
                        .iter()
                        .map(|(_, t)| t.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                );
                self.closure_defs.insert(
                    name.clone(),
                    Function {
                        type_params: self.type_params.clone(),
                        asynchronous: false,
                        effects: None,
                        params: params.clone(),
                        ret: ret.clone(),
                        body: body.clone(),
                        at: e.at.clone(),
                        test: false,
                        public: false,
                        origin: self.typing_origin.clone(),
                    },
                );
                let callable = if self.type_params.is_empty() {
                    name
                } else {
                    format!(
                        "{name}<{}>",
                        self.type_params
                            .iter()
                            .map(|(p, _)| p.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                };
                self.emit(Op::MakeClosure(callable, ty), &e.at);
            }
            ExprKind::NamedConstructor(name, fields) => {
                for (_, expr) in fields {
                    self.expr(expr)?;
                }
                self.emit(
                    Op::CallNamed(
                        self.aliased(name),
                        fields.iter().map(|(field, _)| field.clone()).collect(),
                    ),
                    &e.at,
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
enum Cleanup {
    Expr(Expr, Vec<BTreeMap<String, Binding>>, usize),
    Callable(Value, usize),
    Handle(u64, usize),
    Group(u64, usize),
    Native(u64, u64, usize),
}
impl Cleanup {
    fn depth(&self) -> usize {
        match self {
            Self::Expr(_, _, depth)
            | Self::Callable(_, depth)
            | Self::Handle(_, depth)
            | Self::Group(_, depth)
            | Self::Native(_, _, depth) => *depth,
        }
    }
}
#[derive(Clone)]
struct VmFrame {
    id: u64,
    name: String,
    ret_ty: String,
    return_pc: usize,
    call_site: Tok,
    scopes: Vec<BTreeMap<String, Binding>>,
    scope_ids: Vec<u64>,
    defers: Vec<Cleanup>,
}
#[derive(Clone)]
struct BranchFrame {
    anchor: BranchAnchor,
    globals: Vec<BTreeMap<String, Binding>>,
    global_scope_ids: Vec<u64>,
    frames: Vec<VmFrame>,
    scheduler: Scheduler,
}
#[derive(Clone)]
struct VmSnapshot {
    generation: usize,
    pc: usize,
    stack_len: usize,
    globals: Vec<BTreeMap<String, Binding>>,
    global_scope_ids: Vec<u64>,
    frames: Vec<VmFrame>,
    branches: Vec<BranchFrame>,
    global_cleanups: Vec<Cleanup>,
    scheduler: Scheduler,
}

fn gc_scope_roots(scopes: &[BTreeMap<String, Binding>], out: &mut Vec<Value>) {
    out.extend(
        scopes
            .iter()
            .flat_map(|s| s.values().map(|b| b.value.clone())),
    );
}
fn gc_cleanup_roots(cleanups: &[Cleanup], out: &mut Vec<Value>) {
    for cleanup in cleanups {
        match cleanup {
            Cleanup::Expr(_, scopes, _) => gc_scope_roots(scopes, out),
            Cleanup::Callable(value, _) => out.push(value.clone()),
            _ => {}
        }
    }
}
fn gc_frame_roots(frames: &[VmFrame], out: &mut Vec<Value>) {
    for frame in frames {
        gc_scope_roots(&frame.scopes, out);
        gc_cleanup_roots(&frame.defers, out);
    }
}
fn gc_branch_roots(branches: &[BranchFrame], out: &mut Vec<Value>) {
    for branch in branches {
        gc_scope_roots(&branch.globals, out);
        gc_frame_roots(&branch.frames, out);
        branch.scheduler.gc_roots(out);
    }
}
struct Vm<R: BufRead> {
    engine: Engine<R>,
    code: Vec<Inst>,
    functions: BTreeMap<String, usize>,
    globals: Vec<BTreeMap<String, Binding>>,
    global_scope_ids: Vec<u64>,
    frames: Vec<VmFrame>,
    branches: Vec<BranchFrame>,
    snapshots: BTreeMap<String, VmSnapshot>,
    global_cleanups: Vec<Cleanup>,
    pc: usize,
    steps: usize,
    next_frame_id: u64,
    next_scope_id: u64,
    halt_pc: usize,
    specializations: BTreeMap<(String, Vec<String>), usize>,
    template_ends: BTreeMap<String, usize>,
    specialized_instructions: usize,
    scheduler: Scheduler,
    options: RunOptions,
    events: Vec<serde_json::Value>,
    debug_initial: serde_json::Value,
    debug_previous: serde_json::Value,
    debug_deltas: Vec<serde_json::Value>,
    debug_anchors: Vec<serde_json::Value>,
    debug_bytes: usize,
    audit: Vec<serde_json::Value>,
    inspections: Vec<serde_json::Value>,
    task_instructions: BTreeMap<u64, usize>,
    choice_widths: Vec<usize>,
    choices_used: Vec<usize>,
    fingerprint: String,
    entry: String,
    callback_stop: Option<usize>,
    current_pc: Option<usize>,
}

impl<R: BufRead> Vm<R> {
    fn new(
        mut program: Program,
        root: &Path,
        input: R,
        trace: bool,
        test_mode: bool,
        options: &RunOptions,
    ) -> Result<Self> {
        let compiled = Compiler::compile(&program)?;
        let artifact = options
            .artifact
            .clone()
            .map(Ok)
            .unwrap_or_else(|| build_artifact_mode(&program, root, test_mode))?;
        program.functions.extend(compiled.closure_defs.clone());
        let halt_pc = compiled
            .code
            .iter()
            .position(|i| matches!(i.op, Op::Halt))
            .unwrap();
        let mut entries = compiled
            .functions
            .iter()
            .map(|(name, at)| (name.clone(), *at))
            .collect::<Vec<_>>();
        entries.sort_by_key(|(_, at)| *at);
        let template_ends = entries
            .iter()
            .enumerate()
            .map(|(index, (name, _))| {
                (
                    name.clone(),
                    entries
                        .get(index + 1)
                        .map(|(_, at)| *at)
                        .unwrap_or(compiled.code.len()),
                )
            })
            .collect();
        let mut engine = Engine::new(program, root, input)?;
        if let Some(path) = &options.gui_events {
            if fs::metadata(path)?.len() > 1024 * 1024 {
                return Err(Error::InvalidOperation(
                    "GuiEventLimit: event script is limited to 1 MiB".into(),
                ));
            }
            let events = serde_json::from_slice(&fs::read(path)?).map_err(|_| {
                Error::InvalidOperation("GuiInvalidEvent: expected a JSON event array".into())
            })?;
            engine.runtime.configure_gui_events(events)?;
        }
        engine.runtime.configure_environment(
            options.arguments.clone(),
            options.allowed_env.clone(),
            options.secret_env.clone(),
            options.locale.clone().unwrap_or_else(|| "en-US".into()),
        )?;
        if let Some(path) = &options.secret_input {
            if fs::metadata(path)?.len() > 1024 * 1024 {
                return Err(Error::InvalidOperation("secret input exceeds 1 MiB".into()));
            }
            engine.runtime.configure_secret_input(
                fs::read_to_string(path)?
                    .lines()
                    .map(str::to_string)
                    .collect(),
            );
        }
        if options.inspect || options.virtual_publish {
            engine.runtime.enable_virtual_publish();
        }
        let fingerprint = packages::hash(
            serde_json::to_vec(&artifact).map_err(|e| Error::InvalidOperation(e.to_string()))?,
        );
        let entry = engine
            .program
            .root_origin
            .strip_prefix(fs::canonicalize(root)?)
            .map_err(|_| Error::InvalidPath("entry outside root".into()))?
            .to_string_lossy()
            .replace('\\', "/");
        if let Some(replay) = &options.replay {
            if replay["fingerprint"] != fingerprint {
                return Err(Error::InvalidOperation(
                    "ReplayMismatch: source/lock/build fingerprint".into(),
                ));
            }
            engine
                .runtime
                .import_observations(&replay["observations"])?;
        }
        if matches!(
            engine.program.language.as_str(),
            "1.0.0"
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
                | "1.9.0"
                | "2.0.0"
        ) {
            engine.runtime.enable_allocation_accounting();
            if let Some(limit) = options.execution_steps {
                engine.runtime.configure_execution_work(limit);
            }
        }
        if matches!(
            engine.program.language.as_str(),
            "0.9.9"
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
                | "1.9.0"
                | "2.0.0"
        ) {
            engine
                .runtime
                .configure_native_work(options.native_work.unwrap_or(1_000_000));
        }
        if let Some(limit) = options.execution_steps {
            engine.remaining = limit;
        }
        engine.trace = trace;
        engine.trace_json = options.trace_json;
        engine.test_mode = test_mode;
        let mut vm = Self {
            engine,
            code: compiled.code,
            functions: compiled.functions,
            globals: vec![BTreeMap::new()],
            global_scope_ids: vec![0],
            frames: Vec::new(),
            branches: Vec::new(),
            snapshots: BTreeMap::new(),
            global_cleanups: Vec::new(),
            pc: 0,
            steps: options.execution_steps.unwrap_or(1_000_000),
            next_frame_id: 1,
            next_scope_id: 1,
            halt_pc,
            specializations: BTreeMap::new(),
            template_ends,
            specialized_instructions: 0,
            scheduler: Scheduler::default(),
            options: options.clone(),
            events: Vec::new(),
            debug_initial: serde_json::Value::Null,
            debug_previous: serde_json::Value::Null,
            debug_deltas: Vec::new(),
            debug_anchors: Vec::new(),
            debug_bytes: 0,
            audit: Vec::new(),
            inspections: Vec::new(),
            task_instructions: BTreeMap::new(),
            choice_widths: Vec::new(),
            choices_used: Vec::new(),
            fingerprint,
            entry,
            callback_stop: None,
            current_pc: None,
        };
        if language_at_least(&vm.engine.program.language, "1.3.0") {
            vm.engine.runtime.install_begin()?;
            vm.snapshots.insert("begin".into(), vm.snapshot());
        }
        Ok(vm)
    }
    fn error(&self, at: &Tok, message: impl AsRef<str>) -> Error {
        if matches!(
            self.engine.program.language.as_str(),
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
                | "1.9.0"
                | "2.0.0"
        ) {
            let message = self
                .engine
                .runtime
                .masked_value(&Value::Text(message.as_ref().into()));
            let code = v06::diagnostics::code(&message).to_string();
            return Error::Diagnostic(Box::new(rewind::DiagnosticRecord {
                code: code.clone(),
                message,

                source: at.source.clone(),
                line: at.line,
                column: at.col,
                task_id: Some(self.scheduler.active),
                frames: vec![],
                hints: vec![],
                frames_truncated: false,
                causes: vec![],
                wait_edges: if code == "TaskDeadlock" {
                    self.scheduler.wait_edges()
                } else {
                    vec![]
                },
            }));
        }
        diagnostic(at, message)
    }
    fn inspection(&self) -> serde_json::Value {
        serde_json::json!({"event":self.events.len(),"runtime":self.engine.runtime.debug_state(),"scheduler":self.scheduler.debug_json(),"task_frames":self.task_frame_views(),"frames":self.frames.iter().map(|f|serde_json::json!({"function":f.name,"return_pc":f.return_pc,"scopes":f.scopes.iter().map(|scope|scope.iter().map(|(n,b)|(n.clone(),self.engine.runtime.masked_value(&self.resolve(&b.value)))).collect::<BTreeMap<_,_>>()).collect::<Vec<_>>()})).collect::<Vec<_>>()})
    }
    fn index_state(&mut self, at: &Tok) -> Result<()> {
        let value = self.inspection();
        let patch = (!self.debug_previous.is_null())
            .then(|| serde_json::json!(v06::debug::delta(&self.debug_previous, &value)));
        let anchor = self
            .events
            .len()
            .is_multiple_of(256)
            .then(|| serde_json::json!({"event":self.events.len(),"state":value}));
        let mut bytes = self.debug_bytes;
        for item in patch.iter().chain(anchor.iter()) {
            bytes = bytes.saturating_add(
                serde_json::to_vec(item)
                    .map_err(|e| self.error(at, e.to_string()))?
                    .len(),
            );
        }
        if self.debug_previous.is_null() {
            bytes = bytes.saturating_add(
                serde_json::to_vec(&value)
                    .map_err(|e| self.error(at, e.to_string()))?
                    .len(),
            );
        }
        if bytes > 16 * 1024 * 1024 {
            return Err(self.error(at, "TraceBudgetExceeded: debug index (16 MiB)"));
        }
        if self.debug_previous.is_null() {
            self.debug_initial = value.clone();
        }
        if let Some(patch) = patch {
            self.debug_deltas.push(patch);
        }
        if let Some(anchor) = anchor {
            self.debug_anchors.push(anchor);
        }
        self.debug_previous = value;
        self.debug_bytes = bytes;
        Ok(())
    }
    fn record_error(&self, error: &Error, at: &Tok, task: u64) -> rewind::DiagnosticRecord {
        let mut d = v06::diagnostics::record(error, at, task);
        if matches!(error, Error::HistoryBudgetExceeded) {
            d.code = self.engine.runtime.history_budget_kind().into();
        }
        if matches!(
            self.engine.program.language.as_str(),
            "1.1.0"
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
                | "1.9.0"
                | "2.0.0"
        ) {
            v11::enrich(&mut d);
        }
        d
    }
    fn push(&mut self, value: Value) -> Result<()> {
        self.engine.runtime.push_stack(value)
    }
    fn pop(&mut self) -> Result<Value> {
        self.engine.runtime.pop_stack()
    }
    fn args(&mut self, count: usize) -> Result<Vec<Value>> {
        let mut args = Vec::with_capacity(count);
        for _ in 0..count {
            args.push(self.pop()?);
        }
        args.reverse();
        Ok(args)
    }
    fn get(&self, name: &str) -> Option<&Binding> {
        self.frames
            .last()
            .and_then(|f| f.scopes.iter().rev().find_map(|s| s.get(name)))
            .or_else(|| self.globals.iter().rev().find_map(|s| s.get(name)))
    }
    fn resolve(&self, value: &Value) -> Value {
        if let Value::CellRef(id) = value {
            self.engine
                .runtime
                .heap_get(*id)
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            value.clone()
        }
    }
    fn declare(
        &mut self,
        name: String,
        value: Value,
        mutable: bool,
        ty: Option<String>,
        at: &Tok,
    ) -> Result<()> {
        self.declare_mode(name, value, mutable, ty, at, true)
    }
    fn declare_mode(
        &mut self,
        name: String,
        mut value: Value,
        mutable: bool,
        ty: Option<String>,
        at: &Tok,
        own: bool,
    ) -> Result<()> {
        let actual = value_type(&value, &self.engine.runtime);
        let ty = ty.unwrap_or(actual.clone());
        if !compatible(&ty, &actual) {
            return Err(self.error(at, format!("type mismatch: expected {ty}, found {actual}")));
        }
        if let Some((id, lease)) = if own {
            self.engine.runtime.claim_native(&mut value)?
        } else {
            None
        } {
            if let Some(frame) = self.frames.last_mut() {
                frame
                    .defers
                    .push(Cleanup::Native(id, lease, frame.scopes.len()));
            } else {
                self.global_cleanups
                    .push(Cleanup::Native(id, lease, self.globals.len()));
            }
        }
        let stored = if mutable {
            Value::CellRef(self.engine.runtime.alloc(value.clone())?)
        } else {
            value.clone()
        };
        let binding = Binding {
            value: stored.clone(),
            mutable,
            ty,
        };
        if let Some(frame) = self.frames.last_mut() {
            let scope = frame.scopes.last_mut().unwrap();
            if scope.contains_key(&name) {
                return Err(self.error(at, format!("duplicate binding {name}")));
            }
            scope.insert(name.clone(), binding);
            self.engine.runtime.set_local(name, stored)?;
        } else {
            if self.globals.last().unwrap().contains_key(&name) {
                return Err(self.error(at, format!("duplicate binding {name}")));
            }
            self.globals
                .last_mut()
                .unwrap()
                .insert(name.clone(), binding);
            self.engine.runtime.set_global(name, stored)?;
        }
        Ok(())
    }
    fn store(&mut self, name: &str, op: &str, rhs: Value, at: &Tok) -> Result<()> {
        let old = self
            .get(name)
            .cloned()
            .ok_or_else(|| self.error(at, format!("unknown variable {name}")))?;
        if !old.mutable {
            return Err(self.error(at, format!("cannot assign to let binding {name}")));
        }
        let mut value = if op == "=" {
            rhs
        } else {
            unflow(binary(&op[..1], self.resolve(&old.value), rhs, at), at)?
        };
        let actual = value_type(&value, &self.engine.runtime);
        if !compatible(&old.ty, &actual) {
            return Err(self.error(
                at,
                format!("type mismatch: expected {}, found {actual}", old.ty),
            ));
        }
        if let Some((id, lease)) = self.engine.runtime.claim_native(&mut value)? {
            if let Some(frame) = self.frames.last_mut() {
                let depth = frame
                    .scopes
                    .iter()
                    .rposition(|scope| scope.contains_key(name))
                    .unwrap()
                    + 1;
                frame.defers.push(Cleanup::Native(id, lease, depth));
            } else {
                let depth = self
                    .globals
                    .iter()
                    .rposition(|scope| scope.contains_key(name))
                    .unwrap()
                    + 1;
                self.global_cleanups.push(Cleanup::Native(id, lease, depth));
            }
        }
        let previous = self.resolve(&old.value);
        if let Some((id, lease)) = rewind::native_resources::token(&previous) {
            if self.engine.runtime.native_owner_matches(id, lease)
                && rewind::native_resources::token(&value) != Some((id, lease))
            {
                self.engine.runtime.close_native_resource(id)?;
                self.engine.runtime.forget_native_owner(id);
            }
        }
        if let Value::CellRef(id) = old.value {
            self.engine.runtime.heap_set(id, value)?;
            return Ok(());
        }
        if let Some(frame) = self.frames.last_mut() {
            for scope in frame.scopes.iter_mut().rev() {
                if let Some(binding) = scope.get_mut(name) {
                    binding.value = value.clone();
                    self.engine.runtime.set_local(name.to_string(), value)?;
                    return Ok(());
                }
            }
        }
        for scope in self.globals.iter_mut().rev() {
            if let Some(binding) = scope.get_mut(name) {
                binding.value = value.clone();
                break;
            }
        }
        self.engine.runtime.set_global(name.to_string(), value)
    }
    fn snapshot(&self) -> VmSnapshot {
        VmSnapshot {
            generation: self.scheduler.ticks as usize,
            pc: self.pc,
            stack_len: self.engine.runtime.state().stack.len(),
            globals: self.globals.clone(),
            global_scope_ids: self.global_scope_ids.clone(),
            frames: self.frames.clone(),
            branches: self.branches.clone(),
            global_cleanups: self.global_cleanups.clone(),
            scheduler: self.scheduler.clone(),
        }
    }
    fn collect_native_resources(&mut self) -> Result<()> {
        if !self.engine.runtime.has_native_resources() {
            return Ok(());
        }
        let mut roots = self.engine.runtime.state().stack.as_ref().clone();
        gc_scope_roots(&self.globals, &mut roots);
        gc_frame_roots(&self.frames, &mut roots);
        gc_cleanup_roots(&self.global_cleanups, &mut roots);
        gc_branch_roots(&self.branches, &mut roots);
        self.scheduler.gc_roots(&mut roots);
        self.engine.runtime.collect_native_resources(&roots)
    }
    fn restore(&mut self, snap: VmSnapshot) -> Result<()> {
        let retained = snap.scheduler.host_operations();
        for operation in self.scheduler.host_operations().difference(&retained) {
            self.engine.runtime.cancel_http(*operation)?;
        }
        self.pc = snap.pc;
        self.globals = snap.globals;
        self.global_scope_ids = snap.global_scope_ids;
        self.frames = snap.frames;
        self.branches = snap.branches;
        self.global_cleanups = snap.global_cleanups;
        self.scheduler = snap.scheduler;
        self.collect_native_resources()
    }
    fn call_user(&mut self, name: &str, args: Vec<Value>, at: &Tok) -> Result<()> {
        self.call_user_captured(name, args, BTreeMap::new(), at)
    }
    fn call_value(&mut self, callable: Value, args: Vec<Value>, at: &Tok) -> Result<()> {
        match callable {
            Value::Function(name, _) => {
                if self.engine.program.functions[name.split('<').next().unwrap_or(&name)]
                    .asynchronous
                {
                    let task = self.create_async_task(&name, args, at)?;
                    self.push(task)
                } else {
                    self.call_user(&name, args, at)
                }
            }
            Value::HeapRef(id) => match self.engine.runtime.heap_get(id).cloned() {
                Some(Value::Closure(name, _, captures)) => {
                    self.call_user_captured(&name, args, captures, at)
                }
                _ => Err(self.error(at, "value is not callable")),
            },
            _ => Err(self.error(at, "value is not callable")),
        }
    }
    fn run_cleanup(&mut self, cleanup: Cleanup, at: &Tok) -> Result<()> {
        match cleanup {
            Cleanup::Native(id, lease, _) => {
                if self.engine.runtime.native_owner_matches(id, lease) {
                    self.engine.runtime.close_native_resource(id)?;
                    self.engine.runtime.forget_native_owner(id);
                }
                Ok(())
            }
            Cleanup::Group(id, _) => self.cancel_group(id, at),
            Cleanup::Handle(id, _) => self.engine.runtime.close_handle(id),
            Cleanup::Expr(expr, captured, _) => {
                self.engine.scopes = self.globals.clone();
                self.engine.scopes.extend(captured);
                self.engine.function_depth = self.frames.len();
                unflow(self.engine.eval(&expr), at).map(|_| ())
            }
            Cleanup::Callable(value, _) => {
                let Value::HeapRef(id) = value else {
                    return Err(self.error(at, "defer expects a closure"));
                };
                let Some(Value::Closure(name, _, captures)) =
                    self.engine.runtime.heap_get(id).cloned()
                else {
                    return Err(self.error(at, "defer expects a closure"));
                };
                self.engine.scopes = self.globals.clone();
                let captured = captures
                    .into_iter()
                    .map(|(name, value)| {
                        let ty = value_type(&value, &self.engine.runtime);
                        let mutable = matches!(value, Value::CellRef(_));
                        (name, Binding { value, mutable, ty })
                    })
                    .collect();
                self.engine.scopes.push(captured);
                self.engine.function_depth = self.frames.len();
                unflow(self.engine.call(&name, Vec::new(), at), at).map(|_| ())
            }
        }
    }
    fn cleanup_scope(&mut self, depth: usize, at: &Tok) -> Result<()> {
        let entries = if let Some(frame) = self.frames.last_mut() {
            &mut frame.defers
        } else {
            &mut self.global_cleanups
        };
        let mut pending = Vec::new();
        let mut i = 0;
        while i < entries.len() {
            if entries[i].depth() == depth {
                pending.push(entries.remove(i));
            } else {
                i += 1;
            }
        }
        self.run_cleanups(pending, at)
    }
    fn run_cleanups(&mut self, pending: Vec<Cleanup>, at: &Tok) -> Result<()> {
        let mut first_error = None;
        let mut suppressed = Vec::new();
        let mut typed_causes = Vec::new();
        for cleanup in pending.into_iter().rev() {
            if let Err(error) = self.run_cleanup(cleanup, at) {
                if first_error.is_none() {
                    first_error = Some(error);
                } else {
                    typed_causes.push(self.record_error(&error, at, self.scheduler.active));
                    suppressed.push(error.to_string());
                }
            }
        }
        if matches!(
            self.engine.program.language.as_str(),
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
                | "1.9.0"
                | "2.0.0"
        ) {
            return first_error.map_or(Ok(()), |e| {
                let mut d = self.record_error(&e, at, self.scheduler.active);
                d.causes.extend(typed_causes);
                Err(Error::Diagnostic(Box::new(d)))
            });
        }
        if matches!(
            self.engine.program.language.as_str(),
            "0.5"
                | "0.6"
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
                | "1.9.0"
                | "2.0.0"
        ) {
            first_error.map_or(Ok(()), |e| {
                let mut message = e.to_string();
                for cause in suppressed {
                    message.push_str("; cleanup: ");
                    message.push_str(&cause);
                }
                Err(Error::InvalidOperation(message))
            })
        } else {
            first_error.map_or(Ok(()), Err)
        }
    }
    fn call_user_captured(
        &mut self,
        name: &str,
        args: Vec<Value>,
        captures: BTreeMap<String, Value>,
        at: &Tok,
    ) -> Result<()> {
        let invocation = name;
        let name = name.split('<').next().unwrap_or(name);
        let def = self
            .engine
            .program
            .functions
            .get(name)
            .ok_or_else(|| self.error(at, format!("unknown function {name}")))?;
        if args.len() != def.params.len() {
            return Err(self.error(at, format!("{name} expects {} arguments", def.params.len())));
        }
        let actuals = args
            .iter()
            .map(|a| value_type(a, &self.engine.runtime))
            .collect::<Vec<_>>();
        let params = def
            .type_params
            .iter()
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        let substitutions = infer_call_arguments(invocation, &def.params, &params, &actuals, at)?;
        if substitutions
            .values()
            .any(|ty| ty.len() > 4096 || ty.chars().filter(|c| *c == '<').count() > 64)
        {
            return Err(self.error(at, "TypeExpansionBudgetExceeded"));
        }
        let target = if params.is_empty() {
            self.functions[name]
        } else {
            let key = (
                name.to_string(),
                params
                    .iter()
                    .map(|p| substitutions[p].clone())
                    .collect::<Vec<_>>(),
            );
            if let Some(target) = self.specializations.get(&key) {
                *target
            } else {
                if self.specializations.len() >= 4096 {
                    return Err(self.error(at, "MonomorphizationBudgetExceeded: instances"));
                }
                let start = self.functions[name];
                let end = self.template_ends[name];
                let size = end - start;
                if self.specialized_instructions.saturating_add(size) > 100_000 {
                    return Err(self.error(at, "MonomorphizationBudgetExceeded: instructions"));
                }
                let target = self.code.len();
                for index in start..end {
                    let mut inst = self.code[index].clone();
                    match &mut inst.op {
                        Op::Jump(to)
                        | Op::JumpFalse(to)
                        | Op::JumpTrue(to)
                        | Op::PatternTest(_, to)
                        | Op::BorrowPatternTest(_, to)
                            if *to >= start && *to <= end =>
                        {
                            *to = target + (*to - start)
                        }
                        Op::Declare(_, _, Some(ty)) | Op::Call(ty, _) | Op::CallNamed(ty, _) => {
                            *ty = substitute_type(ty, &substitutions)
                        }
                        Op::MakeClosure(name, ty) => {
                            *name = substitute_type(name, &substitutions);
                            *ty = substitute_type(ty, &substitutions);
                        }
                        _ => {}
                    }
                    self.code.push(inst);
                }
                self.specialized_instructions += size;
                self.specializations.insert(key, target);
                target
            }
        };
        if self.frames.len() >= 1024 {
            return Err(self.error(at, "ExecutionBudgetExceeded: call depth"));
        }
        self.engine.runtime.push_frame(self.pc)?;
        let mut scope = captures
            .into_iter()
            .map(|(name, value)| {
                let ty = value_type(&value, &self.engine.runtime);
                let mutable = matches!(value, Value::CellRef(_));
                (name, Binding { value, mutable, ty })
            })
            .collect::<BTreeMap<_, _>>();
        let mut native_cleanups = Vec::new();
        for binding in scope.values_mut() {
            if let Some((id, lease)) = self.engine.runtime.claim_native(&mut binding.value)? {
                native_cleanups.push(Cleanup::Native(id, lease, 1));
            }
        }
        for ((param, ty), mut arg) in def.params.iter().zip(args) {
            let actual = value_type(&arg, &self.engine.runtime);
            let expected = substitute_type(ty, &substitutions);
            if !compatible(&expected, &actual) {
                return Err(self.error(at, format!("{name} expects {expected}, found {actual}")));
            }
            if !expected.starts_with('&') {
                if let Some((id, lease)) = self.engine.runtime.claim_native(&mut arg)? {
                    native_cleanups.push(Cleanup::Native(id, lease, 1));
                }
            }
            self.engine.runtime.set_local(param.clone(), arg.clone())?;
            scope.insert(
                param.clone(),
                Binding {
                    value: arg,
                    mutable: false,
                    ty: expected,
                },
            );
        }
        self.frames.push(VmFrame {
            id: self.next_frame_id,
            name: name.into(),
            ret_ty: substitute_type(&def.ret, &substitutions),
            return_pc: self.pc,
            call_site: at.clone(),
            scopes: vec![scope],
            scope_ids: vec![0],
            defers: native_cleanups,
        });
        self.next_frame_id += 1;
        self.pc = target;
        Ok(())
    }
    fn finish_call(&mut self, mut value: Value, at: &Tok) -> Result<()> {
        let Some(frame) = self.frames.last().cloned() else {
            return Err(self.error(at, "return outside function"));
        };
        let expected = &frame.ret_ty;
        let actual = value_type(&value, &self.engine.runtime);
        if !compatible(expected, &actual) {
            return Err(self.error(
                at,
                format!("{} returns {actual}, expected {expected}", frame.name),
            ));
        }
        let resource_frame = if self.engine.runtime.has_native_resources() {
            let mut values = Vec::new();
            gc_scope_roots(&frame.scopes, &mut values);
            !self.engine.runtime.unclaimed_native_ids(&values).is_empty()
        } else {
            false
        };
        self.engine.runtime.move_native(&mut value)?;
        let deferred = std::mem::take(&mut self.frames.last_mut().unwrap().defers);
        self.run_cleanups(deferred, at)?;
        self.frames.pop();
        self.engine.runtime.pop_frame()?;
        self.pc = frame.return_pc;
        self.push(value)?;
        if resource_frame {
            self.collect_native_resources()?;
        }
        Ok(())
    }
    fn run(&mut self) -> Result<()> {
        if self.scheduler.active == 0 {
            self.scheduler.reactivate_main();
        }
        let mut result = loop {
            match self.run_inner() {
                Err(error) if self.scheduler.active != 0 => {
                    let at = self
                        .code
                        .get(self.pc.saturating_sub(1))
                        .map(|i| i.at.clone())
                        .unwrap_or(Tok {
                            source: String::new(),
                            text: "<task>".into(),
                            line: 0,
                            col: 0,
                        });
                    let mut causes = Vec::new();
                    let mut failure = self.record_error(&error, &at, self.scheduler.active);
                    for frame in self.frames.iter().rev().cloned().collect::<Vec<_>>() {
                        if let Err(e) = self.run_cleanups(frame.defers, &at) {
                            failure
                                .causes
                                .push(self.record_error(&e, &at, self.scheduler.active));
                            causes.push(e.to_string());
                        }
                    }
                    let pending = std::mem::take(&mut self.global_cleanups);
                    if let Err(e) = self.run_cleanups(pending, &at) {
                        failure
                            .causes
                            .push(self.record_error(&e, &at, self.scheduler.active));
                        causes.push(e.to_string());
                    }
                    self.frames.clear();
                    if let Err(error) = self.cancel_children(&at) {
                        break Err(error);
                    }
                    let mut message = error.to_string();
                    for cause in causes {
                        message.push_str("; cleanup: ");
                        message.push_str(&cause);
                    }
                    self.set_task_failure(failure);
                    match self.finish_scheduled_task(Err(message), &at) {
                        Ok(true) => continue,
                        Ok(false) => break Ok(()),
                        Err(error) => break Err(error),
                    }
                }
                result => break result,
            }
        };
        if result.is_ok()
            && matches!(
                self.engine.program.language.as_str(),
                "0.5"
                    | "0.6"
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
                    | "1.9.0"
                    | "2.0.0"
            )
        {
            if matches!(
                self.engine.program.language.as_str(),
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
                    | "1.9.0"
                    | "2.0.0"
            ) {
                if let Some(error) = self.unhandled_task_diagnostic() {
                    result = Err(Error::Diagnostic(Box::new(error)));
                }
            } else if let Some(error) = self.unhandled_task_error() {
                result = Err(Error::InvalidOperation(error));
            }
        }
        if result.is_err() {
            let mut causes = Vec::new();
            let mut typed_causes = Vec::new();
            let at = self
                .code
                .get(self.pc.saturating_sub(1))
                .map(|i| i.at.clone())
                .unwrap_or(Tok {
                    source: String::new(),
                    text: "<task>".into(),
                    line: 0,
                    col: 0,
                });
            if let Err(e) = self.cancel_children(&at) {
                typed_causes.push(self.record_error(&e, &at, self.scheduler.active));
                causes.push(e.to_string());
            }
            for frame in self.frames.iter().rev().cloned().collect::<Vec<_>>() {
                for cleanup in frame.defers.into_iter().rev() {
                    let at = self
                        .code
                        .get(self.pc.saturating_sub(1))
                        .map(|i| i.at.clone())
                        .unwrap_or(Tok {
                            source: String::new(),
                            text: "<cleanup>".into(),
                            line: 0,
                            col: 0,
                        });
                    if let Err(e) = self.run_cleanup(cleanup, &at) {
                        typed_causes.push(self.record_error(&e, &at, self.scheduler.active));
                        causes.push(e.to_string());
                    }
                }
            }
            let at = self
                .code
                .get(self.pc.saturating_sub(1))
                .map(|i| i.at.clone())
                .unwrap_or(Tok {
                    source: String::new(),
                    text: "<cleanup>".into(),
                    line: 0,
                    col: 0,
                });
            for cleanup in std::mem::take(&mut self.global_cleanups).into_iter().rev() {
                if let Err(e) = self.run_cleanup(cleanup, &at) {
                    typed_causes.push(self.record_error(&e, &at, self.scheduler.active));
                    causes.push(e.to_string());
                }
            }
            if matches!(
                self.engine.program.language.as_str(),
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
                    | "1.9.0"
                    | "2.0.0"
            ) {
                let mut failure =
                    self.record_error(&result.unwrap_err(), &at, self.scheduler.active);
                failure.causes.extend(typed_causes);
                result = Err(Error::Diagnostic(Box::new(failure)));
            } else if matches!(
                self.engine.program.language.as_str(),
                "0.5"
                    | "0.6"
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
                    | "1.9.0"
                    | "2.0.0"
            ) && !causes.is_empty()
            {
                let mut message = result.unwrap_err().to_string();
                for cause in causes {
                    message.push_str("; cleanup: ");
                    message.push_str(&cause);
                }
                result = Err(Error::InvalidOperation(message));
            }
        }
        result
    }
    fn finish_tools(&self, execution_error: Option<&Error>) -> Result<()> {
        let digest = self.engine.runtime.state_digest()?;
        let mut outcome = serde_json::json!({"ok":execution_error.is_none(),"error":execution_error.map(|e|self.engine.runtime.masked_value(&Value::Text(e.to_string())))});
        if matches!(
            self.engine.program.language.as_str(),
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
                | "1.9.0"
                | "2.0.0"
        ) {
            outcome["diagnostic"] = execution_error
                .and_then(|e| {
                    if let Error::Diagnostic(d) = e {
                        Some(v06::diagnostics::masked_record(d, &self.engine.runtime))
                    } else {
                        None
                    }
                })
                .unwrap_or(serde_json::Value::Null);
        }
        if let Some(replay) = &self.options.replay {
            if replay["events"]
                .as_array()
                .is_none_or(|events| events.len() != self.events.len())
                || replay["state_digest"] != digest
                || (!replay["result"].is_null() && replay["result"] != outcome)
            {
                return Err(Error::InvalidOperation(
                    "ReplayMismatch: final state or event count".into(),
                ));
            }
        }
        if let Some(path) = &self.options.record {
            let mut trace = serde_json::json!({"format":1,"compiler":env!("CARGO_PKG_VERSION"),"standalone":self.options.standalone,"test":self.options.recorded_test,"entry":self.entry,"fingerprint":self.fingerprint,"task_steps":self.options.task_steps.unwrap_or(100_000),"native_work":self.options.native_work,"execution_steps":self.options.execution_steps,"schedule_choices":self.choices_used,"result":outcome,"observations":self.engine.runtime.export_observations()?,"events":self.events,"state_digest":digest,"virtual_publish":self.options.inspect||self.options.virtual_publish,"debug":{"checkpoints":self.inspections,"final":{"runtime":self.engine.runtime.debug_state(),"scheduler":self.scheduler.debug_json()}}});
            if matches!(
                self.engine.program.language.as_str(),
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
                    | "1.9.0"
                    | "2.0.0"
            ) {
                let mut deltas = self.debug_deltas.clone();
                if !self.debug_previous.is_null() {
                    deltas.push(serde_json::json!(v06::debug::delta(
                        &self.debug_previous,
                        &self.inspection()
                    )));
                }
                trace["debug"]["index"]=self.engine.runtime.mask_debug_json(&serde_json::json!({"format":1,"initial":self.debug_initial,"deltas":deltas,"anchors":self.debug_anchors}));
                trace["debug"]["checkpoints"] = self
                    .engine
                    .runtime
                    .mask_debug_json(&trace["debug"]["checkpoints"]);
                trace["debug"]["final"] = self
                    .engine
                    .runtime
                    .mask_debug_json(&trace["debug"]["final"]);
                trace["audit"] = self
                    .engine
                    .runtime
                    .mask_debug_json(&serde_json::json!(self.audit));
                trace["artifact_sha256"] = serde_json::json!(self.fingerprint);
                if let Some(path) = &self.options.artifact_path {
                    trace["artifact_entry"] = serde_json::json!(path);
                }
            }
            let bytes = serde_json::to_string_pretty(&trace)
                .map_err(|e| Error::InvalidOperation(e.to_string()))?
                + "\n";
            if bytes.len() > 128 * 1024 * 1024 {
                return Err(Error::InvalidOperation(
                    "TraceBudgetExceeded: 128 MiB".into(),
                ));
            }
            fs::write(path, bytes)?;
        }
        if self.options.inspect {
            eprintln!(
                "{}",
                serde_json::json!({"checkpoints":self.inspections,"final":{"runtime":self.engine.runtime.debug_state(),"scheduler":self.scheduler.debug_json()}})
            );
        }
        if self.options.profile {
            eprintln!(
                "{}",
                serde_json::json!({"storage_work":self.engine.runtime.storage_metrics(),"task_instructions":self.task_instructions.iter().map(|(id,count)|(id.to_string(),*count)).collect::<BTreeMap<_,_>>(),"runtime":self.engine.runtime.debug_state(),"scheduler":self.scheduler.debug_json(),"schedule_choices":self.choices_used})
            );
        }
        Ok(())
    }
    fn run_inner(&mut self) -> Result<()> {
        let caller_pc = self.current_pc;
        let result = self.run_inner_body();
        if result.is_ok() {
            self.current_pc = caller_pc;
        }
        result.map_err(|error| {
            if !matches!(
                self.engine.program.language.as_str(),
                "1.1.0"
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
                    | "1.9.0"
                    | "2.0.0"
            ) {
                return error;
            }
            let at = self
                .current_pc
                .and_then(|pc| self.code.get(pc))
                .map(|i| i.at.clone())
                .unwrap_or(Tok {
                    source: String::new(),
                    text: String::new(),
                    line: 0,
                    col: 0,
                });
            let mut d = self.record_error(&error, &at, self.scheduler.active);
            v11::enrich(&mut d);
            if d.frames.is_empty() {
                let mut location = Tok {
                    source: d.source.clone(),
                    line: d.line,
                    col: d.column,
                    text: String::new(),
                };
                for frame in self.frames.iter().rev().take(32) {
                    d.frames.push(rewind::DiagnosticFrame {
                        function: v11::display_symbol(&frame.name),
                        source: location.source.clone(),
                        line: location.line,
                        column: location.col,
                    });
                    location = frame.call_site.clone();
                }
                if self.frames.len() < 32 {
                    d.frames.push(rewind::DiagnosticFrame {
                        function: "<entry>".into(),
                        source: location.source,
                        line: location.line,
                        column: location.col,
                    });
                } else {
                    d.frames_truncated = true;
                }
            }
            d.message = self.engine.runtime.masked_value(&Value::Text(d.message));
            Error::Diagnostic(Box::new(d))
        })
    }
    fn run_inner_body(&mut self) -> Result<()> {
        loop {
            self.current_pc = Some(self.pc);
            if self
                .callback_stop
                .is_some_and(|depth| self.frames.len() <= depth)
            {
                return Ok(());
            }
            if self
                .options
                .pause_after
                .is_some_and(|limit| self.events.len() >= limit)
            {
                return Ok(());
            }
            if matches!(
                self.engine.program.language.as_str(),
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
                    | "1.9.0"
                    | "2.0.0"
            ) {
                self.expire_timeouts(&self.code[self.pc].at.clone())?;
            }
            if self.cancellation_requested() {
                return Err(self.error(&self.code[self.pc].at, "TaskCancelled"));
            }
            if matches!(
                self.engine.program.language.as_str(),
                "0.9.9"
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
                    | "1.9.0"
                    | "2.0.0"
            ) && self.engine.runtime.collection_due()
            {
                self.collect_native_resources()?;
                let mut roots = Vec::new();
                gc_scope_roots(&self.globals, &mut roots);
                gc_scope_roots(&self.engine.scopes, &mut roots);
                gc_frame_roots(&self.frames, &mut roots);
                gc_cleanup_roots(&self.global_cleanups, &mut roots);
                gc_branch_roots(&self.branches, &mut roots);
                self.scheduler.gc_roots(&mut roots);
                let limit = self
                    .engine
                    .runtime
                    .execution_work_remaining()
                    .map_or(self.steps, |n| n.min(self.steps));
                let (_, work) = self.engine.runtime.collect_heap(&roots, limit)?;
                self.engine.runtime.charge_execution_work(work)?;
                self.steps -= work;
            }
            let scheduler_bytes = self.scheduler.storage_bytes()
                + self
                    .snapshots
                    .values()
                    .map(|s| s.scheduler.storage_bytes())
                    .sum::<usize>()
                + self
                    .branches
                    .iter()
                    .map(|b| b.scheduler.storage_bytes())
                    .sum::<usize>();
            if scheduler_bytes > 16 * 1024 * 1024 {
                return Err(self.error(
                    &self.code[self.pc].at,
                    "TaskBudgetExceeded: scheduler storage (16 MiB)",
                ));
            }
            if self.steps == 0 {
                return Err(self.error(&self.code[self.pc].at, "ExecutionBudgetExceeded"));
            }
            self.engine.runtime.charge_execution_work(1)?;
            self.steps -= 1;
            self.scheduler.ticks = self.scheduler.ticks.saturating_add(1);
            let inst = self.code.get(self.pc).cloned().ok_or_else(|| {
                Error::InvalidOperation(format!("invalid program counter {}", self.pc))
            })?;
            if self.scheduler.active != 0
                && self
                    .task_instructions
                    .get(&self.scheduler.active)
                    .copied()
                    .unwrap_or(0)
                    >= self.options.task_steps.unwrap_or(100_000)
            {
                return Err(self.error(
                    &inst.at,
                    format!(
                        "TaskBudgetExceeded: instructions for task {}",
                        self.scheduler.active
                    ),
                ));
            }
            *self
                .task_instructions
                .entry(self.scheduler.active)
                .or_default() += 1;
            if self.options.record.is_some() || self.options.replay.is_some() {
                let operation = format!("{:?}", inst.op)
                    .split(['(', '{'])
                    .next()
                    .unwrap()
                    .to_string();
                if matches!(
                    self.engine.program.language.as_str(),
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
                        | "1.9.0"
                        | "2.0.0"
                ) {
                    self.index_state(&inst.at)?;
                }
                let mut event = serde_json::json!({"task":self.scheduler.active,"pc":self.pc,"operation":operation});
                if matches!(
                    self.engine.program.language.as_str(),
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
                        | "1.9.0"
                        | "2.0.0"
                ) {
                    event["source"] = serde_json::json!(inst.at.source);
                    event["line"] = serde_json::json!(inst.at.line);
                    event["column"] = serde_json::json!(inst.at.col);
                }
                if let Some(replay) = &self.options.replay {
                    if replay["events"].get(self.events.len()) != Some(&event) {
                        return Err(self.error(
                            &inst.at,
                            format!(
                                "ReplayMismatch: event {} task {} pc {}",
                                self.events.len(),
                                self.scheduler.active,
                                self.pc
                            ),
                        ));
                    }
                }
                self.events.push(event);
            }
            self.engine.runtime.set_program_counter(self.pc);
            self.pc += 1;
            match inst.op {
                Op::Halt => {
                    if self.drain_scope_groups(None, &inst.at)? {
                        continue;
                    }
                    let pending = std::mem::take(&mut self.global_cleanups);
                    self.run_cleanups(pending, &inst.at)?;
                    let value = if self.scheduler.active == 0 {
                        Value::Null
                    } else {
                        self.pop()?
                    };
                    if self.finish_scheduled_task(Ok(value), &inst.at)? {
                        continue;
                    }
                    return Ok(());
                }
                Op::Spawn => {
                    self.engine.runtime.require_internal()?;
                    let task = self.pop()?;
                    self.start_task(&task, &inst.at)?;
                    self.push(task)?;
                }
                Op::Await => {
                    self.engine.runtime.require_internal()?;
                    let task = self.pop()?;
                    self.wait_task(task, &inst.at)?;
                }
                Op::Push(value) => self.push(value)?,
                Op::Load(name) => {
                    let value = if let Some(binding) = self.get(&name) {
                        self.resolve(&binding.value)
                    } else if let Some(f) = self.engine.program.functions.get(&name) {
                        Value::Function(
                            name.clone(),
                            if matches!(
                                self.engine.program.language.as_str(),
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
                                    | "1.9.0"
                                    | "2.0.0"
                            ) {
                                let ty = v06::fn_type(
                                    &f.params,
                                    &if f.asynchronous {
                                        format!("Task<{}>", f.ret)
                                    } else {
                                        f.ret.clone()
                                    },
                                    &v06::function_effects(&self.engine.program, &name),
                                );
                                let names = v05::needed_globals(&self.engine.program, &name);
                                let can = |shared: bool| {
                                    names.iter().all(|n| {
                                        self.get(n).is_none_or(|b| {
                                            (!shared || !b.mutable)
                                                && v06::captures::value_flags(
                                                    &self.engine.program,
                                                    &self.engine.runtime,
                                                    &b.value,
                                                    shared,
                                                    &mut BTreeSet::new(),
                                                )
                                        })
                                    })
                                };
                                v06::captures::with_flags(&ty, can(false), can(true))
                            } else {
                                format!(
                                    "fn({})->{}",
                                    f.params
                                        .iter()
                                        .map(|(_, t)| t.as_str())
                                        .collect::<Vec<_>>()
                                        .join(","),
                                    if f.asynchronous {
                                        format!("Task<{}>", f.ret)
                                    } else {
                                        f.ret.clone()
                                    }
                                )
                            },
                        )
                    } else if name.contains("::") {
                        unflow(self.engine.call(&name, Vec::new(), &inst.at), &inst.at)?
                    } else {
                        return Err(self.error(&inst.at, format!("unknown name {name}")));
                    };
                    self.push(value)?;
                }
                Op::Declare(name, mutable, ty) => {
                    let value = self.pop()?;
                    self.declare(name, value, mutable, ty, &inst.at)?;
                }
                Op::Store(name, op) => {
                    let value = self.pop()?;
                    self.store(&name, &op, value, &inst.at)?;
                }
                Op::StoreField(field, op) => {
                    let rhs = self.pop()?;
                    let receiver = self.pop()?;
                    let Value::HeapRef(id) = receiver else {
                        return Err(self.error(&inst.at, "field assignment requires a struct"));
                    };
                    let mut data = self
                        .engine
                        .runtime
                        .heap_get(id)
                        .cloned()
                        .ok_or_else(|| self.error(&inst.at, "unknown object"))?;
                    let Value::Struct(name, fields) = &mut data else {
                        return Err(self.error(&inst.at, "field assignment requires a struct"));
                    };
                    let old = fields
                        .get(&field)
                        .cloned()
                        .ok_or_else(|| self.error(&inst.at, "unknown field"))?;
                    let value = if op == "=" {
                        rhs
                    } else {
                        unflow(binary(&op[..1], old, rhs, &inst.at), &inst.at)?
                    };
                    let def = &self.engine.program.structs[name.split('<').next().unwrap_or(name)];
                    let expected = &def.fields.iter().find(|(n, _)| n == &field).unwrap().1;
                    let concrete = name
                        .split_once('<')
                        .map(|(_, inner)| split_type_args(outer_type_end(inner)))
                        .unwrap_or_default();
                    let substitutions = def
                        .type_params
                        .iter()
                        .cloned()
                        .zip(concrete.iter().map(|s| s.to_string()))
                        .collect::<BTreeMap<_, _>>();
                    let expected = substitute_type(expected, &substitutions);
                    let actual = value_type(&value, &self.engine.runtime);
                    if !compatible(&expected, &actual) {
                        return Err(self.error(
                            &inst.at,
                            format!("field {field} expects {expected}, found {actual}"),
                        ));
                    }
                    fields.insert(field, value);
                    self.engine.runtime.heap_set(id, data)?;
                }
                Op::Unary(op) => {
                    let value = self.pop()?;
                    let result = match (op.as_str(), value) {
                        ("-", Value::Int(n)) => n
                            .checked_neg()
                            .map(Value::Int)
                            .ok_or_else(|| self.error(&inst.at, "integer overflow"))?,
                        ("-", Value::Float(n)) => Value::Float((-f64::from_bits(n)).to_bits()),
                        ("!", Value::Bool(v)) => Value::Bool(!v),
                        ("move", mut v) => {
                            self.engine.runtime.move_native(&mut v)?;
                            v
                        }
                        ("borrow" | "borrowMut", v) => v,
                        ("$capture:value", v) => v05::task_copy(&mut self.engine.runtime, &v)?,
                        ("$capture:move", mut v) => {
                            self.engine.runtime.move_native(&mut v)?;
                            v
                        }
                        ("$capture:borrow", v) => {
                            if let Value::Closure(name, ty, captures) = self.resolve(&v) {
                                Value::HeapRef(self.engine.runtime.alloc(Value::Closure(
                                    name,
                                    v06::captures::with_flags(&ty, false, false),
                                    captures,
                                ))?)
                            } else {
                                v
                            }
                        }
                        _ => return Err(self.error(&inst.at, "invalid unary operand")),
                    };
                    self.push(result)?;
                }
                Op::Binary(op) => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    if matches!(
                        self.engine.program.language.as_str(),
                        "1.0.0"
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
                            | "1.9.0"
                            | "2.0.0"
                    ) && (matches!(
                        lhs,
                        Value::Text(_) | Value::Bytes(_) | Value::Decimal(_) | Value::BigInt(_)
                    ) || matches!(
                        rhs,
                        Value::Text(_) | Value::Bytes(_) | Value::Decimal(_) | Value::BigInt(_)
                    )) {
                        let work = v100::argument_work(&lhs, &self.engine.runtime)
                            .saturating_add(v100::argument_work(&rhs, &self.engine.runtime));
                        self.engine.runtime.charge_native_work(work)?;
                    }
                    self.push(unflow(binary(&op, lhs, rhs, &inst.at), &inst.at)?)?;
                }
                Op::Property(field) => {
                    let value = self.pop()?;
                    let secret = v05::unsecret(&value).cloned();
                    let value = secret.clone().unwrap_or(value);
                    if let Some(Value::Struct(_, fields)) = v05::unfrozen(&value) {
                        let value = fields
                            .get(&field)
                            .cloned()
                            .ok_or_else(|| self.error(&inst.at, "unknown frozen field"))?;
                        let value = v05::freeze_member(value, &self.engine.runtime);
                        self.push(if secret.is_some() {
                            v05::secret(value.clone(), value_type(&value, &self.engine.runtime))
                        } else {
                            value
                        })?;
                        continue;
                    }
                    let result = match value {
                        Value::FileError(error) => match field.as_str() {
                            "code" => Value::Text(error.code),
                            "path" => Value::Text(error.path),
                            "cause" => Value::Option(
                                error
                                    .causes
                                    .first()
                                    .cloned()
                                    .map(|s| Box::new(Value::Text(s))),
                            ),
                            "causes" => {
                                Value::HeapRef(self.engine.runtime.alloc(Value::TypedList(
                                    "String".into(),
                                    error.causes.into_iter().map(Value::Text).collect(),
                                ))?)
                            }
                            _ => return Err(self.error(&inst.at, "unknown FileError field")),
                        },
                        Value::Handle(id) if field == "position" => {
                            Value::Int(self.engine.runtime.handle(id)?.position as i64)
                        }
                        Value::HeapRef(id) => match self.engine.runtime.heap_get(id) {
                            Some(Value::Struct(_, fields)) => fields
                                .get(&field)
                                .cloned()
                                .ok_or_else(|| self.error(&inst.at, "unknown field"))?,
                            _ => return Err(self.error(&inst.at, "unknown property")),
                        },
                        Value::Struct(_, fields) => fields
                            .get(&field)
                            .cloned()
                            .ok_or_else(|| self.error(&inst.at, "unknown field"))?,
                        _ => return Err(self.error(&inst.at, "unknown property")),
                    };
                    let result = if secret.is_some() {
                        v05::secret(result.clone(), value_type(&result, &self.engine.runtime))
                    } else {
                        result
                    };
                    self.push(result)?;
                }
                Op::Call(name, count) => {
                    let args = self.args(count)?;
                    if matches!(
                        self.engine.program.language.as_str(),
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
                            | "1.9.0"
                            | "2.0.0"
                    ) && name == "reveal"
                    {
                        self.audit.push(serde_json::json!({"kind":"reveal","source":inst.at.source,"line":inst.at.line,"column":inst.at.col,"task":self.scheduler.active,"event":self.events.len()}));
                    }
                    if (name == "property"
                        || name == "propertyCandidates" && program_v09(&self.engine.program))
                        && program_v07(&self.engine.program)
                    {
                        let value =
                            self.property_case(&args, &inst.at, name == "propertyCandidates")?;
                        self.push(value)?;
                        continue;
                    }
                    if name == "propertyInt"
                        && matches!(
                            self.engine.program.language.as_str(),
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
                                | "1.9.0"
                                | "2.0.0"
                        )
                    {
                        let value = self.property_int(&args, &inst.at)?;
                        self.push(value)?;
                        continue;
                    }
                    if let Some(value) = self.scheduler_constructor(&name, &args, &inst.at)? {
                        self.push(value)?;
                        continue;
                    }
                    if self
                        .engine
                        .program
                        .functions
                        .get(name.split('<').next().unwrap_or(&name))
                        .is_some_and(|f| f.asynchronous)
                    {
                        let value = self.create_async_task(&name, args, &inst.at)?;
                        self.push(value)?;
                        continue;
                    }
                    if let Some(binding) = self.get(&name) {
                        let callable = self.resolve(&binding.value);
                        self.call_value(callable, args, &inst.at)?;
                    } else if self
                        .functions
                        .contains_key(name.split('<').next().unwrap_or(&name))
                    {
                        self.call_user(&name, args, &inst.at)?;
                    } else {
                        let value = unflow(self.engine.call(&name, args, &inst.at), &inst.at)?;
                        self.push(value)?;
                    }
                }
                Op::CallValue(count) => {
                    let args = self.args(count)?;
                    let callable = self.pop()?;
                    self.call_value(callable, args, &inst.at)?;
                }
                Op::CallNamed(name, fields) => {
                    let values = self.args(fields.len())?;
                    let value = unflow(
                        self.engine.call_named(
                            &name,
                            fields.into_iter().zip(values).collect(),
                            &inst.at,
                        ),
                        &inst.at,
                    )?;
                    self.push(value)?;
                }
                Op::MakeClosure(name, ty) => {
                    let mut captures = BTreeMap::new();
                    let needed = v05::needed_globals(&self.engine.program, &name);
                    for scope in &self.globals {
                        for (name, binding) in scope {
                            if matches!(
                                self.engine.program.language.as_str(),
                                "0.5"
                                    | "0.6"
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
                                    | "1.9.0"
                                    | "2.0.0"
                            ) && !needed.contains(name)
                            {
                                continue;
                            }
                            captures.insert(
                                name.clone(),
                                if binding.mutable {
                                    binding.value.clone()
                                } else {
                                    self.resolve(&binding.value)
                                },
                            );
                        }
                    }
                    if let Some(frame) = self.frames.last() {
                        for scope in &frame.scopes {
                            for (name, binding) in scope {
                                if matches!(
                                    self.engine.program.language.as_str(),
                                    "0.5"
                                        | "0.6"
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
                                        | "1.9.0"
                                        | "2.0.0"
                                ) && !needed.contains(name)
                                {
                                    continue;
                                }
                                captures.insert(
                                    name.clone(),
                                    if binding.mutable {
                                        binding.value.clone()
                                    } else {
                                        self.resolve(&binding.value)
                                    },
                                );
                            }
                        }
                    }
                    let ty = if matches!(
                        self.engine.program.language.as_str(),
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
                            | "1.9.0"
                            | "2.0.0"
                    ) {
                        let f =
                            &self.engine.program.functions[name.split('<').next().unwrap_or(&name)];
                        let checker = Checker {
                            program: &self.engine.program,
                            scopes: vec![captures
                                .iter()
                                .map(|(n, v)| {
                                    (
                                        n.clone(),
                                        (value_type(&self.resolve(v), &self.engine.runtime), false),
                                    )
                                })
                                .collect()],
                            return_ty: None,
                            loop_depth: 0,
                            bounds: f
                                .type_params
                                .iter()
                                .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                                .collect(),
                            origin: self.engine.program.root_origin.clone(),
                        };
                        let effects = v06::closure_effects(&checker, &f.params, &f.body)?;
                        let ty = format!(
                            "{ty}!{{{}}}",
                            effects.into_iter().collect::<Vec<_>>().join("+")
                        );
                        let can = |shared: bool| {
                            captures.values().all(|v| {
                                v06::captures::value_flags(
                                    &self.engine.program,
                                    &self.engine.runtime,
                                    v,
                                    shared,
                                    &mut BTreeSet::new(),
                                )
                            })
                        };
                        v06::captures::with_flags(&ty, can(false), can(true))
                    } else {
                        ty
                    };
                    let id = self
                        .engine
                        .runtime
                        .alloc(Value::Closure(name, ty, captures))?;
                    self.push(Value::HeapRef(id))?;
                }
                Op::Method(method, count) => {
                    let mut args = self.args(count)?;
                    let target = self.pop()?;
                    if matches!(
                        self.engine.program.language.as_str(),
                        "1.0.0"
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
                            | "1.9.0"
                            | "2.0.0"
                    ) {
                        self.engine.runtime.charge_native_work(v100::method_work(
                            &target,
                            &method,
                            &args,
                            &self.engine.runtime,
                        ))?;
                    }
                    if matches!(
                        self.engine.program.language.as_str(),
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
                            | "1.9.0"
                            | "2.0.0"
                    ) {
                        if let Some(value) =
                            self.result_adapter(&target, &method, &args, &inst.at)?
                        {
                            self.push(value)?;
                            continue;
                        }
                        if let Some(value) =
                            self.iterator_adapter(&target, &method, &args, &inst.at)?
                        {
                            self.push(value)?;
                            continue;
                        }
                    }
                    if let Some(value) = self.scheduler_method(&target, &method, &args, &inst.at)? {
                        self.push(value)?;
                        continue;
                    }
                    let ty = value_type(&target, &self.engine.runtime);
                    let matching = matching_methods(&self.engine.program, &ty, &method);
                    if matching.len() > 1 {
                        return Err(
                            self.error(&inst.at, format!("ambiguous method {method} for {ty}"))
                        );
                    }
                    if let Some(symbol) = matching.first() {
                        args.insert(0, target);
                        self.call_user(symbol, args, &inst.at)?;
                        continue;
                    }
                    let value = unflow(
                        self.engine.method_inner(target, &method, args, &inst.at),
                        &inst.at,
                    )?;
                    self.push(value)?;
                }
                Op::Builtin(receiver, method, count) => {
                    let args = self.args(count)?;
                    if matches!(
                        self.engine.program.language.as_str(),
                        "1.0.0"
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
                            | "1.9.0"
                            | "2.0.0"
                    ) {
                        self.engine.runtime.charge_native_work(v100::call_work(
                            &format!("{receiver}.{method}"),
                            &args,
                            &self.engine.runtime,
                        ))?;
                    }
                    if matches!(
                        self.engine.program.language.as_str(),
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
                            | "1.9.0"
                            | "2.0.0"
                    ) && matches!(
                        (receiver.as_str(), method.as_str()),
                        ("Env", "getSecret") | ("In", "readSecretLine")
                    ) {
                        self.audit.push(serde_json::json!({"kind":if receiver=="Env" {"secretEnvironment"}else{"secretInput"},"source":inst.at.source,"line":inst.at.line,"column":inst.at.col,"task":self.scheduler.active,"event":self.events.len()}));
                    }
                    let value = unflow(
                        self.engine.builtin(&receiver, &method, args, &inst.at),
                        &inst.at,
                    )?;
                    self.push(value)?;
                }
                Op::Try => match self.pop()? {
                    Value::Result(Ok(value)) => self.push(*value)?,
                    Value::Result(Err(value)) => {
                        self.finish_call(Value::Result(Err(value)), &inst.at)?
                    }
                    _ => return Err(self.error(&inst.at, "'?' requires Result")),
                },
                Op::Pop => {
                    let value = self.pop()?;
                    if !self
                        .engine
                        .runtime
                        .unclaimed_native_ids(&[value])
                        .is_empty()
                    {
                        self.collect_native_resources()?;
                    }
                }
                Op::EnterExternal(fresh) => {
                    if (self.scheduler.active != 0
                        && !language_at_least(&self.engine.program.language, "1.6.0"))
                        || !self.branches.is_empty()
                    {
                        return Err(self.error(
                            &inst.at,
                            "ExternalBoundary: external requires the main task outside branches",
                        ));
                    }
                    self.engine
                        .runtime
                        .enter_external_task(fresh, self.scheduler.active)?;
                }
                Op::ExitExternal => self.engine.runtime.exit_external()?,
                Op::Enter => {
                    let id = self.next_scope_id;
                    self.next_scope_id = id.checked_add(1).ok_or_else(|| {
                        self.error(&inst.at, "ScopeBudgetExceeded: scope identities")
                    })?;
                    if let Some(frame) = self.frames.last_mut() {
                        frame.scopes.push(BTreeMap::new());
                        frame.scope_ids.push(id);
                    } else {
                        self.globals.push(BTreeMap::new());
                        self.global_scope_ids.push(id);
                    }
                }
                Op::Exit => {
                    let resource_scope = if self.engine.runtime.has_native_resources() {
                        let mut values = Vec::new();
                        if let Some(frame) = self.frames.last() {
                            gc_scope_roots(&frame.scopes[frame.scopes.len() - 1..], &mut values);
                        } else {
                            gc_scope_roots(&self.globals[self.globals.len() - 1..], &mut values);
                        }
                        !self.engine.runtime.unclaimed_native_ids(&values).is_empty()
                    } else {
                        false
                    };
                    let depth = self
                        .frames
                        .last()
                        .map_or(self.globals.len(), |f| f.scopes.len());
                    if self.drain_scope_groups(Some(depth), &inst.at)? {
                        continue;
                    }
                    self.cleanup_scope(depth, &inst.at)?;
                    if let Some(frame) = self.frames.last_mut() {
                        if frame.scopes.len() <= 1 {
                            return Err(self.error(&inst.at, "scope underflow"));
                        }
                        let removed = frame.scopes.pop().unwrap();
                        frame.scope_ids.pop();
                        for name in removed.keys() {
                            if let Some(value) = frame.scopes.iter().rev().find_map(|s| s.get(name))
                            {
                                self.engine
                                    .runtime
                                    .set_local(name.clone(), value.value.clone())?;
                            } else {
                                self.engine.runtime.remove_local(name)?;
                            }
                        }
                    } else if self.globals.len() > 1 {
                        let removed = self.globals.pop().unwrap();
                        self.global_scope_ids.pop();
                        for name in removed.keys() {
                            if let Some(value) = self.globals.iter().rev().find_map(|s| s.get(name))
                            {
                                self.engine
                                    .runtime
                                    .set_global(name.clone(), value.value.clone())?;
                            } else {
                                self.engine.runtime.remove_global(name);
                            }
                        }
                    } else {
                        return Err(self.error(&inst.at, "scope underflow"));
                    }
                    if resource_scope {
                        self.collect_native_resources()?;
                    }
                }
                Op::ApplicationEntry(to) => {
                    if self.engine.test_mode {
                        self.pc = to;
                    }
                }
                Op::Jump(to) => self.pc = to,
                Op::JumpFalse(to) => match self.pop()? {
                    Value::Bool(false) => self.pc = to,
                    Value::Bool(true) => {}
                    _ => return Err(self.error(&inst.at, "condition must be Bool")),
                },
                Op::JumpTrue(to) => match self.pop()? {
                    Value::Bool(true) => self.pc = to,
                    Value::Bool(false) => {}
                    _ => return Err(self.error(&inst.at, "condition must be Bool")),
                },
                Op::Return => {
                    let failed = matches!(
                        self.engine.program.language.as_str(),
                        "0.5"
                            | "0.6"
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
                            | "1.9.0"
                            | "2.0.0"
                    ) && matches!(
                        self.engine.runtime.state().stack.last(),
                        Some(Value::Result(Err(_)))
                    );
                    if !failed && self.drain_scope_groups(None, &inst.at)? {
                        continue;
                    }
                    let value = self.pop()?;
                    self.finish_call(value, &inst.at)?;
                }
                Op::Defer(expr) => {
                    let Some(frame) = self.frames.last_mut() else {
                        return Err(self.error(&inst.at, "defer requires a function"));
                    };
                    frame.defers.push(Cleanup::Expr(
                        expr,
                        frame.scopes.clone(),
                        frame.scopes.len(),
                    ));
                }
                Op::DeferValue => {
                    let value = self.pop()?;
                    let Some(frame) = self.frames.last_mut() else {
                        return Err(self.error(&inst.at, "defer requires a function"));
                    };
                    frame
                        .defers
                        .push(Cleanup::Callable(value, frame.scopes.len()));
                }
                Op::Using(name) => {
                    let binding = self
                        .get(&name)
                        .ok_or_else(|| self.error(&inst.at, "unknown using resource"))?;
                    let value = self.resolve(&binding.value);
                    if rewind::native_resources::token(&value).is_some() {
                        continue;
                    }
                    if value_type(&value, &self.engine.runtime) == "TaskGroup" {
                        let id = scheduler::handle_id(&value)
                            .ok_or_else(|| self.error(&inst.at, "invalid TaskGroup"))?;
                        if let Some(frame) = self.frames.last_mut() {
                            frame.defers.push(Cleanup::Group(id, frame.scopes.len()));
                        } else {
                            self.global_cleanups
                                .push(Cleanup::Group(id, self.globals.len()));
                        }
                        continue;
                    }
                    let Value::Handle(id) = value else {
                        return Err(self.error(&inst.at, "using requires FileHandle"));
                    };
                    if let Some(frame) = self.frames.last_mut() {
                        frame.defers.push(Cleanup::Handle(id, frame.scopes.len()));
                    } else {
                        self.global_cleanups
                            .push(Cleanup::Handle(id, self.globals.len()));
                    }
                }
                Op::Commit(name) => {
                    self.engine.runtime.set_program_counter(self.pc);
                    self.engine.runtime.commit(name.clone())?;
                    self.snapshots.insert(name.clone(), self.snapshot());
                    if self.options.inspect || self.options.record.is_some() {
                        self.inspections.push(serde_json::json!({"checkpoint":name,"event_index":self.events.len(),"runtime":self.engine.runtime.debug_state(),"scheduler":self.scheduler.debug_json(),"frames":self.frames.iter().map(|f|serde_json::json!({"function":f.name,"return_pc":f.return_pc,"scopes":f.scopes.iter().map(|s|s.iter().map(|(n,b)|(n.clone(),self.engine.runtime.masked_value(&self.resolve(&b.value)))).collect::<BTreeMap<_,_>>()).collect::<Vec<_>>() })).collect::<Vec<_>>() }));
                    }
                    if self.engine.trace {
                        eprintln!(
                            "{}",
                            if self.engine.trace_json {
                                self.engine.runtime.trace_checkpoint_json(&name)?
                            } else {
                                self.engine.runtime.trace_checkpoint(&name)?
                            }
                        );
                    }
                }
                Op::Revert(name) => {
                    self.engine.runtime.require_internal()?;
                    let mut snap = self.snapshots.get(&name).cloned().ok_or_else(|| {
                        self.error(&inst.at, format!("unknown checkpoint {name}"))
                    })?;
                    if name == "begin" && language_at_least(&self.engine.program.language, "1.3.0")
                    {
                        // Retain only the live continuation skeleton. All bindings,
                        // resources, deferred cleanups and tasks are initial again.
                        if !self.branches.is_empty()
                            || self.scheduler.active != 0
                            || !self.engine.runtime.state().stack.is_empty()
                        {
                            return Err(self.error(&inst.at, "InvalidContinuation: revert begin requires the main task outside branches with no pending expression operands"));
                        }
                        let after = self.pc;
                        let mut call_frames = (*self.engine.runtime.state().call_frames).clone();
                        for frame in &mut call_frames {
                            frame.locals.clear();
                        }
                        snap.globals.resize_with(self.globals.len(), BTreeMap::new);
                        snap.global_scope_ids = self.global_scope_ids.clone();
                        snap.frames = self.frames.clone();
                        for frame in &mut snap.frames {
                            for scope in &mut frame.scopes {
                                scope.clear();
                            }
                            frame.defers.clear();
                        }
                        self.engine.runtime.reset_begin()?;
                        self.engine.runtime.set_task_compute(
                            std::sync::Arc::new(Vec::new()),
                            std::sync::Arc::new(call_frames),
                            std::sync::Arc::new(BTreeMap::new()),
                        )?;
                        self.restore(snap)?;
                        self.snapshots.retain(|label, _| label == "begin");
                        self.pc = after;
                        self.engine.runtime.set_program_counter(after);
                        continue;
                    }
                    let saved_ids = snap.frames.iter().map(|f| f.id).collect::<Vec<_>>();
                    let current_ids = self.frames.iter().map(|f| f.id).collect::<Vec<_>>();
                    let scopes_match = self.global_scope_ids.starts_with(&snap.global_scope_ids)
                        && snap
                            .frames
                            .iter()
                            .zip(&self.frames)
                            .all(|(a, b)| b.scope_ids.starts_with(&a.scope_ids));
                    if saved_ids != current_ids
                        || !scopes_match
                        || snap.stack_len != self.engine.runtime.state().stack.len()
                        || snap.branches.len() != self.branches.len()
                    {
                        return Err(self.error(
                            &inst.at,
                            "InvalidContinuation: checkpoint call frames, scopes, stack or branch context are incompatible with the current continuation",
                        ));
                    }
                    // Revert restores data, but continues at the current instruction.
                    // Keep the active block structure so subsequent Exit/break/return
                    // instructions still unwind the correct number of scopes. Bindings
                    // and cleanups created after the checkpoint must not survive.
                    snap.globals.resize_with(self.globals.len(), BTreeMap::new);
                    snap.global_scope_ids = self.global_scope_ids.clone();
                    for (saved, current) in snap.frames.iter_mut().zip(&self.frames) {
                        saved
                            .scopes
                            .resize_with(current.scopes.len(), BTreeMap::new);
                        saved.scope_ids = current.scope_ids.clone();
                    }
                    let after = self.pc;
                    self.engine.runtime.revert(&name)?;
                    self.restore(snap)?;
                    self.pc = after;
                    self.engine.runtime.set_program_counter(after);
                }
                Op::Resume(name) => {
                    self.engine.runtime.require_internal()?;
                    let snap = self.snapshots.get(&name).cloned().ok_or_else(|| {
                        self.error(&inst.at, format!("unknown checkpoint {name}"))
                    })?;
                    if name == "begin" && language_at_least(&self.engine.program.language, "1.3.0")
                    {
                        self.engine.runtime.reset_begin()?;
                        self.snapshots.retain(|label, _| label == "begin");
                    } else {
                        self.engine.runtime.revert(&name)?;
                    }
                    self.restore(snap)?;
                }
                Op::Drop(name) => {
                    self.engine.runtime.drop_checkpoint(&name)?;
                    self.snapshots.remove(&name);
                }
                Op::Publish(force) => {
                    self.engine.runtime.require_internal()?;
                    if matches!(
                        self.engine.program.language.as_str(),
                        "0.5"
                            | "0.6"
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
                            | "1.9.0"
                            | "2.0.0"
                    ) {
                        if let Some(error) = self.unhandled_task_error() {
                            return Err(self.error(&inst.at, error));
                        }
                    }
                    if self.scheduler.active != 0
                        || (self.scheduler.has_live_tasks()
                            && !language_at_least(&self.engine.program.language, "1.6.0"))
                    {
                        return Err(self.error(&inst.at,"publish requires the application task and completed/cancelled children"));
                    }
                    if self.engine.test_mode {
                        return Err(
                            self.error(&inst.at, "publish is unavailable during rewind test")
                        );
                    }
                    self.engine.runtime.publish(
                        force,
                        &mut io::stdout().lock(),
                        &mut io::stderr().lock(),
                    )?;
                }
                Op::BeginBranch => {
                    self.engine.runtime.require_internal()?;
                    self.branches.push(BranchFrame {
                        anchor: self.engine.runtime.begin_branch(),
                        globals: self.globals.clone(),
                        global_scope_ids: self.global_scope_ids.clone(),
                        frames: self.frames.clone(),
                        scheduler: self.scheduler.clone(),
                    });
                }
                Op::EndBranch(name) => {
                    self.engine.runtime.set_program_counter(self.pc);
                    let branch = self
                        .branches
                        .pop()
                        .ok_or_else(|| self.error(&inst.at, "branch underflow"))?;
                    self.snapshots.insert(name.clone(), self.snapshot());
                    self.engine
                        .runtime
                        .end_branch(name.clone(), branch.anchor)?;
                    self.globals = branch.globals;
                    self.global_scope_ids = branch.global_scope_ids;
                    self.frames = branch.frames;
                    self.scheduler = branch.scheduler;
                    if self.engine.trace {
                        eprintln!(
                            "{}",
                            if self.engine.trace_json {
                                self.engine.runtime.trace_checkpoint_json(&name)?
                            } else {
                                self.engine.runtime.trace_checkpoint(&name)?
                            }
                        );
                    }
                }
                Op::Runtime(budget, steps) => {
                    self.engine.runtime.set_budget(budget)?;
                    if let Some(steps) = steps {
                        self.steps = steps;
                    }
                }
                Op::PatternTest(pattern, miss) | Op::BorrowPatternTest(pattern, miss) => {
                    let value = self.pop()?;
                    let mut bindings = BTreeMap::new();
                    if pattern_matches(&pattern, &value, &self.engine.runtime, &mut bindings) {
                        for (name, value) in bindings {
                            let borrow =
                                matches!(self.code[self.pc - 1].op, Op::BorrowPatternTest(..));
                            self.declare_mode(name, value, false, None, &inst.at, !borrow)?;
                        }
                    } else {
                        self.pc = miss;
                    }
                }
                Op::FailMatch => return Err(self.error(&inst.at, "non-exhaustive match")),
            }
        }
    }
}

fn unflow<T>(result: Exec<T>, at: &Tok) -> Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(Flow::Error(error)) => Err(error),
        Err(_) => Err(diagnostic(at, "invalid control flow in expression")),
    }
}

pub(super) fn execute(
    program: Program,
    root: &Path,
    trace: bool,
    mode: &str,
    options: RunOptions,
) -> Result<()> {
    if mode == "test" {
        let tests = program
            .functions
            .iter()
            .filter(|(_, f)| f.test)
            .filter(|(name, _)| options.recorded_test.as_ref().is_none_or(|t| *name == t))
            .filter(|(name, _)| {
                options
                    .test_filter
                    .as_ref()
                    .is_none_or(|filter| name.contains(filter))
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for test in tests {
            let mut config = options.clone();
            config.recorded_test = Some(test.clone());
            let mut vm = Vm::new(
                program.clone(),
                root,
                io::stdin().lock(),
                trace,
                true,
                &config,
            )?;
            let result = (|| {
                vm.run()?;
                let at = program.functions[&test].at.clone();
                vm.pc = vm.halt_pc;
                vm.call_user(&test, Vec::new(), &at)?;
                vm.run()
            })();
            if result.is_ok()
                || matches!(
                    program.language.as_str(),
                    "0.5"
                        | "0.6"
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
                        | "1.9.0"
                        | "2.0.0"
                )
            {
                vm.finish_tools(result.as_ref().err())?;
            }
            result.map_err(|error| Error::InvalidOperation(format!("test {test}: {error}")))?;
            eprintln!("ok {test}");
            if options.explore > 0 {
                let widths = vm.choice_widths.clone();
                let used = vm.choices_used.clone();
                drop(vm);
                explore_test(&program, root, &test, trace, &options, &widths, &used)?;
            }
        }
        Ok(())
    } else {
        let mut vm = Vm::new(program, root, io::stdin().lock(), trace, false, &options)?;
        let result = vm.run();
        if result.is_ok()
            || matches!(
                vm.engine.program.language.as_str(),
                "0.5"
                    | "0.6"
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
                    | "1.9.0"
                    | "2.0.0"
            )
            || options.inspect
            || options.profile
        {
            vm.finish_tools(result.as_ref().err())?;
        }
        result
    }
}
pub(super) fn validate(program: &Program) -> Result<()> {
    Compiler::compile(program).map(|_| ())
}
pub(super) fn verified_code(program: &Program) -> Result<serde_json::Value> {
    let compiler = Compiler::compile(program)?;
    if compiler.code.len() > 1_000_000 {
        return Err(Error::InvalidOperation(
            "ArtifactBudgetExceeded: instructions".into(),
        ));
    }
    let code=compiler.code.iter().map(|inst|serde_json::json!({"operation":format!("{:?}",inst.op),"module":inst.at.source,"line":inst.at.line,"column":inst.at.col})).collect::<Vec<_>>();
    Ok(serde_json::json!({"bytecode":code,"function_entries":compiler.functions}))
}
pub(super) fn debug_view(
    program: Program,
    root: &Path,
    trace: &serde_json::Value,
    limit: usize,
    mut options: RunOptions,
) -> Result<serde_json::Value> {
    options.replay = Some(trace.clone());
    options.pause_after = Some(limit);
    options.virtual_publish = true;
    options.native_work = trace["native_work"].as_u64().map(|v| v as usize);
    options.execution_steps = trace["execution_steps"].as_u64().map(|v| v as usize);
    options.task_steps = trace["task_steps"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok());
    options.choices = serde_json::from_value(trace["schedule_choices"].clone()).unwrap_or_default();
    let mut vm = Vm::new(program, root, io::stdin().lock(), false, false, &options)?;
    let mut result = vm.run();
    if result.is_ok() && vm.events.len() < limit {
        if let Some(test) = trace["test"].as_str() {
            vm.pc = vm.halt_pc;
            vm.call_user(
                test,
                Vec::new(),
                &vm.engine.program.functions[test].at.clone(),
            )?;
            result = vm.run();
        }
    }
    Ok(
        serde_json::json!({"event":vm.events.len(),"runtime":vm.engine.runtime.debug_state(),"scheduler":vm.scheduler.debug_json(),"frames":vm.frames.iter().map(|f|serde_json::json!({"function":f.name,"return_pc":f.return_pc,"scopes":f.scopes.iter().map(|scope|scope.iter().map(|(n,b)|(n.clone(),vm.engine.runtime.masked_value(&vm.resolve(&b.value)))).collect::<BTreeMap<_,_>>()).collect::<Vec<_>>()})).collect::<Vec<_>>(),"error":result.err().map(|e|e.to_string())}),
    )
}
pub(super) fn build_artifact(program: &Program, root: &Path) -> Result<serde_json::Value> {
    build_artifact_mode(program, root, false)
}
fn build_artifact_mode(
    program: &Program,
    root: &Path,
    include_dev: bool,
) -> Result<serde_json::Value> {
    let compiler = Compiler::compile(program)?;
    let canonical = fs::canonicalize(root)?;
    let sources = program
        .included_modules
        .iter()
        .map(|path| {
            Ok((
                path.strip_prefix(&canonical)
                    .map_err(|_| Error::InvalidPath(path.display().to_string()))?
                    .to_string_lossy()
                    .replace('\\', "/"),
                packages::hash(fs::read(path)?),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let functions=program.functions.iter().map(|(name,f)| (name.clone(),serde_json::json!({"async":f.asynchronous,"type_params":f.type_params,"parameters":f.params,"return":f.ret}))).collect::<BTreeMap<_,_>>();
    let code=compiler.code.iter().map(|inst|serde_json::json!({"operation":format!("{:?}",inst.op),"module":inst.at.source,"line":inst.at.line,"column":inst.at.col})).collect::<Vec<_>>();
    let document = if matches!(
        program.language.as_str(),
        "0.9.4"
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
            | "1.9.0"
            | "2.0.0"
    ) && !root.join("rewind.toml").exists()
    {
        serde_json::to_string(&functions).map_err(|e| Error::InvalidOperation(e.to_string()))?
    } else {
        super::documentation_mode(&program.root_origin.to_string_lossy(), root, include_dev)?
    };
    let lock = root.join("rewind.lock");
    Ok(
        serde_json::json!({"format":1,"compiler":env!("CARGO_PKG_VERSION"),"sources":sources,"lock_sha256":if lock.exists(){Some(if let Some(config)=project::ProjectConfig::load(root)? {config.execution_lock_hash(root)?} else {packages::hash(fs::read(lock)?)})}else{None},"api_document_sha256":packages::hash(document),"functions":functions,"function_entries":compiler.functions,"bytecode":code}),
    )
}
fn explore_test(
    program: &Program,
    root: &Path,
    test: &str,
    trace: bool,
    options: &RunOptions,
    initial_widths: &[usize],
    initial_used: &[usize],
) -> Result<()> {
    use std::collections::VecDeque;
    let mut queue = VecDeque::new();
    let mut seen = BTreeSet::new();
    seen.insert(Vec::new());
    fn alternatives(
        widths: &[usize],
        used: &[usize],
        queue: &mut VecDeque<Vec<usize>>,
        seen: &mut BTreeSet<Vec<usize>>,
    ) {
        for (i, width) in widths.iter().enumerate() {
            for choice in 0..*width {
                if used[i] != choice {
                    let mut path = used[..i].to_vec();
                    path.push(choice);
                    if seen.insert(path.clone()) {
                        queue.push_back(path);
                    }
                }
            }
        }
    }
    alternatives(initial_widths, initial_used, &mut queue, &mut seen);
    let mut explored = 1;
    while explored < options.explore {
        let Some(choices) = queue.pop_front() else {
            break;
        };
        let mut config = options.clone();
        config.choices = choices.clone();
        config.explore = 0;
        config.record = options.record.clone();
        config.recorded_test = Some(test.into());
        config.inspect = false;
        config.profile = false;
        let mut vm = Vm::new(
            program.clone(),
            root,
            io::stdin().lock(),
            trace,
            true,
            &config,
        )?;
        let run = (|| {
            vm.run()?;
            vm.pc = vm.halt_pc;
            vm.call_user(test, Vec::new(), &program.functions[test].at)?;
            vm.run()
        })();
        if run.is_err()
            && matches!(
                program.language.as_str(),
                "0.5"
                    | "0.6"
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
                    | "1.9.0"
                    | "2.0.0"
            )
        {
            vm.options.record = options.record.clone();
            vm.finish_tools(run.as_ref().err())?;
        }
        run.map_err(|e| {
            Error::InvalidOperation(format!("test {test}: schedule {choices:?}: {e}"))
        })?;
        alternatives(&vm.choice_widths, &vm.choices_used, &mut queue, &mut seen);
        explored += 1;
    }
    eprintln!(
        "explored {test}: {explored} schedules, {} pending at budget {}",
        queue.len(),
        options.explore
    );
    Ok(())
}

// Session execution is private: it cannot weaken the public replay fingerprint checks.
pub(super) fn session(
    program: Program,
    root: &Path,
    tape: Option<&serde_json::Value>,
) -> Result<(Result<()>, serde_json::Value, serde_json::Value)> {
    let (result, tape, state, _) = session_mode(program, root, tape, false, 100_000)?;
    Ok((result, tape, state))
}
pub(super) fn session_mode(
    program: Program,
    root: &Path,
    tape: Option<&serde_json::Value>,
    strict: bool,
    budget: usize,
) -> Result<(Result<()>, serde_json::Value, serde_json::Value, usize)> {
    let options = RunOptions {
        virtual_publish: true,
        artifact: Some(serde_json::json!({"kind":"checked-repl"})),
        ..RunOptions::default()
    };
    let mut vm = Vm::new(
        program,
        root,
        io::Cursor::new(Vec::<u8>::new()),
        false,
        false,
        &options,
    )?;
    if let Some(tape) = tape {
        if strict {
            vm.engine.runtime.import_observations(tape)?;
        } else {
            vm.engine.runtime.import_session_observations(tape)?;
        }
    }
    vm.engine.remaining = budget;
    vm.steps = budget;
    vm.engine.runtime.set_budget(ResourceBudget {
        history_memory: 4 * 1024 * 1024,
        history_storage: 4 * 1024 * 1024,
        spill_threshold: 4 * 1024 * 1024,
    })?;
    let outcome = vm.run();
    let tape = vm.engine.runtime.export_observations()?;
    let mut state = vm.engine.runtime.debug_state();
    state["checkpoint_generations"] = serde_json::json!(vm
        .snapshots
        .iter()
        .map(|(name, s)| (name.clone(), s.generation))
        .collect::<BTreeMap<_, _>>());
    state["globals"] = serde_json::json!(vm
        .globals
        .iter()
        .flat_map(|scope| scope.iter())
        .map(|(name, binding)| (
            name.clone(),
            vm.engine.runtime.masked_value(&vm.resolve(&binding.value))
        ))
        .collect::<BTreeMap<_, _>>());
    state["stdout"] = serde_json::json!(vm.engine.runtime.masked_value(&Value::Text(
        String::from_utf8_lossy(&vm.engine.runtime.state().stdout.bytes()?).into_owned()
    )));
    state["stderr"] = serde_json::json!(vm.engine.runtime.masked_value(&Value::Text(
        String::from_utf8_lossy(&vm.engine.runtime.state().stderr.bytes()?).into_owned()
    )));
    let outcome = outcome.map_err(|e| match e {
        Error::Diagnostic(d) => {
            match serde_json::from_value(v06::diagnostics::masked_record(&d, &vm.engine.runtime)) {
                Ok(d) => Error::Diagnostic(Box::new(d)),
                Err(_) => Error::InvalidOperation(
                    vm.engine
                        .runtime
                        .masked_value(&Value::Text(d.message.clone())),
                ),
            }
        }
        e => Error::InvalidOperation(vm.engine.runtime.masked_value(&Value::Text(e.to_string()))),
    });
    Ok((
        outcome,
        tape,
        vm.engine.runtime.mask_debug_json(&state),
        budget - vm.steps,
    ))
}
