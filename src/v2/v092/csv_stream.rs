use super::*;
use rewind::csv_stream::{self as csv, Reader};
pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if !language_at_least(&p.language, "1.8.7") {
        return Ok(());
    }
    if p.structs.contains_key("CsvStreamState")
        || p.enums.contains_key("CsvStreamState")
        || p.aliases.contains_key("CsvStreamState")
    {
        return Err(Error::InvalidOperation("reserved CSV stream type".into()));
    }
    p.structs.insert(
        "CsvStreamState".into(),
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
    Ok(())
}
fn signature(n: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match n {
        "stdCsvStreamNew" => (&[], "Result<CsvStreamState,StdError>"),
        "stdCsvStreamFeed" => (
            &["&CsvStreamState", "Bytes", "Bool"],
            "Result<CsvStreamState,StdError>",
        ),
        "stdCsvStreamPeek" => (
            &["&CsvStreamState"],
            "Result<Option<List<String>>,StdError>",
        ),
        "stdCsvStreamAdvance" | "stdCsvStreamCancel" => {
            (&["&CsvStreamState"], "Result<CsvStreamState,StdError>")
        }
        "stdCsvStreamPosition" => (&["&CsvStreamState"], "Result<Int,StdError>"),
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
    if !language_at_least(&p.language, "1.8.7") {
        return Err(diagnostic(at, "CSV stream requires language 1.8.7"));
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
        Value::CsvStream(v) => Some(v),
        _ => None,
    }
}
pub(super) fn work(n: &str, args: &[Value], rt: &Runtime) -> Option<usize> {
    signature(n).map(|_| {
        let Some(r) = args.first().and_then(|v| reader(v, rt)) else {
            return 256;
        };
        match n {
            "stdCsvStreamFeed" => {
                let b = match args.get(1) {
                    Some(Value::Bytes(b)) => b.as_slice(),
                    _ => &[],
                };
                r.feed_work(b.len()).saturating_add(
                    if b.iter().any(|b| matches!(b, b',' | b'\n' | b'\r'))
                        || matches!(args.get(2), Some(Value::Bool(true)))
                    {
                        r.completion_work()
                    } else {
                        0
                    },
                )
            }
            "stdCsvStreamPeek" => r
                .peek()
                .map_or(64, |row| row.iter().map(|s| s.len() + 64).sum()),
            "stdCsvStreamPosition" => 128,
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
    if n == "stdCsvStreamNew" {
        rt.check_native_allocation(8192)?;
        return Ok(Some(ok(Value::CsvStream(Reader::new()))));
    }
    let Some(r) = reader(&args[0], rt).cloned() else {
        return Ok(Some(error("CsvType", 0)));
    };
    let out = match n {
        "stdCsvStreamFeed" => {
            let (Some(Value::Bytes(b)), Some(Value::Bool(finish))) = (args.get(1), args.get(2))
            else {
                return Ok(Some(error("CsvType", r.position())));
            };
            if b.len() > csv::CHUNK {
                return Ok(Some(error("CsvChunkLimit", r.position())));
            }
            rt.check_native_allocation(r.feed_scratch(b.len()))?;
            match r.feed(b, *finish) {
                Ok(r) => ok(Value::CsvStream(r)),
                Err(e) => error(e.code, e.position),
            }
        }
        "stdCsvStreamPeek" => {
            let bytes = r
                .peek()
                .map_or(0, |row| row.iter().map(|s| s.len() + 256).sum());
            rt.check_native_allocation(bytes.saturating_mul(3).saturating_add(8192))?;
            ok(Value::Option(r.peek().map(|row| {
                Box::new(Value::TypedList(
                    "String".into(),
                    row.iter()
                        .cloned()
                        .map(|text| Value::Text(text.into()))
                        .collect::<Vec<_>>()
                        .into(),
                ))
            })))
        }
        "stdCsvStreamAdvance" => {
            rt.check_native_allocation(r.clone_work().saturating_mul(4) + 8192)?;
            ok(Value::CsvStream(r.advance()))
        }
        "stdCsvStreamCancel" => {
            rt.check_native_allocation(r.clone_work().saturating_mul(4) + 8192)?;
            ok(Value::CsvStream(r.cancel()))
        }
        "stdCsvStreamPosition" => match i64::try_from(r.position()) {
            Ok(pos) => ok(Value::Int(pos)),
            Err(_) => error("CsvPositionLimit", r.position()),
        },
        _ => unreachable!(),
    };
    Ok(Some(out))
}
