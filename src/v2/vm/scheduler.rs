use super::*;
use std::collections::VecDeque;
use std::sync::Arc;
pub(super) mod gc;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum TaskPhase {
    Cold,
    Ready,
    Running,
    Waiting(u64),
    WaitingChannel,
    Done,
}
#[derive(Clone)]
enum TaskBody {
    Main,
    HostOperation(usize),
    Function(String, Vec<Value>),
    Send(u64, Value),
    Receive(u64),
    Join(u64),
    Select(u64, u64),
    Timeout(u64, u64, Option<u64>),
}
#[derive(Clone)]
struct Context {
    pc: usize,
    globals: Vec<BTreeMap<String, Binding>>,
    global_scope_ids: Vec<u64>,
    frames: Vec<VmFrame>,
    branches: Vec<BranchFrame>,
    cleanups: Vec<Cleanup>,
    stack: Arc<Vec<Value>>,
    runtime_frames: Arc<Vec<rewind::CallFrame>>,
    runtime_globals: Arc<BTreeMap<String, Value>>,
}
#[derive(Clone)]
struct Task {
    at: Tok,
    timed_out: bool,
    observed: bool,
    ignored: bool,
    cancel_requested: bool,
    phase: TaskPhase,
    body: TaskBody,
    priority: i64,
    context: Option<Context>,
    result: Option<std::result::Result<Value, String>>,
    failure: Option<rewind::DiagnosticRecord>,
    globals: Vec<BTreeMap<String, Binding>>,
    global_scope_ids: Vec<u64>,
    parent: u64,
}
#[derive(Clone)]
struct Channel {
    capacity: usize,
    closed: bool,
    values: VecDeque<Value>,
}
#[derive(Clone)]
pub(super) struct Scheduler {
    pub active: u64,
    pub(super) ticks: u64,
    next_id: u64,
    since_gc: usize,
    tasks: BTreeMap<u64, Task>,
    channels: BTreeMap<u64, Channel>,
    groups: BTreeMap<u64, Vec<u64>>,
}
impl Default for Scheduler {
    fn default() -> Self {
        let main = Task {
            at: Tok {
                source: String::new(),
                text: String::new(),
                line: 1,
                col: 1,
            },
            timed_out: false,
            observed: true,
            ignored: false,
            cancel_requested: false,
            phase: TaskPhase::Running,
            body: TaskBody::Main,
            priority: 0,
            context: None,
            result: None,
            failure: None,
            globals: Vec::new(),
            global_scope_ids: Vec::new(),
            parent: 0,
        };
        Self {
            active: 0,
            ticks: 0,
            next_id: 1,
            since_gc: 0,
            tasks: BTreeMap::from([(0, main)]),
            channels: BTreeMap::new(),
            groups: BTreeMap::new(),
        }
    }
}
impl Scheduler {
    pub(super) fn host_operations(&self) -> BTreeSet<usize> {
        self.tasks
            .values()
            .filter_map(|t| {
                if t.phase != TaskPhase::Done {
                    if let TaskBody::HostOperation(id) = t.body {
                        return (id != usize::MAX).then_some(id);
                    }
                }
                None
            })
            .collect()
    }
    pub(super) fn gc_roots(&self, out: &mut Vec<Value>) {
        for task in self.tasks.values() {
            if task.phase != TaskPhase::Done {
                gc_scope_roots(&task.globals, out);
            }
            if let Some(Ok(value)) = &task.result {
                out.push(value.clone());
            }
            match &task.body {
                TaskBody::Function(_, args) if task.phase != TaskPhase::Done => {
                    out.extend(args.iter().cloned())
                }
                TaskBody::Send(_, value) if task.phase != TaskPhase::Done => {
                    out.push(value.clone())
                }
                _ => {}
            }
            if let Some(context) = &task.context {
                gc_scope_roots(&context.globals, out);
                gc_frame_roots(&context.frames, out);
                gc_cleanup_roots(&context.cleanups, out);
                gc_branch_roots(&context.branches, out);
                out.extend(context.stack.iter().cloned());
                out.extend(context.runtime_globals.values().cloned());
                out.extend(
                    context
                        .runtime_frames
                        .iter()
                        .flat_map(|f| f.locals.values().cloned()),
                );
            }
        }
        for channel in self.channels.values() {
            out.extend(channel.values.iter().cloned());
        }
    }
    pub(super) fn wait_edges(&self) -> Vec<rewind::WaitEdge> {
        let mut edges = Vec::new();
        for (id, t) in &self.tasks {
            let targets = match (&t.phase, &t.body) {
                (TaskPhase::Waiting(on), _) => vec![rewind::WaitTarget::Task(*on)],
                (
                    TaskPhase::WaitingChannel,
                    TaskBody::Send(channel, _) | TaskBody::Receive(channel),
                ) => vec![rewind::WaitTarget::Channel(*channel)],
                (TaskPhase::WaitingChannel, TaskBody::Join(group)) => {
                    vec![rewind::WaitTarget::Group(*group)]
                }
                (TaskPhase::WaitingChannel, TaskBody::Select(a, b)) => {
                    vec![rewind::WaitTarget::Task(*a), rewind::WaitTarget::Task(*b)]
                }
                (TaskPhase::WaitingChannel, TaskBody::Timeout(task, _, _)) => {
                    vec![rewind::WaitTarget::Task(*task)]
                }
                _ => Vec::new(),
            };
            edges.extend(
                targets
                    .into_iter()
                    .map(|target| rewind::WaitEdge { task: *id, target }),
            );
        }
        edges
    }
    // A conservative logical byte count, including retained task contexts.
    // Checkpoint copies are charged separately by the VM; this is not RSS.
    pub(super) fn storage_bytes(&self) -> usize {
        self.storage_bytes_using(&|v| {
            std::mem::size_of::<Value>().saturating_add(rewind::Runtime::value_bytes(v))
        })
    }
    pub(super) fn accounted_storage_bytes(&self, runtime: &rewind::Runtime) -> usize {
        self.storage_bytes_using(&|v| {
            std::mem::size_of::<Value>().saturating_add(runtime.retained_payload_bytes(v))
        })
    }
    fn storage_bytes_using<F: Fn(&Value) -> usize>(&self, value: &F) -> usize {
        fn bindings<F: Fn(&Value) -> usize>(
            scopes: &[BTreeMap<String, Binding>],
            value: &F,
        ) -> usize {
            scopes
                .iter()
                .flat_map(|s| s.iter())
                .map(|(n, b)| {
                    n.len()
                        .saturating_add(b.ty.len())
                        .saturating_add(value(&b.value))
                })
                .sum()
        }
        self.tasks
            .values()
            .map(|t| {
                let body = match &t.body {
                    TaskBody::Function(n, args) => n.len() + args.iter().map(value).sum::<usize>(),
                    TaskBody::Send(_, v) => value(v),
                    _ => 0,
                };
                let result = t.result.as_ref().map_or(0, |r| match r {
                    Ok(v) => value(v),
                    Err(e) => e.len(),
                });
                let context = t.context.as_ref().map_or(0, |c| {
                    bindings(&c.globals, value)
                        + c.stack.iter().map(value).sum::<usize>()
                        + c.runtime_globals
                            .iter()
                            .map(|(n, v)| n.len() + value(v))
                            .sum::<usize>()
                        + c.runtime_frames
                            .iter()
                            .map(|f| {
                                f.locals
                                    .iter()
                                    .map(|(n, v)| n.len() + value(v))
                                    .sum::<usize>()
                            })
                            .sum::<usize>()
                        + c.frames
                            .iter()
                            .map(|f| bindings(&f.scopes, value) + f.defers.len() * 128 + 128)
                            .sum::<usize>()
                        + c.branches
                            .iter()
                            .map(|b| b.scheduler.storage_bytes_using(value))
                            .sum::<usize>()
                        + c.cleanups.len() * 128
                });
                256 + body + result + context + bindings(&t.globals, value)
            })
            .sum::<usize>()
            + self
                .channels
                .values()
                .map(|c| 128 + c.values.iter().map(value).sum::<usize>())
                .sum::<usize>()
            + self
                .groups
                .values()
                .map(|g| 128 + g.len() * 8)
                .sum::<usize>()
    }
    pub(super) fn has_live_tasks(&self) -> bool {
        self.tasks
            .iter()
            .any(|(id, t)| *id != 0 && !matches!(t.phase, TaskPhase::Done | TaskPhase::Cold))
    }
    pub(super) fn reactivate_main(&mut self) {
        let main = self.tasks.get_mut(&0).unwrap();
        main.phase = TaskPhase::Running;
        main.result = None;
    }
    pub(super) fn debug_json(&self) -> serde_json::Value {
        serde_json::json!({"active":self.active,"logical_storage_bytes":self.storage_bytes(),"tasks":self.tasks.iter().map(|(id,t)|(id.to_string(),serde_json::json!({"phase":format!("{:?}",t.phase),"priority":t.priority,"parent":t.parent,"pc":t.context.as_ref().map(|c|c.pc),"frames":t.context.as_ref().map(|c|c.frames.len())}))).collect::<BTreeMap<_,_>>(),"channels":self.channels.iter().map(|(id,c)|(id.to_string(),serde_json::json!({"capacity":c.capacity,"queued":c.values.len(),"closed":c.closed}))).collect::<BTreeMap<_,_>>()})
    }
}
fn handle(ty: String, id: u64) -> Value {
    Value::Struct(ty, BTreeMap::from([("$id".into(), Value::Int(id as i64))]))
}
pub(super) fn handle_id(value: &Value) -> Option<u64> {
    if let Value::Struct(_, fields) = value {
        if let Some(Value::Int(id)) = fields.get("$id") {
            return u64::try_from(*id).ok();
        }
    }
    None
}
impl<R: BufRead> Vm<R> {
    pub(super) fn task_frame_views(&self) -> BTreeMap<String, serde_json::Value> {
        self.scheduler.tasks.iter().map(|(id,t)| {
            let frames = t.context.as_ref().map(|c| c.frames.as_slice()).unwrap_or(&[]);
            (id.to_string(), serde_json::json!(frames.iter().map(|f|serde_json::json!({"function":f.name,"return_pc":f.return_pc,"scopes":f.scopes.iter().map(|scope|scope.iter().map(|(n,b)|(n.clone(),self.engine.runtime.masked_value(&self.resolve(&b.value)))).collect::<BTreeMap<_,_>>()).collect::<Vec<_>>()})).collect::<Vec<_>>()))
        }).collect()
    }

