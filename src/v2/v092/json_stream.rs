use super::*;
use rewind::json_stream::{self as js, Event, Reader};
pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if !language_at_least(&p.language, "1.8.8") {
        return Ok(());
    }
    if p.structs.contains_key("JsonStreamState")
        || p.enums.contains_key("JsonStreamState")
        || p.aliases.contains_key("JsonStreamState")
    {
        return Err(Error::InvalidOperation("reserved JSON stream type".into()));
    }
    p.structs.insert(
        "JsonStreamState".into(),
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
    if p.structs.contains_key("JsonStreamEvent")
        || p.enums.contains_key("JsonStreamEvent")
        || p.aliases.contains_key("JsonStreamEvent")
    {
        return Err(Error::InvalidOperation(
            "reserved JSON stream event type".into(),
        ));
    }
    let mut variants = BTreeMap::new();
    for name in ["ObjectStart", "ObjectEnd", "ArrayStart", "ArrayEnd", "Null"] {
        variants.insert(name.into(), vec![("0".into(), "Int".into())]);
    }
    for name in ["Key", "Text", "Number"] {
        variants.insert(
            name.into(),
            vec![("0".into(), "String".into()), ("1".into(), "Int".into())],
        );
    }
    variants.insert(
        "Bool".into(),
        vec![("0".into(), "Bool".into()), ("1".into(), "Int".into())],
    );
    p.enums.insert(
        "JsonStreamEvent".into(),
        EnumDef {
            type_params: vec![],
            public: true,
            origin: p.root_origin.clone(),
            variants,
        },
    );
    Ok(())
}
fn signature(n: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match n {
        "stdJsonStreamNew" => (&[], "Result<JsonStreamState,StdError>"),
        "stdJsonStreamFeed" => (
            &["&JsonStreamState", "Bytes", "Bool"],
            "Result<JsonStreamState,StdError>",
        ),
        "stdJsonStreamPeek" => (
            &["&JsonStreamState"],
            "Result<Option<JsonStreamEvent>,StdError>",
        ),
        "stdJsonStreamAdvance" | "stdJsonStreamCancel" => {
            (&["&JsonStreamState"], "Result<JsonStreamState,StdError>")
        }
        "stdJsonStreamPosition" => (&["&JsonStreamState"], "Result<Int,StdError>"),
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
    if !language_at_least(&p.language, "1.8.8") {
        return Err(diagnostic(at, "JSON stream requires language 1.8.8"));
    }
    if args.len() != params.len() || params.iter().zip(args).any(|(p, a)| !compatible(p, a)) {
        return Err(diagnostic(
            at,
            format!("{n} requires ({})", params.join(",")),
        ));
    }
    Ok(Some(ret.into()))
}
fn reader<'a>(v: &'a Value, rt: &'a Runtime) -> Option<&'a Reader> {
    let v = match v {
        Value::HeapRef(id) | Value::CellRef(id) => rt.heap_get(*id).unwrap_or(v),
        _ => v,
    };
    match v {
        Value::JsonStream(v) => Some(v),
        _ => None,
    }
}
pub(super) fn work(n: &str, args: &[Value], rt: &Runtime) -> Option<usize> {
    signature(n).map(|_| {
        let Some(r) = args.first().and_then(|v| reader(v, rt)) else {
            return 256;
        };
        match n {
            "stdJsonStreamFeed" => {
                let b = match args.get(1) {
                    Some(Value::Bytes(b)) => b.as_slice(),
                    _ => &[],
                };
                r.feed_work(b, matches!(args.get(2), Some(Value::Bool(true))))
            }
            "stdJsonStreamPeek" => r.peek().map_or(64, |row| row.bytes()),
            "stdJsonStreamPosition" => 128,
            _ => r.clone_work(),
        }
    })
}
fn ok(v: Value) -> Value {
    Value::Result(Ok(Box::new(v)))
}
fn error(code: &str, pos: u64) -> Value {
    Value::Result(Err(Box::new(err(code, pos.min(i64::MAX as u64) as usize))))
}
pub(super) fn call(n: &str, args: &[Value], rt: &mut Runtime) -> Result<Option<Value>> {
    if signature(n).is_none() {
        return Ok(None);
    }
    if n == "stdJsonStreamNew" {
        rt.check_native_allocation(8192)?;
        return Ok(Some(ok(Value::JsonStream(Reader::new()))));
    }
    let Some(r) = reader(&args[0], rt).cloned() else {
        return Ok(Some(error("JsonType", 0)));
    };
    let out = match n {
        "stdJsonStreamFeed" => {
            let (Some(Value::Bytes(b)), Some(Value::Bool(finish))) = (args.get(1), args.get(2))
            else {
                return Ok(Some(error("JsonType", r.position())));
            };
            if b.len() > js::CHUNK {
                return Ok(Some(error("JsonChunkLimit", r.position())));
            }
            rt.check_native_allocation(r.feed_scratch(b.len()))?;
            match r.feed(b, *finish) {
                Ok(r) => ok(Value::JsonStream(r)),
                Err(e) => error(e.code, e.position),
            }
        }
        "stdJsonStreamPeek" => {
            let bytes = r.peek().map_or(0, Event::bytes);
            rt.check_native_allocation(bytes.saturating_mul(3).saturating_add(8192))?;
            ok(Value::Option(r.peek().map(|e| Box::new(event(e)))))
        }
        "stdJsonStreamAdvance" => {
            rt.check_native_allocation(r.clone_work().saturating_mul(4) + 8192)?;
            ok(Value::JsonStream(r.advance()))
        }
        "stdJsonStreamCancel" => {
            rt.check_native_allocation(r.clone_work().saturating_mul(4) + 8192)?;
            ok(Value::JsonStream(r.cancel()))
        }
        "stdJsonStreamPosition" => match i64::try_from(r.position()) {
            Ok(pos) => ok(Value::Int(pos)),
            Err(_) => error("JsonPositionLimit", r.position()),
        },
        _ => unreachable!(),
    };
    Ok(Some(out))
}

fn event(e: &Event) -> Value {
    let pos = Value::Int(e.position() as i64);
    let (name, values) = match e {
        Event::ObjectStart(_) => ("ObjectStart", vec![pos]),
        Event::ObjectEnd(_) => ("ObjectEnd", vec![pos]),
        Event::ArrayStart(_) => ("ArrayStart", vec![pos]),
        Event::ArrayEnd(_) => ("ArrayEnd", vec![pos]),
        Event::Null(_) => ("Null", vec![pos]),
        Event::Key(s, _) => ("Key", vec![Value::Text(s.clone()), pos]),
        Event::Text(s, _) => ("Text", vec![Value::Text(s.clone()), pos]),
        Event::Number(s, _) => ("Number", vec![Value::Text(s.clone()), pos]),
        Event::Bool(b, _) => ("Bool", vec![Value::Bool(*b), pos]),
    };
    Value::Enum(
        "JsonStreamEvent".into(),
        name.into(),
        values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
    )
}
