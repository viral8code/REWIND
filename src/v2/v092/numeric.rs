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
    if language_at_least(&p.language, "1.9.42") {
        if p.structs.contains_key("LeastSquaresWork")
            || p.enums.contains_key("LeastSquaresWork")
            || p.aliases.contains_key("LeastSquaresWork")
        {
            return Err(Error::InvalidOperation(
                "reserved least squares work type".into(),
            ));
        }
        p.structs.insert(
            "LeastSquaresWork".into(),
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
    if language_at_least(&p.language, "1.9.38") {
        if p.structs.contains_key("EigenWork")
            || p.enums.contains_key("EigenWork")
            || p.aliases.contains_key("EigenWork")
        {
            return Err(Error::InvalidOperation("reserved eigen work type".into()));
        }
        p.structs.insert(
            "EigenWork".into(),
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
    if language_at_least(&p.language, "1.9.37") {
        for (name, fields) in [
            ("QrWork", vec![]),
            (
                "QrArrayResult",
                vec![
                    ("q", "FloatArray"),
                    ("r", "FloatArray"),
                    ("permutation", "IntArray"),
                    ("rank", "Int"),
                ],
            ),
        ] {
            if p.structs.contains_key(name)
                || p.enums.contains_key(name)
                || p.aliases.contains_key(name)
            {
                return Err(Error::InvalidOperation(
                    "reserved cooperative QR type".into(),
                ));
            }
            p.structs.insert(
                name.into(),
                StructDef {
                    private_fields: if name == "QrWork" {
                        BTreeSet::from(["$native".into()])
                    } else {
                        BTreeSet::new()
                    },
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
    if language_at_least(&p.language, "1.9.33") {
        if p.structs.contains_key("SolveWork")
            || p.enums.contains_key("SolveWork")
            || p.aliases.contains_key("SolveWork")
        {
            return Err(Error::InvalidOperation("reserved solve work type".into()));
        }
        p.structs.insert(
            "SolveWork".into(),
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
    if language_at_least(&p.language, "1.9.27") {
        if p.structs.contains_key("SparseWork")
            || p.enums.contains_key("SparseWork")
            || p.aliases.contains_key("SparseWork")
        {
            return Err(Error::InvalidOperation("reserved sparse work type".into()));
        }
        p.structs.insert(
            "SparseWork".into(),
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
    if language_at_least(&p.language, "1.9.26") {
        if p.structs.contains_key("FftWork")
            || p.enums.contains_key("FftWork")
            || p.aliases.contains_key("FftWork")
        {
            return Err(Error::InvalidOperation("reserved FFT work type".into()));
        }
        p.structs.insert(
            "FftWork".into(),
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
fn vector_operation(
    name: &Value,
    factor: &Value,
) -> std::result::Result<rewind::numeric::VectorOperation, NumericError> {
    use rewind::numeric::VectorOperation;
    Ok(match operation(name)? {
        "scale" => VectorOperation::Scale(floating(factor)?),
        "add" => VectorOperation::Add,
        "sub" => VectorOperation::Sub,
        "mul" => VectorOperation::Mul,
        "div" => VectorOperation::Div,
        _ => return Err(NumericError::Domain),
    })
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
        "stdNumericGraphAdjacency" | "stdNumericGraphBfs" | "stdNumericRangeInt"
    ) && !language_at_least(&p.language, "1.9.49")
    {
        return Err(diagnostic(
            at,
            "native graph/range kernels require language 1.9.49",
        ));
    }
    if matches!(
        n,
        "stdNumericGetFlatFloat"
            | "stdNumericGetFlatInt"
            | "stdNumericWithFlatFloat"
            | "stdNumericWithFlatInt"
            | "stdNumericLengthFloat"
            | "stdNumericLengthInt"
    ) && !language_at_least(&p.language, "1.9.48")
    {
        return Err(diagnostic(
            at,
            "flat numeric indexing requires language 1.9.48",
        ));
    }
    if matches!(
        n,
        "stdNumericLeastSquaresInit"
            | "stdNumericLeastSquaresStep"
            | "stdNumericLeastSquaresDone"
            | "stdNumericLeastSquaresResult"
    ) && !language_at_least(&p.language, "1.9.42")
    {
        return Err(diagnostic(
            at,
            "cooperative least squares requires language 1.9.42",
        ));
    }
    if matches!(
        n,
        "stdNumericEigenInit"
            | "stdNumericEigenStep"
            | "stdNumericEigenDone"
            | "stdNumericEigenResult"
    ) && !language_at_least(&p.language, "1.9.38")
    {
        return Err(diagnostic(
            at,
            "cooperative eigen decomposition requires language 1.9.38",
        ));
    }
    if matches!(
        n,
        "stdNumericQrInit" | "stdNumericQrStep" | "stdNumericQrDone" | "stdNumericQrResult"
    ) && !language_at_least(&p.language, "1.9.37")
    {
        return Err(diagnostic(at, "cooperative QR requires language 1.9.37"));
    }
    if matches!(
        n,
        "stdNumericSolveInit"
            | "stdNumericSolveStep"
            | "stdNumericSolveDone"
            | "stdNumericSolveResult"
    ) && !language_at_least(&p.language, "1.9.33")
    {
        return Err(diagnostic(at, "cooperative solve requires language 1.9.33"));
    }
    if matches!(
        n,
        "stdNumericSparseInit"
            | "stdNumericSparseStep"
            | "stdNumericSparseDone"
            | "stdNumericSparseResult"
    ) && !language_at_least(&p.language, "1.9.27")
    {
        return Err(diagnostic(
            at,
            "cooperative sparse kernels require language 1.9.27",
        ));
    }
    if matches!(
        n,
        "stdNumericFftInit" | "stdNumericFftStep" | "stdNumericFftDone" | "stdNumericFftResult"
    ) && !language_at_least(&p.language, "1.9.26")
    {
        return Err(diagnostic(at, "cooperative FFT requires language 1.9.26"));
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
        "stdNumericVectorInit" | "stdNumericVectorStep" | "stdNumericNormStep"
    ) && !language_at_least(&p.language, "1.9.31")
    {
        return Err(diagnostic(
            at,
            "cooperative vector kernels require language 1.9.31",
        ));
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
    if matches!(
        n,
        "stdNumericAffine"
            | "stdNumericActivation"
            | "stdNumericSumToShape"
            | "stdNumericCheckFinite"
            | "stdNumericTensorKey"
            | "stdNumericReshapeLogical"
            | "stdNumericModelEncode"
            | "stdNumericModelDecode"
            | "stdNumericModelCheck"
    ) && !language_at_least(&p.language, "1.9.16")
    {
        return Err(diagnostic(at, "tensor primitives require language 1.9.16"));
    }
    if matches!(n, "stdNumericSgdStep" | "stdNumericAdamStep")
        && !language_at_least(&p.language, "1.9.16")
    {
        return Err(diagnostic(at, "optimizer kernels require language 1.9.16"));
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
fn least_squares_work(
    value: &Value,
    rt: &Runtime,
) -> std::result::Result<rewind::numeric::LeastSquaresWork, NumericError> {
    let Value::Struct(ty, fields) = target(value, rt) else {
        return Err(NumericError::Type);
    };
    if ty != "LeastSquaresWork" {
        return Err(NumericError::Type);
    }
    let a = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(|v| array(v, rt))
            .cloned()
    };
    let i = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(usize_arg)
    };
    let f = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(floating)
    };
    let work = rewind::numeric::LeastSquaresWork {
        qr: qr_work(fields.get("$qr").ok_or(NumericError::Type)?, rt)?,
        right: a("$right")?,
        projected: a("$projected")?,
        output: a("$output")?,
        tolerance_bits: f("$tolerance")?.to_bits(),
        phase: u8::try_from(i("$phase")?).map_err(|_| NumericError::Domain)?,
        cursor: i("$cursor")?,
        column: i("$column")?,
        row: i("$row")?,
        sum: f("$sum")?,
        correction: f("$correction")?,
    };
    work.validate()?;
    Ok(work)
}
fn least_squares_work_value(w: rewind::numeric::LeastSquaresWork) -> Value {
    Value::Struct(
        "LeastSquaresWork".into(),
        BTreeMap::from([
            ("$qr".into(), qr_work_value(w.qr)),
            ("$right".into(), Value::NumericArray(w.right)),
            ("$projected".into(), Value::NumericArray(w.projected)),
            ("$output".into(), Value::NumericArray(w.output)),
            ("$tolerance".into(), Value::Float(w.tolerance_bits)),
            ("$phase".into(), Value::Int(w.phase as i64)),
            ("$cursor".into(), Value::Int(w.cursor as i64)),
            ("$column".into(), Value::Int(w.column as i64)),
            ("$row".into(), Value::Int(w.row as i64)),
            ("$sum".into(), Value::Float(w.sum.to_bits())),
            ("$correction".into(), Value::Float(w.correction.to_bits())),
        ]),
    )
}
fn eigen_work(
    value: &Value,
    rt: &Runtime,
) -> std::result::Result<rewind::numeric::EigenWork, NumericError> {
    let Value::Struct(ty, fields) = target(value, rt) else {
        return Err(NumericError::Type);
    };
    if ty != "EigenWork" {
        return Err(NumericError::Type);
    }
    let a = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(|v| array(v, rt))
            .cloned()
    };
    let i = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(usize_arg)
    };
    let f = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(floating)
    };
    let w = rewind::numeric::EigenWork {
        input: a("$input")?,
        matrix: a("$matrix")?,
        vectors: a("$vectors")?,
        order: a("$order")?,
        order_scratch: a("$order_scratch")?,
        sorted: a("$sorted")?,
        values: a("$values")?,
        tolerance: f("$tolerance")?,
        scale: f("$scale")?,
        largest: f("$largest")?,
        cosine: f("$cosine")?,
        sine: f("$sine")?,
        max_sweeps: i("$max_sweeps")?,
        sweeps: i("$sweeps")?,
        cursor: i("$cursor")?,
        p: i("$p")?,
        q: i("$q")?,
        width: i("$width")?,
        base: i("$base")?,
        left: i("$left")?,
        mid: i("$mid")?,
        right: i("$right")?,
        end: i("$end")?,
        out: i("$out")?,
        phase: u8::try_from(i("$phase")?).map_err(|_| NumericError::Domain)?,
    };
    w.validate()?;
    Ok(w)
}
fn eigen_work_value(w: rewind::numeric::EigenWork) -> Value {
    Value::Struct(
        "EigenWork".into(),
        BTreeMap::from([
            ("$input".into(), Value::NumericArray(w.input)),
            ("$matrix".into(), Value::NumericArray(w.matrix)),
            ("$vectors".into(), Value::NumericArray(w.vectors)),
            ("$order".into(), Value::NumericArray(w.order)),
            (
                "$order_scratch".into(),
                Value::NumericArray(w.order_scratch),
            ),
            ("$sorted".into(), Value::NumericArray(w.sorted)),
            ("$values".into(), Value::NumericArray(w.values)),
            ("$tolerance".into(), Value::Float(w.tolerance.to_bits())),
            ("$scale".into(), Value::Float(w.scale.to_bits())),
            ("$largest".into(), Value::Float(w.largest.to_bits())),
            ("$cosine".into(), Value::Float(w.cosine.to_bits())),
            ("$sine".into(), Value::Float(w.sine.to_bits())),
            ("$max_sweeps".into(), Value::Int(w.max_sweeps as i64)),
            ("$sweeps".into(), Value::Int(w.sweeps as i64)),
            ("$cursor".into(), Value::Int(w.cursor as i64)),
            ("$p".into(), Value::Int(w.p as i64)),
            ("$q".into(), Value::Int(w.q as i64)),
            ("$width".into(), Value::Int(w.width as i64)),
            ("$base".into(), Value::Int(w.base as i64)),
            ("$left".into(), Value::Int(w.left as i64)),
            ("$mid".into(), Value::Int(w.mid as i64)),
            ("$right".into(), Value::Int(w.right as i64)),
            ("$end".into(), Value::Int(w.end as i64)),
            ("$out".into(), Value::Int(w.out as i64)),
            ("$phase".into(), Value::Int(w.phase as i64)),
        ]),
    )
}
fn qr_work(
    value: &Value,
    rt: &Runtime,
) -> std::result::Result<rewind::numeric::QrWork, NumericError> {
    let Value::Struct(ty, fields) = target(value, rt) else {
        return Err(NumericError::Type);
    };
    if ty != "QrWork" {
        return Err(NumericError::Type);
    }
    let a = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(|v| array(v, rt))
            .cloned()
    };
    let i = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(usize_arg)
    };
    let f = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(floating)
    };
    let work = rewind::numeric::QrWork {
        input: a("$input")?,
        matrix: a("$matrix")?,
        reflectors: a("$reflectors")?,
        q: a("$q")?,
        r: a("$r")?,
        permutation: a("$permutation")?,
        tolerance: f("$tolerance")?,
        scale: f("$scale")?,
        original_norm: f("$original_norm")?,
        pivot_norm: f("$pivot_norm")?,
        sign: f("$sign")?,
        v_norm: f("$v_norm")?,
        sum: f("$sum")?,
        correction: f("$correction")?,
        cursor: i("$cursor")?,
        k: i("$k")?,
        col: i("$col")?,
        pivot: i("$pivot")?,
        rank: i("$rank")?,
        phase: u8::try_from(i("$phase")?).map_err(|_| NumericError::Domain)?,
    };
    work.validate()?;
    Ok(work)
}
fn qr_work_value(w: rewind::numeric::QrWork) -> Value {
    Value::Struct(
        "QrWork".into(),
        BTreeMap::from([
            ("$input".into(), Value::NumericArray(w.input)),
            ("$matrix".into(), Value::NumericArray(w.matrix)),
            ("$reflectors".into(), Value::NumericArray(w.reflectors)),
            ("$q".into(), Value::NumericArray(w.q)),
            ("$r".into(), Value::NumericArray(w.r)),
            ("$permutation".into(), Value::NumericArray(w.permutation)),
            ("$tolerance".into(), Value::Float(w.tolerance.to_bits())),
            ("$scale".into(), Value::Float(w.scale.to_bits())),
            (
                "$original_norm".into(),
                Value::Float(w.original_norm.to_bits()),
            ),
            ("$pivot_norm".into(), Value::Float(w.pivot_norm.to_bits())),
            ("$sign".into(), Value::Float(w.sign.to_bits())),
            ("$v_norm".into(), Value::Float(w.v_norm.to_bits())),
            ("$sum".into(), Value::Float(w.sum.to_bits())),
            ("$correction".into(), Value::Float(w.correction.to_bits())),
            ("$cursor".into(), Value::Int(w.cursor as i64)),
            ("$k".into(), Value::Int(w.k as i64)),
            ("$col".into(), Value::Int(w.col as i64)),
            ("$pivot".into(), Value::Int(w.pivot as i64)),
            ("$rank".into(), Value::Int(w.rank as i64)),
            ("$phase".into(), Value::Int(w.phase as i64)),
        ]),
    )
}
fn solve_work(
    value: &Value,
    rt: &Runtime,
) -> std::result::Result<rewind::numeric::SolveWork, NumericError> {
    let Value::Struct(ty, fields) = target(value, rt) else {
        return Err(NumericError::Type);
    };
    if ty != "SolveWork" {
        return Err(NumericError::Type);
    }
    let a = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(|v| array(v, rt))
            .cloned()
    };
    let i = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(usize_arg)
    };
    let f = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(floating)
    };
    let work = rewind::numeric::SolveWork {
        input: a("$input")?,
        right: a("$right")?,
        matrix: a("$matrix")?,
        rhs: a("$rhs")?,
        output: a("$output")?,
        tolerance: f("$tolerance")?,
        scale: f("$scale")?,
        phase: u8::try_from(i("$phase")?).map_err(|_| NumericError::Domain)?,
        cursor: i("$cursor")?,
        col: i("$col")?,
        row: i("$row")?,
        pivot: i("$pivot")?,
        factor: f("$factor")?,
        value: f("$value")?,
    };
    work.validate()?;
    Ok(work)
}
fn solve_work_value(w: rewind::numeric::SolveWork) -> Value {
    Value::Struct(
        "SolveWork".into(),
        BTreeMap::from([
            ("$input".into(), Value::NumericArray(w.input)),
            ("$right".into(), Value::NumericArray(w.right)),
            ("$matrix".into(), Value::NumericArray(w.matrix)),
            ("$rhs".into(), Value::NumericArray(w.rhs)),
            ("$output".into(), Value::NumericArray(w.output)),
            ("$tolerance".into(), Value::Float(w.tolerance.to_bits())),
            ("$scale".into(), Value::Float(w.scale.to_bits())),
            ("$phase".into(), Value::Int(w.phase as i64)),
            ("$cursor".into(), Value::Int(w.cursor as i64)),
            ("$col".into(), Value::Int(w.col as i64)),
            ("$row".into(), Value::Int(w.row as i64)),
            ("$pivot".into(), Value::Int(w.pivot as i64)),
            ("$factor".into(), Value::Float(w.factor.to_bits())),
            ("$value".into(), Value::Float(w.value.to_bits())),
        ]),
    )
}
fn sparse_work(
    value: &Value,
    rt: &Runtime,
) -> std::result::Result<rewind::numeric::SparseWork, NumericError> {
    let Value::Struct(ty, fields) = target(value, rt) else {
        return Err(NumericError::Type);
    };
    if ty != "SparseWork" {
        return Err(NumericError::Type);
    }
    let a = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(|v| array(v, rt))
            .cloned()
    };
    let i = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(usize_arg)
    };
    let f = |name: &str| match fields.get(name) {
        Some(Value::Float(v)) => Ok(f64::from_bits(*v)),
        _ => Err(NumericError::Type),
    };
    let Some(Value::Int(previous)) = fields.get("$previous") else {
        return Err(NumericError::Type);
    };
    let work = rewind::numeric::SparseWork {
        rows: i("$rows")?,
        cols: i("$cols")?,
        phase: i("$phase")?,
        cursor: i("$cursor")?,
        entry: i("$entry")?,
        offsets: a("$offsets")?,
        indices: a("$indices")?,
        values: a("$values")?,
        right: a("$right")?,
        output: a("$output")?,
        previous: *previous,
        sum: f("$sum")?,
        correction: f("$correction")?,
    };
    work.validate()?;
    Ok(work)
}
fn sparse_work_value(work: rewind::numeric::SparseWork) -> Value {
    Value::Struct(
        "SparseWork".into(),
        BTreeMap::from([
            ("$rows".into(), Value::Int(work.rows as i64)),
            ("$cols".into(), Value::Int(work.cols as i64)),
            ("$phase".into(), Value::Int(work.phase as i64)),
            ("$cursor".into(), Value::Int(work.cursor as i64)),
            ("$entry".into(), Value::Int(work.entry as i64)),
            ("$offsets".into(), Value::NumericArray(work.offsets)),
            ("$indices".into(), Value::NumericArray(work.indices)),
            ("$values".into(), Value::NumericArray(work.values)),
            ("$right".into(), Value::NumericArray(work.right)),
            ("$output".into(), Value::NumericArray(work.output)),
            ("$previous".into(), Value::Int(work.previous)),
            ("$sum".into(), Value::Float(work.sum.to_bits())),
            (
                "$correction".into(),
                Value::Float(work.correction.to_bits()),
            ),
        ]),
    )
}
fn fft_work(
    value: &Value,
    rt: &Runtime,
) -> std::result::Result<rewind::numeric::FftWork, NumericError> {
    let Value::Struct(ty, fields) = target(value, rt) else {
        return Err(NumericError::Type);
    };
    if ty != "FftWork" {
        return Err(NumericError::Type);
    }
    let a = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(|value| array(value, rt))
            .cloned()
    };
    let i = |name: &str| {
        fields
            .get(name)
            .ok_or(NumericError::Type)
            .and_then(usize_arg)
    };
    let Some(Value::Bool(inverse)) = fields.get("$inverse") else {
        return Err(NumericError::Type);
    };
    let work = rewind::numeric::FftWork {
        input_real: a("$inputReal")?,
        input_imag: a("$inputImag")?,
        real: a("$real")?,
        imag: a("$imag")?,
        twiddle_real: a("$twiddleReal")?,
        twiddle_imag: a("$twiddleImag")?,
        inverse: *inverse,
        phase: i("$phase")?,
        cursor: i("$cursor")?,
        width: i("$width")?,
    };
    work.validate()?;
    Ok(work)
}
fn fft_work_value(work: rewind::numeric::FftWork) -> Value {
    Value::Struct(
        "FftWork".into(),
        BTreeMap::from([
            ("$inputReal".into(), Value::NumericArray(work.input_real)),
            ("$inputImag".into(), Value::NumericArray(work.input_imag)),
            ("$real".into(), Value::NumericArray(work.real)),
            ("$imag".into(), Value::NumericArray(work.imag)),
            (
                "$twiddleReal".into(),
                Value::NumericArray(work.twiddle_real),
            ),
            (
                "$twiddleImag".into(),
                Value::NumericArray(work.twiddle_imag),
            ),
            ("$inverse".into(), Value::Bool(work.inverse)),
            ("$phase".into(), Value::Int(work.phase as i64)),
            ("$cursor".into(), Value::Int(work.cursor as i64)),
            ("$width".into(), Value::Int(work.width as i64)),
        ]),
    )
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
fn model_parameters<'a>(
    v: &'a Value,
    rt: &'a Runtime,
) -> std::result::Result<Vec<(&'a str, &'a Array)>, NumericError> {
    let Value::TypedMap(key, value, entries) = target(v, rt) else {
        return Err(NumericError::Type);
    };
    if key != "String" || value != "FloatArray" {
        return Err(NumericError::Type);
    }
    if entries.len() > rewind::numeric::MAX_MODEL_PARAMETERS {
        return Err(NumericError::Size);
    }
    entries
        .iter()
        .map(|(key, value)| {
            let rewind::MapKey::Text(name) = key else {
                return Err(NumericError::Type);
            };
            Ok((name.as_str(), array(value, rt)?))
        })
        .collect()
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
    let cost = if name == "RangeInt"
        && matches!(args.first(), Some(Value::Int(0)))
        && matches!(args.get(1), Some(Value::Int(0)))
    {
        1024
    } else if name == "RangeInt" {
        args.get(2)
            .and_then(|v| usize_arg(v).ok())
            .unwrap_or(0)
            .saturating_mul(4)
            .saturating_add(1024)
    } else if name == "GraphAdjacency" {
        length(1).saturating_mul(64).saturating_add(1024)
    } else if name == "GraphBfs" {
        length(0)
            .saturating_add(args.get(3).and_then(|v| usize_arg(v).ok()).unwrap_or(0))
            .saturating_mul(128)
            .saturating_add(1024)
    } else if name == "LeastSquaresInit" {
        32768
    } else if name == "LeastSquaresStep" {
        least_squares_work(&args[0], rt).map_or(2048, |w| {
            if w.phase == 0 {
                w.right
                    .len()
                    .saturating_sub(w.cursor)
                    .min(rewind::numeric::LEAST_SQUARES_CHUNK)
                    .saturating_mul(32)
                    .saturating_add(2048)
            } else if w.phase == 1 && !w.qr.done() {
                let tolerance = f64::from_bits(w.tolerance_bits);
                if !tolerance.is_finite() || tolerance < 0.0 {
                    2048
                } else if w.output.len() == 0 {
                    // Empty QR only advances validation/empty permutation phases.
                    16 * 128 + 2048
                } else {
                    rewind::numeric::QR_CHUNK * 128 + 2048
                }
            } else {
                let n = w.output.len();
                w.right
                    .len()
                    .saturating_mul(n)
                    .saturating_add(n.saturating_mul(n))
                    .saturating_add(n.saturating_mul(8))
                    .saturating_add(16)
                    .min(rewind::numeric::LEAST_SQUARES_CHUNK)
                    .saturating_mul(64)
                    .saturating_add(2048)
            }
        })
    } else if matches!(name, "LeastSquaresDone" | "LeastSquaresResult") {
        2048
    } else if name == "EigenInit" {
        32768
    } else if name == "EigenStep" {
        eigen_work(&args[0], rt).map_or(2048, |w| {
            let n = w.values.len();
            let sweeps = if n < 2 {
                0
            } else {
                w.max_sweeps.saturating_sub(w.sweeps)
            };
            n.saturating_pow(3)
                .saturating_mul(6)
                .saturating_mul(sweeps)
                .saturating_add(n.saturating_mul(n).saturating_mul(8))
                .saturating_add(n.saturating_mul(32))
                .saturating_add(32)
                .min(rewind::numeric::EIGEN_CHUNK)
                .saturating_mul(192)
                .saturating_add(2048)
        })
    } else if matches!(name, "EigenDone" | "EigenResult") {
        2048
    } else if name == "QrInit" {
        32768
    } else if name == "QrStep" {
        rewind::numeric::QR_CHUNK * 128 + 2048
    } else if matches!(name, "QrDone" | "QrResult") {
        2048
    } else if name == "SolveInit" {
        4096
    } else if name == "SolveStep" {
        solve_work(&args[0], rt).map_or(1024, |w| {
            let n = w.right.len();
            n.saturating_pow(3)
                .saturating_mul(3)
                .saturating_add(n.saturating_mul(n).saturating_mul(2))
                .saturating_add(n.saturating_mul(10))
                .saturating_add(1)
                .min(rewind::numeric::SOLVE_CHUNK)
                .saturating_mul(64)
                .saturating_add(1024)
        })
    } else if matches!(name, "SolveDone" | "SolveResult") {
        512
    } else if name == "SparseInit" {
        32768
    } else if name == "SparseStep" {
        sparse_work(&args[0], rt).map_or(2048, |w| {
            let items = if w.phase == 0 {
                w.cols.saturating_sub(w.cursor)
            } else if w.phase == 1 {
                w.rows
                    .saturating_sub(w.cursor)
                    .saturating_add(w.values.len().saturating_sub(w.entry))
            } else {
                0
            };
            items
                .min(rewind::numeric::SPARSE_CHUNK)
                .saturating_mul(128)
                .saturating_add(2048)
        })
    } else if matches!(name, "SparseDone" | "SparseResult") {
        512
    } else if name == "FftInit" {
        32768
    } else if name == "FftStep" {
        rewind::numeric::FFT_CHUNK * 64 + 1024
    } else if matches!(name, "FftDone" | "FftResult") {
        256
    } else if name == "SgdStep" {
        length(0).saturating_mul(32).saturating_add(128)
    } else if name == "AdamStep" {
        length(0).saturating_mul(128).saturating_add(256)
    } else if name == "ReshapeLogical" {
        if a(0).is_some_and(|a| a.reshape_logical_scratch() == 2048) {
            128
        } else {
            length(0).saturating_mul(24)
        }
    } else if matches!(name, "ModelEncode" | "ModelCheck") {
        model_parameters(&args[0], rt).map_or(1, |p| {
            p.iter().fold(8192usize, |n, (_, a)| {
                n.saturating_add(a.len().saturating_mul(64))
            })
        })
    } else if name == "ModelDecode" {
        match args.first() {
            Some(Value::Bytes(b)) if b.len() <= rewind::numeric::MAX_MODEL_BYTES => {
                b.len().saturating_mul(64).saturating_add(8192)
            }
            _ => 1,
        }
    } else if name == "TensorKey" {
        // Trace/debug formatting may warm the digest cache. Admission must not
        // depend on that cache, or debug and compact replay could diverge.
        a(4).map_or(1024, Array::storage_digest_work)
    } else if name == "Activation" {
        length(1).saturating_mul(64).saturating_add(64)
    } else if name == "SumToShape" {
        length(0).saturating_mul(256).saturating_add(128)
    } else if name == "Norm2" {
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
    } else if name == "VectorInit" || name == "MatmulInit" || name.starts_with("Zeros") {
        1024
    } else if name == "VectorStep" {
        length(1)
            .saturating_sub(args.get(5).and_then(|v| usize_arg(v).ok()).unwrap_or(0))
            .min(rewind::numeric::COOPERATIVE_MACS)
            .saturating_mul(32)
            .saturating_add(256)
    } else if name == "NormStep" {
        length(0)
            .saturating_sub(args.get(1).and_then(|v| usize_arg(v).ok()).unwrap_or(0))
            .min(rewind::numeric::COOPERATIVE_MACS)
            .saturating_mul(16)
            .saturating_add(128)
    } else if name == "DotStep" {
        length(0)
            .saturating_sub(args.get(2).and_then(|v| usize_arg(v).ok()).unwrap_or(0))
            .min(rewind::numeric::COOPERATIVE_MACS)
            .saturating_mul(32)
            .saturating_add(128)
    } else if name == "MatmulStep" {
        a(0).zip(a(1))
            .and_then(|(a, b)| a.matmul_work(b).ok())
            .map_or(0, |(_, _, _, total)| total)
            .saturating_sub(args.get(3).and_then(|v| usize_arg(v).ok()).unwrap_or(0))
            .min(rewind::numeric::COOPERATIVE_MACS)
            .saturating_mul(32)
            .saturating_add(256)
    } else if name.starts_with("From") {
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
        || name.starts_with("Length")
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
    if name == "RangeInt"
        && matches!(args.first(), Some(Value::Int(0)))
        && matches!(args.get(1), Some(Value::Int(0)))
    {
        2048
    } else if name == "RangeInt" {
        Array::storage_estimate(
            args.get(2)
                .and_then(|v| usize_arg(v).ok())
                .unwrap_or(0)
                .min(rewind::numeric::MAX_ELEMENTS),
        )
        .saturating_add(4096)
    } else if name == "GraphAdjacency" {
        Array::storage_estimate(
            args.first()
                .and_then(|v| usize_arg(v).ok())
                .unwrap_or(0)
                .min(rewind::numeric::MAX_GRAPH_ITEMS),
        )
        .saturating_add(Array::storage_estimate(
            length(1).min(rewind::numeric::MAX_GRAPH_ITEMS),
        ))
        .saturating_add(4096)
    } else if name == "GraphBfs" {
        let vertices = length(0).min(rewind::numeric::MAX_GRAPH_ITEMS);
        Array::storage_estimate(vertices)
            .saturating_add(vertices.saturating_mul(16))
            .saturating_add(
                args.get(3)
                    .and_then(|v| usize_arg(v).ok())
                    .unwrap_or(0)
                    .min(rewind::numeric::MAX_GRAPH_ITEMS),
            )
            .saturating_add(4096)
    } else if name == "LeastSquaresInit" {
        1024 * 1024 + 65536
    } else if name == "LeastSquaresStep" {
        least_squares_work(&args[0], rt).map_or(32768, |w| {
            if w.phase == 1 && !w.qr.done() {
                scratch("QrStep", &[qr_work_value(w.qr.clone())], rt).saturating_add(32768)
            } else if w.phase == 0 {
                32768
            } else {
                [&w.projected, &w.output].iter().fold(32768usize, |sum, a| {
                    sum.saturating_add(
                        Array::storage_estimate(a.len()).min(
                            a.update_estimate()
                                .saturating_mul(rewind::numeric::LEAST_SQUARES_CHUNK),
                        ),
                    )
                })
            }
        })
    } else if matches!(name, "LeastSquaresDone" | "LeastSquaresResult") {
        32768
    } else if name == "EigenInit" {
        1024 * 1024
    } else if name == "EigenStep" {
        eigen_work(&args[0], rt).map_or(32768, |w| {
            if w.phase == 0
                && w.matrix.len().saturating_sub(w.cursor) >= rewind::numeric::EIGEN_CHUNK
            {
                return Array::storage_estimate(w.matrix.len())
                    .min(
                        w.matrix
                            .update_estimate()
                            .saturating_mul(rewind::numeric::EIGEN_CHUNK.div_ceil(256) + 1),
                    )
                    .saturating_add(rewind::numeric::EIGEN_CHUNK * 16 + 32768);
            }
            let arrays = [
                &w.matrix,
                &w.vectors,
                &w.order,
                &w.order_scratch,
                &w.sorted,
                &w.values,
            ];
            let nodes = arrays.iter().fold(0usize, |sum, a| {
                sum.saturating_add(a.len().div_ceil(256).saturating_mul(4).saturating_add(17))
            });
            let dirty = nodes
                .min(rewind::numeric::EIGEN_CHUNK * 6 * 17)
                .saturating_mul(32)
                .saturating_add(1024);
            arrays
                .iter()
                // Four bounded row/column staging buffers can coexist.
                .fold(
                    32768usize
                        .saturating_add(dirty)
                        .saturating_add(rewind::numeric::EIGEN_CHUNK * 32),
                    |sum, a| {
                        sum.saturating_add(
                            Array::storage_estimate(a.len()).min(
                                a.update_estimate()
                                    .saturating_mul(rewind::numeric::EIGEN_CHUNK * 6),
                            ),
                        )
                    },
                )
        })
    } else if matches!(name, "EigenDone" | "EigenResult") {
        32768
    } else if name == "QrInit" {
        // All zero arrays have shared logarithmic storage, including the permutation.
        1024 * 1024
    } else if name == "QrStep" {
        qr_work(&args[0], rt).map_or(32768, |w| {
            if w.phase == 0 && w.matrix.len().saturating_sub(w.cursor) >= rewind::numeric::QR_CHUNK
            {
                return Array::storage_estimate(w.matrix.len())
                    .min(
                        w.matrix
                            .update_estimate()
                            .saturating_mul(rewind::numeric::QR_CHUNK.div_ceil(256) + 1),
                    )
                    .saturating_add(rewind::numeric::QR_CHUNK * 16 + 32768);
            }
            let arrays = [&w.matrix, &w.reflectors, &w.q, &w.r, &w.permutation];
            let nodes = arrays.iter().fold(0usize, |sum, a| {
                sum.saturating_add(a.len().div_ceil(256).saturating_mul(2).saturating_add(1))
            });
            // At most two cell writes per scalar unit; each touches a bounded tree path.
            // 32 bytes per touched node bounds HashSet buckets/control and growth slack.
            let dirty_bytes = nodes
                .min(rewind::numeric::QR_CHUNK * 2 * 17)
                .saturating_mul(32)
                .saturating_add(1024);
            arrays
                .iter()
                .fold(32768usize.saturating_add(dirty_bytes), |sum, a| {
                    sum.saturating_add(
                        Array::storage_estimate(a.len()).min(
                            a.update_estimate()
                                .saturating_mul(rewind::numeric::QR_CHUNK * 2),
                        ),
                    )
                })
        })
    } else if matches!(name, "QrDone" | "QrResult") {
        32768
    } else if name == "SolveInit" {
        Array::storage_estimate(length(0))
            .min(131072)
            .saturating_add(
                Array::storage_estimate(length(1))
                    .min(131072)
                    .saturating_mul(2),
            )
            .saturating_add(16384)
    } else if name == "SolveStep" {
        solve_work(&args[0], rt).map_or(16384, |w| {
            if w.phase == 0
                && w.matrix.len().saturating_sub(w.cursor) >= rewind::numeric::SOLVE_CHUNK
            {
                // The entire step stays in contiguous input copy. Reserve only
                // touched pages and their paths, rather than a full private LU.
                return Array::storage_estimate(w.matrix.len())
                    .min(w.matrix.update_estimate().saturating_mul(
                        rewind::numeric::SOLVE_CHUNK.div_ceil(256).saturating_add(1),
                    ))
                    .saturating_add(rewind::numeric::SOLVE_CHUNK * 16 + 16384);
            }
            // Each bounded work unit writes at most two matrix cells, two rhs
            // cells or one result cell. Cap COW reservation by full private
            // storage, but include bounded row-copy scratch and value metadata.
            Array::storage_estimate(w.matrix.len())
                .min(
                    w.matrix
                        .update_estimate()
                        .saturating_mul(rewind::numeric::SOLVE_CHUNK * 2),
                )
                .saturating_add(
                    Array::storage_estimate(w.rhs.len()).min(
                        w.rhs
                            .update_estimate()
                            .saturating_mul(rewind::numeric::SOLVE_CHUNK * 2),
                    ),
                )
                .saturating_add(
                    Array::storage_estimate(w.output.len()).min(
                        w.output
                            .update_estimate()
                            .saturating_mul(rewind::numeric::SOLVE_CHUNK),
                    ),
                )
                .saturating_add(rewind::numeric::SOLVE_CHUNK * 16 + 16384)
        })
    } else if matches!(name, "SolveDone" | "SolveResult") {
        16384
    } else if name == "VectorInit" {
        Array::storage_estimate(length(1)).min(131072)
    } else if name == "MatmulInit" {
        a(0).zip(a(1))
            .and_then(|(a, b)| a.matmul_work(b).ok())
            .map_or(0, |(m, _, n, _)| {
                Array::storage_estimate(m.saturating_mul(n)).min(131072)
            })
    } else if name.starts_with("Zeros") {
        args.first()
            .and_then(|s| indices(s, rt).ok())
            .and_then(|s| elements(&s).ok())
            .map_or(0, |n| Array::storage_estimate(n).min(131072))
    } else if name == "VectorStep" {
        a(3).map_or(0, Array::update_estimate)
            .saturating_mul(rewind::numeric::COOPERATIVE_MACS.div_ceil(256) + 1)
            .saturating_add(rewind::numeric::COOPERATIVE_MACS * 8 + 16384)
    } else if name == "NormStep" {
        1024
    } else if name == "SparseInit" {
        131072
    } else if name == "SparseStep" {
        sparse_work(&args[0], rt).map_or(16384, |w| {
            w.output
                .update_estimate()
                .saturating_mul(rewind::numeric::SPARSE_CHUNK.div_ceil(256) + 2)
                .saturating_add(rewind::numeric::SPARSE_CHUNK * 8 + 16384)
        })
    } else if matches!(name, "SparseDone" | "SparseResult") {
        16384
    } else if name == "FftInit" {
        131072
    } else if name == "FftStep" {
        fft_work(&args[0], rt).map_or(16384, |work| {
            work.real
                .update_estimate()
                .saturating_add(work.imag.update_estimate())
                .saturating_mul((rewind::numeric::FFT_CHUNK * 2).div_ceil(256) + 2)
                .saturating_add(rewind::numeric::FFT_CHUNK * 32 + 16384)
        })
    } else if matches!(name, "FftDone" | "FftResult") {
        16384
    } else if name == "SgdStep" {
        Array::storage_estimate(length(0)).saturating_add(length(0).saturating_mul(8))
    } else if name == "AdamStep" {
        Array::storage_estimate(length(0))
            .saturating_mul(3)
            .saturating_add(length(0).saturating_mul(24))
    } else if name == "ReshapeLogical" {
        a(0).map_or(0, Array::reshape_logical_scratch)
    } else if name == "ModelEncode" {
        model_parameters(&args[0], rt)
            .and_then(|p| rewind::numeric::model_size(&p))
            .unwrap_or(0)
            .saturating_add(32768)
    } else if name == "ModelDecode" {
        match args.first() {
            Some(Value::Bytes(b)) if b.len() <= rewind::numeric::MAX_MODEL_BYTES => {
                b.len().saturating_mul(3).saturating_add(32768)
            }
            _ => 0,
        }
    } else if name == "ModelCheck" {
        8192
    } else if matches!(name, "TensorKey" | "CheckFinite") {
        1024
    } else if name == "SumToShape" {
        args.get(1)
            .and_then(|v| indices(v, rt).ok())
            .and_then(|s| elements(&s).ok())
            .map_or(0, |n| {
                Array::storage_estimate(n).saturating_add(n.saturating_mul(16))
            })
    } else if matches!(name, "Affine" | "Activation") {
        let n = length(if name == "Activation" { 1 } else { 0 });
        Array::storage_estimate(n).saturating_add(n.saturating_mul(8))
    } else if name == "Norm2" {
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
    } else if name.starts_with("From") {
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
            "RangeInt" => array_value(Array::integer_range(
                integer(&args[0])?,
                integer(&args[1])?,
                i(2)?,
            ))?,
            "GraphAdjacency" => {
                let (heads, links) = rewind::numeric::graph_adjacency(i(0)?, a(1)?, a(2)?)?;
                Value::Struct(
                    "Tuple<IntArray,IntArray>".into(),
                    BTreeMap::from([
                        ("_0".into(), Value::NumericArray(heads)),
                        ("_1".into(), Value::NumericArray(links)),
                    ]),
                )
            }
            "GraphBfs" => array_value(rewind::numeric::graph_bfs(
                a(0)?,
                a(1)?,
                a(2)?,
                i(3)?,
                i(4)?,
            ))?,
            "LeastSquaresInit" => least_squares_work_value(rewind::numeric::LeastSquaresWork::new(
                a(0)?,
                a(1)?,
                f(2)?,
            )?),
            "LeastSquaresStep" => {
                least_squares_work_value(least_squares_work(&args[0], rt)?.step()?)
            }
            "LeastSquaresDone" => Value::Bool(least_squares_work(&args[0], rt)?.done()),
            "LeastSquaresResult" => array_value(least_squares_work(&args[0], rt)?.result())?,
            "EigenInit" => eigen_work_value(rewind::numeric::EigenWork::new(a(0)?, f(1)?, i(2)?)?),
            "EigenStep" => eigen_work_value(eigen_work(&args[0], rt)?.step()?),
            "EigenDone" => Value::Bool(eigen_work(&args[0], rt)?.done()),
            "EigenResult" => {
                let w = eigen_work(&args[0], rt)?.result()?;
                Value::Struct(
                    "EigenResult".into(),
                    BTreeMap::from([
                        ("values".into(), Value::NumericArray(w.values)),
                        ("vectors".into(), Value::NumericArray(w.vectors)),
                        ("sweeps".into(), Value::Int(w.sweeps as i64)),
                    ]),
                )
            }
            "QrInit" => qr_work_value(rewind::numeric::QrWork::new(a(0)?, f(1)?)?),
            "QrStep" => qr_work_value(qr_work(&args[0], rt)?.step()?),
            "QrDone" => Value::Bool(qr_work(&args[0], rt)?.done()),
            "QrResult" => {
                let (q, r, permutation, rank) = qr_work(&args[0], rt)?.result()?;
                Value::Struct(
                    "QrArrayResult".into(),
                    BTreeMap::from([
                        ("q".into(), Value::NumericArray(q)),
                        ("r".into(), Value::NumericArray(r)),
                        ("permutation".into(), Value::NumericArray(permutation)),
                        ("rank".into(), Value::Int(rank as i64)),
                    ]),
                )
            }
            "SolveInit" => solve_work_value(rewind::numeric::SolveWork::new(a(0)?, a(1)?, f(2)?)?),
            "SolveStep" => solve_work_value(solve_work(&args[0], rt)?.step()?),
            "SolveDone" => Value::Bool(solve_work(&args[0], rt)?.done()),
            "SolveResult" => Value::NumericArray(solve_work(&args[0], rt)?.result()?),
            "SparseInit" => sparse_work_value(rewind::numeric::SparseWork::new(
                i(0)?,
                i(1)?,
                a(2)?,
                a(3)?,
                a(4)?,
                a(5)?,
            )?),
            "SparseStep" => sparse_work_value(sparse_work(&args[0], rt)?.step()?),
            "SparseDone" => Value::Bool(sparse_work(&args[0], rt)?.done()),
            "SparseResult" => Value::NumericArray(sparse_work(&args[0], rt)?.result()?),
            "FftInit" => {
                let Some(Value::Bool(inverse)) = args.get(2) else {
                    return Err(NumericError::Type);
                };
                fft_work_value(rewind::numeric::FftWork::new(a(0)?, a(1)?, *inverse)?)
            }
            "FftStep" => fft_work_value(fft_work(&args[0], rt)?.step()?),
            "FftDone" => Value::Bool(fft_work(&args[0], rt)?.done()),
            "FftResult" => {
                let (real, imag) = fft_work(&args[0], rt)?.result()?;
                Value::Struct(
                    "Tuple<FloatArray,FloatArray>".into(),
                    BTreeMap::from([
                        ("_0".into(), Value::NumericArray(real)),
                        ("_1".into(), Value::NumericArray(imag)),
                    ]),
                )
            }
            "SgdStep" => array_value(a(0)?.sgd_step(a(1)?, f(2)?))?,
            "AdamStep" => {
                let (w, m, v) = a(0)?.adam_step(
                    a(1)?,
                    a(2)?,
                    a(3)?,
                    f(4)?,
                    f(5)?,
                    f(6)?,
                    f(7)?,
                    f(8)?,
                    f(9)?,
                )?;
                Value::Struct(
                    "Tuple<FloatArray,FloatArray,FloatArray>".into(),
                    BTreeMap::from([
                        ("_0".into(), Value::NumericArray(w)),
                        ("_1".into(), Value::NumericArray(m)),
                        ("_2".into(), Value::NumericArray(v)),
                    ]),
                )
            }
            "ReshapeLogical" => array_value(a(0)?.reshape_logical(ids(1)?))?,
            "ModelCheck" => {
                let parameters = model_parameters(&args[0], rt)?;
                rewind::numeric::model_size(&parameters)?;
                for (_, array) in parameters {
                    array.check_finite()?;
                }
                Value::Null
            }
            "ModelEncode" => Value::Bytes(
                rewind::numeric::encode_model(&model_parameters(&args[0], rt)?)?.into(),
            ),
            "ModelDecode" => {
                let Value::Bytes(bytes) = &args[0] else {
                    return Err(NumericError::Type);
                };
                let decoded = rewind::numeric::decode_model(bytes)?;
                let mut map = rewind::map_storage::PersistentMap::new();
                for (name, array) in decoded {
                    map.set(rewind::MapKey::Text(name), Value::NumericArray(array));
                }
                Value::TypedMap("String".into(), "FloatArray".into(), map)
            }
            "Affine" => array_value(a(0)?.affine(f(1)?, f(2)?))?,
            "Activation" => array_value(a(1)?.activation(operation(&args[0])?))?,
            "SumToShape" => array_value(a(0)?.sum_to_shape(ids(1)?))?,
            "CheckFinite" => {
                a(0)?.check_finite()?;
                Value::Null
            }
            "TensorKey" => {
                let (Value::Bytes(left), Value::Bytes(right)) = (&args[2], &args[3]) else {
                    return Err(NumericError::Type);
                };
                Value::Bytes(
                    a(4)?
                        .tensor_key(operation(&args[0])?, integer(&args[1])?, left, right)?
                        .to_vec()
                        .into(),
                )
            }
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
            "LengthFloat" | "LengthInt" => Value::Int(a(0)?.len() as i64),
            "GetFlatFloat" => Value::Float(a(0)?.float_flat(i(1)?)?.to_bits()),
            "GetFlatInt" => Value::Int(a(0)?.integer_flat(i(1)?)?),
            "WithFlatFloat" | "WithFlatInt" => {
                let mut array = a(0)?.clone();
                if name == "WithFlatFloat" {
                    array.set_float_flat(i(1)?, f(2)?)?;
                } else {
                    array.set_integer_flat(i(1)?, integer(&args[2])?)?;
                }
                Value::NumericArray(array)
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
            "VectorInit" => {
                array_value(a(1)?.vector_init(a(2)?, vector_operation(&args[0], &args[3])?))?
            }
            "VectorStep" => {
                let (out, cursor) = a(1)?.vector_step(
                    a(2)?,
                    a(3)?,
                    vector_operation(&args[0], &args[4])?,
                    usize_arg(&args[5])?,
                )?;
                Value::Struct(
                    "Tuple<FloatArray,Int,Bool>".into(),
                    BTreeMap::from([
                        ("_0".into(), Value::NumericArray(out)),
                        ("_1".into(), Value::Int(cursor as i64)),
                        ("_2".into(), Value::Bool(cursor == a(1)?.len())),
                    ]),
                )
            }
            "NormStep" => {
                let state = a(0)?.norm_step(rewind::numeric::NormProgress {
                    cursor: usize_arg(&args[1])?,
                    scale: f(2)?,
                    sum: f(3)?,
                })?;
                let done = state.cursor == a(0)?.len();
                let value = if done { state.value()? } else { 0.0 };
                Value::Struct(
                    "Tuple<Int,Float,Float,Float,Bool>".into(),
                    BTreeMap::from([
                        ("_0".into(), Value::Int(state.cursor as i64)),
                        ("_1".into(), Value::Float(state.scale.to_bits())),
                        ("_2".into(), Value::Float(state.sum.to_bits())),
                        ("_3".into(), Value::Float(value.to_bits())),
                        ("_4".into(), Value::Bool(done)),
                    ]),
                )
            }
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
        "stdNumericLeastSquaresInit" => (
            &["&FloatArray", "&FloatArray", "Float"],
            "Result<LeastSquaresWork,StdError>",
        ),
        "stdNumericLeastSquaresStep" => {
            (&["&LeastSquaresWork"], "Result<LeastSquaresWork,StdError>")
        }
        "stdNumericLeastSquaresDone" => (&["&LeastSquaresWork"], "Result<Bool,StdError>"),
        "stdNumericLeastSquaresResult" => (&["&LeastSquaresWork"], "Result<FloatArray,StdError>"),
        "stdNumericEigenInit" => (
            &["&FloatArray", "Float", "Int"],
            "Result<EigenWork,StdError>",
        ),
        "stdNumericEigenStep" => (&["&EigenWork"], "Result<EigenWork,StdError>"),
        "stdNumericEigenDone" => (&["&EigenWork"], "Result<Bool,StdError>"),
        "stdNumericEigenResult" => (&["&EigenWork"], "Result<EigenResult,StdError>"),
        "stdNumericQrInit" => (&["&FloatArray", "Float"], "Result<QrWork,StdError>"),
        "stdNumericQrStep" => (&["&QrWork"], "Result<QrWork,StdError>"),
        "stdNumericQrDone" => (&["&QrWork"], "Result<Bool,StdError>"),
        "stdNumericQrResult" => (&["&QrWork"], "Result<QrArrayResult,StdError>"),
        "stdNumericSolveInit" => (
            &["&FloatArray", "&FloatArray", "Float"],
            "Result<SolveWork,StdError>",
        ),
        "stdNumericSolveStep" => (&["&SolveWork"], "Result<SolveWork,StdError>"),
        "stdNumericSolveDone" => (&["&SolveWork"], "Result<Bool,StdError>"),
        "stdNumericSolveResult" => (&["&SolveWork"], "Result<FloatArray,StdError>"),
        "stdNumericSparseInit" => (
            &[
                "Int",
                "Int",
                "&IntArray",
                "&IntArray",
                "&FloatArray",
                "&FloatArray",
            ],
            "Result<SparseWork,StdError>",
        ),
        "stdNumericSparseStep" => (&["&SparseWork"], "Result<SparseWork,StdError>"),
        "stdNumericSparseDone" => (&["&SparseWork"], "Result<Bool,StdError>"),
        "stdNumericSparseResult" => (&["&SparseWork"], "Result<FloatArray,StdError>"),
        "stdNumericFftInit" => (
            &["&FloatArray", "&FloatArray", "Bool"],
            "Result<FftWork,StdError>",
        ),
        "stdNumericFftStep" => (&["&FftWork"], "Result<FftWork,StdError>"),
        "stdNumericFftDone" => (&["&FftWork"], "Result<Bool,StdError>"),
        "stdNumericFftResult" => (
            &["&FftWork"],
            "Result<Tuple<FloatArray,FloatArray>,StdError>",
        ),
        "stdNumericSgdStep" => (
            &["&FloatArray", "&FloatArray", "Float"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericAdamStep" => (
            &[
                "&FloatArray",
                "&FloatArray",
                "&FloatArray",
                "&FloatArray",
                "Float",
                "Float",
                "Float",
                "Float",
                "Float",
                "Float",
            ],
            "Result<Tuple<FloatArray,FloatArray,FloatArray>,StdError>",
        ),
        "stdNumericReshapeLogical" => (
            &["&FloatArray", "&List<Int>"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericModelCheck" => (&["&Map<String,FloatArray>"], "Result<Unit,StdError>"),
        "stdNumericModelEncode" => (&["&Map<String,FloatArray>"], "Result<Bytes,StdError>"),
        "stdNumericModelDecode" => (&["Bytes"], "Result<Map<String,FloatArray>,StdError>"),
        "stdNumericAffine" => (
            &["&FloatArray", "Float", "Float"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericActivation" => (&["String", "&FloatArray"], "Result<FloatArray,StdError>"),
        "stdNumericSumToShape" => (
            &["&FloatArray", "&List<Int>"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericCheckFinite" => (&["&FloatArray"], "Result<Unit,StdError>"),
        "stdNumericTensorKey" => (
            &["String", "Int", "Bytes", "Bytes", "&FloatArray"],
            "Result<Bytes,StdError>",
        ),
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
        "stdNumericRangeInt" => (&["Int", "Int", "Int"], "Result<IntArray,StdError>"),
        "stdNumericGraphAdjacency" => (
            &["Int", "&IntArray", "&IntArray"],
            "Result<Tuple<IntArray,IntArray>,StdError>",
        ),
        "stdNumericGraphBfs" => (
            &["&IntArray", "&IntArray", "&IntArray", "Int", "Int"],
            "Result<IntArray,StdError>",
        ),
        "stdNumericLengthFloat" => (&["&FloatArray"], "Result<Int,StdError>"),
        "stdNumericLengthInt" => (&["&IntArray"], "Result<Int,StdError>"),
        "stdNumericGetFlatFloat" => (&["&FloatArray", "Int"], "Result<Float,StdError>"),
        "stdNumericGetFlatInt" => (&["&IntArray", "Int"], "Result<Int,StdError>"),
        "stdNumericWithFlatFloat" => (
            &["&FloatArray", "Int", "Float"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericWithFlatInt" => (&["&IntArray", "Int", "Int"], "Result<IntArray,StdError>"),
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
        "stdNumericVectorInit" => (
            &["String", "&FloatArray", "&FloatArray", "Float"],
            "Result<FloatArray,StdError>",
        ),
        "stdNumericVectorStep" => (
            &[
                "String",
                "&FloatArray",
                "&FloatArray",
                "&FloatArray",
                "Float",
                "Int",
            ],
            "Result<Tuple<FloatArray,Int,Bool>,StdError>",
        ),
        "stdNumericNormStep" => (
            &["&FloatArray", "Int", "Float", "Float"],
            "Result<Tuple<Int,Float,Float,Float,Bool>,StdError>",
        ),
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