    pub(super) fn cancel_children(&mut self, at: &Tok) -> Result<()> {
        let children = self
            .scheduler
            .tasks
            .iter()
            .filter(|(id, t)| {
                **id != self.scheduler.active
                    && t.parent == self.scheduler.active
                    && t.phase != TaskPhase::Done
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in children.into_iter().rev() {
            self.cancel_task(id, at)?;
        }
        Ok(())
    }
    fn capture_context(&self) -> Context {
        Context {
            pc: self.pc,
            globals: self.globals.clone(),
            global_scope_ids: self.global_scope_ids.clone(),
            frames: self.frames.clone(),
            branches: self.branches.clone(),
            cleanups: self.global_cleanups.clone(),
            stack: self.engine.runtime.state().stack.clone(),
            runtime_frames: self.engine.runtime.state().call_frames.clone(),
            runtime_globals: self.engine.runtime.state().globals.clone(),
        }
    }
    fn install_context(&mut self, context: Context) -> Result<()> {
        self.pc = context.pc;
        self.globals = context.globals;
        self.global_scope_ids = context.global_scope_ids;
        self.frames = context.frames;
        self.branches = context.branches;
        self.global_cleanups = context.cleanups;
        self.engine.runtime.set_task_compute(
            context.stack,
            context.runtime_frames,
            context.runtime_globals,
        )
    }
    fn next_scheduler_id(&mut self, at: &Tok) -> Result<u64> {
        if self.scheduler.tasks.len() + self.scheduler.channels.len() + self.scheduler.groups.len()
            >= 4096
        {
            return Err(self.error(at, "TaskBudgetExceeded: objects"));
        }
        let id = self.scheduler.next_id;
        self.scheduler.next_id += 1;
        self.scheduler.since_gc = self.scheduler.since_gc.saturating_add(1);
        Ok(id)
    }
    fn new_action(
        &mut self,
        body: TaskBody,
        ty: String,
        globals: Vec<BTreeMap<String, Binding>>,
        at: &Tok,
    ) -> Result<Value> {
        let id = self.next_scheduler_id(at)?;
        self.scheduler.tasks.insert(
            id,
            Task {
                at: at.clone(),
                timed_out: false,
                observed: false,
                ignored: false,
                cancel_requested: false,
                phase: TaskPhase::Cold,
                body,
                priority: 0,
                context: None,
                result: None,
                failure: None,
                globals,
                global_scope_ids: self.global_scope_ids.clone(),
                parent: self.scheduler.active,
            },
        );
        Ok(handle(format!("Task<{ty}>"), id))
    }
    pub(super) fn create_async_task(
        &mut self,
        name: &str,
        args: Vec<Value>,
        at: &Tok,
    ) -> Result<Value> {
        let def = &self.engine.program.functions[name.split('<').next().unwrap_or(name)];
        let params = def
            .type_params
            .iter()
            .map(|(p, _)| p.clone())
            .collect::<Vec<_>>();
        let actual = args
            .iter()
            .map(|v| value_type(v, &self.engine.runtime))
            .collect::<Vec<_>>();
        let substitutions = infer_call_arguments(name, &def.params, &params, &actual, at)?;
        let ty = substitute_type(&def.ret, &substitutions);
        let v5 = matches!(
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
                | "2.0.0"
        );
        let mut needed = v05::needed_globals(&self.engine.program, name);
        if v5 {
            for arg in &args {
                if let Value::Function(symbol, _) = self.resolve(arg) {
                    needed.extend(v05::needed_globals(&self.engine.program, &symbol));
                }
            }
            loop {
                let old = needed.clone();
                for scope in &self.globals {
                    for (n, b) in scope {
                        if needed.contains(n) {
                            if let Value::Function(symbol, _) = self.resolve(&b.value) {
                                needed.extend(v05::needed_globals(&self.engine.program, &symbol));
                            }
                        }
                    }
                }
                if old == needed {
                    break;
                }
            }
        }
        let args = args
            .iter()
            .map(|v| {
                if v5 {
                    return v05::task_copy(&mut self.engine.runtime, v);
                }
                let snapshot = unflow(self.engine.ordered_key(v, at), at)?;
                if v5 && v05::unfrozen(&snapshot).is_none() {
                    v05::thaw(&mut self.engine.runtime, &snapshot)
                } else {
                    Ok(snapshot)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let mut globals = Vec::new();
        for scope in self.globals.clone() {
            let mut frozen = BTreeMap::new();
            for (n, binding) in scope {
                if v5 && !needed.contains(&n) {
                    continue;
                }
                let original = self.resolve(&binding.value);
                if !self
                    .engine
                    .runtime
                    .live_native_ids(std::slice::from_ref(&original))
                    .is_empty()
                {
                    return Err(self.error(at,"NativeResourceRequiresMove: pass the resource as an explicit moved argument"));
                }
                let value = if v5 {
                    v05::task_copy(&mut self.engine.runtime, &original)?
                } else if matches!(original, Value::Function(_, _)) {
                    original
                } else {
                    unflow(self.engine.ordered_key(&original, at), at)?
                };
                let cell = self.engine.runtime.alloc(value)?;
                frozen.insert(
                    n,
                    Binding {
                        value: Value::CellRef(cell),
                        ..binding
                    },
                );
            }
            globals.push(frozen);
        }
        self.new_action(TaskBody::Function(name.into(), args), ty, globals, at)
    }
    pub(super) fn start_task(&mut self, value: &Value, at: &Tok) -> Result<u64> {
        self.engine.runtime.require_internal()?;
        let id = handle_id(value).ok_or_else(|| self.error(at, "expected Task"))?;
        let task = self
            .scheduler
            .tasks
            .get_mut(&id)
            .ok_or_else(|| diagnostic(at, "unknown Task"))?;
        let start = task.phase == TaskPhase::Cold;
        if start {
            task.phase = TaskPhase::Ready;
        }
        let body = task.body.clone();
        if start {
            match body {
                TaskBody::Select(left, right) => {
                    self.start_task(&handle("Task<Unknown>".into(), left), at)?;
                    self.start_task(&handle("Task<Unknown>".into(), right), at)?;
                }
                TaskBody::Timeout(target, budget, _) => {
                    self.scheduler.tasks.get_mut(&id).unwrap().body = TaskBody::Timeout(
                        target,
                        budget,
                        Some(self.scheduler.ticks.saturating_add(budget)),
                    );
                    self.start_task(&handle("Task<Unknown>".into(), target), at)?;
                }
                _ => {}
            }
        }
        Ok(id)
    }
    pub(super) fn scheduler_constructor(
        &mut self,
        name: &str,
        args: &[Value],
        at: &Tok,
    ) -> Result<Option<Value>> {
        if name.starts_with("stdExternalDb")
            && !matches!(
                name,
                "stdExternalDbPrivateParameter" | "stdExternalDbCredentials"
            )
        {
            use rewind::database::Operation;
            let invalid = || self.error(at, "invalid database arguments");
            let Some(Value::Int(timeout)) = args.last() else {
                return Err(invalid());
            };
            let (operation, result_type) = if name == "stdExternalDbCleanup" {
                (Operation::Cleanup, "Result<Unit,DbError>")
            } else if name == "stdExternalDbPostgres" {
                let [Value::Text(alias), _] = args else {
                    return Err(invalid());
                };
                (
                    Operation::Postgres {
                        alias: alias.clone(),
                    },
                    "Result<DbConnection,DbError>",
                )
            } else if name == "stdExternalDbSqlite" {
                let [Value::Text(path), Value::Bool(read_only), _] = args else {
                    return Err(invalid());
                };
                (
                    Operation::Sqlite {
                        path: path.clone(),
                        read_only: *read_only,
                    },
                    "Result<DbConnection,DbError>",
                )
            } else {
                let resource = self.resolve(&args[0]);
                let id = self.engine.runtime.check_native(&resource)? as usize;
                match name {
                    "stdExternalDbExecuteMany" => {
                        let Some(Value::Text(sql)) = args.get(1) else {
                            return Err(invalid());
                        };
                        let values = self.resolve(&args[2]);
                        let values = v05::unfrozen(&values).unwrap_or(&values);
                        let Value::TypedList(_, rows) = values else {
                            return Err(invalid());
                        };
                        if rows.len() > 1024
                            || rows.iter().try_fold(0usize, |total, row| {
                                database_parameter_bytes(v05::unfrozen(row).unwrap_or(row))
                                    .and_then(|bytes| {
                                        total
                                            .checked_add(bytes)
                                            .ok_or_else(|| self.error(at, "DbLimit: batch bytes"))
                                    })
                            })? > 1048576
                        {
                            return Err(self.error(at, "DbLimit: batch parameters"));
                        }
                        let parameters = rows
                            .iter()
                            .map(|row| database_parameters(v05::unfrozen(row).unwrap_or(row)))
                            .collect::<Result<Vec<_>>>()?;
                        (
                            Operation::ExecuteMany {
                                connection: id,
                                sql: sql.clone(),
                                parameters,
                            },
                            "Result<Int,DbError>",
                        )
                    }
                    "stdExternalDbExecute"
                    | "stdExternalDbQuery"
                    | "stdExternalDbExecuteStatement"
                    | "stdExternalDbQueryStatement" => {
                        let statement = name.ends_with("Statement");
                        let sql = if statement {
                            String::new()
                        } else {
                            let Some(Value::Text(sql)) = args.get(1) else {
                                return Err(invalid());
                            };
                            sql.clone()
                        };
                        let params = self.resolve(&args[if statement { 1 } else { 2 }]);
                        let params = v05::unfrozen(&params).unwrap_or(&params);
                        let parameters = database_parameters(params)?;
                        if name == "stdExternalDbExecuteStatement" {
                            (
                                Operation::ExecuteStatement {
                                    statement: id,
                                    parameters,
                                },
                                "Result<Int,DbError>",
                            )
                        } else if name == "stdExternalDbQueryStatement" {
                            (
                                Operation::QueryStatement {
                                    statement: id,
                                    parameters,
                                },
                                "Result<DbCursor,DbError>",
                            )
                        } else if name.ends_with("Execute") {
                            (
                                Operation::Execute {
                                    connection: id,
                                    sql: sql.clone(),
                                    parameters,
                                },
                                "Result<Int,DbError>",
                            )
                        } else {
                            (
                                Operation::Query {
                                    connection: id,
                                    sql: sql.clone(),
                                    parameters,
                                },
                                "Result<DbCursor,DbError>",
                            )
                        }
                    }
                    "stdExternalDbPrepare" => {
                        let Some(Value::Text(sql)) = args.get(1) else {
                            return Err(invalid());
                        };
                        (
                            Operation::Prepare {
                                connection: id,
                                sql: sql.clone(),
                            },
                            "Result<DbStatement,DbError>",
                        )
                    }
                    "stdExternalDbCloseStatement" => (
                        Operation::CloseStatement { statement: id },
                        "Result<Unit,DbError>",
                    ),
                    "stdExternalDbNext" => {
                        let [_, Value::Int(rows), Value::Int(bytes), _] = args else {
                            return Err(invalid());
                        };
                        (
                            Operation::Next {
                                cursor: id,
                                rows: usize::try_from(*rows).unwrap_or(usize::MAX),
                                bytes: usize::try_from(*bytes).unwrap_or(usize::MAX),
                            },
                            "Result<DbBatch,DbError>",
                        )
                    }
                    "stdExternalDbCloseCursor" => (
                        Operation::CloseCursor { cursor: id },
                        "Result<Unit,DbError>",
                    ),
                    "stdExternalDbBegin" => {
                        (Operation::Begin { connection: id }, "Result<Unit,DbError>")
                    }
                    "stdExternalDbCommit" => {
                        (Operation::Commit { connection: id }, "Result<Unit,DbError>")
                    }
                    "stdExternalDbRollback" => (
                        Operation::Rollback { connection: id },
                        "Result<Unit,DbError>",
                    ),
                    "stdExternalDbClose" => {
                        (Operation::Close { connection: id }, "Result<Unit,DbError>")
                    }
                    _ => return Err(invalid()),
                }
            };
            let timeout = u64::try_from(*timeout).unwrap_or(u64::MAX);
            let task = self.new_action(
                TaskBody::HostOperation(usize::MAX),
                result_type.into(),
                Vec::new(),
                at,
            )?;
            let operation = self.engine.runtime.start_database(operation, timeout)?;
            let state = self
                .scheduler
                .tasks
                .get_mut(&handle_id(&task).unwrap())
                .unwrap();
            state.body = TaskBody::HostOperation(operation);
            state.phase = TaskPhase::Ready;
            return Ok(Some(task));
        }
        if matches!(
            name,
            "stdExternalHttpRead"
                | "stdExternalHttpClose"
                | "stdExternalHttpWrite"
                | "stdExternalHttpFinish"
                | "stdExternalHttpCloseUpload"
        ) {
            let resource = self.resolve(&args[0]);
            let stream = self.engine.runtime.check_native(&resource)? as usize;
            let task = self.new_action(
                TaskBody::HostOperation(usize::MAX),
                if name.ends_with("Finish") {
                    "Result<HttpResponse,HttpError>"
                } else if name.ends_with("Read") {
                    "Result<Option<Bytes>,HttpError>"
                } else {
                    "Result<Unit,HttpError>"
                }
                .into(),
                Vec::new(),
                at,
            )?;
            let operation = if name.ends_with("Write") {
                let Some(Value::Bytes(body)) = args.get(1) else {
                    return Err(self.error(at, "invalid upload bytes"));
                };
                self.engine.runtime.write_http(stream, body.clone())?
            } else if name.ends_with("Finish") {
                let Value::Struct(_, fields) = &resource else {
                    return Err(self.error(at, "invalid upload"));
                };
                let Some(Value::Int(limit)) = fields.get("maxBytes") else {
                    return Err(self.error(at, "invalid upload limit"));
                };
                self.engine.runtime.finish_http(stream, *limit as usize)?
            } else if name.ends_with("Read") {
                let Some(Value::Int(limit)) = args.get(1) else {
                    return Err(self.error(at, "invalid read limit"));
                };
                self.engine
                    .runtime
                    .read_http(stream, usize::try_from(*limit).unwrap_or(usize::MAX))?
            } else {
                self.engine.runtime.close_http(stream)?
            };
            let state = self
                .scheduler
                .tasks
                .get_mut(&handle_id(&task).unwrap())
                .unwrap();
            state.body = TaskBody::HostOperation(operation);
            state.phase = TaskPhase::Ready;
            return Ok(Some(task));
        }
        if matches!(
            name,
            "stdExternalHttpStart"
                | "stdExternalHttpConfigured"
                | "stdExternalHttpAuthenticated"
                | "stdExternalHttpDownload"
                | "stdExternalHttpUpload"
        ) {
            let [Value::Text(method), Value::Text(url), Value::Bytes(body), Value::Int(timeout), Value::Int(limit)] =
                &args[..5]
            else {
                return Err(self.error(at, "invalid HTTP arguments"));
            };
            let (headers, ca) = if name != "stdExternalHttpStart" {
                let [header_value, Value::Bytes(ca)] = &args[5..7] else {
                    return Err(self.error(at, "invalid HTTP configuration"));
                };
                let inner = v05::unfrozen(header_value)
                    .ok_or_else(|| self.error(at, "HTTP headers require Frozen list"))?;
                let Value::TypedList(_, values) = inner else {
                    return Err(self.error(at, "HTTP headers require list"));
                };
                let mut headers = Vec::new();
                for value in values.iter() {
                    let Value::Struct(_, fields) = value else {
                        return Err(self.error(at, "invalid HTTP header"));
                    };
                    let (Some(Value::Text(name)), Some(Value::Bytes(value))) =
                        (fields.get("name"), fields.get("value"))
                    else {
                        return Err(self.error(at, "invalid HTTP header"));
                    };
                    headers.push((name.clone(), value.as_ref().clone()));
                }
                (headers, ca.clone())
            } else {
                (vec![], Arc::new(vec![]))
            };
            // Allocate the Task before any host operation can begin.
            let task = self.new_action(
                TaskBody::HostOperation(usize::MAX),
                if name == "stdExternalHttpUpload" {
                    "Result<HttpUpload,HttpError>"
                } else if name == "stdExternalHttpDownload" {
                    "Result<HttpDownload,HttpError>"
                } else {
                    "Result<HttpResponse,HttpError>"
                }
                .into(),
                Vec::new(),
                at,
            )?;
            let task_id = handle_id(&task).unwrap();
            let operation = self.engine.runtime.start_http(rewind::network::Request {
                method: method.clone(),
                url: url.clone(),
                body: body.clone(),
                timeout_ms: u64::try_from(*timeout).unwrap_or(0),
                limit: usize::try_from(*limit).unwrap_or(usize::MAX),
                headers,
                ca,
                download: name == "stdExternalHttpDownload",
                upload_limit: if name == "stdExternalHttpUpload" {
                    Some(
                        args.get(8)
                            .and_then(|v| {
                                if let Value::Int(n) = v {
                                    usize::try_from(*n).ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0),
                    )
                } else {
                    None
                },
                credential: if let Some(Value::Text(alias)) = args.get(7) {
                    alias.clone()
                } else {
                    String::new()
                },
            })?;
            let state = self.scheduler.tasks.get_mut(&task_id).unwrap();
            state.body = TaskBody::HostOperation(operation);
            state.phase = TaskPhase::Ready;
            return Ok(Some(task));
        }
        if name == "TaskGroup" {
            if !args.is_empty() {
                return Err(self.error(at, "TaskGroup takes no arguments"));
            }
            let id = self.next_scheduler_id(at)?;
            self.scheduler.groups.insert(id, Vec::new());
            return Ok(Some(handle("TaskGroup".into(), id)));
        }
        if name.starts_with("Channel<") {
            let [Value::Int(capacity)] = args else {
                return Err(self.error(at, "Channel requires an Int capacity"));
            };
            if !(0..=65536).contains(capacity) {
                return Err(self.error(at, "Channel capacity must be 0..65536"));
            }
            let id = self.next_scheduler_id(at)?;
            self.scheduler.channels.insert(
                id,
                Channel {
                    capacity: *capacity as usize,
                    closed: false,
                    values: VecDeque::new(),
                },
            );
            return Ok(Some(handle(name.into(), id)));
        }
        Ok(None)
    }
    pub(super) fn scheduler_method(
        &mut self,
        value: &Value,
        method: &str,
        args: &[Value],
        at: &Tok,
    ) -> Result<Option<Value>> {
        let Value::Struct(ty, _) = value else {
            return Ok(None);
        };
        let Some(id) = handle_id(value) else {
            return Ok(None);
        };
        if ty.starts_with("Channel<") {
            if !self.scheduler.channels.contains_key(&id) {
                return Err(self.error(at, "unknown Channel"));
            }
            return Ok(Some(match (method, args) {
                ("send", [value]) => {
                    let value = if matches!(
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
                            | "2.0.0"
                    ) {
                        v05::task_copy(&mut self.engine.runtime, value)?
                    } else {
                        unflow(self.engine.ordered_key(value, at), at)?
                    };
                    self.new_action(TaskBody::Send(id, value), "Unit".into(), Vec::new(), at)?
                }
                ("receive", []) => self.new_action(
                    TaskBody::Receive(id),
                    ty.strip_prefix("Channel<")
                        .unwrap()
                        .strip_suffix('>')
                        .unwrap()
                        .into(),
                    Vec::new(),
                    at,
                )?,
                ("close", []) => {
                    self.scheduler.channels.get_mut(&id).unwrap().closed = true;
                    Value::Null
                }
                _ => return Err(self.error(at, "invalid Channel method")),
            }));
        }
        if ty.starts_with("Task<") {
            if method == "isDone"
                && args.is_empty()
                && language_at_least(&self.engine.program.language, "1.6.0")
            {
                self.engine.runtime.require_internal()?;
                self.pump_actions()?;
                return Ok(Some(Value::Bool(
                    self.scheduler
                        .tasks
                        .get(&id)
                        .ok_or_else(|| diagnostic(at, "unknown Task"))?
                        .phase
                        == TaskPhase::Done,
                )));
            }
            match (method, args) {
                ("ignore" | "detach", [])
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
                            | "2.0.0"
                    ) =>
                {
                    self.scheduler
                        .tasks
                        .get_mut(&id)
                        .ok_or_else(|| diagnostic(at, "unknown Task"))?
                        .ignored = true;
                    if let Some(result) = self.scheduler.tasks[&id].result.clone() {
                        self.complete_task(id, result)?;
                    }
                }
                ("timeout", [Value::Int(steps)])
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
                            | "2.0.0"
                    ) =>
                {
                    if *steps < 0 {
                        return Err(self.error(at, "timeout steps must be nonnegative"));
                    }
                    return self
                        .new_action(
                            TaskBody::Timeout(id, *steps as u64, None),
                            ty.strip_prefix("Task<")
                                .unwrap()
                                .strip_suffix('>')
                                .unwrap()
                                .into(),
                            Vec::new(),
                            at,
                        )
                        .map(Some);
                }
                ("select", [other])
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
                            | "2.0.0"
                    ) =>
                {
                    let other =
                        handle_id(other).ok_or_else(|| self.error(at, "select requires a Task"))?;
                    if !self.scheduler.tasks.contains_key(&other) {
                        return Err(self.error(at, "unknown Task"));
                    }
                    let item = ty.strip_prefix("Task<").unwrap().strip_suffix('>').unwrap();
                    return self
                        .new_action(
                            TaskBody::Select(id, other),
                            format!("Tuple<Int,Result<{item},TaskError>>"),
                            Vec::new(),
                            at,
                        )
                        .map(Some);
                }
                ("cancel", []) => self.cancel_task(id, at)?,
                ("requestCancel", [])
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
                            | "2.0.0"
                    ) =>
                {
                    let task = self
                        .scheduler
                        .tasks
                        .get_mut(&id)
                        .ok_or_else(|| diagnostic(at, "unknown Task"))?;
                    task.cancel_requested = true;
                    if matches!(
                        task.phase,
                        TaskPhase::Waiting(_) | TaskPhase::WaitingChannel
                    ) {
                        task.phase = TaskPhase::Ready;
                    }
                }
                ("cancelAndJoin", [])
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
                            | "2.0.0"
                    ) =>
                {
                    self.cancel_task(id, at)?;
                    return Ok(Some(value.clone()));
                }
                ("setPriority", [Value::Int(priority)]) => {
                    self.scheduler
                        .tasks
                        .get_mut(&id)
                        .ok_or_else(|| diagnostic(at, "unknown Task"))?
                        .priority = *priority
                }
                _ => return Err(self.error(at, "invalid Task method")),
            }
            return Ok(Some(Value::Null));
        }
        if ty == "TaskGroup" {
            match (method, args) {
                ("add", [task]) => {
                    let task = self.start_task(task, at)?;
                    let group = self
                        .scheduler
                        .groups
                        .get_mut(&id)
                        .ok_or_else(|| diagnostic(at, "unknown TaskGroup"))?;
                    if !group.contains(&task) {
                        group.push(task);
                    }
                }
                ("join", []) => {
                    return self
                        .new_action(TaskBody::Join(id), "Unit".into(), Vec::new(), at)
                        .map(Some)
                }
                ("cancel", []) => self.cancel_group(id, at)?,
                _ => return Err(self.error(at, "invalid TaskGroup method")),
            }
            return Ok(Some(Value::Null));
        }
        Ok(None)
    }
    pub(super) fn task_result(&mut self, id: u64) -> Option<Value> {
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
                | "2.0.0"
        ) {
            if let Some(failure) = self.scheduler.tasks.get(&id)?.failure.clone() {
                self.scheduler.tasks.get_mut(&id)?.observed = true;
                return Some(Value::Result(Err(Box::new(v06::diagnostics::task_error(
                    &failure,
                )))));
            }
        }
        if let Some(task) = self.scheduler.tasks.get_mut(&id) {
            if task.result.is_some() {
                task.observed = true;
            }
        }
        let result = self.scheduler.tasks.get(&id)?.result.clone();
        if let Some(Ok(value)) = &result {
            if !self
                .engine
                .runtime
                .live_native_ids(std::slice::from_ref(value))
                .is_empty()
            {
                self.scheduler.tasks.get_mut(&id)?.result =
                    Some(Err("TaskResultTransferred".into()));
            }
        }
        result.map(|result| {
            Value::Result(match result {
                Ok(value)
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
                            | "2.0.0"
                    ) =>
                {
                    v05::task_copy(&mut self.engine.runtime, &value)
                        .map(Box::new)
                        .map_err(|e| {
                            Box::new(
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
                                        | "2.0.0"
                                ) {
                                    v06::diagnostics::task_error(&self.record_error(
                                        &e,
                                        &Tok {
                                            source: String::new(),
                                            text: String::new(),
                                            line: 0,
                                            col: 0,
                                        },
                                        id,
                                    ))
                                } else {
                                    v05::task_error(&e.to_string())
                                },
                            )
                        })
                }
                Ok(value) => Ok(Box::new(value)),
                Err(error) => Err(Box::new(
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
                            | "2.0.0"
                    ) {
                        v05::task_error(&error)
                    } else {
                        Value::Text(error)
                    },
                )),
            })
        })
    }
    fn complete_task(
        &mut self,
        id: u64,
        mut result: std::result::Result<Value, String>,
    ) -> Result<()> {
        if self.scheduler.tasks[&id].ignored {
            if let Ok(value) = &result {
                let resources = self
                    .engine
                    .runtime
                    .live_native_ids(std::slice::from_ref(value));
                if !resources.is_empty() {
                    for id in resources {
                        self.engine.runtime.close_native_resource(id)?;
                        self.engine.runtime.forget_native_owner(id);
                    }
                    result = Err("NativeResultDiscarded".into());
                }
            }
        }
        let task = self.scheduler.tasks.get_mut(&id).unwrap();
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
                | "2.0.0"
        ) && task.failure.is_none()
        {
            if let Err(message) = &result {
                task.failure = Some(rewind::DiagnosticRecord {
                    code: if message == "ChannelClosed" {
                        "ChannelClosed"
                    } else if message == "TaskCancelled" {
                        "TaskCancelled"
                    } else {
                        "TaskFailed"
                    }
                    .into(),
                    message: message.clone(),
                    source: task.at.source.clone(),
                    line: task.at.line,
                    column: task.at.col,
                    task_id: Some(id),
                    frames: vec![],
                    hints: vec![],
                    frames_truncated: false,
                    causes: vec![],
                    wait_edges: vec![],
                });
            }
        }
        task.phase = TaskPhase::Done;
        task.result = Some(result);
        task.context = None;
        // Preserve older artifact accounting; new tasks discard execution-only inputs.
        if language_at_least(&self.engine.program.language, "1.9.2") {
            task.globals.clear();
            task.global_scope_ids.clear();
            match &mut task.body {
                TaskBody::Function(_, args) => args.clear(),
                TaskBody::Send(_, value) => *value = Value::Null,
                _ => {}
            }
        }
        Ok(())
    }
    pub(super) fn set_task_failure(&mut self, failure: rewind::DiagnosticRecord) {
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
                | "2.0.0"
        ) {
            self.scheduler
                .tasks
                .get_mut(&self.scheduler.active)
                .unwrap()
                .failure = Some(failure);
        }
    }
    pub(super) fn expire_timeouts(&mut self, at: &Tok) -> Result<()> {
        let expired = self
            .scheduler
            .tasks
            .iter()
            .filter_map(|(id, task)| {
                if let TaskBody::Timeout(target, _, Some(deadline)) = task.body {
                    if !task.timed_out
                        && matches!(task.phase, TaskPhase::Ready | TaskPhase::WaitingChannel)
                        && deadline <= self.scheduler.ticks
                        && self.scheduler.tasks[&target].phase != TaskPhase::Done
                    {
                        return Some((*id, target));
                    }
                }
                None
            })
            .collect::<Vec<_>>();
        for (wrapper, target) in expired {
            self.scheduler.tasks.get_mut(&wrapper).unwrap().timed_out = true;
            self.cancel_task(target, at)?;
        }
        Ok(())
    }
    fn pump_actions(&mut self) -> Result<()> {
        loop {
            let mut progress = false;
            let actions = self
                .scheduler
                .tasks
                .iter()
                .filter(|(_, t)| matches!(t.phase, TaskPhase::Ready | TaskPhase::WaitingChannel))
                .map(|(id, t)| (*id, t.body.clone()))
                .collect::<Vec<_>>();
            for (id, body) in &actions {
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
                        | "2.0.0"
                ) && self.scheduler.tasks[id].cancel_requested
                    && matches!(
                        body,
                        TaskBody::Send(..)
                            | TaskBody::Receive(..)
                            | TaskBody::Join(..)
                            | TaskBody::Select(..)
                            | TaskBody::Timeout(..)
                            | TaskBody::HostOperation(..)
                    )
                {
                    if let TaskBody::HostOperation(operation) = body {
                        self.engine.runtime.cancel_http(*operation)?;
                    }
                    self.complete_task(*id, Err("TaskCancelled".into()))?;
                    progress = true;
                    continue;
                }
                let result = match body {
                    TaskBody::HostOperation(operation) => self
                        .engine
                        .runtime
                        .poll_external(*operation)?
                        .map(|value| match value {
                            Ok(value) => http_value(value, &mut self.engine.runtime)
                                .map_err(|_| "HttpRecordedResult".into()),
                            Err(code) => Err(code),
                        }),
                    TaskBody::Send(channel, value) => {
                        let c = self.scheduler.channels.get_mut(channel).unwrap();
                        if c.closed {
                            Some(Err("ChannelClosed".into()))
                        } else if c.values.len() < c.capacity {
                            c.values.push_back(value.clone());
                            Some(Ok(Value::Null))
                        } else if c.capacity == 0 {
                            if let Some((receiver, _)) = actions.iter().find(|(receiver, body)| {
                                receiver != id
                                    && matches!(body,TaskBody::Receive(target) if target==channel)
                                    && self.scheduler.tasks[receiver].phase != TaskPhase::Done
                            }) {
                                self.complete_task(*receiver, Ok(value.clone()))?;
                                Some(Ok(Value::Null))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    TaskBody::Receive(channel) => {
                        if self.scheduler.tasks[id].phase == TaskPhase::Done {
                            continue;
                        }
                        let c = self.scheduler.channels.get_mut(channel).unwrap();
                        if let Some(value) = c.values.pop_front() {
                            Some(Ok(value))
                        } else if c.closed {
                            Some(Err("ChannelClosed".into()))
                        } else {
                            None
                        }
                    }
                    TaskBody::Select(left, right) => {
                        let winner = [*left, *right].into_iter().enumerate().find(|(_, target)| {
                            self.scheduler.tasks[target].phase == TaskPhase::Done
                        });
                        if let Some((index, target)) = winner {
                            let result = self.task_result(target).unwrap();
                            let ty = value_type(&result, &self.engine.runtime);
                            Some(Ok(Value::Struct(
                                format!("Tuple<Int,{ty}>"),
                                BTreeMap::from([
                                    ("_0".into(), Value::Int(index as i64)),
                                    ("_1".into(), result),
                                ]),
                            )))
                        } else {
                            None
                        }
                    }
                    TaskBody::Timeout(target, _, _) => {
                        if self.scheduler.tasks[target].phase == TaskPhase::Done {
                            self.scheduler.tasks.get_mut(target).unwrap().observed = true;
                            if self.scheduler.tasks[id].timed_out {
                                let at = self.scheduler.tasks[id].at.clone();
                                let causes = self.scheduler.tasks[target]
                                    .failure
                                    .as_ref()
                                    .map(|d| d.causes.clone())
                                    .unwrap_or_default();
                                self.scheduler.tasks.get_mut(id).unwrap().failure =
                                    Some(rewind::DiagnosticRecord {
                                        code: "LogicalTimeout".into(),
                                        message: "logical timeout".into(),
                                        source: at.source,
                                        line: at.line,
                                        column: at.col,
                                        task_id: Some(*id),
                                        frames: vec![],
                                        hints: vec![],
                                        frames_truncated: false,
                                        causes,
                                        wait_edges: vec![],
                                    });
                                Some(Err("logical timeout".into()))
                            } else {
                                self.scheduler.tasks.get_mut(id).unwrap().failure =
                                    self.scheduler.tasks[target].failure.clone();
                                let result = self.scheduler.tasks[target].result.clone();
                                if let Some(Ok(value)) = &result {
                                    if !self
                                        .engine
                                        .runtime
                                        .live_native_ids(std::slice::from_ref(value))
                                        .is_empty()
                                    {
                                        self.scheduler.tasks.get_mut(target).unwrap().result =
                                            Some(Err("TaskResultTransferred".into()));
                                    }
                                }
                                result
                            }
                        } else {
                            None
                        }
                    }
                    TaskBody::Join(group) => {
                        self.scheduler.groups.get(group).and_then(|children| {
                            if children
                                .iter()
                                .all(|id| self.scheduler.tasks[id].phase == TaskPhase::Done)
                            {
                                Some(
                                    children
                                        .iter()
                                        .find_map(|id| {
                                            self.scheduler.tasks[id]
                                                .result
                                                .as_ref()
                                                .and_then(|r| r.as_ref().err().cloned())
                                        })
                                        .map_or(Ok(Value::Null), Err),
                                )
                            } else {
                                None
                            }
                        })
                    }
                    _ => continue,
                };
                if let Some(result) = result {
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
                            | "2.0.0"
                    ) {
                        if let TaskBody::Join(group) = body {
                            let failures = self
                                .scheduler
                                .groups
                                .get(group)
                                .into_iter()
                                .flatten()
                                .filter_map(|id| self.scheduler.tasks[id].failure.clone())
                                .collect::<Vec<_>>();
                            if let Some(mut first) = failures.first().cloned() {
                                first.causes.extend(failures.into_iter().skip(1));
                                self.scheduler.tasks.get_mut(id).unwrap().failure = Some(first);
                            }
                        }
                    }
                    if let TaskBody::Join(group) = body {
                        if let Some(children) = self.scheduler.groups.get(group) {
                            for child in children {
                                self.scheduler.tasks.get_mut(child).unwrap().observed = true;
                            }
                        }
                    }
                    self.complete_task(*id, result)?;
                    progress = true;
                } else {
                    self.scheduler.tasks.get_mut(id).unwrap().phase = TaskPhase::WaitingChannel;
                }
            }
            let waiting = self
                .scheduler
                .tasks
                .iter()
                .filter_map(|(id, t)| {
                    if let TaskPhase::Waiting(on) = t.phase {
                        (self
                            .scheduler
                            .tasks
                            .get(&on)
                            .is_some_and(|t| t.phase == TaskPhase::Done))
                        .then_some(*id)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            for id in waiting {
                self.scheduler.tasks.get_mut(&id).unwrap().phase = TaskPhase::Ready;
                progress = true;
            }
            if !progress {
                break;
            }
        }
        Ok(())
    }
    pub(super) fn scheduler_yield(&mut self, at: &Tok) -> Result<()> {
        self.scheduler
            .tasks
            .get_mut(&self.scheduler.active)
            .unwrap()
            .phase = TaskPhase::Ready;
        if !self.schedule(at)? {
            return Err(self.error(at, "TaskDeadlock"));
        }
        Ok(())
    }
    pub(super) fn schedule(&mut self, at: &Tok) -> Result<bool> {
        let active = self.scheduler.active;
        if self.scheduler.tasks[&active].phase != TaskPhase::Done {
            let context = self.capture_context();
            self.scheduler.tasks.get_mut(&active).unwrap().context = Some(context);
        }
        self.expire_timeouts(at)?;
        self.pump_actions()?;
        if !self.scheduler.tasks.values().any(|t| {
            t.phase == TaskPhase::Ready && matches!(t.body, TaskBody::Main | TaskBody::Function(..))
        }) {
            if let Some(deadline) = self
                .scheduler
                .tasks
                .values()
                .filter(|t| {
                    !t.timed_out && matches!(t.phase, TaskPhase::Ready | TaskPhase::WaitingChannel)
                })
                .filter_map(|t| {
                    if let TaskBody::Timeout(_, _, deadline) = t.body {
                        deadline
                    } else {
                        None
                    }
                })
                .min()
            {
                self.scheduler.ticks = self.scheduler.ticks.max(deadline);
                self.expire_timeouts(at)?;
                self.pump_actions()?;
            }
        }
        while !self.scheduler.tasks.values().any(|t| {
            t.phase == TaskPhase::Ready && matches!(t.body, TaskBody::Main | TaskBody::Function(..))
        }) && self
            .scheduler
            .tasks
            .values()
            .any(|t| t.phase != TaskPhase::Done && matches!(t.body, TaskBody::HostOperation(_)))
        {
            self.engine.runtime.wait_external_completion();
            self.pump_actions()?;
        }
        let mut candidates = self
            .scheduler
            .tasks
            .iter()
            .filter(|(_, t)| {
                t.phase == TaskPhase::Ready
                    && matches!(t.body, TaskBody::Main | TaskBody::Function(_, _))
            })
            .map(|(id, t)| (t.priority, *id))
            .collect::<Vec<_>>();
        candidates.sort();
        if let Some((priority, _)) = candidates.first().copied() {
            candidates.retain(|(p, _)| *p == priority);
        }
        let next = if candidates.is_empty() {
            None
        } else {
            let choice = self
                .options
                .choices
                .get(self.choices_used.len())
                .copied()
                .unwrap_or_else(|| {
                    if language_at_least(&self.engine.program.language, "1.9.10") {
                        candidates
                            .iter()
                            .position(|(_, id)| *id > active)
                            .unwrap_or(0)
                    } else {
                        0
                    }
                });
            if choice >= candidates.len() {
                return Err(self.error(at, "ScheduleMismatch: choice is not runnable"));
            }
            self.choice_widths.push(candidates.len());
            self.choices_used.push(choice);
            Some(candidates[choice].1)
        };
        let Some(id) = next else {
            if self
                .scheduler
                .tasks
                .values()
                .any(|t| matches!(t.phase, TaskPhase::Waiting(_) | TaskPhase::WaitingChannel))
            {
                let graph = self
                    .scheduler
                    .tasks
                    .iter()
                    .filter(|(_, t)| {
                        matches!(t.phase, TaskPhase::Waiting(_) | TaskPhase::WaitingChannel)
                    })
                    .map(|(id, t)| format!("{id}:{:?}", t.phase))
                    .collect::<Vec<_>>()
                    .join(",");
                return Err(self.error(at, format!("TaskDeadlock: {graph}")));
            }
            return Ok(false);
        };
        self.scheduler.active = id;
        let task = self.scheduler.tasks.get_mut(&id).unwrap();
        task.phase = TaskPhase::Running;
        if let Some(context) = task.context.take() {
            self.install_context(context)?;
        } else {
            let TaskBody::Function(name, args) = task.body.clone() else {
                return Err(self.error(at, "missing task context"));
            };
            self.globals = task.globals.clone();
            self.global_scope_ids = task.global_scope_ids.clone();
            self.frames.clear();
            self.branches.clear();
            self.global_cleanups.clear();
            self.pc = self.halt_pc;
            self.engine.runtime.set_task_compute(
                Arc::new(Vec::new()),
                Arc::new(Vec::new()),
                Arc::new(BTreeMap::new()),
            )?;
            self.call_user(&name, args, at)?;
        }
        Ok(true)
    }
    pub(super) fn wait_task(&mut self, value: Value, at: &Tok) -> Result<()> {
        self.engine.runtime.require_internal()?;
        let id = self.start_task(&value, at)?;
        if let Some(result) = self.task_result(id) {
            self.push(result)?;
            return Ok(());
        }
        self.push(value)?;
        self.pc -= 1;
        self.scheduler
            .tasks
            .get_mut(&self.scheduler.active)
            .unwrap()
            .phase = TaskPhase::Waiting(id);
        if !self.schedule(at)? {
            return Err(self.error(at, "TaskDeadlock"));
        }
        Ok(())
    }
    pub(super) fn finish_scheduled_task(
        &mut self,
        result: std::result::Result<Value, String>,
        at: &Tok,
    ) -> Result<bool> {
        let result = match result {
            Ok(value) => Ok(
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
                        | "2.0.0"
                ) {
                    v05::task_copy(&mut self.engine.runtime, &value)?
                } else {
                    unflow(self.engine.ordered_key(&value, at), at)?
                },
            ),
            Err(error) => Err(error),
        };
        let id = self.scheduler.active;
        self.complete_task(id, result)?;
        self.schedule(at)
    }
    pub(super) fn cancel_group(&mut self, id: u64, at: &Tok) -> Result<()> {
        let children = self
            .scheduler
            .groups
            .get(&id)
            .cloned()
            .ok_or_else(|| self.error(at, "unknown TaskGroup"))?;
        for child in children.into_iter().rev() {
            self.cancel_task(child, at)?;
        }
        Ok(())
    }
    pub(super) fn unhandled_task_error(&self) -> Option<String> {
        self.scheduler.tasks.iter().find_map(|(id, t)| {
            if *id != 0 && !t.observed && !t.ignored {
                t.result
                    .as_ref()
                    .and_then(|r| r.as_ref().err())
                    .map(|e| format!("UnhandledTaskError: task {id}: {e}"))
            } else {
                None
            }
        })
    }
    pub(super) fn unhandled_task_diagnostic(&self) -> Option<rewind::DiagnosticRecord> {
        self.scheduler.tasks.iter().find_map(|(id, t)| {
            if *id == 0 || t.observed || t.ignored {
                return None;
            }
            let failure = t.failure.clone()?;
            Some(rewind::DiagnosticRecord {
                code: "UnhandledTaskError".into(),
                message: format!("unhandled failure in task {id}"),
                source: failure.source.clone(),
                line: failure.line,
                column: failure.column,
                task_id: Some(0),
                frames: vec![],
                hints: vec![],
                frames_truncated: false,
                causes: vec![failure],
                wait_edges: vec![],
            })
        })
    }
    pub(super) fn cancellation_requested(&self) -> bool {
        self.scheduler.tasks[&self.scheduler.active].cancel_requested
    }
    pub(super) fn drain_scope_groups(&mut self, depth: Option<usize>, at: &Tok) -> Result<bool> {
        if !matches!(
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
                | "2.0.0"
        ) {
            return Ok(false);
        }
        let cleanups = self
            .frames
            .last()
            .map(|f| &f.defers)
            .unwrap_or(&self.global_cleanups);
        let groups = cleanups
            .iter()
            .rev()
            .filter_map(|c| {
                if let Cleanup::Group(id, d) = c {
                    if depth.is_none_or(|depth| depth == *d) {
                        Some(*id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for group in groups {
            if let Some(id) = self.scheduler.tasks.iter().find_map(|(id, t)| {
                if matches!(t.body,TaskBody::Join(g) if g==group)
                    && t.parent == self.scheduler.active
                    && t.phase == TaskPhase::Done
                    && !t.observed
                    && !t.ignored
                {
                    Some(*id)
                } else {
                    None
                }
            }) {
                let result = self.task_result(id).unwrap();
                if let Value::Result(Err(e)) = result {
                    return Err(self.error(at, format!("TaskGroupFailed: {e}")));
                }
            }
            if self.scheduler.groups[&group].iter().any(|id| {
                let t = &self.scheduler.tasks[id];
                t.phase != TaskPhase::Done
                    || (!t.observed && !t.ignored && t.result.as_ref().is_some_and(|r| r.is_err()))
            }) {
                let handle =
                    self.new_action(TaskBody::Join(group), "Unit".into(), Vec::new(), at)?;
                let id = self.start_task(&handle, at)?;
                self.scheduler
                    .tasks
                    .get_mut(&self.scheduler.active)
                    .unwrap()
                    .phase = TaskPhase::Waiting(id);
                self.pc -= 1;
                self.schedule(at)?;
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(super) fn cancel_task(&mut self, id: u64, at: &Tok) -> Result<()> {
        let task = self
            .scheduler
            .tasks
            .get(&id)
            .cloned()
            .ok_or_else(|| self.error(at, "unknown Task"))?;
        if task.phase == TaskPhase::Done {
            return Ok(());
        }
        if let TaskBody::HostOperation(operation) = task.body {
            self.engine.runtime.cancel_http(operation)?;
        }
        self.scheduler.tasks.get_mut(&id).unwrap().ignored = true;
        let children = self
            .scheduler
            .tasks
            .iter()
            .filter(|(child, t)| **child != id && t.parent == id && t.phase != TaskPhase::Done)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for child in children.into_iter().rev() {
            self.cancel_task(child, at)?;
        }
        if id == self.scheduler.active {
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
                    | "2.0.0"
            ) {
                self.scheduler.tasks.get_mut(&id).unwrap().cancel_requested = true;
                return Ok(());
            }
            return Err(self.error(at, "cannot synchronously cancel active task"));
        }
        let mut error = String::from("TaskCancelled");
        let mut failure = rewind::DiagnosticRecord {
            code: "TaskCancelled".into(),
            message: error.clone(),
            source: at.source.clone(),
            line: at.line,
            column: at.col,
            task_id: Some(id),
            frames: vec![],
            hints: vec![],
            frames_truncated: false,
            causes: vec![],
            wait_edges: vec![],
        };
        if let Some(context) = task.context {
            let current = self.capture_context();
            self.install_context(context)?;
            for frame in self.frames.iter().rev().cloned().collect::<Vec<_>>() {
                if let Err(e) = self.run_cleanups(frame.defers, at) {
                    failure.causes.push(self.record_error(&e, at, id));
                    error.push_str("; cleanup: ");
                    error.push_str(&e.to_string());
                }
            }
            let cleanups = std::mem::take(&mut self.global_cleanups);
            if let Err(e) = self.run_cleanups(cleanups, at) {
                failure.causes.push(self.record_error(&e, at, id));
                error.push_str("; cleanup: ");
                error.push_str(&e.to_string());
            }
            self.install_context(current)?;
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
                | "2.0.0"
        ) {
            self.scheduler.tasks.get_mut(&id).unwrap().failure = Some(failure);
        }
        self.complete_task(id, Err(error))?;
        Ok(())
    }
}

fn database_parameters(value: &Value) -> Result<Vec<rewind::database::Parameter>> {
    use rewind::database::Parameter;
    let invalid = || Error::InvalidOperation("invalid typed database parameters".into());
    let Value::TypedList(_, values) = value else {
        return Err(invalid());
    };
    if database_parameter_bytes(value)? > 1048576 {
        return Err(Error::InvalidOperation("DbLimit: parameter bytes".into()));
    }
    if values.len() > 1024 {
        return Err(Error::InvalidOperation("DbLimit: parameter count".into()));
    }
    values
        .iter()
        .map(|value| {
            let Value::Enum(ty, variant, fields) = value else {
                return Err(invalid());
            };
            if ty != "DbValue" {
                return Err(invalid());
            }
            Ok(match (variant.as_str(), fields.first().map(|(_, v)| v)) {
                ("Null", None) => Parameter::Null,
                ("Bool", Some(Value::Bool(v))) => Parameter::Bool(*v),
                ("Int", Some(Value::Int(v))) => Parameter::Int(*v),
                ("Float", Some(Value::Float(v))) => Parameter::Float(f64::from_bits(*v)),
                ("Text", Some(Value::Text(v))) => Parameter::Text(v.clone()),
                ("Private", Some(Value::Text(v))) => Parameter::Private(v.clone()),
                ("Bytes", Some(Value::Bytes(v))) => Parameter::Bytes(v.as_ref().clone()),
                _ => return Err(invalid()),
            })
        })
        .collect()
}
fn database_parameter_bytes(value: &Value) -> Result<usize> {
    let Value::TypedList(_, values) = value else {
        return Err(Error::InvalidOperation(
            "invalid database parameters".into(),
        ));
    };
    values.iter().try_fold(0usize, |total, value| {
        let payload = match value {
            Value::Enum(_, _, fields) => fields.first().map_or(0, |(_, v)| match v {
                Value::Text(v) => v.len(),
                Value::Bytes(v) => v.len(),
                _ => 0,
            }),
            _ => 0,
        };
        total
            .checked_add(payload.saturating_add(32))
            .ok_or_else(|| Error::InvalidOperation("DbLimit: parameter bytes".into()))
    })
}
fn database_value(json: serde_json::Value, runtime: &mut Runtime) -> Result<Value> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let invalid = || Error::InvalidOperation("ReplayMismatch: invalid database response".into());
    if let Some(error) = json.get("error") {
        return Ok(Value::Result(Err(Box::new(Value::Struct(
            "DbError".into(),
            BTreeMap::from([
                (
                    "sqlState".into(),
                    Value::Option(
                        error["sqlState"]
                            .as_str()
                            .map(|s| Box::new(Value::Text(s.into()))),
                    ),
                ),
                (
                    "code".into(),
                    Value::Text(error["code"].as_str().ok_or_else(invalid)?.into()),
                ),
                (
                    "phase".into(),
                    Value::Text(error["phase"].as_str().ok_or_else(invalid)?.into()),
                ),
                (
                    "sqlCode".into(),
                    Value::Option(error["sqlCode"].as_i64().map(|n| Box::new(Value::Int(n)))),
                ),
            ]),
        )))));
    }
    let value = if let Some(id) = json.get("connection") {
        runtime.native_value(
            "DbConnection",
            id.as_u64().ok_or_else(invalid)?,
            BTreeMap::from([(
                "backend".into(),
                Value::Text(json["backend"].as_str().ok_or_else(invalid)?.into()),
            )]),
        )?
    } else if json.get("cursor").is_some() || json.get("statement").is_some() {
        let statement = json.get("statement").is_some();
        let id = &json[if statement { "statement" } else { "cursor" }];
        let columns = json["columns"]
            .as_array()
            .ok_or_else(invalid)?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(|s| Value::Text(s.into()))
                    .ok_or_else(invalid)
            })
            .collect::<Result<Vec<_>>>()?;
        let columns = v05::frozen(Value::TypedList("String".into(), columns.into()), runtime);
        let mut fields = BTreeMap::from([("columns".into(), columns)]);
        if statement {
            fields.insert(
                "parameters".into(),
                Value::Int(json["parameters"].as_i64().ok_or_else(invalid)?),
            );
        }
        runtime.native_value(
            if statement { "DbStatement" } else { "DbCursor" },
            id.as_u64().ok_or_else(invalid)?,
            fields,
        )?
    } else if let Some(count) = json.get("changed") {
        Value::Int(count.as_i64().ok_or_else(invalid)?)
    } else if let Some(rows) = json.get("rows") {
        let mut values = Vec::new();
        for row in rows.as_array().ok_or_else(invalid)? {
            let mut items = Vec::new();
            for item in row.as_array().ok_or_else(invalid)? {
                let variant = item["type"].as_str().ok_or_else(invalid)?;
                let inner = match variant {
                    "Null" => None,
                    "Bool" => Some(Value::Bool(item["value"].as_bool().ok_or_else(invalid)?)),
                    "Int" => Some(Value::Int(item["value"].as_i64().ok_or_else(invalid)?)),
                    "Float" => Some(Value::Float(item["bits"].as_u64().ok_or_else(invalid)?)),
                    "Text" => Some(Value::Text(
                        item["value"].as_str().ok_or_else(invalid)?.into(),
                    )),
                    "Bytes" => Some(Value::Bytes(Arc::new(
                        STANDARD
                            .decode(item["value"].as_str().ok_or_else(invalid)?)
                            .map_err(|_| invalid())?,
                    ))),
                    _ => return Err(invalid()),
                };
                items.push(Value::Enum(
                    "DbValue".into(),
                    variant.into(),
                    inner.map(|v| vec![("0".into(), v)]).unwrap_or_default(),
                ));
            }
            let items = v05::frozen(Value::TypedList("DbValue".into(), items.into()), runtime);
            values.push(Value::Struct(
                "DbRow".into(),
                BTreeMap::from([("values".into(), items)]),
            ));
        }
        let rows = v05::frozen(Value::TypedList("DbRow".into(), values.into()), runtime);
        Value::Struct(
            "DbBatch".into(),
            BTreeMap::from([
                ("rows".into(), rows),
                (
                    "done".into(),
                    Value::Bool(json["done"].as_bool().ok_or_else(invalid)?),
                ),
            ]),
        )
    } else if json["unit"] == true {
        Value::Null
    } else {
        return Err(invalid());
    };
    Ok(Value::Result(Ok(Box::new(value))))
}
fn http_value(json: serde_json::Value, runtime: &mut Runtime) -> Result<Value> {
    if json["adapter"] == "db" {
        return database_value(json, runtime);
    }
    use base64::{engine::general_purpose::STANDARD, Engine};
    let invalid = || Error::InvalidOperation("ReplayMismatch: invalid HTTP response".into());
    if let Some(error) = json.get("error") {
        return Ok(Value::Result(Err(Box::new(Value::Struct(
            "HttpError".into(),
            BTreeMap::from([
                (
                    "code".into(),
                    Value::Text(error["code"].as_str().ok_or_else(invalid)?.into()),
                ),
                (
                    "phase".into(),
                    Value::Text(error["phase"].as_str().ok_or_else(invalid)?.into()),
                ),
                (
                    "status".into(),
                    Value::Int(error["status"].as_i64().ok_or_else(invalid)?),
                ),
            ]),
        )))));
    }
    if let Some(id) = json.get("upload") {
        return Ok(Value::Result(Ok(Box::new(runtime.native_value(
            "HttpUpload",
            id.as_u64().ok_or_else(invalid)?,
            BTreeMap::from([(
                "maxBytes".into(),
                Value::Int(json["maxBytes"].as_i64().ok_or_else(invalid)?),
            )]),
        )?))));
    }
    if json.get("written").is_some() {
        return Ok(Value::Result(Ok(Box::new(Value::Null))));
    }
    if json.get("closed").is_some() {
        return Ok(Value::Result(Ok(Box::new(Value::Null))));
    }
    if json.get("stream").is_some() {
        let body = if json["body"].is_null() {
            None
        } else {
            Some(Box::new(Value::Bytes(Arc::new(
                STANDARD
                    .decode(json["body"].as_str().ok_or_else(invalid)?)
                    .map_err(|_| invalid())?,
            ))))
        };
        return Ok(Value::Result(Ok(Box::new(Value::Option(body)))));
    }
    let body = if json.get("download").is_some() {
        vec![]
    } else {
        STANDARD
            .decode(json["body"].as_str().ok_or_else(invalid)?)
            .map_err(|_| invalid())?
    };
    let mut headers = Vec::new();
    for header in json["headers"].as_array().ok_or_else(invalid)? {
        headers.push(Value::Struct(
            "HttpHeader".into(),
            BTreeMap::from([
                (
                    "name".into(),
                    Value::Text(header["name"].as_str().ok_or_else(invalid)?.into()),
                ),
                (
                    "value".into(),
                    Value::Bytes(Arc::new(
                        STANDARD
                            .decode(header["value"].as_str().ok_or_else(invalid)?)
                            .map_err(|_| invalid())?,
                    )),
                ),
            ]),
        ));
    }
    let headers = v05::frozen(
        Value::TypedList("HttpHeader".into(), headers.into()),
        runtime,
    );
    if let Some(id) = json.get("download") {
        return Ok(Value::Result(Ok(Box::new(runtime.native_value(
            "HttpDownload",
            id.as_u64().ok_or_else(invalid)?,
            BTreeMap::from([
                (
                    "status".into(),
                    Value::Int(json["status"].as_i64().ok_or_else(invalid)?),
                ),
                ("headers".into(), headers),
            ]),
        )?))));
    }
    Ok(Value::Result(Ok(Box::new(Value::Struct(
        "HttpResponse".into(),
        BTreeMap::from([
            (
                "status".into(),
                Value::Int(json["status"].as_i64().ok_or_else(invalid)?),
            ),
            ("headers".into(), headers),
            ("body".into(), Value::Bytes(Arc::new(body))),
        ]),
    )))))
}
