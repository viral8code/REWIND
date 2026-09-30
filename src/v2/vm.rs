use super::*;
use rewind::BranchAnchor;
mod scheduler;
use scheduler::Scheduler;

#[derive(Clone, Debug)]
enum Op {
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
    FailMatch,
    Halt,
}

#[derive(Clone)]
struct Inst {
    op: Op,
    at: Tok,
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
}
impl Compiler {
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
            Op::Jump(to) | Op::JumpFalse(to) | Op::JumpTrue(to) | Op::PatternTest(_, to) => {
                *to = target
            }
            _ => unreachable!(),
        }
    }
    fn enter(&mut self, at: &Tok) {
        self.emit(Op::Enter, at);
        self.depth += 1;
    }
    fn exit(&mut self, at: &Tok) {
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
        for stmt in &program.stmts {
            c.stmt(stmt)?;
        }
        let end = program.stmts.last().map(|s| s.at.clone()).unwrap_or(Tok {
            source: String::new(),
            text: "<eof>".into(),
            line: 1,
            col: 1,
        });
        c.emit(Op::Halt, &end);
        for (name, f) in &program.functions {
            c.type_params = f.type_params.clone();
            c.functions.insert(name.clone(), c.code.len());
            c.depth = 1;
            for stmt in &f.body {
                c.stmt(stmt)?;
            }
            c.emit(Op::Push(Value::Null), &f.at);
            c.emit(Op::Return, &f.at);
            c.depth = 0;
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
                c.type_params = f.type_params.clone();
                compiled.insert(name.clone());
                c.functions.insert(name, c.code.len());
                c.depth = 1;
                for stmt in &f.body {
                    c.stmt(stmt)?;
                }
                c.emit(Op::Push(Value::Null), &f.at);
                c.emit(Op::Return, &f.at);
                c.depth = 0;
            }
        }
        Ok(c)
    }
    fn stmt(&mut self, stmt: &Stmt) -> Result<()> {
        let at = &stmt.at;
        match &stmt.kind {
            StmtKind::Let(name, mutable, ty, value) => {
                self.expr(value)?;
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
                self.hidden += 1;
                let hidden = format!("$match{}", self.hidden);
                self.enter(at);
                self.expr(value)?;
                self.emit(Op::Declare(hidden.clone(), false, None), at);
                let mut ends = Vec::new();
                for (pattern, guard, body) in arms {
                    self.enter(at);
                    self.emit(Op::Load(hidden.clone()), at);
                    let miss = self.emit(
                        Op::PatternTest(resolve_pattern_alias(pattern, &self.aliases), 0),
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
                    if let Some(symbol) = self.aliases.get(&format!("{name}.{field}")).cloned() {
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
                        if let Some(symbol) = self.aliases.get(&qualified).cloned() {
                            for arg in args {
                                self.expr(arg)?;
                            }
                            self.emit(Op::Call(symbol, args.len()), &e.at);
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
                    self.emit(Op::Call(self.aliased(name), args.len()), &e.at);
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
                self.hidden += 1;
                let hidden = format!("$match{}", self.hidden);
                self.enter(&e.at);
                self.expr(value)?;
                self.emit(Op::Declare(hidden.clone(), false, None), &e.at);
                let mut ends = Vec::new();
                for (pattern, guard, arm) in arms {
                    self.enter(&e.at);
                    self.emit(Op::Load(hidden.clone()), &e.at);
                    let miss = self.emit(
                        Op::PatternTest(resolve_pattern_alias(pattern, &self.aliases), 0),
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
                        origin: PathBuf::new(),
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
}
impl Cleanup {
    fn depth(&self) -> usize {
        match self {
            Self::Expr(_, _, depth)
            | Self::Callable(_, depth)
            | Self::Handle(_, depth)
            | Self::Group(_, depth) => *depth,
        }
    }
}
#[derive(Clone)]
struct VmFrame {
    id: u64,
    name: String,
    ret_ty: String,
    return_pc: usize,
    scopes: Vec<BTreeMap<String, Binding>>,
    defers: Vec<Cleanup>,
}
#[derive(Clone)]
struct BranchFrame {
    anchor: BranchAnchor,
    globals: Vec<BTreeMap<String, Binding>>,
    frames: Vec<VmFrame>,
    scheduler: Scheduler,
}
#[derive(Clone)]
struct VmSnapshot {
    pc: usize,
    stack_len: usize,
    globals: Vec<BTreeMap<String, Binding>>,
    frames: Vec<VmFrame>,
    branches: Vec<BranchFrame>,
    global_cleanups: Vec<Cleanup>,
    scheduler: Scheduler,
}

struct Vm<R: BufRead> {
    engine: Engine<R>,
    code: Vec<Inst>,
    functions: BTreeMap<String, usize>,
    globals: Vec<BTreeMap<String, Binding>>,
    frames: Vec<VmFrame>,
    branches: Vec<BranchFrame>,
    snapshots: BTreeMap<String, VmSnapshot>,
    global_cleanups: Vec<Cleanup>,
    pc: usize,
    steps: usize,
    next_frame_id: u64,
    halt_pc: usize,
    specializations: BTreeMap<(String, Vec<String>), usize>,
    template_ends: BTreeMap<String, usize>,
    specialized_instructions: usize,
    scheduler: Scheduler,
    options: RunOptions,
    events: Vec<serde_json::Value>,
    inspections: Vec<serde_json::Value>,
    task_instructions: BTreeMap<u64, usize>,
    choice_widths: Vec<usize>,
    choices_used: Vec<usize>,
    fingerprint: String,
    entry: String,
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
            .unwrap_or_else(|| build_artifact(&program, root))?;
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
        engine.runtime.configure_environment(
            options.arguments.clone(),
            options.allowed_env.clone(),
            options.secret_env.clone(),
            options.locale.clone().unwrap_or_else(|| "en-US".into()),
        )?;
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
        engine.trace = trace;
        engine.trace_json = options.trace_json;
        engine.test_mode = test_mode;
        Ok(Self {
            engine,
            code: compiled.code,
            functions: compiled.functions,
            globals: vec![BTreeMap::new()],
            frames: Vec::new(),
            branches: Vec::new(),
            snapshots: BTreeMap::new(),
            global_cleanups: Vec::new(),
            pc: 0,
            steps: 1_000_000,
            next_frame_id: 1,
            halt_pc,
            specializations: BTreeMap::new(),
            template_ends,
            specialized_instructions: 0,
            scheduler: Scheduler::default(),
            options: options.clone(),
            events: Vec::new(),
            inspections: Vec::new(),
            task_instructions: BTreeMap::new(),
            choice_widths: Vec::new(),
            choices_used: Vec::new(),
            fingerprint,
            entry,
        })
    }
    fn error(&self, at: &Tok, message: impl AsRef<str>) -> Error {
        diagnostic(at, message)
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
        let actual = value_type(&value, &self.engine.runtime);
        let ty = ty.unwrap_or(actual.clone());
        if !compatible(&ty, &actual) {
            return Err(self.error(at, format!("type mismatch: expected {ty}, found {actual}")));
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
        let value = if op == "=" {
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
            pc: self.pc,
            stack_len: self.engine.runtime.state().stack.len(),
            globals: self.globals.clone(),
            frames: self.frames.clone(),
            branches: self.branches.clone(),
            global_cleanups: self.global_cleanups.clone(),
            scheduler: self.scheduler.clone(),
        }
    }
    fn restore(&mut self, snap: VmSnapshot) {
        self.pc = snap.pc;
        self.globals = snap.globals;
        self.frames = snap.frames;
        self.branches = snap.branches;
        self.global_cleanups = snap.global_cleanups;
        self.scheduler = snap.scheduler;
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
        for cleanup in pending.into_iter().rev() {
            if let Err(error) = self.run_cleanup(cleanup, at) {
                if first_error.is_none() {
                    first_error = Some(error);
                } else {
                    suppressed.push(error.to_string());
                }
            }
        }
        if self.engine.program.language == "0.5" {
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
        for ((param, ty), arg) in def.params.iter().zip(args) {
            let actual = value_type(&arg, &self.engine.runtime);
            let expected = substitute_type(ty, &substitutions);
            if !compatible(&expected, &actual) {
                return Err(self.error(at, format!("{name} expects {expected}, found {actual}")));
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
            scopes: vec![scope],
            defers: Vec::new(),
        });
        self.next_frame_id += 1;
        self.pc = target;
        Ok(())
    }
    fn finish_call(&mut self, value: Value, at: &Tok) -> Result<()> {
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
        let deferred = std::mem::take(&mut self.frames.last_mut().unwrap().defers);
        self.run_cleanups(deferred, at)?;
        self.frames.pop();
        self.engine.runtime.pop_frame()?;
        self.pc = frame.return_pc;
        self.push(value)
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
                    for frame in self.frames.iter().rev().cloned().collect::<Vec<_>>() {
                        if let Err(e) = self.run_cleanups(frame.defers, &at) {
                            causes.push(e.to_string());
                        }
                    }
                    let pending = std::mem::take(&mut self.global_cleanups);
                    if let Err(e) = self.run_cleanups(pending, &at) {
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
                    match self.finish_scheduled_task(Err(message), &at) {
                        Ok(true) => continue,
                        Ok(false) => break Ok(()),
                        Err(error) => break Err(error),
                    }
                }
                result => break result,
            }
        };
        if result.is_ok() && self.engine.program.language == "0.5" {
            if let Some(error) = self.unhandled_task_error() {
                result = Err(Error::InvalidOperation(error));
            }
        }
        if result.is_err() {
            let mut causes = Vec::new();
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
                    causes.push(e.to_string());
                }
            }
            if self.engine.program.language == "0.5" && !causes.is_empty() {
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
        let outcome = serde_json::json!({"ok":execution_error.is_none(),"error":execution_error.map(|e|self.engine.runtime.masked_value(&Value::Text(e.to_string())))});
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
            let trace = serde_json::json!({"format":1,"compiler":env!("CARGO_PKG_VERSION"),"test":self.options.recorded_test,"entry":self.entry,"fingerprint":self.fingerprint,"task_steps":self.options.task_steps.unwrap_or(100_000),"schedule_choices":self.choices_used,"result":outcome,"observations":self.engine.runtime.export_observations()?,"events":self.events,"state_digest":digest,"virtual_publish":self.options.inspect||self.options.virtual_publish,"debug":{"checkpoints":self.inspections,"final":{"runtime":self.engine.runtime.debug_state(),"scheduler":self.scheduler.debug_json()}}});
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
                serde_json::json!({"task_instructions":self.task_instructions.iter().map(|(id,count)|(id.to_string(),*count)).collect::<BTreeMap<_,_>>(),"runtime":self.engine.runtime.debug_state(),"scheduler":self.scheduler.debug_json(),"schedule_choices":self.choices_used})
            );
        }
        Ok(())
    }
    fn run_inner(&mut self) -> Result<()> {
        loop {
            if self
                .options
                .pause_after
                .is_some_and(|limit| self.events.len() >= limit)
            {
                return Ok(());
            }
            if self.cancellation_requested() {
                return Err(self.error(&self.code[self.pc].at, "TaskCancelled"));
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
            self.steps -= 1;
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
                let event = serde_json::json!({"task":self.scheduler.active,"pc":self.pc,"operation":operation});
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
                    let task = self.pop()?;
                    self.start_task(&task, &inst.at)?;
                    self.push(task)?;
                }
                Op::Await => {
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
                            ),
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
                        ("move" | "borrow" | "borrowMut", v) => v,
                        _ => return Err(self.error(&inst.at, "invalid unary operand")),
                    };
                    self.push(result)?;
                }
                Op::Binary(op) => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    self.push(unflow(binary(&op, lhs, rhs, &inst.at), &inst.at)?)?;
                }
                Op::Property(field) => {
                    let value = self.pop()?;
                    if let Some(Value::Struct(_, fields)) = v05::unfrozen(&value) {
                        let value = fields
                            .get(&field)
                            .cloned()
                            .ok_or_else(|| self.error(&inst.at, "unknown frozen field"))?;
                        self.push(v05::freeze_member(value, &self.engine.runtime))?;
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
                    self.push(result)?;
                }
                Op::Call(name, count) => {
                    let args = self.args(count)?;
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
                            if self.engine.program.language == "0.5" && !needed.contains(name) {
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
                                if self.engine.program.language == "0.5" && !needed.contains(name) {
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
                    let id = self
                        .engine
                        .runtime
                        .alloc(Value::Closure(name, ty, captures))?;
                    self.push(Value::HeapRef(id))?;
                }
                Op::Method(method, count) => {
                    let mut args = self.args(count)?;
                    let target = self.pop()?;
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
                        self.engine.method(target, &method, args, &inst.at),
                        &inst.at,
                    )?;
                    self.push(value)?;
                }
                Op::Builtin(receiver, method, count) => {
                    let args = self.args(count)?;
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
                    self.pop()?;
                }
                Op::Enter => {
                    if let Some(frame) = self.frames.last_mut() {
                        frame.scopes.push(BTreeMap::new());
                    } else {
                        self.globals.push(BTreeMap::new());
                    }
                }
                Op::Exit => {
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
                    let failed = self.engine.program.language == "0.5"
                        && matches!(
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
                    let snap = self.snapshots.get(&name).cloned().ok_or_else(|| {
                        self.error(&inst.at, format!("unknown checkpoint {name}"))
                    })?;
                    let saved_ids = snap.frames.iter().map(|f| f.id).collect::<Vec<_>>();
                    let current_ids = self.frames.iter().map(|f| f.id).collect::<Vec<_>>();
                    let scopes_match = snap.globals.len() == self.globals.len()
                        && snap
                            .frames
                            .iter()
                            .zip(&self.frames)
                            .all(|(a, b)| a.scopes.len() == b.scopes.len());
                    if saved_ids != current_ids
                        || !scopes_match
                        || snap.stack_len != self.engine.runtime.state().stack.len()
                        || snap.branches.len() != self.branches.len()
                    {
                        return Err(self.error(
                            &inst.at,
                            "InvalidContinuation: call frame is no longer present",
                        ));
                    }
                    let after = self.pc;
                    self.engine.runtime.revert(&name)?;
                    self.restore(snap);
                    self.pc = after;
                    self.engine.runtime.set_program_counter(after);
                }
                Op::Resume(name) => {
                    let snap = self.snapshots.get(&name).cloned().ok_or_else(|| {
                        self.error(&inst.at, format!("unknown checkpoint {name}"))
                    })?;
                    self.engine.runtime.revert(&name)?;
                    self.restore(snap);
                }
                Op::Drop(name) => {
                    self.engine.runtime.drop_checkpoint(&name)?;
                    self.snapshots.remove(&name);
                }
                Op::Publish(force) => {
                    if self.engine.program.language == "0.5" {
                        if let Some(error) = self.unhandled_task_error() {
                            return Err(self.error(&inst.at, error));
                        }
                    }
                    if self.scheduler.active != 0 || self.scheduler.has_live_tasks() {
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
                    self.branches.push(BranchFrame {
                        anchor: self.engine.runtime.begin_branch(),
                        globals: self.globals.clone(),
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
                Op::PatternTest(pattern, miss) => {
                    let value = self.pop()?;
                    let mut bindings = BTreeMap::new();
                    if pattern_matches(&pattern, &value, &self.engine.runtime, &mut bindings) {
                        for (name, value) in bindings {
                            self.declare(name, value, false, None, &inst.at)?;
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
            if result.is_ok() || program.language == "0.5" {
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
            || vm.engine.program.language == "0.5"
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
    let document = super::documentation(&program.root_origin.to_string_lossy(), root)?;
    let lock = root.join("rewind.lock");
    Ok(
        serde_json::json!({"format":1,"compiler":env!("CARGO_PKG_VERSION"),"sources":sources,"lock_sha256":if lock.exists(){Some(packages::hash(fs::read(lock)?))}else{None},"api_document_sha256":packages::hash(document),"functions":functions,"function_entries":compiler.functions,"bytecode":code}),
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
        if run.is_err() && program.language == "0.5" {
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
