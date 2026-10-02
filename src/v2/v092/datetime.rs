use super::*;
use rewind::datetime::{self as dt, Duration, Error as DateError, Instant};
pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if !language_at_least(&p.language, "1.8.4") {
        return Ok(());
    }
    for n in ["Instant", "Duration"] {
        if p.structs.contains_key(n) || p.enums.contains_key(n) || p.aliases.contains_key(n) {
            return Err(Error::InvalidOperation("reserved datetime type".into()));
        }
        p.structs.insert(
            n.into(),
            StructDef {
                private_fields: BTreeSet::from(["$native".into()]),
                bounds: BTreeMap::new(),
                immutable: true,
                type_params: vec![],
                public: true,
                origin: p.root_origin.clone(),
                fields: vec![],
            },
        );
    }
    Ok(())
}
fn signature(n: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match n {
        "stdDateTimeInstant" => (&["Int", "Int"], "Result<Instant,StdError>"),
        "stdDateTimeDuration" => (&["Int", "Int"], "Result<Duration,StdError>"),
        "stdDateTimeParse" => (&["String"], "Result<Instant,StdError>"),
        "stdDateTimeFormat" => (&["&Instant"], "Result<String,StdError>"),
        "stdDateTimeFormatOffset" => (&["&Instant", "Int"], "Result<String,StdError>"),
        "stdDateTimeSeconds" | "stdDateTimeNanos" => (&["&Instant"], "Result<Int,StdError>"),
        "stdDateTimeDurationSeconds" | "stdDateTimeDurationNanos" => {
            (&["&Duration"], "Result<Int,StdError>")
        }
        "stdDateTimeAdd" => (&["&Instant", "&Duration"], "Result<Instant,StdError>"),
        "stdDateTimeDifference" => (&["&Instant", "&Instant"], "Result<Duration,StdError>"),
        "stdDateTimeDurationAdd" | "stdDateTimeDurationSubtract" => {
            (&["&Duration", "&Duration"], "Result<Duration,StdError>")
        }
        "stdDateTimeDurationNegate" => (&["&Duration"], "Result<Duration,StdError>"),
        "stdDateTimeCalendar" => (&["&Instant", "String"], "Result<List<Int>,StdError>"),
        "stdDateTimeCalendarOffset" => (&["&Instant", "Int"], "Result<List<Int>,StdError>"),
        "stdDateTimeResolve" => (&["&List<Int>", "String", "Int"], "Result<Instant,StdError>"),
        "stdDateTimeResolveOffset" => (&["&List<Int>", "Int"], "Result<Instant,StdError>"),
        "stdDateTimeDatabaseVersion" => (&[], "Result<String,StdError>"),
        "stdExternalInstant" => (&[], "Result<Instant,StdError>"),
        _ => return None,
    })
}
pub(super) fn parameter(n: &str, i: usize) -> Option<&'static str> {
    signature(n)?.0.get(i).copied()
}
pub(super) fn call_type(p: &Program, n: &str, args: &[String], at: &Tok) -> Result<Option<String>> {
    let Some((params, ret)) = signature(n) else {
        return Ok(None);
    };
    if !language_at_least(&p.language, "1.8.4") {
        return Err(diagnostic(at, "datetime requires language 1.8.4"));
    }
    if args.len() != params.len() || params.iter().zip(args).any(|(p, a)| !compatible(p, a)) {
        return Err(diagnostic(
            at,
            format!("{n} requires ({})", params.join(",")),
        ));
    }
    Ok(Some(ret.into()))
}
pub(super) fn work(n: &str, args: &[Value], _: &Runtime) -> Option<usize> {
    signature(n).map(|_| {
        256usize.saturating_add(
            args.iter()
                .map(|v| if let Value::Text(s) = v { s.len() } else { 0 })
                .sum::<usize>(),
        )
    })
}
fn target<'a>(v: &'a Value, rt: &'a Runtime) -> &'a Value {
    match v {
        Value::HeapRef(id) | Value::CellRef(id) => rt.heap_get(*id).unwrap_or(v),
        _ => v,
    }
}
fn int(v: &Value) -> std::result::Result<i64, DateError> {
    match v {
        Value::Int(v) => Ok(*v),
        _ => Err(DateError::Type),
    }
}
fn text(v: &Value) -> std::result::Result<&str, DateError> {
    match v {
        Value::Text(v) => Ok(v),
        _ => Err(DateError::Type),
    }
}
fn instant(v: &Value, rt: &Runtime) -> std::result::Result<Instant, DateError> {
    match target(v, rt) {
        Value::Instant(v) => Ok(*v),
        _ => Err(DateError::Type),
    }
}
fn duration(v: &Value, rt: &Runtime) -> std::result::Result<Duration, DateError> {
    match target(v, rt) {
        Value::Duration(v) => Ok(*v),
        _ => Err(DateError::Type),
    }
}
fn calendar(v: &Value, rt: &Runtime) -> std::result::Result<Vec<i64>, DateError> {
    match target(v, rt) {
        Value::TypedList(t, v) if t == "Int" && v.len() == 7 => v.iter().map(int).collect(),
        _ => Err(DateError::Calendar),
    }
}
fn result(v: std::result::Result<Value, DateError>) -> Value {
    Value::Result(
        v.map(Box::new)
            .map_err(|e| Box::new(err(&format!("DateTime{e:?}"), 0))),
    )
}
pub(super) fn call(n: &str, args: &[Value], rt: &mut Runtime) -> Result<Option<Value>> {
    if signature(n).is_none() {
        return Ok(None);
    }
    rt.check_native_allocation(4096)?;
    if n == "stdExternalInstant" {
        let observation = rt.external_operation("clock.instant", b"", 256, || {
            let now = std::time::SystemTime::now();
            let (seconds, nanos) = match now.duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => (
                    i64::try_from(d.as_secs()).map_err(|_| "DateTimeRange".to_string())?,
                    d.subsec_nanos() as i64,
                ),
                Err(e) => {
                    let d = e.duration();
                    let seconds =
                        i64::try_from(d.as_secs()).map_err(|_| "DateTimeRange".to_string())?;
                    if d.subsec_nanos() == 0 {
                        (-seconds, 0)
                    } else {
                        (
                            seconds
                                .checked_neg()
                                .and_then(|v| v.checked_sub(1))
                                .ok_or_else(|| "DateTimeRange".to_string())?,
                            1_000_000_000 - d.subsec_nanos() as i64,
                        )
                    }
                }
            };
            Ok(serde_json::json!({"seconds":seconds,"nanos":nanos}))
        })?;
        return Ok(Some(match observation {
            Ok(v) => result(
                serde_json::from_value::<Instant>(v)
                    .map(Value::Instant)
                    .map_err(|_| DateError::Range),
            ),
            Err(_) => Value::Result(Err(Box::new(err("ExternalFailure", 0)))),
        }));
    }
    let value = (|| -> std::result::Result<Value, DateError> {
        let i = |j: usize| instant(&args[j], rt);
        let d = |j: usize| duration(&args[j], rt);
        let list = |v: Vec<i64>| {
            Value::TypedList(
                "Int".into(),
                v.into_iter().map(Value::Int).collect::<Vec<_>>().into(),
            )
        };
        Ok(match n {
            "stdDateTimeInstant" => Value::Instant(Instant::new(int(&args[0])?, int(&args[1])?)?),
            "stdDateTimeDuration" => {
                Value::Duration(Duration::new(int(&args[0])?, int(&args[1])?)?)
            }
            "stdDateTimeParse" => Value::Instant(Instant::parse(text(&args[0])?)?),
            "stdDateTimeFormat" => Value::Text(i(0)?.format()),
            "stdDateTimeFormatOffset" => Value::Text(i(0)?.format_offset(int(&args[1])?)?),
            "stdDateTimeSeconds" => Value::Int(i(0)?.seconds()),
            "stdDateTimeNanos" => Value::Int(i(0)?.nanos()),
            "stdDateTimeDurationSeconds" => Value::Int(d(0)?.seconds()),
            "stdDateTimeDurationNanos" => Value::Int(d(0)?.nanos()),
            "stdDateTimeAdd" => Value::Instant(i(0)?.add(d(1)?)?),
            "stdDateTimeDifference" => Value::Duration(i(0)?.difference(i(1)?)?),
            "stdDateTimeDurationAdd" => Value::Duration(d(0)?.add(d(1)?)?),
            "stdDateTimeDurationSubtract" => Value::Duration(d(0)?.subtract(d(1)?)?),
            "stdDateTimeDurationNegate" => Value::Duration(d(0)?.negate()?),
            "stdDateTimeCalendar" => list(i(0)?.calendar(text(&args[1])?)?),
            "stdDateTimeCalendarOffset" => list(i(0)?.calendar_offset(int(&args[1])?)?),
            "stdDateTimeResolve" => Value::Instant(dt::resolve(
                &calendar(&args[0], rt)?,
                text(&args[1])?,
                int(&args[2])?,
            )?),
            "stdDateTimeResolveOffset" => Value::Instant(dt::resolve_offset(
                &calendar(&args[0], rt)?,
                int(&args[1])?,
            )?),
            "stdDateTimeDatabaseVersion" => Value::Text(dt::database_version().into()),
            _ => return Err(DateError::Type),
        })
    })();
    Ok(Some(result(value)))
}
