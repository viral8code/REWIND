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
    if language_at_least(&p.language, "1.8.1") {
        for (name, fields) in [
            (
                "QrResult",
                vec![
                    ("q", "FloatArray"),
                    ("r", "FloatArray"),
                    ("permutation", "Frozen<List<Int>>"),
                    ("rank", "Int"),
                ],
            ),
            (
                "EigenResult",
                vec![
                    ("values", "FloatArray"),
                    ("vectors", "FloatArray"),
                    ("sweeps", "Int"),
                ],
            ),
            (
                "HistogramResult",
                vec![
                    ("counts", "IntArray"),
                    ("underflow", "Int"),
                    ("overflow", "Int"),
                ],
            ),
        ] {
            if p.structs.contains_key(name)
                || p.enums.contains_key(name)
                || p.aliases.contains_key(name)
            {
                return Err(Error::InvalidOperation(
                    "reserved numeric result type".into(),
                ));
            }
            p.structs.insert(
                name.into(),
                StructDef {
                    private_fields: BTreeSet::new(),
                    bounds: BTreeMap::new(),
                    immutable: true,
                    type_params: vec![],
                    public: true,
                    origin: p.root_origin.clone(),
                    fields: fields
                        .into_iter()
                        .map(|(n, t)| (n.into(), t.into()))
                        .collect(),
                },
            );
        }
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
    if matches!(
        n,
        "stdNumericQr"
            | "stdNumericLeastSquares"
            | "stdNumericEigenSymmetric"
            | "stdNumericCovariance"
            | "stdNumericCorrelation"
            | "stdNumericQuantile"
            | "stdNumericHistogram"
    ) && !language_at_least(&p.language, "1.8.1")
    {
        return Err(diagnostic(
            at,
            "advanced numeric primitives require language 1.8.1",
        ));
    }
    if matches!(n, "stdNumericSuffixArray" | "stdNumericSuffixSearch")
        && !language_at_least(&p.language, "1.9.8")
    {
        return Err(diagnostic(at, "suffix primitives require language 1.9.8"));
    }
    if matches!(
        n,
        "stdNumericDotStep" | "stdNumericMatmulInit" | "stdNumericMatmulStep"
    ) && !language_at_least(&p.language, "1.9.10")
    {
        return Err(diagnostic(
            at,
            "cooperative numeric primitives require language 1.9.10",
        ));
    }
    if matches!(
        n,
        "stdNumericFft"
            | "stdNumericConvolve"
            | "stdNumericCsr"
            | "stdNumericSparseMatvec"
            | "stdNumericScale"
            | "stdNumericNorm2"
    ) && !language_at_least(&p.language, "1.9.15")
    {
        return Err(diagnostic(
            at,
            "transform and sparse primitives require language 1.9.15",
        ));
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
    let cost = if name == "Norm2" {
        length(0).saturating_mul(64).saturating_add(64)
    } else if name == "Fft" {
        let n = length(0);
        if n > rewind::transforms::MAX_FFT {
            1
        } else {
            n.saturating_mul((usize::BITS - n.max(1).leading_zeros()) as usize)
                .saturating_mul(32)
                .saturating_add(4096)
        }
    } else if name == "Convolve" {
        let n = length(0)
            .saturating_add(length(1))
            .saturating_sub(1)
            .checked_next_power_of_two()
            .unwrap_or(usize::MAX);
        if n > rewind::transforms::MAX_FFT {
            1
        } else {
            n.saturating_mul((usize::BITS - n.max(1).leading_zeros()) as usize)
                .saturating_mul(96)
                .saturating_add(4096)
        }
    } else if name == "Csr" {
        let n = length(4);
        if n > rewind::transforms::MAX_SPARSE {
            1
        } else {
            n.saturating_mul((usize::BITS - n.max(1).leading_zeros()) as usize)
                .saturating_mul(32)
                .saturating_add(
                    args.first()
                        .and_then(|v| usize_arg(v).ok())
                        .unwrap_or(0)
                        .min(rewind::transforms::MAX_SPARSE)
                        .saturating_mul(8),
                )
                .saturating_add(4096)
        }
    } else if name == "SparseMatvec" {
        length(4)
            .saturating_add(length(2))
            .saturating_add(length(5))
            .saturating_mul(32)
            .saturating_add(4096)
    } else if matches!(name, "SuffixArray" | "SuffixSearch") {
        if matches!(args.first(),Some(Value::Bytes(b)) if b.len()>rewind::suffix::MAX_BYTES)
            || (name == "SuffixSearch"
                && matches!(args.get(2),Some(Value::Bytes(b)) if b.len()>rewind::suffix::MAX_BYTES))
        {
            return Some(1);
        }
        let length = match args.first() {
            Some(Value::Bytes(b)) => b.len().min(rewind::suffix::MAX_BYTES),
            _ => 0,
        };
        let log = (usize::BITS - length.max(1).leading_zeros()) as usize;
        if name == "SuffixArray" {
            length
                .saturating_mul(log)
                .saturating_mul(16)
                .saturating_add(4096)
        } else {
            match args.get(2) {
                Some(Value::Bytes(b)) => b
                    .len()
                    .min(rewind::suffix::MAX_BYTES)
                    .saturating_mul(log)
                    .saturating_mul(2)
                    .saturating_add(128),
                _ => 1,
            }
        }
    } else if matches!(name, "Qr" | "LeastSquares") {
        match a(0).map(Array::shape) {
            Some([m, n]) => m
                .saturating_mul(*n)
                .saturating_mul((*m).min(*n))
                .saturating_mul(64)
                .saturating_add(m.saturating_mul(8))
                .saturating_add(n.saturating_mul(256)),
            _ => 1,
        }
    } else if name == "EigenSymmetric" {
        match a(0).map(Array::shape) {
            Some([n, m]) if n == m => {
                let sweeps = args
                    .get(2)
                    .and_then(|v| usize_arg(v).ok())
                    .filter(|&n| n <= 10_000)
                    .unwrap_or(0);
                n.saturating_pow(3)
                    .saturating_mul(sweeps)
                    .saturating_mul(24)
                    .saturating_add(n.saturating_mul(*n).saturating_mul(16))
            }
            _ => 1,
        }
    } else if name == "Quantile" {
        length(0)
            .saturating_mul((usize::BITS - length(0).max(1).leading_zeros()) as usize)
            .saturating_mul(8)
    } else if name == "Histogram" {
        length(0)
            .saturating_mul((usize::BITS - length(1).max(1).leading_zeros()) as usize)
            .saturating_mul(8)
            .saturating_add(length(1).saturating_mul(8))
    } else if name.starts_with("Zeros") || name.starts_with("From") {
        args.first()
            .and_then(|s| indices(s, rt).ok())
            .and_then(|s| elements(&s).ok())
            .unwrap_or(1)
            .saturating_mul(2)
    } else if matches!(name, "DotStep" | "MatmulStep") {
        rewind::numeric::COOPERATIVE_MACS
            .saturating_mul(32)
            .saturating_add(1024)
    } else if name == "MatmulInit" {
        a(0).zip(a(1))
            .and_then(|(a, b)| a.matmul_work(b).ok())
            .map_or(1, |(m, _, n, _)| {
                m.saturating_mul(n).saturating_mul(2).saturating_add(1)
            })
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
    if name == "Norm2" {
        128
    } else if name == "Fft" {
        if length(0) > rewind::transforms::MAX_FFT {
            0
        } else {
            length(0).saturating_mul(64).saturating_add(8192)
        }
    } else if name == "Convolve" {
        let n = length(0)
            .saturating_add(length(1))
            .saturating_sub(1)
            .checked_next_power_of_two()
            .unwrap_or(usize::MAX);
        if n > rewind::transforms::MAX_FFT {
            0
        } else {
            n.saturating_mul(64).saturating_add(8192)
        }
    } else if name == "Csr" {
        if length(4) > rewind::transforms::MAX_SPARSE {
            0
        } else {
            length(4)
                .saturating_mul(112)
                .saturating_add(
                    args.first()
                        .and_then(|v| usize_arg(v).ok())
                        .unwrap_or(0)
                        .min(rewind::transforms::MAX_SPARSE)
                        .saturating_add(1)
                        .saturating_mul(24),
                )
                .saturating_add(8192)
        }
    } else if name == "SparseMatvec" {
        length(5)
            .saturating_mul(16)
            .saturating_add(length(2).saturating_mul(24))
            .saturating_add(8192)
    } else if name == "SuffixArray" {
        match args.first() {
            Some(Value::Bytes(b)) if b.len() <= rewind::suffix::MAX_BYTES => {
                b.len().saturating_mul(96).saturating_add(8192)
            }
            _ => 0,
        }
    } else if name == "SuffixSearch" {
        128
    } else if matches!(name, "Qr" | "LeastSquares") {
        match a(0).map(Array::shape) {
            Some([m, n]) => {
                let p = (*m).min(*n);
                m.saturating_mul(*n)
                    .saturating_mul(48)
                    .saturating_add(p.saturating_mul(*n).saturating_mul(32))
                    .saturating_add(n.saturating_mul(256))
                    .saturating_add(m.saturating_mul(8))
                    .saturating_add(4096)
            }
            _ => 0,
        }
    } else if name == "EigenSymmetric" {
        length(0).saturating_mul(48).saturating_add(4096)
    } else if name == "Quantile" {
        length(0).saturating_mul(8).saturating_add(2048)
    } else if name == "Histogram" {
        length(1).saturating_mul(32).saturating_add(4096)
    } else if name.starts_with("Zeros") || name.starts_with("From") {
        args.first()
            .and_then(|s| indices(s, rt).ok())
            .and_then(|s| elements(&s).ok())
            .map_or(0, Array::storage_estimate)
    } else if name == "DotStep" {
        256
    } else if name == "MatmulStep" {
        let cells = rewind::numeric::COOPERATIVE_MACS;
        a(2).map_or(0, Array::update_estimate)
            .saturating_mul(cells.div_ceil(256).saturating_add(1))
            .saturating_add(cells.saturating_mul(8))
            .saturating_add(1024)
    } else if name == "MatmulInit" {
        a(0).zip(a(1))
            .and_then(|(a, b)| a.matmul_work(b).ok())
            .map_or(0, |(m, _, n, _)| {
                Array::storage_estimate(m.saturating_mul(n))
            })
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
            "Fft" => {
                let inverse = match args.get(2) {
                    Some(Value::Bool(v)) => *v,
                    _ => return Err(NumericError::Type),
                };
                let (re, im) = rewind::transforms::fft(a(0)?, a(1)?, inverse)?;
                Value::Struct(
                    "Tuple<FloatArray,FloatArray>".into(),
                    BTreeMap::from([
                        ("_0".into(), Value::NumericArray(re)),
                        ("_1".into(), Value::NumericArray(im)),
                    ]),
                )
            }
            "Convolve" => array_value(rewind::transforms::convolve(a(0)?, a(1)?))?,
            "Csr" => {
                let (offsets, indices, values) =
                    rewind::transforms::csr(i(0)?, i(1)?, a(2)?, a(3)?, a(4)?)?;
                Value::Struct(
                    "Tuple<IntArray,IntArray,FloatArray>".into(),
                    BTreeMap::from([
                        ("_0".into(), Value::NumericArray(offsets)),
                        ("_1".into(), Value::NumericArray(indices)),
                        ("_2".into(), Value::NumericArray(values)),
                    ]),
                )
            }
            "SparseMatvec" => array_value(rewind::transforms::matvec(
                i(0)?,
                i(1)?,
                a(2)?,
                a(3)?,
                a(4)?,
                a(5)?,
            ))?,
            "Norm2" => Value::Float(rewind::transforms::norm2(a(0)?)?.to_bits()),
            "Scale" => {
                let factor = f(1)?;
                if !factor.is_finite() {
                    return Err(NumericError::NonFinite);
                }
                array_value(a(0)?.map_float(|n| {
                    if !n.is_finite() {
                        return Err(NumericError::NonFinite);
                    }
                    let v = n * factor;
                    if v.is_finite() {
                        Ok(v)
                    } else {
                        Err(NumericError::Overflow)
                    }
                }))?
            }
            "SuffixArray" => {
                let Some(Value::Bytes(input)) = args.first() else {
                    return Err(NumericError::Type);
                };
                let (sa, lcp) = rewind::suffix::build(input).ok_or(NumericError::Size)?;
                Value::Struct(
                    "Tuple<IntArray,IntArray>".into(),
                    BTreeMap::from([
                        (
                            "_0".into(),
                            Value::NumericArray(Array::integers(vec![sa.len()], &sa)?),
                        ),
                        (
                            "_1".into(),
                            Value::NumericArray(Array::integers(vec![lcp.len()], &lcp)?),
                        ),
                    ]),
                )
            }
            "SuffixSearch" => {
                let (Some(Value::Bytes(input)), Some(Value::Bytes(pattern))) =
                    (args.first(), args.get(2))
                else {
                    return Err(NumericError::Type);
                };
                if input.len() > rewind::suffix::MAX_BYTES
                    || pattern.len() > rewind::suffix::MAX_BYTES
                {
                    return Err(NumericError::Size);
                }
                let order = a(1)?;
                if order.dtype() != DType::Int64 || order.shape() != [input.len()] {
                    return Err(NumericError::Shape);
                }
                let mut bounds = [0i64; 2];
                for (which, bound) in bounds.iter_mut().enumerate() {
                    let (mut low, mut high) = (0usize, input.len());
                    while low < high {
                        let mid = low + (high - low) / 2;
                        let pos = usize::try_from(order.integer(&[mid])?)
                            .map_err(|_| NumericError::Index)?;
                        let suffix = input
                            .get(pos..)
                            .filter(|_| pos < input.len())
                            .ok_or(NumericError::Index)?;
                        let cmp = if suffix.starts_with(pattern) {
                            std::cmp::Ordering::Equal
                        } else {
                            suffix.cmp(pattern)
                        };
                        if cmp == std::cmp::Ordering::Less
                            || (which == 1 && cmp == std::cmp::Ordering::Equal)
                        {
                            low = mid + 1;
                        } else {
                            high = mid;
                        }
                    }
                    *bound = low as i64;
                }
                Value::Struct(
                    "Tuple<Int,Int>".into(),
                    bounds
                        .into_iter()
                        .enumerate()
                        .map(|(i, n)| (format!("_{i}"), Value::Int(n)))
                        .collect(),
                )
            }
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
            "Qr" => {
                let result = a(0)?.qr(f(1)?)?;
                let permutation = v05::frozen(
                    Value::TypedList(
                        "Int".into(),
                        result.permutation.into_iter().map(Value::Int).collect(),
                    ),
                    rt,
                );
                Value::Struct(
                    "QrResult".into(),
                    BTreeMap::from([
                        ("q".into(), Value::NumericArray(result.q)),
                        ("r".into(), Value::NumericArray(result.r)),
                        ("permutation".into(), permutation),
                        ("rank".into(), Value::Int(result.rank as i64)),
                    ]),
                )
            }
            "LeastSquares" => array_value(a(0)?.least_squares(a(1)?, f(2)?))?,
            "EigenSymmetric" => {
                let result = a(0)?.eigen_symmetric(f(1)?, i(2)?)?;
                Value::Struct(
                    "EigenResult".into(),
                    BTreeMap::from([
                        ("values".into(), Value::NumericArray(result.values)),
                        ("vectors".into(), Value::NumericArray(result.vectors)),
                        ("sweeps".into(), Value::Int(result.sweeps as i64)),
                    ]),
                )
            }
            "Covariance" => Value::Float(a(0)?.covariance(a(1)?, i(2)?)?.to_bits()),
            "Correlation" => Value::Float(a(0)?.correlation(a(1)?)?.to_bits()),
            "Quantile" => Value::Float(a(0)?.quantile(f(1)?)?.to_bits()),
            "Histogram" => {
                let result = a(0)?.histogram(a(1)?)?;
                Value::Struct(
                    "HistogramResult".into(),
                    BTreeMap::from([
                        ("counts".into(), Value::NumericArray(result.counts)),
                        ("underflow".into(), Value::Int(result.underflow as i64)),
                        ("overflow".into(), Value::Int(result.overflow as i64)),
                    ]),
                )
            }
            "Sum" => Value::Float(a(0)?.sum()?.to_bits()),
            "Mean" => Value::Float(a(0)?.mean_variance(0)?.0.to_bits()),
            "Variance" => Value::Float(a(0)?.mean_variance(i(1)?)?.1.to_bits()),
            "DotStep" => {
                let state = rewind::numeric::KernelProgress {
                    cursor: usize_arg(&args[2])?,
                    sum: f(3)?,
                    correction: f(4)?,
                };
                let progress = a(0)?.dot_step(a(1)?, state)?;
                Value::Struct(
                    "Tuple<Int,Float,Float,Bool>".into(),
                    BTreeMap::from([
                        ("_0".into(), Value::Int(progress.cursor as i64)),
                        ("_1".into(), Value::Float(progress.sum.to_bits())),
                        ("_2".into(), Value::Float(progress.correction.to_bits())),
                        ("_3".into(), Value::Bool(progress.cursor == a(0)?.len())),
                    ]),
                )
            }
            "MatmulInit" => {
                let (m, _, n, total) = a(0)?.matmul_work(a(1)?)?;
                i64::try_from(total).map_err(|_| NumericError::Size)?;
                array_value(Array::zeros(DType::Float64, vec![m, n]))?
            }
            "MatmulStep" => {
                let state = rewind::numeric::KernelProgress {
                    cursor: usize_arg(&args[3])?,
                    sum: f(4)?,
                    correction: f(5)?,
                };
                let total = a(0)?.matmul_work(a(1)?)?.3;
                i64::try_from(total).map_err(|_| NumericError::Size)?;
                let (out, progress) = a(0)?.matmul_step(a(1)?, a(2)?, state)?;
                Value::Struct(
                    "Tuple<FloatArray,Int,Float,Float,Bool>".into(),
                    BTreeMap::from([
                        ("_0".into(), Value::NumericArray(out)),
                        ("_1".into(), Value::Int(progress.cursor as i64)),
                        ("_2".into(), Value::Float(progress.sum.to_bits())),
                        ("_3".into(), Value::Float(progress.correction.to_bits())),
                        ("_4".into(), Value::Bool(progress.cursor == total)),
                    ]),
                )
            }
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
        "stdNumericFft" => (
            &["&FloatArray", "&FloatArray", "Bool"],
            "Result<Tuple<FloatArray,FloatArray>,StdError>",
        ),
        "stdNumericConvolve" => (
            &["&FloatArray", "&FloatArray"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericCsr" => (
            &["Int", "Int", "&IntArray", "&IntArray", "&FloatArray"],
            "Result<Tuple<IntArray,IntArray,FloatArray>,StdError>",
        ),
        "stdNumericSparseMatvec" => (
            &[
                "Int",
                "Int",
                "&IntArray",
                "&IntArray",
                "&FloatArray",
                "&FloatArray",
            ],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericNorm2" => (&["&FloatArray"], "Result<Float,StdError>"),
        "stdNumericScale" => (&["&FloatArray", "Float"], "Result<FloatArray,StdError>"),
        "stdNumericSuffixArray" => (&["Bytes"], "Result<Tuple<IntArray,IntArray>,StdError>"),
        "stdNumericSuffixSearch" => (
            &["Bytes", "&IntArray", "Bytes"],
            "Result<Tuple<Int,Int>,StdError>",
        ),
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
        "stdNumericDotStep" => (
            &["&FloatArray", "&FloatArray", "Int", "Float", "Float"],
            "Result<Tuple<Int,Float,Float,Bool>,StdError>",
        ),
        "stdNumericMatmulInit" => (
            &["&FloatArray", "&FloatArray"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericMatmulStep" => (
            &[
                "&FloatArray",
                "&FloatArray",
                "&FloatArray",
                "Int",
                "Float",
                "Float",
            ],
            "Result<Tuple<FloatArray,Int,Float,Float,Bool>,StdError>",
        ),
        "stdNumericDot" => (&["&FloatArray", "&FloatArray"], "Result<Float,StdError>"),
        "stdNumericMatmul" => (
            &["&FloatArray", "&FloatArray"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericSolve" => (
            &["&FloatArray", "&FloatArray", "Float"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericQr" => (&["&FloatArray", "Float"], "Result<QrResult,StdError>"),
        "stdNumericLeastSquares" => (
            &["&FloatArray", "&FloatArray", "Float"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericEigenSymmetric" => (
            &["&FloatArray", "Float", "Int"],
            "Result<EigenResult,StdError>",
        ),
        "stdNumericCovariance" => (
            &["&FloatArray", "&FloatArray", "Int"],
            "Result<Float,StdError>",
        ),
        "stdNumericCorrelation" => (&["&FloatArray", "&FloatArray"], "Result<Float,StdError>"),
        "stdNumericQuantile" => (&["&FloatArray", "Float"], "Result<Float,StdError>"),
        "stdNumericHistogram" => (
            &["&FloatArray", "&FloatArray"],
            "Result<HistogramResult,StdError>",
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
