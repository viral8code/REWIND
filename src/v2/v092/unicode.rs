use super::*;
use rewind::unicode::{self as uni, Error as UnicodeError};
fn signature(n: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match n {
        "stdUnicodeNormalize" => (&["&String", "String"], "Result<String,StdError>"),
        "stdUnicodeIsNormalized" => (&["&String", "String"], "Result<Bool,StdError>"),
        "stdUnicodeCount" => (&["&String"], "Result<Int,StdError>"),
        "stdUnicodeSlice" => (&["&String", "Int", "Int"], "Result<String,StdError>"),
        "stdUnicodeSplit" => (&["&String", "String"], "Result<List<String>,StdError>"),
        "stdUnicodeOffsets" => (&["&String"], "Result<List<Int>,StdError>"),
        "stdUnicodeCase" => (&["&String", "String"], "Result<String,StdError>"),
        "stdUnicodeVersions" => (&[], "Result<String,StdError>"),
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
    if !language_at_least(&p.language, "1.8.5") {
        return Err(diagnostic(at, "Unicode API requires language 1.8.5"));
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
fn text<'a>(v: &'a Value, rt: &'a Runtime) -> std::result::Result<&'a str, UnicodeError> {
    match target(v, rt) {
        Value::Text(v) => Ok(v),
        _ => Err(UnicodeError::Operation),
    }
}
pub(super) fn work(n: &str, args: &[Value], rt: &Runtime) -> Option<usize> {
    signature(n).map(|_| {
        let len = args
            .first()
            .and_then(|v| text(v, rt).ok())
            .map_or(0, str::len);
        let factor = if matches!(n, "stdUnicodeNormalize" | "stdUnicodeIsNormalized") {
            (usize::BITS - len.max(1).leading_zeros()) as usize * 16
        } else {
            16
        };
        len.saturating_mul(factor).saturating_add(64)
    })
}
pub(super) fn call(n: &str, args: &[Value], rt: &mut Runtime) -> Result<Option<Value>> {
    if signature(n).is_none() {
        return Ok(None);
    }
    let len = args
        .first()
        .and_then(|v| text(v, rt).ok())
        .map_or(0, str::len);
    if len > uni::MAX_INPUT {
        return Ok(Some(Value::Result(Err(Box::new(err(
            "UnicodeInputLimit",
            0,
        ))))));
    }
    let scratch = match n {
        "stdUnicodeNormalize" | "stdUnicodeIsNormalized" => len.saturating_mul(64).saturating_add(
            len.saturating_mul(24)
                .min(uni::MAX_OUTPUT)
                .saturating_mul(2),
        ),
        "stdUnicodeSplit" => len
            .min(uni::MAX_ITEMS)
            .saturating_mul(384)
            .saturating_add(len.saturating_mul(8)),
        "stdUnicodeOffsets" => len.min(uni::MAX_ITEMS).saturating_mul(256),
        "stdUnicodeCount" => 128,
        _ => len.saturating_mul(8),
    };
    rt.check_native_allocation(scratch.saturating_add(4096))?;
    let result = (|| -> std::result::Result<Value, UnicodeError> {
        let t = |i: usize| text(&args[i], rt);
        let index = |i: usize| match &args[i] {
            Value::Int(v) => usize::try_from(*v).map_err(|_| UnicodeError::Index),
            _ => Err(UnicodeError::Index),
        };
        Ok(match n {
            "stdUnicodeNormalize" => Value::Text(uni::normalize(t(0)?, t(1)?)?),
            "stdUnicodeIsNormalized" => Value::Bool(uni::is_normalized(t(0)?, t(1)?)?),
            "stdUnicodeCount" => Value::Int(uni::grapheme_count(t(0)?)? as i64),
            "stdUnicodeSlice" => Value::Text(uni::grapheme_slice(t(0)?, index(1)?, index(2)?)?),
            "stdUnicodeSplit" => Value::TypedList(
                "String".into(),
                uni::split(t(0)?, t(1)?)?
                    .into_iter()
                    .map(Value::Text)
                    .collect::<Vec<_>>()
                    .into(),
            ),
            "stdUnicodeOffsets" => Value::TypedList(
                "Int".into(),
                uni::grapheme_offsets(t(0)?)?
                    .into_iter()
                    .map(Value::Int)
                    .collect::<Vec<_>>()
                    .into(),
            ),
            "stdUnicodeCase" => Value::Text(uni::case(t(0)?, t(1)?)?),
            "stdUnicodeVersions" => Value::Text(uni::versions()),
            _ => return Err(UnicodeError::Operation),
        })
    })();
    Ok(Some(Value::Result(
        result
            .map(Box::new)
            .map_err(|e| Box::new(err(&format!("Unicode{e:?}"), 0))),
    )))
}
