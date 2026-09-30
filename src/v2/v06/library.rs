use super::*;
pub(super) fn method_type(
    _checker: &Checker<'_>,
    ty: &str,
    m: &str,
    args: &[String],
    at: &Tok,
) -> Result<Option<String>> {
    let result = match (ty, m, args) {
        ("String", "byteLen" | "charLen" | "utf16Len", []) | ("Bytes", "byteLen", []) => "Int",
        ("String", "encodeUtf8", []) => "Bytes",
        ("Bytes", "decodeUtf8", []) => "Result<String,String>",
        ("String", "contains" | "startsWith" | "endsWith", [t]) if t == "String" => "Bool",
        ("String", "find", [t]) if t == "String" => "Option<Int>",
        ("String", "split", [t]) if t == "String" => "List<String>",
        ("String", "slice", [a, b]) if a == "Int" && b == "Int" => "Result<String,String>",
        ("Bytes", "slice", [a, b]) if a == "Int" && b == "Int" => "Result<Bytes,String>",
        ("String", "parseInt", []) => "Result<Int,String>",
        ("String", "parseFloat", []) => "Result<Float,String>",
        ("Int" | "Float", "format", []) => "String",
        ("Float", "toIntChecked", []) => "Result<Int,String>",
        ("Int", "toFloatChecked", []) => "Result<Float,String>",
        ("Float", "isFinite" | "isNaN", []) => "Bool",
        _ => {
            if let Some(item) = ty.strip_prefix("List<").and_then(|t| t.strip_suffix('>')) {
                match (m, args) {
                    ("add" | "push", [t]) if compatible(item, t) => return Ok(Some("Unit".into())),
                    ("set", [index, t]) if index == "Int" && compatible(item, t) => {
                        return Ok(Some("Unit".into()))
                    }
                    _ => {}
                }
            }
            let item = if ty.starts_with("Iterator<") {
                v05::iterator_item(ty)
            } else {
                trait_method_return(_checker.program, ty, "next", &[], at)?.and_then(|t| {
                    t.strip_prefix("Option<")
                        .and_then(|t| t.strip_suffix('>'))
                        .map(str::to_string)
                })
            };
            if let Some(item) = item {
                match (m, args) {
                    ("collect", []) => return Ok(Some(format!("List<{item}>"))),
                    ("enumerate", []) => return Ok(Some(format!("Iterator<Tuple<Int,{item}>>"))),
                    ("zip", [other]) => {
                        let right = if other.starts_with("Iterator<") {
                            v05::iterator_item(other)
                        } else {
                            trait_method_return(_checker.program, other, "next", &[], at)?.and_then(
                                |t| {
                                    t.strip_prefix("Option<")
                                        .and_then(|t| t.strip_suffix('>'))
                                        .map(str::to_string)
                                },
                            )
                        };
                        let Some(right) = right else {
                            return Err(diagnostic(at, "zip requires an Iterator"));
                        };
                        return Ok(Some(format!("Iterator<Tuple<{item},{right}>>")));
                    }
                    ("take", [n]) if n == "Int" => return Ok(Some(format!("Iterator<{item}>"))),
                    ("map" | "filter", [f]) | ("fold", [_, f]) => {
                        let Some((params, ret)) = function_signature(f) else {
                            return Err(diagnostic(at, "iterator adapter requires a function"));
                        };
                        let expected = if m == "fold" {
                            vec![args[0].clone(), item.clone()]
                        } else {
                            vec![item.clone()]
                        };
                        if params.len() != expected.len()
                            || !params.iter().zip(expected).all(|(p, e)| compatible(p, &e))
                        {
                            return Err(diagnostic(at, "iterator callback parameter mismatch"));
                        }
                        if ret.starts_with("Task<")
                            || type_effects(f).contains("tasks")
                            || type_effects(f).contains("publish")
                        {
                            return Err(diagnostic(at, "adapter callback must be synchronous"));
                        }
                        return Ok(Some(match m {
                            "filter" => {
                                if ret != "Bool" {
                                    return Err(diagnostic(at, "filter callback must return Bool"));
                                }
                                format!("Iterator<{item}>")
                            }
                            "fold" => {
                                if !compatible(&args[0], &ret) {
                                    return Err(diagnostic(at, "fold accumulator mismatch"));
                                }
                                ret
                            }
                            _ => format!("Iterator<{ret}>"),
                        }));
                    }
                    _ => {}
                }
            }
            if let Some(inner) = ty.strip_prefix("Result<").and_then(|t| t.strip_suffix('>')) {
                let parts = split_type_args(inner);
                if parts.len() != 2 {
                    return Ok(None);
                }
                match (m, args) {
                    ("flatten", []) if parts[0].starts_with("Result<") => {
                        let nested = split_type_args(outer_type_end(
                            parts[0].strip_prefix("Result<").unwrap(),
                        ));
                        if nested.len() == 2 && compatible(parts[1], nested[1]) {
                            return Ok(Some(parts[0].into()));
                        }
                        return Err(diagnostic(at, "flatten requires the same error type"));
                    }
                    ("map" | "mapErr" | "andThen", [f]) => {
                        let Some((params, ret)) = function_signature(f) else {
                            return Err(diagnostic(at, "Result adapter requires a function"));
                        };
                        let expected = if m == "mapErr" { parts[1] } else { parts[0] };
                        if params.len() != 1 || !compatible(&params[0], expected) {
                            return Err(diagnostic(at, "Result callback parameter mismatch"));
                        }
                        if ret.starts_with("Task<")
                            || type_effects(f).contains("tasks")
                            || type_effects(f).contains("publish")
                        {
                            return Err(diagnostic(at, "adapter callback must be synchronous"));
                        }
                        return Ok(Some(if m == "map" {
                            format!("Result<{ret},{}>", parts[1])
                        } else if m == "mapErr" {
                            format!("Result<{},{}>", parts[0], ret)
                        } else {
                            let nested = ret
                                .strip_prefix("Result<")
                                .map(|t| split_type_args(outer_type_end(t)))
                                .unwrap_or_default();
                            if nested.len() != 2 || !compatible(parts[1], nested[1]) {
                                return Err(diagnostic(
                                    at,
                                    "andThen callback must return Result with the same error type",
                                ));
                            }
                            ret
                        }));
                    }
                    _ => {}
                }
            }
            return Ok(None);
        }
    };
    Ok(Some(result.into()))
}
pub(in crate::v2) fn primitive_method(
    target: &Value,
    method: &str,
    args: &[Value],
) -> Option<Value> {
    let success = |v| Value::Result(Ok(Box::new(v)));
    let failure = |s: &str| Value::Result(Err(Box::new(Value::Text(s.into()))));
    Some(match (target, method, args) {
        (Value::Text(s), "byteLen", []) => Value::Int(s.len() as i64),
        (Value::Text(s), "charLen", []) => Value::Int(s.chars().count() as i64),
        (Value::Text(s), "utf16Len", []) => Value::Int(s.encode_utf16().count() as i64),
        (Value::Bytes(b), "byteLen", []) => Value::Int(b.len() as i64),
        (Value::Text(s), "encodeUtf8", []) => Value::Bytes(s.as_bytes().to_vec()),
        (Value::Bytes(b), "decodeUtf8", []) => match String::from_utf8(b.clone()) {
            Ok(s) => success(Value::Text(s)),
            Err(_) => failure("InvalidUtf8"),
        },
        (Value::Text(s), "contains", [Value::Text(p)]) => Value::Bool(s.contains(p)),
        (Value::Text(s), "startsWith", [Value::Text(p)]) => Value::Bool(s.starts_with(p)),
        (Value::Text(s), "endsWith", [Value::Text(p)]) => Value::Bool(s.ends_with(p)),
        (Value::Text(s), "find", [Value::Text(p)]) => {
            Value::Option(s.find(p).map(|n| Box::new(Value::Int(n as i64))))
        }
        (Value::Text(s), "split", [Value::Text(p)]) => Value::TypedList(
            "String".into(),
            s.split(p).map(|s| Value::Text(s.into())).collect(),
        ),
        (Value::Text(s), "slice", [Value::Int(a), Value::Int(b)]) => {
            if *a >= 0 && *b >= *a {
                s.get(*a as usize..*b as usize)
                    .map(|s| success(Value::Text(s.into())))
                    .unwrap_or_else(|| failure("InvalidUtf8BoundaryOrRange"))
            } else {
                failure("InvalidRange")
            }
        }
        (Value::Bytes(s), "slice", [Value::Int(a), Value::Int(b)]) => {
            if *a >= 0 && *b >= *a {
                s.get(*a as usize..*b as usize)
                    .map(|s| success(Value::Bytes(s.to_vec())))
                    .unwrap_or_else(|| failure("InvalidRange"))
            } else {
                failure("InvalidRange")
            }
        }
        (Value::Text(s), "parseInt", []) => s
            .parse::<i64>()
            .map(|n| success(Value::Int(n)))
            .unwrap_or_else(|_| failure("InvalidIntOrOverflow")),
        (Value::Text(s), "parseFloat", []) => s
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| success(Value::Float(n.to_bits())))
            .unwrap_or_else(|| failure("InvalidFloatOrNonFinite")),
        (Value::Int(n), "format", []) => Value::Text(n.to_string()),
        (Value::Float(n), "format", []) => Value::Text(f64::from_bits(*n).to_string()),
        (Value::Float(n), "isFinite", []) => Value::Bool(f64::from_bits(*n).is_finite()),
        (Value::Float(n), "isNaN", []) => Value::Bool(f64::from_bits(*n).is_nan()),
        (Value::Float(n), "toIntChecked", []) => {
            let n = f64::from_bits(*n);
            if n.is_finite()
                && n.fract() == 0.0
                && n >= i64::MIN as f64
                && n < 9223372036854775808.0
            {
                success(Value::Int(n as i64))
            } else {
                failure("InexactOrOverflow")
            }
        }
        (Value::Int(n), "toFloatChecked", []) => {
            let f = *n as f64;
            if (f as i128) == i128::from(*n) {
                success(Value::Float(f.to_bits()))
            } else {
                failure("InexactConversion")
            }
        }
        _ => return None,
    })
}
