use super::*;
use std::collections::VecDeque;
use std::sync::Arc;

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
    Function(String, Vec<Value>),
    Send(u64, Value),
    Receive(u64),
    Join(u64),
}
#[derive(Clone)]
struct Context {
    pc: usize,
    globals: Vec<BTreeMap<String, Binding>>,
    frames: Vec<VmFrame>,
    branches: Vec<BranchFrame>,
    cleanups: Vec<Cleanup>,
    stack: Arc<Vec<Value>>,
    runtime_frames: Arc<Vec<rewind::CallFrame>>,
    runtime_globals: Arc<BTreeMap<String, Value>>,
}
#[derive(Clone)]
struct Task {
    phase: TaskPhase,
    body: TaskBody,
    priority: i64,
    context: Option<Context>,
    result: Option<std::result::Result<Value, String>>,
    globals: Vec<BTreeMap<String, Binding>>,
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
    next_id: u64,
    tasks: BTreeMap<u64, Task>,
    channels: BTreeMap<u64, Channel>,
    groups: BTreeMap<u64, Vec<u64>>,
}
impl Default for Scheduler {
    fn default() -> Self {
        let main = Task {
            phase: TaskPhase::Running,
            body: TaskBody::Main,
            priority: 0,
            context: None,
            result: None,
            globals: Vec::new(),
            parent: 0,
        };
        Self {
            active: 0,
            next_id: 1,
            tasks: BTreeMap::from([(0, main)]),
            channels: BTreeMap::new(),
            groups: BTreeMap::new(),
        }
    }
}
impl Scheduler {
    // A conservative logical byte count, including retained task contexts.
    // Checkpoint copies are charged separately by the VM; this is not RSS.
    pub(super) fn storage_bytes(&self) -> usize {
        fn value(v: &Value) -> usize {
            std::mem::size_of::<Value>().saturating_add(rewind::Runtime::value_bytes(v))
        }
        fn bindings(scopes: &[BTreeMap<String, Binding>]) -> usize {
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
                    bindings(&c.globals)
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
                            .map(|f| bindings(&f.scopes) + f.defers.len() * 128 + 128)
                            .sum::<usize>()
                        + c.branches
                            .iter()
                            .map(|b| b.scheduler.storage_bytes())
                            .sum::<usize>()
                        + c.cleanups.len() * 128
                });
                256 + body + result + context + bindings(&t.globals)
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
                phase: TaskPhase::Cold,
                body,
                priority: 0,
                context: None,
                result: None,
                globals,
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
        let args = args
            .iter()
            .map(|v| unflow(self.engine.ordered_key(v, at), at))
            .collect::<Result<Vec<_>>>()?;
        let mut globals = Vec::new();
        for scope in self.globals.clone() {
            let mut frozen = BTreeMap::new();
            for (n, binding) in scope {
                let original = self.resolve(&binding.value);
                let value = if matches!(original, Value::Function(_, _)) {
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
        let id = handle_id(value).ok_or_else(|| self.error(at, "expected Task"))?;
        let task = self
            .scheduler
            .tasks
            .get_mut(&id)
            .ok_or_else(|| diagnostic(at, "unknown Task"))?;
        if task.phase == TaskPhase::Cold {
            task.phase = TaskPhase::Ready;
        }
        Ok(id)
    }
    pub(super) fn scheduler_constructor(
        &mut self,
        name: &str,
        args: &[Value],
        at: &Tok,
    ) -> Result<Option<Value>> {
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
                    let value = unflow(self.engine.ordered_key(value, at), at)?;
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
            match (method, args) {
                ("cancel", []) => self.cancel_task(id, at)?,
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
    pub(super) fn task_result(&self, id: u64) -> Option<Value> {
        self.scheduler
            .tasks
            .get(&id)?
            .result
            .as_ref()
            .map(|result| {
                Value::Result(match result {
                    Ok(value) => Ok(Box::new(value.clone())),
                    Err(error) => Err(Box::new(Value::Text(error.clone()))),
                })
            })
    }
    fn complete_task(&mut self, id: u64, result: std::result::Result<Value, String>) {
        let task = self.scheduler.tasks.get_mut(&id).unwrap();
        task.phase = TaskPhase::Done;
        task.result = Some(result);
        task.context = None;
    }
    fn pump_actions(&mut self) {
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
                let result = match body {
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
                                self.complete_task(*receiver, Ok(value.clone()));
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
                    self.complete_task(*id, result);
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
    }
    pub(super) fn schedule(&mut self, at: &Tok) -> Result<bool> {
        let active = self.scheduler.active;
        if self.scheduler.tasks[&active].phase != TaskPhase::Done {
            let context = self.capture_context();
            self.scheduler.tasks.get_mut(&active).unwrap().context = Some(context);
        }
        self.pump_actions();
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
                .unwrap_or(0);
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
            Ok(value) => Ok(unflow(self.engine.ordered_key(&value, at), at)?),
            Err(error) => Err(error),
        };
        let id = self.scheduler.active;
        self.complete_task(id, result);
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
            return Err(self.error(at, "cannot synchronously cancel active task"));
        }
        if let Some(context) = task.context {
            let current = self.capture_context();
            self.install_context(context)?;
            for frame in self.frames.iter().rev().cloned().collect::<Vec<_>>() {
                let _ = self.run_cleanups(frame.defers, at);
            }
            let cleanups = std::mem::take(&mut self.global_cleanups);
            let _ = self.run_cleanups(cleanups, at);
            self.install_context(current)?;
        }
        self.complete_task(id, Err("TaskCancelled".into()));
        Ok(())
    }
}
