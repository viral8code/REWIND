use super::*;
mod artifact;
mod capabilities;
mod ownership;
mod tooling;
pub(super) use artifact::{build as artifact, run as run_artifact};
pub(super) use tooling::{debug_session, lsp};

pub(super) mod pairs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;
    pub fn serialize<S: Serializer, K: Serialize + Ord, V: Serialize>(
        map: &BTreeMap<K, V>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        map.iter().collect::<Vec<_>>().serialize(serializer)
    }
    pub fn deserialize<
        'de,
        D: Deserializer<'de>,
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
    >(
        deserializer: D,
    ) -> std::result::Result<BTreeMap<K, V>, D::Error> {
        let entries = Vec::<(K, V)>::deserialize(deserializer)?;
        let len = entries.len();
        let map = entries.into_iter().collect::<BTreeMap<_, _>>();
        if len != map.len() {
            return Err(serde::de::Error::custom("duplicate map key"));
        }
        Ok(map)
    }
}

pub(super) fn prepare(program: &mut Program) -> Result<()> {
    v06::language::prepare(program)?;
    v091::prepare(program)?;
    v092::prepare(program)?;
    v100::prepare(program)?;
    if !matches!(
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
            | "1.7.0"
            | "1.8.0"
            | "1.9.0"
            | "2.0.0"
    ) {
        return Ok(());
    }
    for name in [
        "Frozen",
        "Secret",
        "Iterator",
        "Task",
        "Channel",
        "TaskGroup",
    ] {
        if program.structs.contains_key(name) || program.enums.contains_key(name) {
            return Err(Error::InvalidOperation(format!(
                "reserved standard type {name}"
            )));
        }
    }
    for name in ["freeze", "thaw", "secret", "reveal"] {
        if program.functions.contains_key(name) {
            return Err(Error::InvalidOperation(format!(
                "reserved standard function {name}"
            )));
        }
    }
    if program.enums.contains_key("TaskError")
        || program.structs.contains_key("Diagnostic")
        || program.structs.contains_key("WaitGraph")
        || program.enums.contains_key("BudgetKind")
    {
        return Err(Error::InvalidOperation(
            "TaskError and Diagnostic are reserved standard types".into(),
        ));
    }
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
            | "1.7.0"
            | "1.8.0"
            | "1.9.0"
            | "2.0.0"
    ) && ["WaitEdge", "WaitTarget", "Tuple"]
        .iter()
        .any(|n| program.structs.contains_key(*n) || program.enums.contains_key(*n))
    {
        return Err(Error::InvalidOperation(
            "WaitEdge, WaitTarget, and Tuple are reserved standard types".into(),
        ));
    }
    program.enums.insert(
        "TaskError".into(),
        EnumDef {
            type_params: Vec::new(),
            variants: BTreeMap::from([
                ("Cancelled".into(), Vec::new()),
                ("ChannelClosed".into(), Vec::new()),
                ("Deadlock".into(), vec![("0".into(), "WaitGraph".into())]),
                (
                    "BudgetExceeded".into(),
                    vec![("0".into(), "BudgetKind".into())],
                ),
                ("Failed".into(), vec![("0".into(), "Diagnostic".into())]),
            ]),
            public: true,
            origin: program.root_origin.clone(),
        },
    );
    program.structs.insert(
        "WaitGraph".into(),
        StructDef {
            private_fields: BTreeSet::new(),
            bounds: BTreeMap::new(),
            immutable: false,
            type_params: Vec::new(),
            fields: vec![("description".into(), "String".into())],
            public: true,
            origin: program.root_origin.clone(),
        },
    );
    program.enums.insert(
        "BudgetKind".into(),
        EnumDef {
            type_params: Vec::new(),
            variants: BTreeMap::from([
                ("ExecutionSteps".into(), vec![]),
                ("TaskSteps".into(), vec![]),
                ("History".into(), vec![]),
                ("Other".into(), vec![("0".into(), "String".into())]),
            ]),
            public: true,
            origin: program.root_origin.clone(),
        },
    );
    program.structs.insert(
        "Diagnostic".into(),
        StructDef {
            private_fields: BTreeSet::new(),
            bounds: BTreeMap::new(),
            immutable: false,
            type_params: Vec::new(),
            fields: vec![
                ("code".into(), "String".into()),
                ("message".into(), "String".into()),
                ("causes".into(), "List<String>".into()),
            ],
            public: true,
            origin: program.root_origin.clone(),
        },
    );
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
            | "1.7.0"
            | "1.8.0"
            | "1.9.0"
            | "2.0.0"
    ) {
        program
            .enums
            .get_mut("TaskError")
            .unwrap()
            .variants
            .insert("TimedOut".into(), vec![]);
        program.structs.get_mut("Diagnostic").unwrap().fields = vec![
            ("code".into(), "String".into()),
            ("message".into(), "String".into()),
            ("source".into(), "String".into()),
            ("line".into(), "Int".into()),
            ("column".into(), "Int".into()),
            ("taskId".into(), "Option<Int>".into()),
            ("causes".into(), "List<Diagnostic>".into()),
            ("waitGraph".into(), "WaitGraph".into()),
        ];
        program
            .structs
            .get_mut("WaitGraph")
            .unwrap()
            .fields
            .push(("edges".into(), "List<WaitEdge>".into()));
        program.structs.insert(
            "WaitEdge".into(),
            StructDef {
                private_fields: BTreeSet::new(),
                bounds: BTreeMap::new(),
                immutable: false,
                type_params: vec![],
                fields: vec![
                    ("task".into(), "Int".into()),
                    ("target".into(), "WaitTarget".into()),
                ],
                public: true,
                origin: program.root_origin.clone(),
            },
        );
        program.enums.insert(
            "WaitTarget".into(),
            EnumDef {
                type_params: vec![],
                variants: ["Task", "Channel", "Group"]
                    .into_iter()
                    .map(|n| (n.into(), vec![("0".into(), "Int".into())]))
                    .collect(),
                public: true,
                origin: program.root_origin.clone(),
            },
        );
        for kind in [
            "SchedulerStorage",
            "SchedulerObjects",
            "TransferDepth",
            "TypeExpansion",
            "Monomorphization",
            "HistoryMemory",
            "HistoryStorage",
            "IteratorItems",
            "EffectInference",
            "DependencyResolution",
            "NativeWork",
            "Compiler",
        ] {
            program
                .enums
                .get_mut("BudgetKind")
                .unwrap()
                .variants
                .insert(kind.into(), vec![]);
        }
        if program.structs.contains_key("PropertyFailure")
            || program.functions.contains_key("propertyInt")
        {
            return Err(Error::InvalidOperation(
                "PropertyFailure and propertyInt are reserved".into(),
            ));
        }
        program.structs.insert(
            "PropertyFailure".into(),
            StructDef {
                private_fields: BTreeSet::new(),
                bounds: BTreeMap::new(),
                immutable: false,
                type_params: Vec::new(),
                fields: vec![
                    ("seed".into(), "Int".into()),
                    ("case".into(), "Int".into()),
                    ("input".into(), "Int".into()),
                    ("shrinks".into(), "Int".into()),
                ],
                public: true,
                origin: program.root_origin.clone(),
            },
        );
        if program_v07(program) {
            if program.structs.contains_key("PropertyCase")
                || program.functions.contains_key("property")
                || program.functions.contains_key("propertyCandidates")
            {
                return Err(Error::InvalidOperation(
                    "PropertyCase and property are reserved".into(),
                ));
            }
            program.structs.insert(
                "PropertyCase".into(),
                StructDef {
                    private_fields: BTreeSet::new(),
                    bounds: BTreeMap::new(),
                    immutable: false,
                    type_params: vec!["T".into()],
                    fields: vec![
                        ("seed".into(), "Int".into()),
                        ("case".into(), "Int".into()),
                        ("input".into(), "T".into()),
                        ("shrinks".into(), "Int".into()),
                    ],
                    public: true,
                    origin: program.root_origin.clone(),
                },
            );
        }
        v06::infer(program)?;
    }
    Ok(())
}

