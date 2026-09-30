use super::*;
pub(in crate::v2) fn record(error: &Error, at: &Tok, task_id: u64) -> rewind::DiagnosticRecord {
    if let Error::Diagnostic(d) = error {
        let mut d = (**d).clone();
        d.task_id = Some(task_id);
        if d.line == 0 {
            d.source = at.source.clone();
            d.line = at.line;
            d.column = at.col;
        }
        return d;
    }
    let code = match error {
        Error::ExternalStateConflict(_) => "ExternalStateConflict",
        Error::PublishPartiallyApplied(_) => "PublishPartiallyApplied",
        Error::HistoryBudgetExceeded => "HistoryMemory",
        Error::Io(_) => "Io",
        Error::MissingFile(_) => "NotFound",
        Error::InvalidPath(_) => "InvalidPath",
        Error::InvalidOperation(message) => code(message),
        _ => "TaskFailed",
    };
    rewind::DiagnosticRecord {
        code: code.into(),
        message: error.to_string(),
        source: at.source.clone(),
        line: at.line,
        column: at.col,
        task_id: Some(task_id),
        causes: vec![],
        wait_edges: vec![],
    }
}
pub(in crate::v2) fn value(d: &rewind::DiagnosticRecord) -> Value {
    Value::Struct(
        "Diagnostic".into(),
        BTreeMap::from([
            ("code".into(), Value::Text(d.code.clone())),
            ("message".into(), Value::Text(d.message.clone())),
            ("source".into(), Value::Text(d.source.clone())),
            ("line".into(), Value::Int(d.line as i64)),
            ("column".into(), Value::Int(d.column as i64)),
            (
                "taskId".into(),
                Value::Option(d.task_id.map(|n| Box::new(Value::Int(n as i64)))),
            ),
            (
                "causes".into(),
                Value::TypedList("Diagnostic".into(), d.causes.iter().map(value).collect()),
            ),
            ("waitGraph".into(), graph(d)),
        ]),
    )
}
pub(in crate::v2) fn task_error(d: &rewind::DiagnosticRecord) -> Value {
    let (variant, fields) = if d.causes.is_empty() && d.code == "TaskCancelled" {
        ("Cancelled", vec![])
    } else if d.causes.is_empty() && d.code == "LogicalTimeout" {
        ("TimedOut", vec![])
    } else if d.code == "ChannelClosed" {
        ("ChannelClosed", vec![])
    } else if d.causes.is_empty() && d.code == "TaskDeadlock" {
        ("Deadlock", vec![("0".into(), graph(d))])
    } else if d.causes.is_empty()
        && matches!(
            d.code.as_str(),
            "ExecutionBudgetExceeded"
                | "SchedulerStorage"
                | "SchedulerObjects"
                | "TaskSteps"
                | "TypeExpansionBudgetExceeded"
                | "MonomorphizationBudgetExceeded"
                | "TransferDepth"
                | "HistoryMemory"
                | "HistoryStorage"
                | "IteratorItems"
                | "EffectInference"
                | "DependencyResolution"
        )
    {
        let kind = match d.code.as_str() {
            "ExecutionBudgetExceeded" => "ExecutionSteps",
            "TypeExpansionBudgetExceeded" => "TypeExpansion",
            "MonomorphizationBudgetExceeded" => "Monomorphization",
            s => s,
        };
        (
            "BudgetExceeded",
            vec![(
                "0".into(),
                Value::Enum("BudgetKind".into(), kind.into(), vec![]),
            )],
        )
    } else {
        ("Failed", vec![("0".into(), value(d))])
    };
    Value::Enum("TaskError".into(), variant.into(), fields)
}
fn graph(d: &rewind::DiagnosticRecord) -> Value {
    Value::Struct(
        "WaitGraph".into(),
        BTreeMap::from([
            ("description".into(), Value::Text(d.message.clone())),
            (
                "edges".into(),
                Value::TypedList(
                    "WaitEdge".into(),
                    d.wait_edges
                        .iter()
                        .map(|edge| {
                            let (kind, id) = match edge.target {
                                rewind::WaitTarget::Task(n) => ("Task", n),
                                rewind::WaitTarget::Channel(n) => ("Channel", n),
                                rewind::WaitTarget::Group(n) => ("Group", n),
                            };
                            Value::Struct(
                                "WaitEdge".into(),
                                BTreeMap::from([
                                    ("task".into(), Value::Int(edge.task as i64)),
                                    (
                                        "target".into(),
                                        Value::Enum(
                                            "WaitTarget".into(),
                                            kind.into(),
                                            vec![("0".into(), Value::Int(id as i64))],
                                        ),
                                    ),
                                ]),
                            )
                        })
                        .collect(),
                ),
            ),
        ]),
    )
}
pub(in crate::v2) fn masked_record(
    d: &rewind::DiagnosticRecord,
    rt: &Runtime,
) -> serde_json::Value {
    let mut value = serde_json::to_value(d).unwrap_or(serde_json::Value::Null);
    value["message"] = serde_json::Value::String(rt.masked_value(&Value::Text(d.message.clone())));
    value["causes"] =
        serde_json::Value::Array(d.causes.iter().map(|d| masked_record(d, rt)).collect());
    value
}

pub(in crate::v2) fn code(message: &str) -> &str {
    if message.starts_with("TaskBudgetExceeded: scheduler storage") {
        "SchedulerStorage"
    } else if message.starts_with("TaskBudgetExceeded: objects") {
        "SchedulerObjects"
    } else if message.starts_with("TaskBudgetExceeded: instructions") {
        "TaskSteps"
    } else if message.starts_with("TaskTransferBudgetExceeded") {
        "TransferDepth"
    } else if message.starts_with("IteratorBudgetExceeded") {
        "IteratorItems"
    } else if message.starts_with("TraceBudgetExceeded")
        || message.starts_with("ArtifactBudgetExceeded")
    {
        "HistoryStorage"
    } else if message.contains("undeclared effect")
        || message.contains("effect ") && message.contains("not allowed")
    {
        "EffectMissing"
    } else if message.starts_with("EffectInferenceBudgetExceeded") {
        "EffectInference"
    } else if message.starts_with("DependencyResolutionBudgetExceeded") {
        "DependencyResolution"
    } else {
        message.split([':', ' ']).next().unwrap_or("TaskFailed")
    }
}
