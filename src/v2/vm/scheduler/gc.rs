//! Reachability of scheduler identities and borrowed VM roots.
use super::*;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Id {
    Task(u64),
    Channel(u64),
    Group(u64),
}
enum Root<'a> {
    Value(&'a Value),
    Id(Id),
}
fn scopes<'a>(s: &'a [BTreeMap<String, Binding>], out: &mut Vec<Root<'a>>) {
    out.extend(
        s.iter()
            .flat_map(|s| s.values())
            .map(|b| Root::Value(&b.value)),
    );
}
fn cleanups<'a>(s: &'a [Cleanup], out: &mut Vec<Root<'a>>) {
    for c in s {
        match c {
            Cleanup::Expr(_, s, _) => scopes(s, out),
            Cleanup::Callable(v, _) => out.push(Root::Value(v)),
            Cleanup::Group(id, _) => out.push(Root::Id(Id::Group(*id))),
            _ => {}
        }
    }
}
fn frames<'a>(s: &'a [VmFrame], out: &mut Vec<Root<'a>>) {
    for f in s {
        scopes(&f.scopes, out);
        cleanups(&f.defers, out);
    }
}
fn context<'a>(c: &'a Context, out: &mut Vec<Root<'a>>) {
    scopes(&c.globals, out);
    frames(&c.frames, out);
    cleanups(&c.cleanups, out);
    out.extend(c.stack.iter().map(Root::Value));
    out.extend(c.runtime_globals.values().map(Root::Value));
    out.extend(
        c.runtime_frames
            .iter()
            .flat_map(|f| f.locals.values())
            .map(Root::Value),
    );
    branches(&c.branches, out);
}
fn branches<'a>(s: &'a [BranchFrame], out: &mut Vec<Root<'a>>) {
    for b in s {
        scopes(&b.globals, out);
        frames(&b.frames, out);
        let mut values = Vec::new();
        b.scheduler.gc_roots_ref(&mut values);
        out.extend(values.into_iter().map(Root::Value));
    }
}
impl Scheduler {
    pub(in crate::v2::vm) fn gc_roots_ref<'a>(&'a self, out: &mut Vec<&'a Value>) {
        let mut roots = Vec::new();
        for t in self.tasks.values() {
            if t.phase != TaskPhase::Done {
                scopes(&t.globals, &mut roots);
            }
            if let Some(Ok(v)) = &t.result {
                roots.push(Root::Value(v));
            }
            match &t.body {
                TaskBody::Function(_, args) if t.phase != TaskPhase::Done => {
                    roots.extend(args.iter().map(Root::Value))
                }
                TaskBody::Send(_, v) if t.phase != TaskPhase::Done => roots.push(Root::Value(v)),
                _ => {}
            }
            if let Some(c) = &t.context {
                context(c, &mut roots);
            }
        }
        for c in self.channels.values() {
            roots.extend(c.values.iter().map(Root::Value));
        }
        out.extend(roots.into_iter().filter_map(|r| {
            if let Root::Value(v) = r {
                Some(v)
            } else {
                None
            }
        }));
    }
    pub(in crate::v2::vm) fn gc_due(&self) -> bool {
        self.since_gc >= 64
    }
}
impl<R: BufRead> Vm<R> {
    pub(in crate::v2::vm) fn collect_scheduler(&mut self) -> Result<()> {
        let limit = self
            .engine
            .runtime
            .execution_work_remaining()
            .map_or(self.steps, |n| n.min(self.steps));
        let limit = self
            .engine
            .runtime
            .native_work_remaining()
            .map_or(limit, |n| n.min(limit));
        let (live, work) = {
            let mut pending = Vec::new();
            scopes(&self.globals, &mut pending);
            scopes(&self.engine.scopes, &mut pending);
            frames(&self.frames, &mut pending);
            cleanups(&self.global_cleanups, &mut pending);
            branches(&self.branches, &mut pending);
            let state = self.engine.runtime.state();
            pending.extend(state.stack.iter().map(Root::Value));
            pending.extend(state.globals.values().map(Root::Value));
            pending.extend(
                state
                    .call_frames
                    .iter()
                    .flat_map(|f| f.locals.values())
                    .map(Root::Value),
            );
            for (id, t) in &self.scheduler.tasks {
                if *id == 0
                    || *id == self.scheduler.active
                    || !matches!(t.phase, TaskPhase::Done | TaskPhase::Cold)
                    || (!t.observed && !t.ignored && t.result.as_ref().is_some_and(|r| r.is_err()))
                {
                    pending.push(Root::Id(Id::Task(*id)));
                }
            }
            let mut live = BTreeSet::new();
            let mut heap = BTreeSet::new();
            let mut work = 0usize;
            while let Some(root) = pending.pop() {
                work = work.saturating_add(1);
                if work.saturating_add(pending.len()) > limit {
                    return Err(self.error(
                        &self.code[self.pc].at,
                        "NativeWorkBudgetExceeded: scheduler collection",
                    ));
                }
                match root {
                    Root::Id(id) => {
                        if !live.insert(id) {
                            continue;
                        }
                        match id {
                            Id::Task(id) => {
                                if let Some(t) = self.scheduler.tasks.get(&id) {
                                    if t.phase != TaskPhase::Done {
                                        scopes(&t.globals, &mut pending);
                                    }
                                    if let Some(Ok(v)) = &t.result {
                                        pending.push(Root::Value(v));
                                    }
                                    if let TaskPhase::Waiting(target) = t.phase {
                                        pending.push(Root::Id(Id::Task(target)));
                                    }
                                    if t.phase != TaskPhase::Done {
                                        match &t.body {
                                            TaskBody::Function(_, args) => {
                                                pending.extend(args.iter().map(Root::Value))
                                            }
                                            TaskBody::Send(id, v) => {
                                                pending.push(Root::Id(Id::Channel(*id)));
                                                pending.push(Root::Value(v));
                                            }
                                            TaskBody::Receive(id) => {
                                                pending.push(Root::Id(Id::Channel(*id)))
                                            }
                                            TaskBody::Join(id) => {
                                                pending.push(Root::Id(Id::Group(*id)))
                                            }
                                            TaskBody::Select(a, b)
                                            | TaskBody::SelectReady(a, b) => {
                                                pending.push(Root::Id(Id::Task(*a)));
                                                pending.push(Root::Id(Id::Task(*b)));
                                            }
                                            TaskBody::Timeout(id, _, _) => {
                                                pending.push(Root::Id(Id::Task(*id)))
                                            }
                                            _ => {}
                                        }
                                    }
                                    if let Some(c) = &t.context {
                                        context(c, &mut pending);
                                    }
                                }
                            }
                            Id::Channel(id) => {
                                if let Some(c) = self.scheduler.channels.get(&id) {
                                    pending.extend(c.values.iter().map(Root::Value));
                                }
                            }
                            Id::Group(id) => {
                                if let Some(g) = self.scheduler.groups.get(&id) {
                                    pending.extend(g.iter().map(|id| Root::Id(Id::Task(*id))));
                                }
                            }
                        }
                    }
                    Root::Value(value) => {
                        let children = match value {
                            Value::List(v) => v.len(),
                            Value::TypedList(_, v) => v.len(),
                            Value::Map(v) | Value::TypedMap(_, _, v) => v.len(),
                            Value::OrderedMap(_, _, v) => v.len().saturating_mul(2),
                            Value::Struct(ty, v) => v.len().saturating_add(usize::from(
                                (ty.starts_with("Task<")
                                    || ty.starts_with("Channel<")
                                    || ty == "TaskGroup")
                                    && matches!(v.get("$id"),Some(Value::Int(id)) if *id>=0),
                            )),
                            Value::Closure(_, _, v) => v.len(),
                            Value::Enum(_, _, v) => v.len(),
                            Value::HeapRef(_)
                            | Value::CellRef(_)
                            | Value::Option(Some(_))
                            | Value::Result(_) => 1,
                            _ => 0,
                        };
                        if children > limit.saturating_sub(work).saturating_sub(pending.len()) {
                            return Err(self.error(
                                &self.code[self.pc].at,
                                "NativeWorkBudgetExceeded: scheduler children",
                            ));
                        }
                        match value {
                            Value::HeapRef(id) | Value::CellRef(id) => {
                                if heap.insert(*id) {
                                    if let Some(v) = self.engine.runtime.heap_get(*id) {
                                        pending.push(Root::Value(v));
                                    }
                                }
                            }
                            Value::List(v) => pending.extend(v.iter().map(Root::Value)),
                            Value::TypedList(_, v) => pending.extend(v.iter().map(Root::Value)),
                            Value::Map(v) | Value::TypedMap(_, _, v) => {
                                pending.extend(v.values().map(Root::Value))
                            }
                            Value::OrderedMap(_, _, v) => {
                                for (k, v) in v {
                                    pending.push(Root::Value(k));
                                    pending.push(Root::Value(v));
                                }
                            }
                            Value::Struct(ty, v) => {
                                if let Some(Value::Int(id)) = v.get("$id") {
                                    if *id >= 0 {
                                        let id = *id as u64;
                                        if ty.starts_with("Task<") {
                                            pending.push(Root::Id(Id::Task(id)));
                                        } else if ty.starts_with("Channel<") {
                                            pending.push(Root::Id(Id::Channel(id)));
                                        } else if ty == "TaskGroup" {
                                            pending.push(Root::Id(Id::Group(id)));
                                        }
                                    }
                                }
                                pending.extend(v.values().map(Root::Value));
                            }
                            Value::Closure(_, _, v) => pending.extend(v.values().map(Root::Value)),
                            Value::Enum(_, _, v) => {
                                pending.extend(v.iter().map(|(_, v)| Root::Value(v)))
                            }
                            Value::Option(Some(v))
                            | Value::Result(Ok(v))
                            | Value::Result(Err(v)) => pending.push(Root::Value(v)),
                            _ => {}
                        }
                    }
                }
            }
            work = work
                .saturating_add(self.scheduler.tasks.len())
                .saturating_add(self.scheduler.channels.len())
                .saturating_add(self.scheduler.groups.len());
            (live, work)
        };
        self.engine.runtime.charge_native_work(work)?;
        self.engine.runtime.charge_execution_work(work)?;
        self.steps = self.steps.checked_sub(work).ok_or_else(|| {
            self.error(
                &self.code[self.pc].at,
                "ExecutionBudgetExceeded: scheduler collection",
            )
        })?;
        self.scheduler
            .tasks
            .retain(|id, _| live.contains(&Id::Task(*id)));
        self.scheduler
            .channels
            .retain(|id, _| live.contains(&Id::Channel(*id)));
        self.scheduler
            .groups
            .retain(|id, _| live.contains(&Id::Group(*id)));
        self.scheduler.since_gc = 0;
        Ok(())
    }
}
pub(in crate::v2::vm) fn borrowed_roots<'a>(
    globals: &'a [BTreeMap<String, Binding>],
    engine_scopes: &'a [BTreeMap<String, Binding>],
    vm_frames: &'a [VmFrame],
    vm_cleanups: &'a [Cleanup],
    vm_branches: &'a [BranchFrame],
    scheduler: &'a Scheduler,
) -> Vec<&'a Value> {
    let mut roots = Vec::new();
    scopes(globals, &mut roots);
    scopes(engine_scopes, &mut roots);
    frames(vm_frames, &mut roots);
    cleanups(vm_cleanups, &mut roots);
    branches(vm_branches, &mut roots);
    let mut out = roots
        .into_iter()
        .filter_map(|r| {
            if let Root::Value(v) = r {
                Some(v)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    scheduler.gc_roots_ref(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn vm() -> Vm<std::io::Cursor<Vec<u8>>> {
        // Windows canonical paths include the extended-length prefix used by Vm::new.
        let root = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let program = Program {
            language: "1.9.3".into(),
            root_origin: root.join("main.rw"),
            ..Program::default()
        };
        Vm::new(
            program,
            &root,
            std::io::Cursor::new(vec![]),
            false,
            false,
            &RunOptions::default(),
        )
        .unwrap()
    }
    fn root_handle(vm: &mut Vm<std::io::Cursor<Vec<u8>>>, value: Value) {
        vm.globals[0].insert(
            "keep".into(),
            Binding {
                ty: "Unknown".into(),
                mutable: false,
                value,
            },
        );
    }
    #[test]
    fn task_quota_is_shared_across_restore_and_released_with_last_checkpoint() {
        let mut vm = vm();
        vm.engine.program.language = "1.9.20".into();
        let mut task = vm.scheduler.tasks[&0].clone();
        task.instructions = Arc::new(std::cell::Cell::new(0));
        task.observed = true;
        let weak = Arc::downgrade(&task.instructions);
        vm.scheduler.tasks.insert(1, task);
        vm.scheduler.active = 1;
        vm.charge_task_instruction();
        let snapshot = vm.snapshot();
        for _ in 0..100 {
            vm.charge_task_instruction();
        }
        vm.restore(snapshot.clone()).unwrap();
        assert_eq!(vm.task_instruction_count(1), 101);
        assert_eq!(vm.task_instruction_total, 101);
        assert!(vm.task_instructions.is_empty());
        vm.snapshots.insert("held".into(), snapshot);
        vm.scheduler.active = 0;
        vm.scheduler.tasks.get_mut(&1).unwrap().phase = TaskPhase::Done;
        vm.collect_scheduler().unwrap();
        assert!(!vm.scheduler.tasks.contains_key(&1));
        assert_eq!(weak.upgrade().unwrap().get(), 101);
        vm.snapshots.remove("held");
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn fresh_task_after_restore_has_its_own_quota_even_when_scheduler_id_repeats() {
        let mut vm = vm();
        vm.engine.program.language = "1.9.20".into();
        let snapshot = vm.snapshot();
        let at = vm.scheduler.tasks[&0].at.clone();
        let first = vm
            .new_action(
                TaskBody::Function("unused".into(), vec![]),
                "Unit".into(),
                vec![],
                &at,
            )
            .unwrap();
        let id = handle_id(&first).unwrap();
        vm.scheduler.active = id;
        vm.charge_task_instruction();
        let old_counter = vm.scheduler.tasks[&id].instructions.clone();
        vm.restore(snapshot).unwrap();
        let second = vm
            .new_action(
                TaskBody::Function("unused".into(), vec![]),
                "Unit".into(),
                vec![],
                &at,
            )
            .unwrap();
        assert_eq!(handle_id(&second), Some(id));
        assert_eq!(vm.task_instruction_count(id), 0);
        assert_eq!(old_counter.get(), 1);
        assert!(!Arc::ptr_eq(
            &old_counter,
            &vm.scheduler.tasks[&id].instructions
        ));
    }
    #[test]
    fn scheduler_collection_budget_failure_preserves_result_storage() {
        let mut vm = vm();
        let bytes = Arc::new(vec![0; 1024]);
        let weak = Arc::downgrade(&bytes);
        let mut task = vm.scheduler.tasks[&0].clone();
        task.phase = TaskPhase::Done;
        task.result = Some(Ok(Value::Bytes(bytes)));
        task.observed = true;
        vm.scheduler.tasks.insert(1, task);
        vm.engine.runtime.configure_native_work(0);
        assert!(vm.collect_scheduler().is_err());
        assert!(vm.scheduler.tasks.contains_key(&1));
        assert!(weak.upgrade().is_some());
    }
    #[test]
    fn scheduler_collection_releases_buffer_only_after_checkpoint_owner_drops() {
        let mut vm = vm();
        let bytes = Arc::new(vec![0; 1024 * 1024]);
        let weak = Arc::downgrade(&bytes);
        let mut task = vm.scheduler.tasks[&0].clone();
        task.phase = TaskPhase::Done;
        task.result = Some(Ok(Value::Bytes(bytes)));
        task.observed = true;
        vm.scheduler.tasks.insert(1, task);
        let snapshot = vm.scheduler.clone();
        vm.collect_scheduler().unwrap();
        assert!(!vm.scheduler.tasks.contains_key(&1));
        assert!(weak.upgrade().is_some());
        drop(snapshot);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn reachable_completed_result_survives_until_its_handle_dies() {
        let mut vm = vm();
        let bytes = Arc::new(vec![0; 1024 * 1024]);
        let weak = Arc::downgrade(&bytes);
        let mut task = vm.scheduler.tasks[&0].clone();
        task.phase = TaskPhase::Done;
        task.result = Some(Ok(Value::Bytes(bytes)));
        task.observed = true;
        vm.scheduler.tasks.insert(1, task);
        root_handle(&mut vm, handle("Task<Bytes>".into(), 1));
        vm.collect_scheduler().unwrap();
        assert!(weak.upgrade().is_some());
        assert!(vm.scheduler.tasks.contains_key(&1));
        vm.globals[0].remove("keep");
        vm.collect_scheduler().unwrap();
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn unreachable_channel_task_cycle_releases_queued_buffer() {
        let mut vm = vm();
        let bytes = Arc::new(vec![0; 1024 * 1024]);
        let weak = Arc::downgrade(&bytes);
        let mut task = vm.scheduler.tasks[&0].clone();
        task.phase = TaskPhase::Cold;
        task.body = TaskBody::Function(
            "unused".into(),
            vec![handle("Channel<Tuple<Task<Int>,Bytes>>".into(), 2)],
        );
        vm.scheduler.tasks.insert(1, task);
        let value = Value::Struct(
            "Tuple<Task<Int>,Bytes>".into(),
            BTreeMap::from([
                ("0".into(), handle("Task<Int>".into(), 1)),
                ("1".into(), Value::Bytes(bytes)),
            ]),
        );
        vm.scheduler.channels.insert(
            2,
            Channel {
                capacity: 1,
                closed: false,
                values: VecDeque::from([value]),
            },
        );
        root_handle(&mut vm, handle("Channel<Tuple<Task<Int>,Bytes>>".into(), 2));
        vm.collect_scheduler().unwrap();
        assert!(weak.upgrade().is_some());
        vm.globals[0].remove("keep");
        vm.collect_scheduler().unwrap();
        assert!(weak.upgrade().is_none());
        assert!(vm.scheduler.channels.is_empty());
        assert_eq!(vm.scheduler.tasks.len(), 1);
    }
}