pub(super) fn task_error(error: &str) -> Value {
    let cleanup = error.contains("; cleanup: ");
    let (variant, fields) = if !cleanup && error.ends_with("TaskCancelled") {
        ("Cancelled", Vec::new())
    } else if error == "ChannelClosed" {
        ("ChannelClosed", Vec::new())
    } else if !cleanup && error.contains("TaskDeadlock") {
        (
            "Deadlock",
            vec![(
                "0".into(),
                Value::Struct(
                    "WaitGraph".into(),
                    BTreeMap::from([("description".into(), Value::Text(error.into()))]),
                ),
            )],
        )
    } else if !cleanup
        && (error.contains("BudgetExceeded") || error.contains("history budget exceeded"))
    {
        (
            "BudgetExceeded",
            vec![(
                "0".into(),
                Value::Enum(
                    "BudgetKind".into(),
                    if error.contains("ExecutionBudget") {
                        "ExecutionSteps"
                    } else if error.contains("TaskBudgetExceeded: instructions") {
                        "TaskSteps"
                    } else if error.contains("history budget exceeded") {
                        "History"
                    } else {
                        "Other"
                    }
                    .into(),
                    if error.contains("ExecutionBudget")
                        || error.contains("history budget exceeded")
                        || error.contains("TaskBudgetExceeded: instructions")
                    {
                        vec![]
                    } else {
                        vec![("0".into(), Value::Text(error.into()))]
                    },
                ),
            )],
        )
    } else {
        (
            "Failed",
            vec![(
                "0".into(),
                Value::Struct(
                    "Diagnostic".into(),
                    BTreeMap::from([
                        (
                            "code".into(),
                            Value::Text(
                                if error.contains("TaskCancelled") {
                                    "TaskCancelled"
                                } else if error.contains("BudgetExceeded")
                                    || error.contains("history budget exceeded")
                                {
                                    "BudgetExceeded"
                                } else if error.contains("TaskDeadlock") {
                                    "TaskDeadlock"
                                } else {
                                    "TaskFailed"
                                }
                                .into(),
                            ),
                        ),
                        (
                            "message".into(),
                            Value::Text(error.split("; cleanup: ").next().unwrap_or(error).into()),
                        ),
                        (
                            "causes".into(),
                            Value::TypedList(
                                "String".into(),
                                error
                                    .split("; cleanup: ")
                                    .skip(1)
                                    .map(|s| Value::Text(s.into()))
                                    .collect(),
                            ),
                        ),
                    ]),
                ),
            )],
        )
    };
    Value::Enum("TaskError".into(), variant.into(), fields)
}

