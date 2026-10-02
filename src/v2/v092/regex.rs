use super::*;
use rewind::regular::{self as re, Error as RegexError, Pattern};
pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if !language_at_least(&p.language, "1.8.6") {
        return Ok(());
    }
    if p.structs.contains_key("Regex")
        || p.enums.contains_key("Regex")
        || p.aliases.contains_key("Regex")
    {
        return Err(Error::InvalidOperation("reserved regex type".into()));
    }
    p.structs.insert(
        "Regex".into(),
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
        "stdRegexCompile" => (
            &["String", "Bool", "Bool", "Bool", "Bool"],
            "Result<Regex,StdError>",
        ),
        "stdRegexFindText" => (
            &["&Regex", "&String", "Int"],
            "Result<Option<List<Int>>,StdError>",
        ),
        "stdRegexFindBytes" => (
            &["&Regex", "Bytes", "Int"],
            "Result<Option<List<Int>>,StdError>",
        ),
        "stdRegexNames" => (&["&Regex"], "Result<List<Option<String>>,StdError>"),
        "stdRegexOptions" => (&["&Regex"], "Result<List<Bool>,StdError>"),
        "stdRegexIsText" => (&["&Regex"], "Result<Bool,StdError>"),
        "stdRegexSource" => (&["&Regex"], "Result<String,StdError>"),
        "stdRegexTextSlice" => (&["&String", "Int", "Int"], "Result<String,StdError>"),
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
    if !language_at_least(&p.language, "1.8.6") {
        return Err(diagnostic(at, "regex requires language 1.8.6"));
    }
    if args.len() != params.len() || params.iter().zip(args).any(|(p, a)| !compatible(p, a)) {
        return Err(diagnostic(
            at,
            format!("{n} requires ({})", params.join(",")),
        ));
    }
    Ok(Some(ret.into()))
}
fn target<'a>(v: &'a Value, rt: &'a Runtime) -> &'a Value {
    match v {
        Value::HeapRef(id) | Value::CellRef(id) => rt.heap_get(*id).unwrap_or(v),
        _ => v,
    }
}
fn pattern<'a>(v: &'a Value, rt: &'a Runtime) -> std::result::Result<&'a Pattern, RegexError> {
    match target(v, rt) {
        Value::Regex(v) => Ok(v),
        _ => Err(RegexError::Type),
    }
}
fn text<'a>(v: &'a Value, rt: &'a Runtime) -> std::result::Result<&'a str, RegexError> {
    match target(v, rt) {
        Value::Text(v) => Ok(v),
        _ => Err(RegexError::Type),
    }
}
fn index(v: &Value) -> std::result::Result<usize, RegexError> {
    match v {
        Value::Int(v) => usize::try_from(*v).map_err(|_| RegexError::Index),
        _ => Err(RegexError::Type),
    }
}
fn flag(v: &Value) -> std::result::Result<bool, RegexError> {
    match v {
        Value::Bool(v) => Ok(*v),
        _ => Err(RegexError::Type),
    }
}
fn length(v: &Value, rt: &Runtime) -> usize {
    match target(v, rt) {
        Value::Text(v) => v.len(),
        Value::Bytes(v) => v.len(),
        _ => 0,
    }
}
fn compile_work(len: usize) -> usize {
    50_000usize.saturating_add(len.saturating_mul(4096))
}
fn search_work(pattern: &Pattern, len: usize) -> usize {
    pattern
        .states()
        .saturating_mul(len.saturating_add(1))
        .saturating_mul(8)
        .saturating_add(256)
}
pub(super) fn work(n: &str, args: &[Value], rt: &Runtime) -> Option<usize> {
    signature(n).map(|_| {
        if n == "stdRegexCompile" {
            return compile_work(args.first().map_or(0, |v| length(v, rt)));
        }
        if n == "stdRegexTextSlice" {
            return args
                .first()
                .map_or(64, |v| length(v, rt).saturating_add(64));
        }
        let Some(p) = args.first().and_then(|v| pattern(v, rt).ok()) else {
            return 64;
        };
        if !p.is_compiled() && !matches!(n, "stdRegexSource" | "stdRegexOptions" | "stdRegexIsText")
        {
            return compile_work(p.source().len());
        }
        if n.starts_with("stdRegexFind") {
            search_work(p, args.get(1).map_or(0, |v| length(v, rt)))
        } else {
            256 + p.source().len()
        }
    })
}
fn result(v: std::result::Result<Value, RegexError>) -> Value {
    Value::Result(
        v.map(Box::new)
            .map_err(|e| Box::new(err(&format!("Regex{e:?}"), 0))),
    )
}
pub(super) fn call(n: &str, args: &[Value], rt: &mut Runtime) -> Result<Option<Value>> {
    if signature(n).is_none() {
        return Ok(None);
    }
    if n == "stdRegexCompile" {
        let Some(Value::Text(source)) = args.first() else {
            return Ok(Some(result(Err(RegexError::Type))));
        };
        if source.len() > re::MAX_PATTERN {
            return Ok(Some(result(Err(RegexError::PatternLimit))));
        }
        rt.check_native_allocation(re::COMPILE_SCRATCH)?;
        return Ok(Some(result((|| {
            Ok(Value::Regex(Pattern::compile(
                source,
                flag(&args[1])?,
                flag(&args[2])?,
                flag(&args[3])?,
                flag(&args[4])?,
            )?))
        })())));
    }
    if n == "stdRegexTextSlice" {
        let size = (|| {
            let input = text(&args[0], rt)?;
            if input.len() > re::MAX_INPUT {
                return Err(RegexError::InputLimit);
            }
            let start = index(&args[1])?;
            let end = index(&args[2])?;
            Ok(input.get(start..end).ok_or(RegexError::Index)?.len())
        })();
        let size = match size {
            Ok(size) => size,
            Err(error) => return Ok(Some(result(Err(error)))),
        };
        rt.check_native_allocation(size.saturating_mul(2).saturating_add(256))?;
        let value = (|| {
            let input = text(&args[0], rt)?;
            Ok(Value::Text(
                input
                    .get(index(&args[1])?..index(&args[2])?)
                    .ok_or(RegexError::Index)?
                    .to_owned(),
            ))
        })();
        return Ok(Some(result(value)));
    }
    let p = match pattern(&args[0], rt) {
        Ok(v) => v.clone(),
        Err(e) => return Ok(Some(result(Err(e)))),
    };
    if n == "stdRegexIsText" {
        return Ok(Some(result(Ok(Value::Bool(p.is_text())))));
    }
    if n == "stdRegexOptions" {
        rt.check_native_allocation(4096)?;
        return Ok(Some(result(Ok(Value::TypedList(
            "Bool".into(),
            p.flags()
                .into_iter()
                .map(Value::Bool)
                .collect::<Vec<_>>()
                .into(),
        )))));
    }
    if n == "stdRegexSource" {
        rt.check_native_allocation(p.source().len() + 256)?;
        return Ok(Some(result(Ok(Value::Text(p.source().into())))));
    }
    let cold = !p.is_compiled();
    if cold {
        rt.check_native_allocation(re::COMPILE_SCRATCH)?;
    }
    if let Err(error) = p.ensure() {
        return Ok(Some(result(Err(error))));
    }
    if n == "stdRegexNames" {
        rt.check_native_allocation(16 * 1024)?;
        return Ok(Some(result(p.names().map(|names| {
            Value::TypedList(
                "Option<String>".into(),
                names
                    .into_iter()
                    .map(|name| Value::Option(name.map(|name| Box::new(Value::Text(name)))))
                    .collect::<Vec<_>>()
                    .into(),
            )
        }))));
    }
    let len = length(&args[1], rt);
    if len > re::MAX_INPUT {
        return Ok(Some(result(Err(RegexError::InputLimit))));
    }
    if cold {
        rt.charge_native_work(search_work(&p, len))?;
    }
    rt.check_native_allocation(p.search_scratch())?;
    let input = target(&args[1], rt);
    let found = (|| {
        let start = index(&args[2])?;
        match (n, input) {
            ("stdRegexFindText", Value::Text(value)) => p.find(value.as_bytes(), start, true),
            ("stdRegexFindBytes", Value::Bytes(value)) => p.find(value, start, false),
            _ => Err(RegexError::Type),
        }
    })();
    Ok(Some(result(found.map(|found| {
        Value::Option(found.map(|values| {
            Box::new(Value::TypedList(
                "Int".into(),
                values
                    .into_iter()
                    .map(Value::Int)
                    .collect::<Vec<_>>()
                    .into(),
            ))
        }))
    }))))
}
