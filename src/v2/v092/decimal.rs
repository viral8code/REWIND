use super::*;
use rewind::decimal::{DecimalValue, Error as DecimalError, Rounding, MAX_SCALE};
pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if !language_at_least(&p.language, "1.8.3") {
        return Ok(());
    }
    for n in ["Decimal", "DecimalRounding"] {
        if p.structs.contains_key(n) || p.enums.contains_key(n) || p.aliases.contains_key(n) {
            return Err(Error::InvalidOperation("reserved decimal type".into()));
        }
    }
    p.structs.insert(
        "Decimal".into(),
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
    p.enums.insert(
        "DecimalRounding".into(),
        EnumDef {
            type_params: vec![],
            public: true,
            origin: p.root_origin.clone(),
            variants: [
                "TowardZero",
                "AwayFromZero",
                "Floor",
                "Ceiling",
                "HalfUp",
                "HalfDown",
                "HalfEven",
                "Exact",
            ]
            .into_iter()
            .map(|n| (n.into(), vec![]))
            .collect(),
        },
    );
    Ok(())
}
fn signature(n: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match n {
        "stdDecimalParse" => (&["String"], "Result<Decimal,StdError>"),
        "stdDecimalFormat" | "stdDecimalRepresentation" => {
            (&["&Decimal"], "Result<String,StdError>")
        }
        "stdDecimalFromCoefficient" => (&["&BigInt", "Int"], "Result<Decimal,StdError>"),
        "stdDecimalCoefficient" => (&["&Decimal"], "Result<BigInt,StdError>"),
        "stdDecimalScale" => (&["&Decimal"], "Result<Int,StdError>"),
        "stdDecimalCompare" => (&["&Decimal", "&Decimal"], "Result<Int,StdError>"),
        "stdDecimalBinary" => (
            &["String", "&Decimal", "&Decimal", "Int", "DecimalRounding"],
            "Result<Decimal,StdError>",
        ),
        "stdDecimalQuantize" => (
            &["&Decimal", "Int", "Int", "DecimalRounding"],
            "Result<Decimal,StdError>",
        ),
        "stdDecimalDivide" => (
            &["&Decimal", "&Decimal", "Int", "Int", "DecimalRounding"],
            "Result<Decimal,StdError>",
        ),
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
    if !language_at_least(&p.language, "1.8.3") {
        return Err(diagnostic(at, "Decimal requires language 1.8.3"));
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
fn decimal<'a>(
    v: &'a Value,
    rt: &'a Runtime,
) -> std::result::Result<&'a DecimalValue, DecimalError> {
    match target(v, rt) {
        Value::Decimal(v) => Ok(v),
        _ => Err(DecimalError::Domain),
    }
}
fn int(v: &Value) -> std::result::Result<i64, DecimalError> {
    match v {
        Value::Int(v) => Ok(*v),
        _ => Err(DecimalError::Domain),
    }
}
fn scale(v: &Value) -> std::result::Result<i32, DecimalError> {
    i32::try_from(int(v)?).map_err(|_| DecimalError::Scale)
}
fn precision(v: &Value) -> std::result::Result<usize, DecimalError> {
    usize::try_from(int(v)?).map_err(|_| DecimalError::Precision)
}
fn rounding(v: &Value, rt: &Runtime) -> std::result::Result<Rounding, DecimalError> {
    let Value::Enum(ty, variant, fields) = target(v, rt) else {
        return Err(DecimalError::Domain);
    };
    if ty != "DecimalRounding" || !fields.is_empty() {
        return Err(DecimalError::Domain);
    }
    Ok(match variant.as_str() {
        "TowardZero" => Rounding::TowardZero,
        "AwayFromZero" => Rounding::AwayFromZero,
        "Floor" => Rounding::Floor,
        "Ceiling" => Rounding::Ceiling,
        "HalfUp" => Rounding::HalfUp,
        "HalfDown" => Rounding::HalfDown,
        "HalfEven" => Rounding::HalfEven,
        "Exact" => Rounding::Exact,
        _ => return Err(DecimalError::Domain),
    })
}
fn dimensions(n: &str, args: &[Value], rt: &Runtime) -> (usize, usize) {
    let input = args
        .iter()
        .map(|v| match target(v, rt) {
            Value::Text(s) => s.len(),
            Value::Decimal(v) => v.digits(),
            Value::BigInt(v) => v.bits() as usize,
            _ => 0,
        })
        .sum::<usize>();
    let scales = args
        .iter()
        .filter_map(|v| decimal(v, rt).ok())
        .map(DecimalValue::scale)
        .collect::<Vec<_>>();
    let spread = match n {
        "stdDecimalBinary" => scales
            .iter()
            .max()
            .zip(scales.iter().min())
            .map_or(0, |(a, b)| (a - b).unsigned_abs() as usize),
        "stdDecimalQuantize" => scales.first().map_or(0, |a| {
            (args
                .get(1)
                .and_then(|v| scale(v).ok())
                .unwrap_or(*a)
                .clamp(-MAX_SCALE, MAX_SCALE)
                - a)
                .unsigned_abs() as usize
        }),
        "stdDecimalDivide" => scales.first().zip(scales.get(1)).map_or(0, |(a, b)| {
            (b + args
                .get(2)
                .and_then(|v| scale(v).ok())
                .unwrap_or(0)
                .clamp(-MAX_SCALE, MAX_SCALE)
                - a)
                .unsigned_abs() as usize
        }),
        _ => 0,
    };
    (input, spread)
}
pub(super) fn work(n: &str, args: &[Value], rt: &Runtime) -> Option<usize> {
    signature(n)?;
    let (input, spread) = dimensions(n, args, rt);
    Some(
        match n {
            "stdDecimalScale" | "stdDecimalCoefficient" => 64,
            "stdDecimalFormat" | "stdDecimalRepresentation" => args
                .first()
                .and_then(|v| decimal(v, rt).ok())
                .map_or(64, DecimalValue::formatted_len),
            "stdDecimalCompare" => input.saturating_mul(8),
            "stdDecimalParse" | "stdDecimalFromCoefficient" => input
                .saturating_mul(input)
                .saturating_div(8)
                .saturating_add(input * 32),
            _ => {
                let digits = input.saturating_add(spread);
                digits
                    .saturating_mul(digits)
                    .saturating_mul(8)
                    .saturating_add(256)
            }
        }
        .max(1),
    )
}
pub(super) fn call(n: &str, args: &[Value], rt: &mut Runtime) -> Result<Option<Value>> {
    if signature(n).is_none() {
        return Ok(None);
    }
    let input = args
        .iter()
        .map(|v| match target(v, rt) {
            Value::Decimal(v) => v.retained_bytes(),
            Value::BigInt(v) => v.retained_bytes(),
            Value::Text(s) => s.len(),
            _ => 0,
        })
        .sum::<usize>();
    let scratch = if matches!(
        n,
        "stdDecimalScale" | "stdDecimalCoefficient" | "stdDecimalCompare"
    ) {
        128
    } else if matches!(n, "stdDecimalFormat" | "stdDecimalRepresentation") {
        args.first()
            .and_then(|v| decimal(v, rt).ok())
            .map_or(128, DecimalValue::formatted_len)
            + 128
    } else {
        input
            .saturating_mul(32)
            .saturating_add(dimensions(n, args, rt).1.saturating_mul(64) + 4096)
    };
    rt.check_native_allocation(scratch)?;
    let result = (|| -> std::result::Result<Value, DecimalError> {
        let d = |i| decimal(&args[i], rt);
        let value = |v: std::result::Result<DecimalValue, DecimalError>| {
            v.map(|v| Value::Decimal(v.into()))
        };
        Ok(match n {
            "stdDecimalParse" => {
                let Value::Text(s) = &args[0] else {
                    return Err(DecimalError::Domain);
                };
                value(DecimalValue::parse(s))?
            }
            "stdDecimalFormat" => Value::Text(d(0)?.to_string().into()),
            "stdDecimalRepresentation" => Value::Text(d(0)?.representation().into()),
            "stdDecimalFromCoefficient" => {
                let Value::BigInt(v) = target(&args[0], rt) else {
                    return Err(DecimalError::Domain);
                };
                value(DecimalValue::from_coefficient(v, scale(&args[1])?))?
            }
            "stdDecimalCoefficient" => Value::BigInt(d(0)?.coefficient()),
            "stdDecimalScale" => Value::Int(d(0)?.scale() as i64),
            "stdDecimalCompare" => Value::Int(match d(0)?.cmp(d(1)?) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }),
            "stdDecimalBinary" => {
                let Value::Text(op) = &args[0] else {
                    return Err(DecimalError::Domain);
                };
                value(d(1)?.binary(op, d(2)?, precision(&args[3])?, rounding(&args[4], rt)?))?
            }
            "stdDecimalQuantize" => value(d(0)?.quantize(
                scale(&args[1])?,
                precision(&args[2])?,
                rounding(&args[3], rt)?,
            ))?,
            "stdDecimalDivide" => value(d(0)?.divide(
                d(1)?,
                scale(&args[2])?,
                precision(&args[3])?,
                rounding(&args[4], rt)?,
            ))?,
            _ => return Err(DecimalError::Domain),
        })
    })();
    Ok(Some(Value::Result(
        result
            .map(Box::new)
            .map_err(|e| Box::new(err(&format!("Decimal{e:?}"), 0))),
    )))
}