pub(super) fn validate(program: &Program, config: &project::ProjectConfig) -> Result<()> {
    ownership::validate(program)?;
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
            | "1.7.0"
            | "1.8.0"
            | "1.9.0"
            | "2.0.0"
    ) {
        return v06::validate(program, config);
    }
    capabilities::validate(program, config)
}

pub(super) fn unfrozen(value: &Value) -> Option<&Value> {
    if let Value::Struct(ty, fields) = value {
        if ty.starts_with("Frozen<") {
            return fields.get("$value");
        }
    }
    None
}
pub(super) fn unsecret(value: &Value) -> Option<&Value> {
    if let Value::Struct(t, f) = value {
        if t.starts_with("Secret<") {
            return f.get("$value");
        }
    }
    None
}
pub(super) fn secret(value: Value, ty: String) -> Value {
    Value::Struct(
        format!("Secret<{ty}>"),
        BTreeMap::from([("$value".into(), value)]),
    )
}
pub(super) fn frozen(value: Value, rt: &Runtime) -> Value {
    if unfrozen(&value).is_some() {
        return value;
    }
    Value::Struct(
        format!("Frozen<{}>", value_type(&value, rt)),
        BTreeMap::from([("$value".into(), value)]),
    )
}
pub(super) fn freeze_member(value: Value, rt: &Runtime) -> Value {
    if matches!(
        value,
        Value::Bool(_)
            | Value::Int(_)
            | Value::Float(_)
            | Value::Text(_)
            | Value::Bytes(_)
            | Value::Null
    ) {
        value
    } else {
        frozen(value, rt)
    }
}
pub(super) fn frozen_type_result(ty: &str) -> String {
    if matches!(ty, "Bool" | "Int" | "Float" | "String" | "Bytes" | "Unit") {
        ty.into()
    } else {
        format!("Frozen<{ty}>")
    }
}
pub(super) fn iterator_item(ty: &str) -> Option<String> {
    if let Some(t) = ty.strip_prefix("Frozen<").and_then(|s| s.strip_suffix('>')) {
        return iterator_item(t);
    }
    if matches!(ty, "Bytes" | "String") {
        return Some(if ty == "Bytes" { "Int" } else { "String" }.into());
    }
    if ty.starts_with("List<") || ty.starts_with("Iterator<") {
        return Some(outer_type_end(ty.split_once('<')?.1).into());
    }
    if ty.starts_with("Map<") {
        return split_type_args(outer_type_end(ty.split_once('<')?.1))
            .first()
            .map(|t| t.to_string());
    }
    None
}
pub(super) fn new_iterator(rt: &mut Runtime, value: &Value) -> Result<Option<Value>> {
    let ty = value_type(value, rt);
    let Some(item) = iterator_item(&ty) else {
        return Ok(None);
    };
    let values = match value {
        Value::List(v) => v.clone(),
        Value::TypedList(_, v) => v.iter().cloned().collect(),
        Value::Map(v) | Value::TypedMap(_, _, v) => v.keys().map(MapKey::value).collect(),
        Value::OrderedMap(_, _, v) => v.iter().map(|(k, _)| k.clone()).collect(),
        Value::Text(s) => s.chars().map(|c| Value::Text(c.to_string())).collect(),
        Value::Bytes(v) => v.iter().map(|b| Value::Int(i64::from(*b))).collect(),
        Value::Struct(t, fields) if t.starts_with("Iterator<") => {
            let Value::TypedList(_, v) = &fields["$values"] else {
                return Err(Error::InvalidOperation("invalid iterator".into()));
            };
            let Value::Int(index) = fields["$index"] else {
                return Err(Error::InvalidOperation("invalid iterator".into()));
            };
            v.iter().skip(index as usize).cloned().collect()
        }
        _ => return Ok(None),
    };
    Ok(Some(Value::HeapRef(rt.alloc(Value::Struct(
        format!("Iterator<{item}>"),
        BTreeMap::from([
            ("$values".into(), Value::TypedList(item, values.into())),
            ("$index".into(), Value::Int(0)),
        ]),
    ))?)))
}
pub(super) fn iterator_next(rt: &mut Runtime, value: &Value) -> Result<Option<Value>> {
    let Value::HeapRef(id) = value else {
        return Ok(None);
    };
    let Some(Value::Struct(ty, mut fields)) = rt.heap_get(*id).cloned() else {
        return Ok(None);
    };
    if !ty.starts_with("Iterator<") {
        return Ok(None);
    }
    let Some(Value::Int(index)) = fields.get("$index") else {
        return Err(Error::InvalidOperation("invalid iterator cursor".into()));
    };
    let Some(Value::TypedList(_, items)) = fields.get("$values") else {
        return Err(Error::InvalidOperation("invalid iterator values".into()));
    };
    let result = items.get(*index as usize).cloned();
    fields.insert("$index".into(), Value::Int(index + 1));
    rt.heap_set(*id, Value::Struct(ty, fields))?;
    Ok(Some(Value::Option(
        result.map(|v| thaw(rt, &v).map(Box::new)).transpose()?,
    )))
}
pub(super) fn thaw(rt: &mut Runtime, value: &Value) -> Result<Value> {
    if let Some(v) = unfrozen(value) {
        return thaw(rt, v);
    }
    let result = match value {
        Value::TypedList(t, items) => Value::TypedList(
            t.clone(),
            items.iter().map(|v| thaw(rt, v)).collect::<Result<_>>()?,
        ),
        Value::List(items) => {
            Value::List(items.iter().map(|v| thaw(rt, v)).collect::<Result<_>>()?)
        }
        Value::TypedMap(k, v, items) => Value::TypedMap(
            k.clone(),
            v.clone(),
            items
                .iter()
                .map(|(k, v)| Ok((k.clone(), thaw(rt, v)?)))
                .collect::<Result<_>>()?,
        ),
        Value::OrderedMap(k, v, items) => Value::OrderedMap(
            k.clone(),
            v.clone(),
            items
                .iter()
                .map(|(k, v)| Ok((thaw(rt, k)?, thaw(rt, v)?)))
                .collect::<Result<_>>()?,
        ),
        Value::Struct(t, fields) if !fields.contains_key("$id") => Value::Struct(
            t.clone(),
            fields
                .iter()
                .map(|(n, v)| Ok((n.clone(), thaw(rt, v)?)))
                .collect::<Result<_>>()?,
        ),
        Value::Enum(t, n, fields) => {
            return Ok(Value::Enum(
                t.clone(),
                n.clone(),
                fields
                    .iter()
                    .map(|(n, v)| Ok((n.clone(), thaw(rt, v)?)))
                    .collect::<Result<_>>()?,
            ))
        }
        Value::Option(v) => {
            return Ok(Value::Option(
                v.as_ref().map(|v| thaw(rt, v).map(Box::new)).transpose()?,
            ))
        }
        Value::Result(Ok(v)) => return Ok(Value::Result(Ok(Box::new(thaw(rt, v)?)))),
        Value::Result(Err(v)) => return Ok(Value::Result(Err(Box::new(thaw(rt, v)?)))),
        other => return Ok(other.clone()),
    };
    Ok(Value::HeapRef(rt.alloc(result)?))
}
// Each task owns an independent graph. Cells captured by closures are copied,
// while scheduler handles keep their VM identity. Cyclic graphs are rejected.
pub(super) fn task_copy(rt: &mut Runtime, value: &Value) -> Result<Value> {
    fn copy(rt: &mut Runtime, v: &Value, seen: &mut BTreeSet<u64>, depth: usize) -> Result<Value> {
        if depth > 64 {
            return Err(Error::InvalidOperation(
                "TaskTransferBudgetExceeded: depth 64".into(),
            ));
        }
        Ok(match v {
            Value::HeapRef(id) | Value::CellRef(id) => {
                if !seen.insert(*id) {
                    return Err(Error::InvalidOperation(
                        "task transfer cannot contain a cyclic graph".into(),
                    ));
                }
                let original = rt
                    .heap_get(*id)
                    .cloned()
                    .ok_or_else(|| Error::InvalidOperation("invalid task reference".into()))?;
                let inner = copy(rt, &original, seen, depth + 1)?;
                seen.remove(id);
                let id = rt.alloc(inner)?;
                if matches!(v, Value::CellRef(_)) {
                    Value::CellRef(id)
                } else {
                    Value::HeapRef(id)
                }
            }
            Value::Handle(_) => {
                return Err(Error::InvalidOperation("FileHandle is not Send".into()))
            }
            Value::Closure(n, t, c) => Value::Closure(
                n.clone(),
                t.clone(),
                c.iter()
                    .map(|(n, v)| Ok((n.clone(), copy(rt, v, seen, depth + 1)?)))
                    .collect::<Result<_>>()?,
            ),
            Value::Struct(t, _) if t.starts_with("Frozen<") => v.clone(),
            Value::Struct(t, f) => Value::Struct(
                t.clone(),
                f.iter()
                    .map(|(n, v)| Ok((n.clone(), copy(rt, v, seen, depth + 1)?)))
                    .collect::<Result<_>>()?,
            ),
            Value::List(items) => Value::List(
                items
                    .iter()
                    .map(|v| copy(rt, v, seen, depth + 1))
                    .collect::<Result<_>>()?,
            ),
            Value::TypedList(t, items) => Value::TypedList(
                t.clone(),
                items
                    .iter()
                    .map(|v| copy(rt, v, seen, depth + 1))
                    .collect::<Result<_>>()?,
            ),
            Value::Map(items) => Value::Map(
                items
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), copy(rt, v, seen, depth + 1)?)))
                    .collect::<Result<_>>()?,
            ),
            Value::TypedMap(k, t, items) => Value::TypedMap(
                k.clone(),
                t.clone(),
                items
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), copy(rt, v, seen, depth + 1)?)))
                    .collect::<Result<_>>()?,
            ),
            Value::OrderedMap(k, t, items) => Value::OrderedMap(
                k.clone(),
                t.clone(),
                items
                    .iter()
                    .map(|(k, v)| {
                        Ok((copy(rt, k, seen, depth + 1)?, copy(rt, v, seen, depth + 1)?))
                    })
                    .collect::<Result<_>>()?,
            ),
            Value::Enum(t, n, f) => Value::Enum(
                t.clone(),
                n.clone(),
                f.iter()
                    .map(|(n, v)| Ok((n.clone(), copy(rt, v, seen, depth + 1)?)))
                    .collect::<Result<_>>()?,
            ),
            Value::Option(v) => Value::Option(
                v.as_ref()
                    .map(|v| copy(rt, v, seen, depth + 1).map(Box::new))
                    .transpose()?,
            ),
            Value::Result(Ok(v)) => Value::Result(Ok(Box::new(copy(rt, v, seen, depth + 1)?))),
            Value::Result(Err(v)) => Value::Result(Err(Box::new(copy(rt, v, seen, depth + 1)?))),
            _ => v.clone(),
        })
    }
    copy(rt, value, &mut BTreeSet::new(), 0)
}
pub(super) fn transfer_type(
    program: &Program,
    ty: &str,
    shared: bool,
    seen: &mut BTreeSet<String>,
) -> bool {
    if ty.starts_with('&') {
        return false;
    }
    if matches!(ty, "Bool" | "Int" | "Float" | "String" | "Bytes" | "Unit")
        || ty.starts_with("Task<")
        || ty.starts_with("Channel<")
        || ty == "TaskGroup"
    {
        return true;
    }
    if ty == "Json"
        && matches!(
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
                | "1.7.0"
                | "1.8.0"
                | "1.9.0"
                | "2.0.0"
        )
    {
        return true;
    }
    if ty == "FileError" {
        return true;
    }
    if matches!(
        program.language.as_str(),
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
            | "1.7.0"
            | "1.8.0"
            | "1.9.0"
            | "2.0.0"
    ) {
        if let Some((base, inner)) = ty.split_once('<') {
            if matches!(base, "Option" | "Result") {
                return split_type_args(outer_type_end(inner))
                    .iter()
                    .all(|t| transfer_type(program, t, shared, seen));
            }
        }
    }
    if ty.starts_with("fn(") {
        return if matches!(
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
                | "1.7.0"
                | "1.8.0"
                | "1.9.0"
                | "2.0.0"
        ) {
            v06::captures::flags(ty).contains(if shared { "Share" } else { "Send" })
        } else {
            !shared
        };
    }
    if matches!(ty, "FileHandle") {
        return false;
    }
    if let Some(t) = ty.strip_prefix("Frozen<").and_then(|s| s.strip_suffix('>')) {
        return transfer_type(program, t, false, seen);
    }
    if let Some(t) = ty.strip_prefix("Secret<").and_then(|s| s.strip_suffix('>')) {
        return transfer_type(program, t, shared, seen);
    }
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
            | "1.7.0"
            | "1.8.0"
            | "1.9.0"
            | "2.0.0"
    ) {
        if let Some(t) = ty.strip_prefix("Tuple<").and_then(|s| s.strip_suffix('>')) {
            return split_type_args(t)
                .iter()
                .all(|t| transfer_type(program, t, shared, seen));
        }
    }
    if shared
        && !(program_v07(program)
            && program
                .enums
                .contains_key(ty.split('<').next().unwrap_or(ty)))
        && !program
            .structs
            .get(ty.split('<').next().unwrap_or(ty))
            .is_some_and(|s| s.immutable)
    {
        return false;
    }
    let base = ty.split('<').next().unwrap_or(ty);
    if !seen.insert(ty.into()) {
        return false;
    }
    let result = if let Some(s) = program.structs.get(base) {
        let args = ty
            .split_once('<')
            .map(|(_, a)| split_type_args(outer_type_end(a)))
            .unwrap_or_default();
        let substitutions = s
            .type_params
            .iter()
            .cloned()
            .zip(args.into_iter().map(str::to_string))
            .collect();
        s.fields
            .iter()
            .all(|(_, t)| transfer_type(program, &substitute_type(t, &substitutions), shared, seen))
    } else if let Some(e) = program.enums.get(base) {
        let args = ty
            .split_once('<')
            .map(|(_, a)| split_type_args(outer_type_end(a)))
            .unwrap_or_default();
        let substitutions = e
            .type_params
            .iter()
            .cloned()
            .zip(args.into_iter().map(str::to_string))
            .collect();
        e.variants
            .values()
            .flatten()
            .all(|(_, t)| transfer_type(program, &substitute_type(t, &substitutions), shared, seen))
    } else if matches!(base, "List" | "Map" | "Option" | "Result") {
        ty.split_once('<').is_some_and(|(_, args)| {
            split_type_args(outer_type_end(args))
                .iter()
                .all(|t| transfer_type(program, t, false, seen))
        })
    } else {
        false
    };
    seen.remove(ty);
    result
}
pub(super) fn transfer_bounded(
    program: &Program,
    ty: &str,
    shared: bool,
    bounds: &BTreeMap<String, String>,
) -> bool {
    let substitutions = bounds
        .iter()
        .filter(|(_, b)| {
            b.split('+')
                .any(|b| b == "Share" || (!shared && b == "Send"))
        })
        .map(|(n, _)| (n.clone(), "Int".into()))
        .collect();
    transfer_type(
        program,
        &substitute_type(ty, &substitutions),
        shared,
        &mut BTreeSet::new(),
    )
}

