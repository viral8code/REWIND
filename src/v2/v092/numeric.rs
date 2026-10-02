use super::*;
use rewind::numeric::{Array, DType, Error as NumericError};
pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if !language_at_least(&p.language, "1.8.0") {
        return Ok(());
    }
    for name in ["FloatArray", "IntArray"] {
        if p.structs.contains_key(name)
            || p.enums.contains_key(name)
            || p.aliases.contains_key(name)
        {
            return Err(Error::InvalidOperation("reserved numeric type".into()));
        }
        p.structs.insert(
            name.into(),
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
pub(super) fn call_type(p: &Program, n: &str, args: &[String], at: &Tok) -> Result<Option<String>> {
    if !n.starts_with("stdNumeric") {
        return Ok(None);
    }
    if !language_at_least(&p.language, "1.8.0") {
        return Err(diagnostic(at, "numeric primitives require language 1.8.0"));
    }
    let Some((params, ret)) = signature(n) else {
        return Ok(None);
    };
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
fn array<'a>(v: &'a Value, rt: &'a Runtime) -> std::result::Result<&'a Array, NumericError> {
    match target(v, rt) {
        Value::NumericArray(a) => Ok(a),
        _ => Err(NumericError::Type),
    }
}
fn list<'a>(
    v: &'a Value,
    rt: &'a Runtime,
) -> std::result::Result<&'a rewind::storage::PagedValues, NumericError> {
    match target(v, rt) {
        Value::TypedList(_, a) => Ok(a),
        _ => Err(NumericError::Type),
    }
}
fn indices(v: &Value, rt: &Runtime) -> std::result::Result<Vec<usize>, NumericError> {
    let values = list(v, rt)?;
    if values.len() > rewind::numeric::MAX_RANK {
        return Err(NumericError::Shape);
    }
    values
        .iter()
        .map(|v| match v {
            Value::Int(n) => usize::try_from(*n).map_err(|_| NumericError::Index),
            _ => Err(NumericError::Type),
        })
        .collect()
}
fn integer(v: &Value) -> std::result::Result<i64, NumericError> {
    match v {
        Value::Int(n) => Ok(*n),
        _ => Err(NumericError::Type),
    }
}
fn usize_arg(v: &Value) -> std::result::Result<usize, NumericError> {
    usize::try_from(integer(v)?).map_err(|_| NumericError::Index)
}
fn floating(v: &Value) -> std::result::Result<f64, NumericError> {
    match v {
        Value::Float(n) => Ok(f64::from_bits(*n)),
        _ => Err(NumericError::Type),
    }
}
fn operation(v: &Value) -> std::result::Result<&str, NumericError> {
    match v {
        Value::Text(n) => Ok(n),
        _ => Err(NumericError::Type),
    }
}
fn elements(shape: &[usize]) -> std::result::Result<usize, NumericError> {
    if shape.len() > rewind::numeric::MAX_RANK
        || shape.iter().any(|&n| n > rewind::numeric::MAX_ELEMENTS)
    {
        return Err(NumericError::Size);
    }
    shape.iter().try_fold(1usize, |n, &d| {
        n.checked_mul(d)
            .filter(|&n| n <= rewind::numeric::MAX_ELEMENTS)
            .ok_or(NumericError::Size)
    })
}
pub(super) fn work(n: &str, args: &[Value], rt: &Runtime) -> Option<usize> {
    signature(n)?;
    let name = n.strip_prefix("stdNumeric")?;
    let a = |i| args.get(i).and_then(|v| array(v, rt).ok());
    let length = |i| a(i).map_or(1, Array::len);
    let cost = if name.starts_with("Zeros") || name.starts_with("From") {
        args.first()
            .and_then(|s| indices(s, rt).ok())
            .and_then(|s| elements(&s).ok())
            .unwrap_or(1)
            .saturating_mul(2)
    } else if name == "Matmul" {
        match (a(0).map(Array::shape), a(1).map(Array::shape)) {
            (Some([m, k]), Some([l, n])) if k == l && elements(&[*m, *n]).is_ok() => m
                .saturating_mul(*n)
                .saturating_mul(*k)
                .saturating_mul(8)
                .saturating_add(m.saturating_mul(*n).saturating_mul(2)),
            _ => 1,
        }
    } else if name == "Solve" {
        match (a(0).map(Array::shape), a(1).map(Array::shape)) {
            (Some([n, m]), Some([right])) if n == m && n == right => n
                .saturating_pow(3)
                .saturating_mul(8)
                .saturating_add(n.saturating_mul(2)),
            _ => 1,
        }
    } else if name.starts_with("With") {
        2048
    } else if name.starts_with("Get")
        || name.starts_with("Shape")
        || name.starts_with("Strides")
        || name.starts_with("Reshape")
        || name.starts_with("Transpose")
        || name.starts_with("Slice")
        || name.starts_with("Broadcast")
        || name == "Math"
    {
        64
    } else if name.starts_with("Values") {
        length(0).saturating_mul(128)
    } else if name.starts_with("Zip") {
        length(1).saturating_mul(24)
    } else if name == "MapFloat" {
        length(1).saturating_mul(24)
    } else {
        length(0).saturating_mul(24)
    };
    Some(cost.max(1))
}
fn scratch(name: &str, args: &[Value], rt: &Runtime) -> usize {
    let a = |i| args.get(i).and_then(|v| array(v, rt).ok());
    let length = |i| a(i).map_or(0, Array::len);
    if name.starts_with("Zeros") || name.starts_with("From") {
        args.first()
            .and_then(|s| indices(s, rt).ok())
            .and_then(|s| elements(&s).ok())
            .map_or(0, Array::storage_estimate)
    } else if name == "Matmul" {
        let count = match (a(0).map(Array::shape), a(1).map(Array::shape)) {
            (Some([m, k]), Some([l, n])) if k == l && elements(&[*m, *n]).is_ok() => {
                m.saturating_mul(*n)
            }
            _ => 0,
        };
        Array::storage_estimate(count).saturating_add(count.saturating_mul(8))
    } else if name == "Solve" {
        match (a(0).map(Array::shape), a(1).map(Array::shape)) {
            (Some([n, m]), Some([right])) if n == m && n == right => {
                Array::storage_estimate(length(1))
                    .saturating_add(length(0).saturating_mul(8))
                    .saturating_add(length(1).saturating_mul(32))
            }
            _ => 0,
        }
    } else if name.starts_with("With") {
        a(0).map_or(0, Array::update_estimate)
    } else if name.starts_with("Values") {
        length(0).saturating_mul(256)
    } else if name.starts_with("Zip") || name == "MapFloat" {
        Array::storage_estimate(length(1)).saturating_add(length(1).saturating_mul(8))
    } else if name.starts_with("Materialize") {
        Array::storage_estimate(length(0))
    } else {
        2048
    }
}
fn math(op: &str, v: f64) -> std::result::Result<f64, NumericError> {
    if !v.is_finite() {
        return Err(NumericError::NonFinite);
    }
    let answer = match op {
        "sqrt" => {
            if v < 0.0 {
                return Err(NumericError::Domain);
            }
            v.sqrt()
        }
        "exp" => v.exp(),
        "expM1" => v.exp_m1(),
        "log" => {
            if v <= 0.0 {
                return Err(NumericError::Domain);
            }
            v.ln()
        }
        "log1p" => {
            if v <= -1.0 {
                return Err(NumericError::Domain);
            }
            v.ln_1p()
        }
        "sin" => v.sin(),
        "cos" => v.cos(),
        "tan" => v.tan(),
        "asin" => {
            if v.abs() > 1.0 {
                return Err(NumericError::Domain);
            }
            v.asin()
        }
        "acos" => {
            if v.abs() > 1.0 {
                return Err(NumericError::Domain);
            }
            v.acos()
        }
        "atan" => v.atan(),
        "sinh" => v.sinh(),
        "cosh" => v.cosh(),
        "tanh" => v.tanh(),
        "abs" => v.abs(),
        "floor" => v.floor(),
        "ceil" => v.ceil(),
        "trunc" => v.trunc(),
        "round" => v.round(),
        "roundEven" => v.round_ties_even(),
        _ => return Err(NumericError::Domain),
    };
    if answer.is_finite() {
        Ok(answer)
    } else {
        Err(NumericError::NonFinite)
    }
}
pub(super) fn call(n: &str, args: &[Value], rt: &mut Runtime) -> Result<Option<Value>> {
    if signature(n).is_none() {
        return Ok(None);
    }
    let Some(name) = n.strip_prefix("stdNumeric") else {
        return Ok(None);
    };
    // All scratch and output allocation has a preflight against the complete VM budget.
    rt.check_native_allocation(scratch(name, args, rt))?;
    let result = (|| -> std::result::Result<Value, NumericError> {
        let a = |i| {
            args.get(i)
                .ok_or(NumericError::Type)
                .and_then(|v| array(v, rt))
        };
        let i = |i| args.get(i).ok_or(NumericError::Type).and_then(usize_arg);
        let f = |i| args.get(i).ok_or(NumericError::Type).and_then(floating);
        let ids = |i| {
            args.get(i)
                .ok_or(NumericError::Type)
                .and_then(|v| indices(v, rt))
        };
        let array_value = |a: std::result::Result<Array, NumericError>| a.map(Value::NumericArray);
        let list_int = |iter: Vec<i64>| {
            Value::TypedList("Int".into(), iter.into_iter().map(Value::Int).collect())
        };
        Ok(match name {
            "ZerosFloat" | "ZerosInt" => Value::NumericArray(Array::zeros(
                if name == "ZerosFloat" {
                    DType::Float64
                } else {
                    DType::Int64
                },
                ids(0)?,
            )?),
            "FromFloat" | "FromInt" => {
                let shape = ids(0)?;
                let values = list(&args[1], rt)?;
                let dtype = if name == "FromFloat" {
                    DType::Float64
                } else {
                    DType::Int64
                };
                if values.iter().any(|v| {
                    !matches!(
                        (dtype, v),
                        (DType::Float64, Value::Float(_)) | (DType::Int64, Value::Int(_))
                    )
                }) {
                    return Err(NumericError::Type);
                }
                Value::NumericArray(Array::from_bits(
                    dtype,
                    shape,
                    values.iter().map(|v| match v {
                        Value::Float(b) => *b,
                        Value::Int(n) => *n as u64,
                        _ => unreachable!(),
                    }),
                )?)
            }
            "ShapeFloat" | "ShapeInt" => {
                list_int(a(0)?.shape().iter().map(|&n| n as i64).collect())
            }
            "StridesFloat" | "StridesInt" => {
                list_int(a(0)?.strides().iter().map(|&n| n as i64).collect())
            }
            "GetFloat" => Value::Float(a(0)?.float(&ids(1)?)?.to_bits()),
            "GetInt" => Value::Int(a(0)?.integer(&ids(1)?)?),
            "WithFloat" | "WithInt" => {
                let mut array = a(0)?.clone();
                if name == "WithFloat" {
                    array.set_float(&ids(1)?, f(2)?)?;
                } else {
                    array.set_integer(&ids(1)?, integer(&args[2])?)?;
                }
                Value::NumericArray(array)
            }
            "ReshapeFloat" | "ReshapeInt" => array_value(a(0)?.reshape(ids(1)?))?,
            "TransposeFloat" | "TransposeInt" => array_value(a(0)?.transpose(&ids(1)?))?,
            "BroadcastFloat" | "BroadcastInt" => array_value(a(0)?.broadcast(ids(1)?))?,
            "SliceFloat" | "SliceInt" => array_value(a(0)?.slice(
                i(1)?,
                i(2)?,
                i(3)?,
                isize::try_from(integer(&args[4])?).map_err(|_| NumericError::Index)?,
            ))?,
            "MaterializeFloat" | "MaterializeInt" => array_value(a(0)?.materialize())?,
            "ValuesFloat" => {
                let array = a(0)?;
                if array.dtype() != DType::Float64 {
                    return Err(NumericError::Type);
                }
                Value::TypedList("Float".into(), array.bits().map(Value::Float).collect())
            }
            "ValuesInt" => {
                let array = a(0)?;
                if array.dtype() != DType::Int64 {
                    return Err(NumericError::Type);
                }
                Value::TypedList(
                    "Int".into(),
                    array.bits().map(|n| Value::Int(n as i64)).collect(),
                )
            }
            "Sum" => Value::Float(a(0)?.sum()?.to_bits()),
            "Mean" => Value::Float(a(0)?.mean_variance(0)?.0.to_bits()),
            "Variance" => Value::Float(a(0)?.mean_variance(i(1)?)?.1.to_bits()),
            "Dot" => Value::Float(a(0)?.dot(a(1)?)?.to_bits()),
            "Matmul" => array_value(a(0)?.matmul(a(1)?))?,
            "Solve" => array_value(a(0)?.solve(a(1)?, f(2)?))?,
            "Math" => Value::Float(math(operation(&args[0])?, f(1)?)?.to_bits()),
            "MapFloat" => {
                let op = operation(&args[0])?;
                // Validate the operation even for empty arrays.
                if !matches!(
                    op,
                    "sqrt"
                        | "exp"
                        | "expM1"
                        | "log"
                        | "log1p"
                        | "sin"
                        | "cos"
                        | "tan"
                        | "asin"
                        | "acos"
                        | "atan"
                        | "sinh"
                        | "cosh"
                        | "tanh"
                        | "abs"
                        | "floor"
                        | "ceil"
                        | "trunc"
                        | "round"
                        | "roundEven"
                ) {
                    return Err(NumericError::Domain);
                }
                array_value(a(1)?.map_float(|v| math(op, v)))?
            }
            "ZipFloat" => {
                let op = operation(&args[0])?;
                if !matches!(op, "add" | "sub" | "mul" | "div") {
                    return Err(NumericError::Domain);
                }
                array_value(a(1)?.zip_float(a(2)?, |a, b| {
                    if !a.is_finite() || !b.is_finite() {
                        return Err(NumericError::NonFinite);
                    }
                    Ok(match op {
                        "add" => a + b,
                        "sub" => a - b,
                        "mul" => a * b,
                        "div" => {
                            if b == 0.0 {
                                return Err(NumericError::Domain);
                            }
                            a / b
                        }
                        _ => return Err(NumericError::Domain),
                    })
                }))?
            }
            "ZipInt" => {
                let op = operation(&args[0])?;
                if !matches!(op, "add" | "sub" | "mul" | "div" | "rem") {
                    return Err(NumericError::Domain);
                }
                array_value(a(1)?.zip_integer(a(2)?, |a, b| match op {
                    "add" => a.checked_add(b),
                    "sub" => a.checked_sub(b),
                    "mul" => a.checked_mul(b),
                    "div" => a.checked_div(b),
                    "rem" => a.checked_rem(b),
                    _ => None,
                }))?
            }
            _ => return Err(NumericError::Type),
        })
    })();
    Ok(Some(Value::Result(result.map(Box::new).map_err(|error| {
        Box::new(err(&format!("Numeric{error:?}"), 0))
    }))))
}

fn signature(n: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match n {
        "stdNumericZerosFloat" => (&["&List<Int>"], "Result<FloatArray,StdError>"),
        "stdNumericFromFloat" => (
            &["&List<Int>", "&List<Float>"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericShapeFloat" => (&["&FloatArray"], "Result<List<Int>,StdError>"),
        "stdNumericStridesFloat" => (&["&FloatArray"], "Result<List<Int>,StdError>"),
        "stdNumericGetFloat" => (&["&FloatArray", "&List<Int>"], "Result<Float,StdError>"),
        "stdNumericWithFloat" => (
            &["&FloatArray", "&List<Int>", "Float"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericReshapeFloat" => (
            &["&FloatArray", "&List<Int>"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericTransposeFloat" => (
            &["&FloatArray", "&List<Int>"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericBroadcastFloat" => (
            &["&FloatArray", "&List<Int>"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericSliceFloat" => (
            &["&FloatArray", "Int", "Int", "Int", "Int"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericMaterializeFloat" => (&["&FloatArray"], "Result<FloatArray,StdError>"),
        "stdNumericValuesFloat" => (&["&FloatArray"], "Result<List<Float>,StdError>"),
        "stdNumericZipFloat" => (
            &["String", "&FloatArray", "&FloatArray"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericZerosInt" => (&["&List<Int>"], "Result<IntArray,StdError>"),
        "stdNumericFromInt" => (&["&List<Int>", "&List<Int>"], "Result<IntArray,StdError>"),
        "stdNumericShapeInt" => (&["&IntArray"], "Result<List<Int>,StdError>"),
        "stdNumericStridesInt" => (&["&IntArray"], "Result<List<Int>,StdError>"),
        "stdNumericGetInt" => (&["&IntArray", "&List<Int>"], "Result<Int,StdError>"),
        "stdNumericWithInt" => (
            &["&IntArray", "&List<Int>", "Int"],
            "Result<IntArray,StdError>",
        ),
        "stdNumericReshapeInt" => (&["&IntArray", "&List<Int>"], "Result<IntArray,StdError>"),
        "stdNumericTransposeInt" => (&["&IntArray", "&List<Int>"], "Result<IntArray,StdError>"),
        "stdNumericBroadcastInt" => (&["&IntArray", "&List<Int>"], "Result<IntArray,StdError>"),
        "stdNumericSliceInt" => (
            &["&IntArray", "Int", "Int", "Int", "Int"],
            "Result<IntArray,StdError>",
        ),
        "stdNumericMaterializeInt" => (&["&IntArray"], "Result<IntArray,StdError>"),
        "stdNumericValuesInt" => (&["&IntArray"], "Result<List<Int>,StdError>"),
        "stdNumericZipInt" => (
            &["String", "&IntArray", "&IntArray"],
            "Result<IntArray,StdError>",
        ),
        "stdNumericMapFloat" => (&["String", "&FloatArray"], "Result<FloatArray,StdError>"),
        "stdNumericSum" => (&["&FloatArray"], "Result<Float,StdError>"),
        "stdNumericMean" => (&["&FloatArray"], "Result<Float,StdError>"),
        "stdNumericVariance" => (&["&FloatArray", "Int"], "Result<Float,StdError>"),
        "stdNumericDot" => (&["&FloatArray", "&FloatArray"], "Result<Float,StdError>"),
        "stdNumericMatmul" => (
            &["&FloatArray", "&FloatArray"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericSolve" => (
            &["&FloatArray", "&FloatArray", "Float"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericMath" => (&["String", "Float"], "Result<Float,StdError>"),
        _ => return None,
    })
}
pub(super) fn parameter(n: &str, index: usize) -> Option<&'static str> {
    signature(n)
        .and_then(|(p, _)| p.get(index).copied())
        .filter(|p| p.starts_with('&'))
}
