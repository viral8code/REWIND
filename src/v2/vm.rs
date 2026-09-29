use super::*;
use rewind::BranchAnchor;

#[derive(Clone)]
enum Op {
    Push(Value),
    Load(String),
    Declare(String, bool, Option<String>),
    Store(String, String),
    StoreField(String, String),
    Unary(String),
    Binary(String),
    Property(String),
    Call(String, usize),
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
    Commit(String),
    Revert(String),
    Resume(String),
    Drop(String),
    Publish(bool),
    BeginBranch,
    EndBranch(String),
    Runtime(ResourceBudget, Option<usize>),
    Match(Vec<(String, usize)>),
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
}
impl Compiler {
    fn new() -> Self {
        Self {
            code: Vec::new(),
            functions: BTreeMap::new(),
            loops: Vec::new(),
            depth: 0,
            hidden: 0,
        }
    }
    fn emit(&mut self, op: Op, at: &Tok) -> usize {
        let pos = self.code.len();
        self.code.push(Inst { op, at: at.clone() });
        pos
    }
    fn patch(&mut self, index: usize, target: usize) {
        match &mut self.code[index].op {
            Op::Jump(to) | Op::JumpFalse(to) | Op::JumpTrue(to) => *to = target,
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
        for stmt in &program.stmts {
            c.stmt(stmt)?;
        }
        let end = program.stmts.last().map(|s| s.at.clone()).unwrap_or(Tok {
            text: "<eof>".into(),
            line: 1,
            col: 1,
        });
        c.emit(Op::Halt, &end);
        for (name, f) in &program.functions {
            c.functions.insert(name.clone(), c.code.len());
            c.depth = 1;
            for stmt in &f.body {
                c.stmt(stmt)?;
            }
            c.emit(Op::Push(Value::Null), &f.at);
            c.emit(Op::Return, &f.at);
            c.depth = 0;
        }
        Ok(c)
    }
    fn stmt(&mut self, stmt: &Stmt) -> Result<()> {
        let at = &stmt.at;
        match &stmt.kind {
            StmtKind::Let(name, mutable, ty, value) => {
                self.expr(value)?;
                self.emit(Op::Declare(name.clone(), *mutable, ty.clone()), at);
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
                self.emit(Op::Defer(expr.clone()), at);
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
                self.expr(value)?;
                let dispatch = self.emit(Op::Match(Vec::new()), at);
                let mut entries = Vec::new();
                let mut ends = Vec::new();
                for (variant, binding, expr) in arms {
                    entries.push((variant.clone(), self.code.len()));
                    self.enter(at);
                    self.emit(Op::Declare(binding.clone(), false, None), at);
                    self.expr(expr)?;
                    self.emit(Op::Pop, at);
                    self.exit(at);
                    ends.push(self.emit(Op::Jump(0), at));
                }
                self.code[dispatch].op = Op::Match(entries);
                let end = self.code.len();
                for jump in ends {
                    self.patch(jump, end);
                }
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
                self.emit(Op::Load(name.clone()), &e.at);
            }
            ExprKind::Unary(op, inner) => {
                self.expr(inner)?;
                self.emit(Op::Unary(op.clone()), &e.at);
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
                self.expr(base)?;
                self.emit(Op::Property(field.clone()), &e.at);
            }
            ExprKind::Call(target, args) => {
                if let ExprKind::Member(base, method) = &target.kind {
                    if let ExprKind::Name(receiver) = &base.kind {
                        if matches!(
                            receiver.as_str(),
                            "Out" | "Err" | "File" | "Directory" | "In" | "Time" | "Random"
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
                    self.emit(Op::Call(name.clone(), args.len()), &e.at);
                } else {
                    return Err(diagnostic(&e.at, "invalid call target"));
                }
            }
            ExprKind::Try(inner) => {
                self.expr(inner)?;
                self.emit(Op::Try, &e.at);
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct VmFrame {
    id: u64,
    name: String,
    return_pc: usize,
    scopes: Vec<BTreeMap<String, Binding>>,
    defers: Vec<(Expr, Vec<BTreeMap<String, Binding>>)>,
}
#[derive(Clone)]
struct BranchFrame {
    anchor: BranchAnchor,
    globals: Vec<BTreeMap<String, Binding>>,
    frames: Vec<VmFrame>,
}
#[derive(Clone)]
struct VmSnapshot {
    pc: usize,
    stack_len: usize,
    globals: Vec<BTreeMap<String, Binding>>,
    frames: Vec<VmFrame>,
    branches: Vec<BranchFrame>,
}

struct Vm<R: BufRead> {
    engine: Engine<R>,
    code: Vec<Inst>,
    functions: BTreeMap<String, usize>,
    globals: Vec<BTreeMap<String, Binding>>,
    frames: Vec<VmFrame>,
    branches: Vec<BranchFrame>,
    snapshots: BTreeMap<String, VmSnapshot>,
    pc: usize,
    steps: usize,
    next_frame_id: u64,
    halt_pc: usize,
}

impl<R: BufRead> Vm<R> {
    fn new(program: Program, root: &Path, input: R, trace: bool, test_mode: bool) -> Result<Self> {
        let compiled = Compiler::compile(&program)?;
        let halt_pc = compiled
            .code
            .iter()
            .position(|i| matches!(i.op, Op::Halt))
            .unwrap();
        let mut engine = Engine::new(program, root, input)?;
        engine.trace = trace;
        engine.test_mode = test_mode;
        Ok(Self {
            engine,
            code: compiled.code,
            functions: compiled.functions,
            globals: vec![BTreeMap::new()],
            frames: Vec::new(),
            branches: Vec::new(),
            snapshots: BTreeMap::new(),
            pc: 0,
            steps: 1_000_000,
            next_frame_id: 1,
            halt_pc,
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
        let binding = Binding {
            value: value.clone(),
            mutable,
            ty,
        };
        if let Some(frame) = self.frames.last_mut() {
            let scope = frame.scopes.last_mut().unwrap();
            if scope.contains_key(&name) {
                return Err(self.error(at, format!("duplicate binding {name}")));
            }
            scope.insert(name.clone(), binding);
            self.engine.runtime.set_local(name, value)?;
        } else {
            if self.globals.last().unwrap().contains_key(&name) {
                return Err(self.error(at, format!("duplicate binding {name}")));
            }
            self.globals
                .last_mut()
                .unwrap()
                .insert(name.clone(), binding);
            self.engine.runtime.set_global(name, value)?;
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
            unflow(binary(&op[..1], old.value, rhs, at), at)?
        };
        let actual = value_type(&value, &self.engine.runtime);
        if !compatible(&old.ty, &actual) {
            return Err(self.error(
                at,
                format!("type mismatch: expected {}, found {actual}", old.ty),
            ));
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
        }
    }
    fn restore(&mut self, snap: VmSnapshot) {
        self.pc = snap.pc;
        self.globals = snap.globals;
        self.frames = snap.frames;
        self.branches = snap.branches;
    }
    fn call_user(&mut self, name: &str, args: Vec<Value>, at: &Tok) -> Result<()> {
        let def = self
            .engine
            .program
            .functions
            .get(name)
            .ok_or_else(|| self.error(at, format!("unknown function {name}")))?;
        if args.len() != def.params.len() {
            return Err(self.error(at, format!("{name} expects {} arguments", def.params.len())));
        }
        if self.frames.len() >= 1024 {
            return Err(self.error(at, "ExecutionBudgetExceeded: call depth"));
        }
        self.engine.runtime.push_frame(self.pc)?;
        let mut scope = BTreeMap::new();
        for ((param, ty), arg) in def.params.iter().zip(args) {
            let actual = value_type(&arg, &self.engine.runtime);
            if !compatible(ty, &actual) {
                return Err(self.error(at, format!("{name} expects {ty}, found {actual}")));
            }
            self.engine.runtime.set_local(param.clone(), arg.clone())?;
            scope.insert(
                param.clone(),
                Binding {
                    value: arg,
                    mutable: false,
                    ty: ty.clone(),
                },
            );
        }
        self.frames.push(VmFrame {
            id: self.next_frame_id,
            name: name.into(),
            return_pc: self.pc,
            scopes: vec![scope],
            defers: Vec::new(),
        });
        self.next_frame_id += 1;
        self.pc = self.functions[name];
        Ok(())
    }
    fn finish_call(&mut self, value: Value, at: &Tok) -> Result<()> {
        let Some(frame) = self.frames.last().cloned() else {
            return Err(self.error(at, "return outside function"));
        };
        let expected = &self.engine.program.functions[&frame.name].ret;
        let actual = value_type(&value, &self.engine.runtime);
        if !compatible(expected, &actual) {
            return Err(self.error(
                at,
                format!("{} returns {actual}, expected {expected}", frame.name),
            ));
        }
        let deferred = std::mem::take(&mut self.frames.last_mut().unwrap().defers);
        for (expr, captured) in deferred.iter().rev() {
            self.engine.scopes = self.globals.clone();
            self.engine.scopes.extend(captured.clone());
            self.engine.function_depth = self.frames.len();
            let result = self.engine.eval(expr);
            unflow(result, at)?;
        }
        self.frames.pop();
        self.engine.runtime.pop_frame()?;
        self.pc = frame.return_pc;
        self.push(value)
    }
    fn run(&mut self) -> Result<()> {
        let result = self.run_inner();
        if result.is_err() {
            for frame in self.frames.iter().rev().cloned().collect::<Vec<_>>() {
                for (expr, captured) in frame.defers.iter().rev() {
                    self.engine.scopes = self.globals.clone();
                    self.engine.scopes.extend(captured.clone());
                    self.engine.function_depth = self.frames.len();
                    let _ = self.engine.eval(expr);
                }
            }
        }
        result
    }
    fn run_inner(&mut self) -> Result<()> {
        loop {
            if self.steps == 0 {
                return Err(self.error(&self.code[self.pc].at, "ExecutionBudgetExceeded"));
            }
            self.steps -= 1;
            let inst = self.code.get(self.pc).cloned().ok_or_else(|| {
                Error::InvalidOperation(format!("invalid program counter {}", self.pc))
            })?;
            self.engine.runtime.set_program_counter(self.pc);
            self.pc += 1;
            match inst.op {
                Op::Halt => return Ok(()),
                Op::Push(value) => self.push(value)?,
                Op::Load(name) => {
                    let value = self
                        .get(&name)
                        .map(|b| b.value.clone())
                        .ok_or_else(|| self.error(&inst.at, format!("unknown name {name}")))?;
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
                    let def = &self.engine.program.structs[name];
                    let expected = &def.fields.iter().find(|(n, _)| n == &field).unwrap().1;
                    let actual = value_type(&value, &self.engine.runtime);
                    if !compatible(expected, &actual) {
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
                    let result = match value {
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
                        _ => return Err(self.error(&inst.at, "unknown property")),
                    };
                    self.push(result)?;
                }
                Op::Call(name, count) => {
                    let args = self.args(count)?;
                    if self.functions.contains_key(&name) {
                        self.call_user(&name, args, &inst.at)?;
                    } else {
                        let value = unflow(self.engine.call(&name, args, &inst.at), &inst.at)?;
                        self.push(value)?;
                    }
                }
                Op::Method(method, count) => {
                    let args = self.args(count)?;
                    let target = self.pop()?;
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
                    let value = self.pop()?;
                    self.finish_call(value, &inst.at)?;
                }
                Op::Defer(expr) => {
                    let Some(frame) = self.frames.last_mut() else {
                        return Err(self.error(&inst.at, "defer requires a function"));
                    };
                    frame.defers.push((expr, frame.scopes.clone()));
                }
                Op::Commit(name) => {
                    self.engine.runtime.set_program_counter(self.pc);
                    self.engine.runtime.commit(name.clone())?;
                    self.snapshots.insert(name.clone(), self.snapshot());
                    if self.engine.trace {
                        eprintln!("{}", self.engine.runtime.trace_checkpoint(&name)?);
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
                    if self.engine.trace {
                        eprintln!("{}", self.engine.runtime.trace_checkpoint(&name)?);
                    }
                }
                Op::Runtime(budget, steps) => {
                    self.engine.runtime.set_budget(budget)?;
                    if let Some(steps) = steps {
                        self.steps = steps;
                    }
                }
                Op::Match(arms) => {
                    let value = self.pop()?;
                    let (variant, inner) = match value {
                        Value::Result(Ok(v)) => ("Ok", *v),
                        Value::Result(Err(v)) => ("Err", *v),
                        Value::Option(Some(v)) => ("Some", *v),
                        Value::Option(None) => ("None", Value::Null),
                        _ => return Err(self.error(&inst.at, "match expects Result or Option")),
                    };
                    let target = arms
                        .iter()
                        .find(|(n, _)| n == variant)
                        .map(|(_, to)| *to)
                        .ok_or_else(|| self.error(&inst.at, "non-exhaustive match"))?;
                    self.push(inner)?;
                    self.pc = target;
                }
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

pub(super) fn execute(program: Program, root: &Path, trace: bool, mode: &str) -> Result<()> {
    if mode == "test" {
        let tests = program
            .functions
            .iter()
            .filter(|(_, f)| f.test)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for test in tests {
            let mut vm = Vm::new(program.clone(), root, io::stdin().lock(), trace, true)?;
            vm.run()?;
            let at = program.functions[&test].at.clone();
            vm.pc = vm.halt_pc;
            vm.call_user(&test, Vec::new(), &at)?;
            vm.run()?;
            eprintln!("ok {test}");
        }
        Ok(())
    } else {
        Vm::new(program, root, io::stdin().lock(), trace, false)?.run()
    }
}
pub(super) fn validate(program: &Program) -> Result<()> {
    Compiler::compile(program).map(|_| ())
}