pub(super) fn expressions(body: &[Stmt], visit: &mut impl FnMut(&Expr)) {
    fn expr(e: &Expr, visit: &mut impl FnMut(&Expr)) {
        visit(e);
        match &e.kind {
            ExprKind::Unary(_, v) | ExprKind::Member(v, _) | ExprKind::Try(v) => expr(v, visit),
            ExprKind::Binary(_, a, b) => {
                expr(a, visit);
                expr(b, visit);
            }
            ExprKind::Call(t, args) => {
                expr(t, visit);
                for a in args {
                    expr(a, visit);
                }
            }
            ExprKind::Closure(_, _, b) => expressions(b, visit),
            ExprKind::Match(v, arms) => {
                expr(v, visit);
                for (_, g, a) in arms {
                    if let Some(g) = g {
                        expr(g, visit);
                    }
                    expr(a, visit);
                }
            }
            ExprKind::NamedConstructor(_, fields) => {
                for (_, v) in fields {
                    expr(v, visit);
                }
            }
            _ => {}
        }
    }
    for s in body {
        match &s.kind {
            StmtKind::Let(_, _, _, e)
            | StmtKind::Using(_, e)
            | StmtKind::Expr(e)
            | StmtKind::Defer(e)
            | StmtKind::Return(Some(e)) => expr(e, visit),
            StmtKind::Assign(a, _, b) => {
                expr(a, visit);
                expr(b, visit);
            }
            StmtKind::External(_, b) | StmtKind::Block(b) | StmtKind::Branch(_, b) => {
                expressions(b, visit)
            }
            StmtKind::If(e, a, b) => {
                expr(e, visit);
                expressions(a, visit);
                expressions(b, visit);
            }
            StmtKind::While(e, b) => {
                expr(e, visit);
                expressions(b, visit);
            }
            StmtKind::For(_, a, b, s) => {
                expr(a, visit);
                expr(b, visit);
                expressions(s, visit);
            }
            StmtKind::Match(e, arms) => {
                expr(e, visit);
                for (_, g, b) in arms {
                    if let Some(g) = g {
                        expr(g, visit);
                    }
                    expressions(std::slice::from_ref(b), visit);
                }
            }
            _ => {}
        }
    }
}
pub(super) fn names(body: &[Stmt]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    expressions(body, &mut |e| {
        if let ExprKind::Name(n) = &e.kind {
            names.insert(n.clone());
        }
    });
    names
}
pub(super) fn free_names(body: &[Stmt], params: &[(String, String)]) -> BTreeSet<String> {
    fn expr(e: &Expr, locals: &BTreeSet<String>, out: &mut BTreeSet<String>) {
        match &e.kind {
            ExprKind::Name(n) => {
                if !locals.contains(n) {
                    out.insert(n.clone());
                }
            }
            ExprKind::Unary(_, v) | ExprKind::Member(v, _) | ExprKind::Try(v) => {
                expr(v, locals, out)
            }
            ExprKind::Binary(_, a, b) => {
                expr(a, locals, out);
                expr(b, locals, out);
            }
            ExprKind::Call(t, args) => {
                expr(t, locals, out);
                for a in args {
                    expr(a, locals, out);
                }
            }
            ExprKind::Closure(params, _, body) => {
                let mut local = locals.clone();
                local.extend(params.iter().map(|(n, _)| n.clone()));
                stmts(body, &mut local, out);
            }
            ExprKind::Match(v, arms) => {
                expr(v, locals, out);
                for (p, g, a) in arms {
                    let mut local = locals.clone();
                    pattern(p, &mut local);
                    if let Some(g) = g {
                        expr(g, &local, out);
                    }
                    expr(a, &local, out);
                }
            }
            ExprKind::NamedConstructor(_, fields) => {
                for (_, v) in fields {
                    expr(v, locals, out);
                }
            }
            _ => {}
        }
    }
    fn pattern(p: &Pattern, locals: &mut BTreeSet<String>) {
        match p {
            Pattern::Bind(n) => {
                locals.insert(n.clone());
            }
            Pattern::Variant(_, p) | Pattern::List(p) => {
                for p in p {
                    pattern(p, locals);
                }
            }
            Pattern::Struct(_, fields) => {
                for (_, p) in fields {
                    pattern(p, locals);
                }
            }
            _ => {}
        }
    }
    fn stmts(body: &[Stmt], locals: &mut BTreeSet<String>, out: &mut BTreeSet<String>) {
        for s in body {
            match &s.kind {
                StmtKind::Let(n, _, _, e) | StmtKind::Using(n, e) => {
                    expr(e, locals, out);
                    locals.insert(n.clone());
                }
                StmtKind::Assign(a, _, b) => {
                    expr(a, locals, out);
                    expr(b, locals, out);
                }
                StmtKind::Expr(e) | StmtKind::Defer(e) | StmtKind::Return(Some(e)) => {
                    expr(e, locals, out)
                }
                StmtKind::External(_, b) | StmtKind::Block(b) | StmtKind::Branch(_, b) => {
                    stmts(b, &mut locals.clone(), out)
                }
                StmtKind::If(e, a, b) => {
                    expr(e, locals, out);
                    stmts(a, &mut locals.clone(), out);
                    stmts(b, &mut locals.clone(), out);
                }
                StmtKind::While(e, b) => {
                    expr(e, locals, out);
                    stmts(b, &mut locals.clone(), out);
                }
                StmtKind::For(n, a, b, body) => {
                    expr(a, locals, out);
                    expr(b, locals, out);
                    let mut local = locals.clone();
                    local.insert(n.clone());
                    stmts(body, &mut local, out);
                }
                StmtKind::Match(v, arms) => {
                    expr(v, locals, out);
                    for (p, g, b) in arms {
                        let mut local = locals.clone();
                        pattern(p, &mut local);
                        if let Some(g) = g {
                            expr(g, &local, out);
                        }
                        stmts(std::slice::from_ref(b), &mut local, out);
                    }
                }
                _ => {}
            }
        }
    }
    let mut names = BTreeSet::new();
    let mut locals = params.iter().map(|(n, _)| n.clone()).collect();
    stmts(body, &mut locals, &mut names);
    names
}
pub(super) fn needed_globals(program: &Program, name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut pending = vec![name.split('<').next().unwrap_or(name).to_string()];
    let mut seen = BTreeSet::new();
    while let Some(n) = pending.pop() {
        let n = resolve_alias(program, &n);
        if !seen.insert(n.clone()) {
            continue;
        }
        if let Some(f) = program.functions.get(&n) {
            let free = free_names(&f.body, &f.params);
            for n in &free {
                if program.functions.contains_key(&resolve_alias(program, n)) {
                    pending.push(n.clone());
                }
            }
            expressions(&f.body, &mut |e| {
                if let ExprKind::Member(v, m) = &e.kind {
                    if let ExprKind::Name(n) = &v.kind {
                        if let Some(symbol) = program.import_aliases.get(&format!("{n}.{m}")) {
                            pending.push(symbol.clone());
                        }
                    }
                }
            });
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
                    | "1.7.0"
                    | "1.8.0"
                    | "1.9.0"
                    | "2.0.0"
            ) {
                let checker = Checker {
                    program,
                    scopes: vec![BTreeMap::new()],
                    return_ty: None,
                    loop_depth: 0,
                    bounds: f
                        .type_params
                        .iter()
                        .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                        .collect(),
                    origin: f.origin.clone(),
                };
                pending.extend(v06::captures::method_dependencies(
                    &checker, &f.params, &f.body,
                ));
            }
            names.extend(free);
        }
    }
    names
}
