use super::*;
use rewind::bigint::{Error as BigError, IntegerValue, MAX_BITS};
pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if !language_at_least(&p.language, "1.8.2") {
        return Ok(());
    }
    if p.structs.contains_key("BigInt")
        || p.enums.contains_key("BigInt")
        || p.aliases.contains_key("BigInt")
    {
        return Err(Error::InvalidOperation("reserved BigInt type".into()));
    }
    p.structs.insert(
        "BigInt".into(),
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
        "stdBigIntParse" => (&["String", "Int"], "Result<BigInt,StdError>"),
        "stdBigIntFromInt" => (&["Int"], "Result<BigInt,StdError>"),
        "stdBigIntToInt" => (&["&BigInt"], "Result<Int,StdError>"),
        "stdBigIntFormat" => (&["&BigInt", "Int"], "Result<String,StdError>"),
        "stdBigIntBinary" => (&["String", "&BigInt", "&BigInt"], "Result<BigInt,StdError>"),
        "stdBigIntUnary" => (&["String", "&BigInt"], "Result<BigInt,StdError>"),
        "stdBigIntShift" => (&["&BigInt", "Bool", "Int"], "Result<BigInt,StdError>"),
        "stdBigIntPow" => (&["&BigInt", "Int"], "Result<BigInt,StdError>"),
        "stdBigIntModPow" => (
            &["&BigInt", "&BigInt", "&BigInt"],
            "Result<BigInt,StdError>",
        ),
        "stdBigIntCompare" => (&["&BigInt", "&BigInt"], "Result<Int,StdError>"),
        "stdBigIntBits" => (&["&BigInt"], "Result<Int,StdError>"),
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
    if !language_at_least(&p.language, "1.8.2") {
        return Err(diagnostic(at, "BigInt requires language 1.8.2"));
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
fn number<'a>(v: &'a Value, rt: &'a Runtime) -> std::result::Result<&'a IntegerValue, BigError> {
    match target(v, rt) {
        Value::BigInt(v) => Ok(v),
        _ => Err(BigError::Domain),
    }
}
fn int(v: &Value) -> std::result::Result<i64, BigError> {
    match v {
        Value::Int(v) => Ok(*v),
        _ => Err(BigError::Domain),
    }
}
fn text(v: &Value) -> std::result::Result<&str, BigError> {
    match v {
        Value::Text(v) => Ok(v),
        _ => Err(BigError::Domain),
    }
}
fn positive(v: &Value) -> std::result::Result<u64, BigError> {
    u64::try_from(int(v)?).map_err(|_| BigError::Domain)
}
pub(super) fn work(n: &str, args: &[Value], rt: &Runtime) -> Option<usize> {
    signature(n)?;
    let bits = |i| {
        args.get(i)
            .and_then(|v| number(v, rt).ok())
            .map_or(64, IntegerValue::bits) as usize
    };
    let words = |i| bits(i).div_ceil(32).max(1);
    let input = args.first().and_then(|v| text(v).ok()).map_or(1, str::len);
    Some(
        match n {
            "stdBigIntParse" => input
                .saturating_mul(input)
                .div_ceil(16)
                .saturating_add(input * 16),
            "stdBigIntBinary" => {
                let op = args.first().and_then(|v| text(v).ok()).unwrap_or("");
                if matches!(op, "add" | "sub" | "and" | "or" | "xor") {
                    bits(1).saturating_add(bits(2)).saturating_mul(8)
                } else {
                    words(1)
                        .saturating_mul(words(2))
                        .saturating_mul(64)
                        .saturating_add(bits(1) + bits(2))
                }
            }
            "stdBigIntFormat" => words(0)
                .saturating_mul(words(0))
                .saturating_mul(32)
                .saturating_add(bits(0)),
            "stdBigIntPow" => {
                let e = args
                    .get(1)
                    .and_then(|v| positive(v).ok())
                    .unwrap_or(0)
                    .min(MAX_BITS) as usize;
                if bits(0) <= 1 {
                    return Some(bits(0) + 64);
                }
                let predicted = bits(0)
                    .saturating_mul(e)
                    .min(MAX_BITS as usize)
                    .div_ceil(32)
                    .max(1);
                predicted
                    .saturating_mul(predicted)
                    .saturating_mul(64)
                    .saturating_mul((usize::BITS - e.leading_zeros()).max(1) as usize)
            }
            "stdBigIntModPow" => words(2)
                .saturating_mul(words(2))
                .saturating_mul(bits(1).max(1))
                .saturating_mul(128)
                .saturating_add(bits(0)),
            "stdBigIntShift" => {
                let amount = args
                    .get(2)
                    .and_then(|v| positive(v).ok())
                    .unwrap_or(0)
                    .min(MAX_BITS) as usize;
                if matches!(args.get(1), Some(Value::Bool(true))) {
                    bits(0).saturating_add(amount)
                } else {
                    bits(0)
                }
            }
            _ => bits(0).saturating_add(bits(1)).saturating_add(64),
        }
        .max(1),
    )
}
pub(super) fn call(n: &str, args: &[Value], rt: &mut Runtime) -> Result<Option<Value>> {
    if signature(n).is_none() {
        return Ok(None);
    }
    let existing = args
        .iter()
        .map(|v| number(v, rt).map_or(0, IntegerValue::retained_bytes))
        .sum::<usize>();
    let bits = |i| {
        args.get(i)
            .and_then(|v| number(v, rt).ok())
            .map_or(64, IntegerValue::bits) as usize
    };
    let output_bits = match n {
        "stdBigIntParse" => args
            .first()
            .and_then(|v| text(v).ok())
            .map_or(1, str::len)
            .saturating_mul(6)
            .min(MAX_BITS as usize),
        "stdBigIntShift" if matches!(args.get(1), Some(Value::Bool(true))) => bits(0)
            .saturating_add(
                args.get(2)
                    .and_then(|v| positive(v).ok())
                    .unwrap_or(0)
                    .min(MAX_BITS) as usize,
            )
            .min(MAX_BITS as usize),
        "stdBigIntPow" if bits(0) > 1 => bits(0)
            .saturating_mul(
                args.get(1)
                    .and_then(|v| positive(v).ok())
                    .unwrap_or(0)
                    .min(MAX_BITS) as usize,
            )
            .min(MAX_BITS as usize),
        "stdBigIntModPow" => bits(2),
        _ => args
            .iter()
            .map(|v| number(v, rt).map_or(0, IntegerValue::bits) as usize)
            .sum::<usize>()
            .saturating_add(64),
    };
    // Includes native temporaries, digest generation and the worst-case base-2 output.
    rt.check_native_allocation(
        existing
            .saturating_mul(16)
            .saturating_add(output_bits.saturating_mul(4) + 4096),
    )?;
    let result = (|| -> std::result::Result<Value, BigError> {
        let a = |i| number(&args[i], rt);
        let integer = |v: std::result::Result<IntegerValue, BigError>| v.map(Value::BigInt);
        Ok(match n {
            "stdBigIntParse" => integer(IntegerValue::parse(
                text(&args[0])?,
                u32::try_from(int(&args[1])?).map_err(|_| BigError::Radix)?,
            ))?,
            "stdBigIntFromInt" => Value::BigInt(IntegerValue::from_int(int(&args[0])?)),
            "stdBigIntToInt" => Value::Int(a(0)?.to_int()?),
            "stdBigIntFormat" => Value::Text(
                a(0)?.format(u32::try_from(int(&args[1])?).map_err(|_| BigError::Radix)?)?,
            ),
            "stdBigIntBinary" => integer(a(1)?.binary(text(&args[0])?, a(2)?))?,
            "stdBigIntUnary" => integer(a(1)?.unary(text(&args[0])?))?,
            "stdBigIntShift" => {
                let Value::Bool(left) = args[1] else {
                    return Err(BigError::Domain);
                };
                integer(a(0)?.shift(left, positive(&args[2])?))?
            }
            "stdBigIntPow" => {
                integer(a(0)?.pow(u32::try_from(positive(&args[1])?).map_err(|_| BigError::Size)?))?
            }
            "stdBigIntModPow" => integer(a(0)?.mod_pow(a(1)?, a(2)?))?,
            "stdBigIntCompare" => Value::Int(match a(0)?.cmp(a(1)?) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }),
            "stdBigIntBits" => Value::Int(a(0)?.bits() as i64),
            _ => return Err(BigError::Domain),
        })
    })();
    Ok(Some(Value::Result(
        result
            .map(Box::new)
            .map_err(|e| Box::new(err(&format!("BigInt{e:?}"), 0))),
    )))
}
