//! Small deterministic primitives for the source standard library.
use super::*;
mod sdk;
pub(super) use sdk::{build as sdk_build, install as sdk_install, verify as sdk_verify};
const LIMIT: usize = 1024 * 1024;
const ITEMS: usize = 65_536;
pub(super) fn names() -> &'static [&'static str] {
    &[
        "stdTextTrim",
        "stdTextSplit",
        "stdTextTokens",
        "stdTextSlice",
        "stdTextFind",
        "stdTextLength",
        "stdDecode",
        "stdEncode",
        "stdBytesGet",
        "stdBytesSlice",
        "stdBytesLength",
        "stdParseInt",
        "stdParseFloat",
        "stdFormatInt",
        "stdFormatFloat",
        "stdBitAnd",
        "stdBitOr",
        "stdBitXor",
        "stdBitNot",
        "stdShiftLeft",
        "stdShiftRight",
        "stdShiftUnsigned",
        "stdCountBits",
        "stdMulMod",
    ]
}
pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if p.language != "0.9.2" {
        return Ok(());
    }
    if p.structs.contains_key("StdError")
        || p.enums.contains_key("StdError")
        || names().iter().any(|n| {
            p.functions.contains_key(*n)
                || p.structs.contains_key(*n)
                || p.enums.contains_key(*n)
                || p.aliases.contains_key(*n)
                || p.consts.contains_key(*n)
        })
    {
        return Err(Error::InvalidOperation(
            "reserved standard library primitive".into(),
        ));
    }
    p.structs.insert(
        "StdError".into(),
        StructDef {
            bounds: BTreeMap::new(),
            immutable: true,
            type_params: vec![],
            public: true,
            origin: p.root_origin.clone(),
            fields: vec![
                ("code".into(), "String".into()),
                ("offset".into(), "Int".into()),
            ],
        },
    );
    Ok(())
}
pub(super) fn call_type(p: &Program, n: &str, args: &[String], at: &Tok) -> Result<Option<String>> {
    if p.language != "0.9.2" || !names().contains(&n) {
        return Ok(None);
    }
    let (params, ret): (&[&str], &str) = match n {
        "stdTextTrim" => (&["String"], "Result<String,StdError>"),
        "stdTextSplit" => (&["String", "String"], "Result<List<String>,StdError>"),
        "stdTextTokens" => (&["String"], "Result<List<String>,StdError>"),
        "stdTextSlice" => (&["String", "Int", "Int"], "Result<String,StdError>"),
        "stdTextFind" => (&["String", "String"], "Option<Int>"),
        "stdTextLength" => (&["String"], "Int"),
        "stdDecode" => (&["Bytes"], "Result<String,StdError>"),
        "stdEncode" => (&["String"], "Result<Bytes,StdError>"),
        "stdBytesGet" => (&["Bytes", "Int"], "Result<Int,StdError>"),
        "stdBytesSlice" => (&["Bytes", "Int", "Int"], "Result<Bytes,StdError>"),
        "stdBytesLength" => (&["Bytes"], "Int"),
        "stdParseInt" => (&["String", "Int"], "Result<Int,StdError>"),
        "stdParseFloat" => (&["String"], "Result<Float,StdError>"),
        "stdFormatInt" => (&["Int", "Int"], "Result<String,StdError>"),
        "stdFormatFloat" => (&["Float"], "Result<String,StdError>"),
        "stdBitAnd" | "stdBitOr" | "stdBitXor" => (&["Int", "Int"], "Int"),
        "stdBitNot" | "stdCountBits" => (&["Int"], "Int"),
        "stdShiftLeft" | "stdShiftRight" | "stdShiftUnsigned" => {
            (&["Int", "Int"], "Result<Int,StdError>")
        }
        "stdMulMod" => (&["Int", "Int", "Int"], "Result<Int,StdError>"),
        _ => unreachable!(),
    };
    if args.len() != params.len() || params.iter().zip(args).any(|(p, a)| !compatible(p, a)) {
        return Err(diagnostic(
            at,
            format!("{n} requires ({})", params.join(",")),
        ));
    }
    Ok(Some(ret.into()))
}
fn err(code: &str, offset: usize) -> Value {
    Value::Struct(
        "StdError".into(),
        BTreeMap::from([
            ("code".into(), Value::Text(code.into())),
            ("offset".into(), Value::Int(offset as i64)),
        ]),
    )
}
fn outcome(v: std::result::Result<Value, (&str, usize)>) -> Value {
    Value::Result(v.map(Box::new).map_err(|(c, p)| Box::new(err(c, p))))
}
fn strings(
    mut iter: impl Iterator<Item = String>,
) -> std::result::Result<Value, (&'static str, usize)> {
    let mut values = Vec::new();
    let mut bytes = 0;
    for s in iter.by_ref() {
        bytes += s.len();
        if values.len() == ITEMS || bytes > LIMIT {
            return Err(("Limit", 0));
        }
        values.push(Value::Text(s));
    }
    Ok(Value::TypedList("String".into(), values))
}
pub(super) fn call(n: &str, args: &[Value]) -> Result<Option<Value>> {
    if !names().contains(&n) {
        return Ok(None);
    }
    let text = |i| {
        if let Some(Value::Text(s)) = args.get(i) {
            Ok(s.as_str())
        } else {
            Err(Error::InvalidOperation("expected String".into()))
        }
    };
    let int = |i| {
        if let Some(Value::Int(n)) = args.get(i) {
            Ok(*n)
        } else {
            Err(Error::InvalidOperation("expected Int".into()))
        }
    };
    let bytes = |i| {
        if let Some(Value::Bytes(b)) = args.get(i) {
            Ok(b.as_slice())
        } else {
            Err(Error::InvalidOperation("expected Bytes".into()))
        }
    };
    // Scalar-only results stay scalar. Bounded codecs consistently return Result.
    let value = match n {
        "stdTextLength" => Value::Int(text(0)?.chars().count() as i64),
        "stdBytesLength" => Value::Int(bytes(0)?.len() as i64),
        "stdTextFind" => {
            let s = text(0)?;
            let needle = text(1)?;
            if s.len() > LIMIT || needle.len() > LIMIT {
                return Err(Error::InvalidOperation("text search limit exceeded".into()));
            }
            Value::Option(
                s.find(needle)
                    .map(|i| Box::new(Value::Int(s[..i].chars().count() as i64))),
            )
        }
        "stdBitAnd" => Value::Int(int(0)? & int(1)?),
        "stdBitOr" => Value::Int(int(0)? | int(1)?),
        "stdBitXor" => Value::Int(int(0)? ^ int(1)?),
        "stdBitNot" => Value::Int(!int(0)?),
        "stdCountBits" => Value::Int(int(0)?.count_ones() as i64),
        _ => {
            let result = match n {
                "stdTextTrim" => {
                    let s = text(0)?;
                    if s.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        Ok(Value::Text(s.trim().into()))
                    }
                }
                "stdTextSplit" => {
                    let s = text(0)?;
                    let delim = text(1)?;
                    if delim.is_empty() {
                        Err(("EmptyDelimiter", 0))
                    } else if s.len() > LIMIT || delim.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        strings(s.split(delim).map(str::to_string))
                    }
                }
                "stdTextTokens" => {
                    let s = text(0)?;
                    if s.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        strings(s.split_ascii_whitespace().map(str::to_string))
                    }
                }
                "stdTextSlice" => {
                    let s = text(0)?;
                    let start = int(1)?;
                    let end = int(2)?;
                    if s.len() > LIMIT {
                        Err(("Limit", 0))
                    } else if start < 0 || end < start {
                        Err(("Range", 0))
                    } else {
                        let positions = s
                            .char_indices()
                            .map(|(i, _)| i)
                            .chain(std::iter::once(s.len()))
                            .collect::<Vec<_>>();
                        match (positions.get(start as usize), positions.get(end as usize)) {
                            (Some(a), Some(b)) => Ok(Value::Text(s[*a..*b].into())),
                            _ => Err(("Range", 0)),
                        }
                    }
                }
                "stdEncode" => {
                    let s = text(0)?;
                    if s.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        Ok(Value::Bytes(s.as_bytes().to_vec()))
                    }
                }
                "stdDecode" => {
                    let b = bytes(0)?;
                    if b.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        std::str::from_utf8(b)
                            .map(|s| Value::Text(s.into()))
                            .map_err(|e| ("Utf8", e.valid_up_to()))
                    }
                }
                "stdBytesGet" => {
                    let b = bytes(0)?;
                    let i = int(1)?;
                    if i < 0 {
                        Err(("Range", 0))
                    } else {
                        b.get(i as usize)
                            .map(|v| Value::Int(i64::from(*v)))
                            .ok_or(("Range", 0))
                    }
                }
                "stdBytesSlice" => {
                    let b = bytes(0)?;
                    let start = int(1)?;
                    let end = int(2)?;
                    if start < 0 || end < start || end as usize > b.len() {
                        Err(("Range", 0))
                    } else if end - start > LIMIT as i64 {
                        Err(("Limit", 0))
                    } else {
                        Ok(Value::Bytes(b[start as usize..end as usize].to_vec()))
                    }
                }
                "stdParseInt" => {
                    let s = text(0)?;
                    let radix = int(1)?;
                    if !(2..=36).contains(&radix) {
                        Err(("Radix", 0))
                    } else if s.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        i64::from_str_radix(s, radix as u32)
                            .map(Value::Int)
                            .map_err(|e| {
                                (
                                    if matches!(
                                        e.kind(),
                                        std::num::IntErrorKind::PosOverflow
                                            | std::num::IntErrorKind::NegOverflow
                                    ) {
                                        "NumberRange"
                                    } else {
                                        "InvalidNumber"
                                    },
                                    0,
                                )
                            })
                    }
                }
                "stdParseFloat" => {
                    let s = text(0)?;
                    if s.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        s.parse::<f64>()
                            .map_err(|_| ("InvalidNumber", 0))
                            .and_then(|n| {
                                if n.is_finite() {
                                    Ok(Value::Float(n.to_bits()))
                                } else {
                                    Err(("NumberRange", 0))
                                }
                            })
                    }
                }
                "stdFormatFloat" => {
                    let Some(Value::Float(bits)) = args.first() else {
                        return Err(Error::InvalidOperation("expected Float".into()));
                    };
                    let n = f64::from_bits(*bits);
                    if n.is_finite() {
                        Ok(Value::Text(n.to_string()))
                    } else {
                        Err(("NumberRange", 0))
                    }
                }
                "stdFormatInt" => {
                    let n = int(0)?;
                    let radix = int(1)?;
                    if !(2..=36).contains(&radix) {
                        Err(("Radix", 0))
                    } else {
                        let mut value = n.unsigned_abs();
                        let mut digits = Vec::new();
                        loop {
                            digits.push(
                                b"0123456789abcdefghijklmnopqrstuvwxyz"
                                    [(value % radix as u64) as usize],
                            );
                            value /= radix as u64;
                            if value == 0 {
                                break;
                            }
                        }
                        if n < 0 {
                            digits.push(b'-');
                        }
                        digits.reverse();
                        Ok(Value::Text(String::from_utf8(digits).unwrap()))
                    }
                }
                "stdShiftLeft" | "stdShiftRight" | "stdShiftUnsigned" => {
                    let a = int(0)?;
                    let shift = int(1)?;
                    if !(0..64).contains(&shift) {
                        Err(("ShiftRange", 0))
                    } else {
                        Ok(Value::Int(match n {
                            "stdShiftLeft" => a.wrapping_shl(shift as u32),
                            "stdShiftRight" => a >> shift,
                            _ => ((a as u64) >> shift) as i64,
                        }))
                    }
                }
                "stdMulMod" => {
                    let a = int(0)?;
                    let b = int(1)?;
                    let m = int(2)?;
                    if m <= 0 {
                        Err(("Modulus", 0))
                    } else {
                        Ok(Value::Int(
                            ((i128::from(a) * i128::from(b)).rem_euclid(i128::from(m))) as i64,
                        ))
                    }
                }
                _ => unreachable!(),
            };
            outcome(result)
        }
    };
    Ok(Some(value))
}
