//! Small deterministic primitives for the source standard library.
use super::*;
mod bigint;
mod csv_stream;
mod datetime;
mod decimal;
mod gui_scene;
mod json_stream;
mod numeric;
mod regex;
mod sdk;
mod unicode;
pub(super) use sdk::modules as std_modules;
pub(super) use sdk::{build as sdk_build, install as sdk_install, verify as sdk_verify};
const LIMIT: usize = 1024 * 1024;
const ITEMS: usize = 65_536;
pub(super) fn native_owned_result(name: &str) -> bool {
    (name.starts_with("stdNumeric")
        || name.starts_with("stdBigInt")
        || name.starts_with("stdDecimal")
        || name.starts_with("stdRegex")
        || name.starts_with("stdCsvStream")
        || name.starts_with("stdJsonStream")
        || name.starts_with("stdUnicode")
        || name.starts_with("stdDateTime")
        || name == "stdExternalInstant")
        && names().contains(&name)
}
pub(super) fn native_parameter(name: &str, index: usize) -> Option<&'static str> {
    gui_scene::parameter(name, index)
        .or_else(|| numeric::parameter(name, index))
        .or_else(|| bigint::parameter(name, index))
        .or_else(|| decimal::parameter(name, index))
        .or_else(|| datetime::parameter(name, index))
        .or_else(|| unicode::parameter(name, index))
        .or_else(|| regex::parameter(name, index))
        .or_else(|| csv_stream::parameter(name, index))
        .or_else(|| json_stream::parameter(name, index))
}
pub(super) fn native_borrow(name: &str) -> Option<&'static str> {
    match name {
        "stdExternalHttpServerNext" | "stdExternalHttpServerClose" => Some("&mut HttpServer"),
        "stdExternalHttpServerRespond" | "stdExternalHttpServerCloseRequest" => {
            Some("&mut HttpServerRequest")
        }
        "stdExternalTcpRead"
        | "stdExternalTcpWrite"
        | "stdExternalTcpShutdownWrite"
        | "stdExternalTcpClose" => Some("&mut TcpSocket"),
        "stdExternalHttpRead" | "stdExternalHttpClose" => Some("&mut HttpDownload"),
        "stdExternalHttpWrite" | "stdExternalHttpFinish" | "stdExternalHttpCloseUpload" => {
            Some("&mut HttpUpload")
        }
        "stdExternalDbExecute"
        | "stdExternalDbExecuteMany"
        | "stdExternalDbPrepare"
        | "stdExternalDbQuery"
        | "stdExternalDbBegin"
        | "stdExternalDbCommit"
        | "stdExternalDbRollback"
        | "stdExternalDbClose" => Some("&mut DbConnection"),
        "stdExternalDbNext" | "stdExternalDbCloseCursor" => Some("&mut DbCursor"),
        "stdExternalDbExecuteStatement"
        | "stdExternalDbQueryStatement"
        | "stdExternalDbCloseStatement" => Some("&mut DbStatement"),
        _ => None,
    }
}
pub(super) fn names() -> &'static [&'static str] {
    &[
        "stdJsonStreamNew",
        "stdJsonStreamFeed",
        "stdJsonStreamPeek",
        "stdJsonStreamAdvance",
        "stdJsonStreamCancel",
        "stdJsonStreamPosition",
        "stdCsvStreamNew",
        "stdCsvStreamFeed",
        "stdCsvStreamPeek",
        "stdCsvStreamAdvance",
        "stdCsvStreamCancel",
        "stdCsvStreamPosition",
        "stdRegexCompile",
        "stdRegexFindText",
        "stdRegexFindBytes",
        "stdRegexNames",
        "stdRegexSource",
        "stdRegexTextSlice",
        "stdRegexOptions",
        "stdRegexIsText",
        "stdUnicodeNormalize",
        "stdUnicodeIsNormalized",
        "stdUnicodeCount",
        "stdUnicodeSlice",
        "stdUnicodeSplit",
        "stdUnicodeOffsets",
        "stdUnicodeCase",
        "stdUnicodeVersions",
        "stdExternalInstant",
        "stdDateTimeInstant",
        "stdDateTimeDuration",
        "stdDateTimeParse",
        "stdDateTimeFormat",
        "stdDateTimeFormatOffset",
        "stdDateTimeSeconds",
        "stdDateTimeNanos",
        "stdDateTimeDurationSeconds",
        "stdDateTimeDurationNanos",
        "stdDateTimeAdd",
        "stdDateTimeDifference",
        "stdDateTimeDurationAdd",
        "stdDateTimeDurationSubtract",
        "stdDateTimeDurationNegate",
        "stdDateTimeCalendar",
        "stdDateTimeCalendarOffset",
        "stdDateTimeResolve",
        "stdDateTimeResolveOffset",
        "stdDateTimeDatabaseVersion",
        "stdDecimalParse",
        "stdDecimalFormat",
        "stdDecimalRepresentation",
        "stdDecimalFromCoefficient",
        "stdDecimalCoefficient",
        "stdDecimalScale",
        "stdDecimalCompare",
        "stdDecimalBinary",
        "stdDecimalQuantize",
        "stdDecimalDivide",
        "stdBigIntParse",
        "stdBigIntFromInt",
        "stdBigIntToInt",
        "stdBigIntFormat",
        "stdBigIntBinary",
        "stdBigIntUnary",
        "stdBigIntShift",
        "stdBigIntPow",
        "stdBigIntModPow",
        "stdBigIntCompare",
        "stdBigIntBits",
        "stdNumericSgdStep",
        "stdNumericAdamStep",
        "stdNumericReshapeLogical",
        "stdNumericModelEncode",
        "stdNumericModelDecode",
        "stdNumericModelCheck",
        "stdNumericAffine",
        "stdNumericActivation",
        "stdNumericSumToShape",
        "stdNumericCheckFinite",
        "stdNumericTensorKey",
        "stdNumericSparseInit",
        "stdNumericEigenInit",
        "stdNumericEigenStep",
        "stdNumericEigenDone",
        "stdNumericEigenResult",
        "stdNumericLeastSquaresInit",
        "stdNumericLeastSquaresStep",
        "stdNumericLeastSquaresDone",
        "stdNumericLeastSquaresResult",
        "stdNumericQrInit",
        "stdNumericQrStep",
        "stdNumericQrDone",
        "stdNumericQrResult",
        "stdNumericSolveInit",
        "stdNumericSolveStep",
        "stdNumericSolveDone",
        "stdNumericSolveResult",
        "stdNumericSparseStep",
        "stdNumericSparseDone",
        "stdNumericSparseResult",
        "stdNumericFftInit",
        "stdNumericFftStep",
        "stdNumericFftDone",
        "stdNumericFftResult",
        "stdNumericFft",
        "stdNumericConvolve",
        "stdNumericCsr",
        "stdNumericSparseMatvec",
        "stdNumericScale",
        "stdNumericNorm2",
        "stdNumericSuffixArray",
        "stdNumericSuffixSearch",
        "stdNumericQr",
        "stdNumericLeastSquares",
        "stdNumericEigenSymmetric",
        "stdNumericCovariance",
        "stdNumericCorrelation",
        "stdNumericQuantile",
        "stdNumericHistogram",
        "stdNumericZerosFloat",
        "stdNumericFromFloat",
        "stdNumericShapeFloat",
        "stdNumericStridesFloat",
        "stdNumericRangeInt",
        "stdNumericGraphAdjacency",
        "stdNumericGraphBfs",
        "stdNumericGetFlatFloat",
        "stdNumericGetFlatInt",
        "stdNumericWithFlatFloat",
        "stdNumericWithFlatInt",
        "stdNumericLengthFloat",
        "stdNumericLengthInt",
        "stdNumericGetFloat",
        "stdNumericWithFloat",
        "stdNumericReshapeFloat",
        "stdNumericTransposeFloat",
        "stdNumericBroadcastFloat",
        "stdNumericSliceFloat",
        "stdNumericMaterializeFloat",
        "stdNumericValuesFloat",
        "stdNumericZipFloat",
        "stdNumericZerosInt",
        "stdNumericFromInt",
        "stdNumericShapeInt",
        "stdNumericStridesInt",
        "stdNumericGetInt",
        "stdNumericWithInt",
        "stdNumericReshapeInt",
        "stdNumericTransposeInt",
        "stdNumericBroadcastInt",
        "stdNumericSliceInt",
        "stdNumericMaterializeInt",
        "stdNumericValuesInt",
        "stdNumericZipInt",
        "stdNumericMapFloat",
        "stdNumericSum",
        "stdNumericMean",
        "stdNumericVariance",
        "stdNumericDot",
        "stdNumericDotStep",
        "stdNumericVectorInit",
        "stdNumericVectorStep",
        "stdNumericNormStep",
        "stdTaskYieldNow",
        "stdNumericMatmulInit",
        "stdNumericMatmulStep",
        "stdNumericMatmul",
        "stdNumericSolve",
        "stdNumericMath",
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
        "stdBytesFromList",
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
        "stdHttpComponent",
        "stdExternalClock",
        "stdExternalDbSqlite",
        "stdExternalDbPostgres",
        "stdExternalDbCredentials",
        "stdExternalDbPrivateParameter",
        "stdExternalDbCleanup",
        "stdExternalDbPrepare",
        "stdExternalDbExecuteStatement",
        "stdExternalDbQueryStatement",
        "stdExternalDbCloseStatement",
        "stdExternalDbExecute",
        "stdExternalDbExecuteMany",
        "stdExternalDbQuery",
        "stdExternalDbNext",
        "stdExternalDbCloseCursor",
        "stdExternalDbBegin",
        "stdExternalDbCommit",
        "stdExternalDbRollback",
        "stdExternalDbClose",
        "stdExternalHttpServerListen",
        "stdExternalHttpServerListenTls",
        "stdExternalHttpServerTlsCredential",
        "stdExternalHttpServerBearerCredential",
        "stdExternalHttpServerListenTlsAuthenticated",
        "stdExternalHttpServerNext",
        "stdExternalHttpServerRespond",
        "stdExternalHttpServerClose",
        "stdExternalHttpServerCloseRequest",
        "stdExternalTcpTls",
        "stdExternalTcpConnect",
        "stdExternalTcpRead",
        "stdExternalTcpWrite",
        "stdExternalTcpShutdownWrite",
        "stdExternalTcpClose",
        "stdExternalHttpUpload",
        "stdExternalHttpWrite",
        "stdExternalHttpFinish",
        "stdExternalHttpCloseUpload",
        "stdExternalHttpDownload",
        "stdExternalHttpRead",
        "stdExternalHttpClose",
        "stdExternalHttpStart",
        "stdExternalHttpConfigured",
        "stdExternalHttpAuthenticated",
        "stdExternalHttpCredential",
        "stdGuiScene",
        "stdGuiStage",
        "stdGuiPollEvent",
        "stdGuiWindowStage",
        "stdGuiWindowClose",
        "stdGuiWindowPoll",
        "stdGuiWindowNext",
        "stdGuiWindowPollAny",
        "stdGuiWindowNextAny",
        "stdGuiWindowNextAnyAsync",
        "stdGuiWindowNextAnyLiveAsync",
        "stdGuiWindowContinueInput",
        "stdGuiEdit",
        "stdGuiEditGrapheme",
        "stdGuiNextEvent",
        "stdGuiClose",
        "stdGuiContinueInput",
    ]
}
pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if !matches!(
        p.language.as_str(),
        "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
            | "1.1.0"
            | "1.2.0"
            | "1.3.0"
            | "1.4.0"
            | "1.5.0"
            | "1.6.0"
            | "1.6.1"
            | "1.7.0"
            | "1.7.1"
            | "1.8.0"
            | "1.8.1"
            | "1.8.2"
            | "1.8.3"
            | "1.8.4"
            | "1.8.5"
            | "1.8.6"
            | "1.8.7"
            | "1.8.8"
            | "1.9.0"
            | "1.9.1"
            | "1.9.2"
            | "1.9.3"
            | "1.9.4"
            | "1.9.5"
            | "1.9.6"
            | "1.9.7"
            | "1.9.8"
            | "1.9.9"
            | "1.9.10"
            | "1.9.11"
            | "1.9.12"
            | "1.9.13"
            | "1.9.14"
            | "1.9.15"
            | "1.9.16"
            | "1.9.17"
            | "1.9.18"
            | "1.9.19"
            | "1.9.20"
            | "1.9.21"
            | "1.9.22"
            | "1.9.23"
            | "1.9.24"
            | "1.9.25"
            | "1.9.26"
            | "1.9.27"
            | "1.9.28"
            | "1.9.29"
            | "1.9.30"
            | "1.9.31"
            | "1.9.32"
            | "1.9.33"
            | "1.9.34"
            | "1.9.35"
            | "1.9.36"
            | "1.9.37"
            | "1.9.38"
            | "1.9.39"
            | "1.9.40"
            | "1.9.41"
            | "1.9.42"
            | "1.9.43"
            | "1.9.44"
            | "1.9.45"
            | "1.9.46"
            | "1.9.47"
            | "1.9.48"
            | "1.9.49"
            | "1.9.50"
            | "1.9.51"
            | "1.9.52"
            | "2.0.0"
    ) {
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
    numeric::prepare(p)?;
    bigint::prepare(p)?;
    decimal::prepare(p)?;
    datetime::prepare(p)?;
    regex::prepare(p)?;
    csv_stream::prepare(p)?;
    json_stream::prepare(p)?;
    p.structs.insert(
        "StdError".into(),
        StructDef {
            private_fields: BTreeSet::new(),
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
    for (name, fields) in [
        ("HttpUpload", vec![("maxBytes", "Int")]),
        (
            "HttpDownload",
            vec![("status", "Int"), ("headers", "Frozen<List<HttpHeader>>")],
        ),
        ("HttpHeader", vec![("name", "String"), ("value", "Bytes")]),
        (
            "HttpResponse",
            vec![
                ("status", "Int"),
                ("headers", "Frozen<List<HttpHeader>>"),
                ("body", "Bytes"),
            ],
        ),
        (
            "HttpError",
            vec![("code", "String"), ("phase", "String"), ("status", "Int")],
        ),
    ] {
        if p.structs.contains_key(name) || p.enums.contains_key(name) {
            return Err(Error::InvalidOperation("reserved HTTP type".into()));
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
                    .map(|(a, b)| (a.into(), b.into()))
                    .collect(),
            },
        );
    }
    if language_at_least(&p.language, "1.9.25") {
        for (name, fields) in [
            (
                "HttpServer",
                vec![("address", "String"), ("port", "Int"), ("maxBytes", "Int")],
            ),
            (
                "HttpServerRequest",
                vec![
                    ("method", "String"),
                    ("target", "String"),
                    ("path", "String"),
                    ("query", "String"),
                    ("headers", "Frozen<List<HttpHeader>>"),
                    ("body", "Bytes"),
                ],
            ),
            (
                "HttpServerError",
                vec![("code", "String"), ("phase", "String"), ("status", "Int")],
            ),
        ] {
            if p.structs.contains_key(name) || p.enums.contains_key(name) {
                return Err(Error::InvalidOperation("reserved HTTP server type".into()));
            }
            p.structs.insert(
                name.into(),
                StructDef {
                    private_fields: if rewind::native_resources::resource_type(name) {
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
    if language_at_least(&p.language, "1.9.23") {
        for (name, fields) in [
            ("TcpSocket", vec![("peer", "String")]),
            (
                "TcpError",
                vec![
                    ("code", "String"),
                    ("phase", "String"),
                    ("acceptedBytes", "Int"),
                ],
            ),
        ] {
            if p.structs.contains_key(name) || p.enums.contains_key(name) {
                return Err(Error::InvalidOperation("reserved TCP type".into()));
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
    if language_at_least(&p.language, "1.7.0") {
        for (name, fields) in [
            ("DbConnection", vec![("backend", "String")]),
            (
                "DbStatement",
                vec![("columns", "Frozen<List<String>>"), ("parameters", "Int")],
            ),
            ("DbCursor", vec![("columns", "Frozen<List<String>>")]),
            ("DbRow", vec![("values", "Frozen<List<DbValue>>")]),
            (
                "DbBatch",
                vec![("rows", "Frozen<List<DbRow>>"), ("done", "Bool")],
            ),
            (
                "DbError",
                vec![
                    ("code", "String"),
                    ("phase", "String"),
                    ("sqlCode", "Option<Int>"),
                    ("sqlState", "Option<String>"),
                ],
            ),
        ] {
            if p.structs.contains_key(name) || p.enums.contains_key(name) {
                return Err(Error::InvalidOperation("reserved database type".into()));
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
        if p.structs.contains_key("DbValue") || p.enums.contains_key("DbValue") {
            return Err(Error::InvalidOperation("reserved database type".into()));
        }
        p.enums.insert(
            "DbValue".into(),
            EnumDef {
                type_params: vec![],
                public: true,
                origin: p.root_origin.clone(),
                variants: [
                    ("Null", None),
                    ("Bool", Some("Bool")),
                    ("Int", Some("Int")),
                    ("Float", Some("Float")),
                    ("Text", Some("String")),
                    ("Bytes", Some("Bytes")),
                    ("Private", Some("String")),
                ]
                .into_iter()
                .map(|(n, t)| {
                    (
                        n.into(),
                        t.map(|t| vec![("0".into(), t.into())]).unwrap_or_default(),
                    )
                })
                .collect(),
            },
        );
    }
    Ok(())
}
pub(super) fn work(name: &str, args: &[Value], runtime: &Runtime) -> Option<usize> {
    if let Some(work) = gui_scene::work(name, args, runtime) {
        return Some(work);
    }
    if let Some(work) = json_stream::work(name, args, runtime) {
        return Some(work);
    }
    if let Some(work) = csv_stream::work(name, args, runtime) {
        return Some(work);
    }
    if let Some(work) = regex::work(name, args, runtime) {
        return Some(work);
    }
    if let Some(work) = unicode::work(name, args, runtime) {
        return Some(work);
    }
    if let Some(work) = datetime::work(name, args, runtime) {
        return Some(work);
    }
    if let Some(work) = decimal::work(name, args, runtime) {
        return Some(work);
    }
    if let Some(work) = bigint::work(name, args, runtime) {
        return Some(work);
    }
    if let Some(work) = numeric::work(name, args, runtime) {
        return Some(work);
    }
    if !names().contains(&name) {
        return None;
    }
    let length = |value: &Value| match value {
        Value::Text(v) => v.len(),
        Value::Bytes(v) => v.len(),
        Value::HeapRef(id) => match runtime.heap_get(*id) {
            Some(Value::TypedList(_, v)) => v.len(),
            _ => 0,
        },
        _ => 0,
    };
    let work = match name {
        "stdBytesLength" | "stdBytesGet" | "stdBitAnd" | "stdBitOr" | "stdBitXor" | "stdBitNot"
        | "stdCountBits" | "stdShiftLeft" | "stdShiftRight" | "stdShiftUnsigned" => 1,
        "stdBytesSlice" => match args {
            [Value::Bytes(bytes), Value::Int(a), Value::Int(b)]
                if *a >= 0 && *b >= *a && (*b as u64) <= bytes.len() as u64 =>
            {
                (*b - *a) as usize
            }
            _ => 1,
        },
        "stdMulMod" => 64,
        "stdFormatInt" | "stdFormatFloat" => 128,
        _ => args.iter().map(length).fold(1usize, usize::saturating_add),
    };
    Some(work.max(1))
}
pub(super) fn call_type(p: &Program, n: &str, args: &[String], at: &Tok) -> Result<Option<String>> {
    if !matches!(
        p.language.as_str(),
        "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
            | "1.1.0"
            | "1.2.0"
            | "1.3.0"
            | "1.4.0"
            | "1.5.0"
            | "1.6.0"
            | "1.6.1"
            | "1.7.0"
            | "1.7.1"
            | "1.8.0"
            | "1.8.1"
            | "1.8.2"
            | "1.8.3"
            | "1.8.4"
            | "1.8.5"
            | "1.8.6"
            | "1.8.7"
            | "1.8.8"
            | "1.9.0"
            | "1.9.1"
            | "1.9.2"
            | "1.9.3"
            | "1.9.4"
            | "1.9.5"
            | "1.9.6"
            | "1.9.7"
            | "1.9.8"
            | "1.9.9"
            | "1.9.10"
            | "1.9.11"
            | "1.9.12"
            | "1.9.13"
            | "1.9.14"
            | "1.9.15"
            | "1.9.16"
            | "1.9.17"
            | "1.9.18"
            | "1.9.19"
            | "1.9.20"
            | "1.9.21"
            | "1.9.22"
            | "1.9.23"
            | "1.9.24"
            | "1.9.25"
            | "1.9.26"
            | "1.9.27"
            | "1.9.28"
            | "1.9.29"
            | "1.9.30"
            | "1.9.31"
            | "1.9.32"
            | "1.9.33"
            | "1.9.34"
            | "1.9.35"
            | "1.9.36"
            | "1.9.37"
            | "1.9.38"
            | "1.9.39"
            | "1.9.40"
            | "1.9.41"
            | "1.9.42"
            | "1.9.43"
            | "1.9.44"
            | "1.9.45"
            | "1.9.46"
            | "1.9.47"
            | "1.9.48"
            | "1.9.49"
            | "1.9.50"
            | "1.9.51"
            | "1.9.52"
            | "2.0.0"
    ) || !names().contains(&n)
    {
        return Ok(None);
    }
    if let Some(result) = json_stream::call_type(p, n, args, at)? {
        return Ok(Some(result));
    }
    if let Some(result) = csv_stream::call_type(p, n, args, at)? {
        return Ok(Some(result));
    }
    if let Some(result) = regex::call_type(p, n, args, at)? {
        return Ok(Some(result));
    }
    if let Some(result) = unicode::call_type(p, n, args, at)? {
        return Ok(Some(result));
    }
    if let Some(result) = datetime::call_type(p, n, args, at)? {
        return Ok(Some(result));
    }
    if let Some(result) = decimal::call_type(p, n, args, at)? {
        return Ok(Some(result));
    }
    if let Some(result) = bigint::call_type(p, n, args, at)? {
        return Ok(Some(result));
    }
    if let Some(result) = numeric::call_type(p, n, args, at)? {
        return Ok(Some(result));
    }
    if n == "stdBytesFromList"
        && !matches!(
            p.language.as_str(),
            "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "1.9.49"
                | "1.9.50"
                | "1.9.51"
                | "1.9.52"
                | "2.0.0"
        )
    {
        return Ok(None);
    }
    if n.starts_with("stdExternal") && !language_at_least(&p.language, "1.5.0") {
        return Err(diagnostic(at, "external operations require language 1.5.0"));
    }
    if n.starts_with("stdExternalHttpServer") && !language_at_least(&p.language, "1.9.25") {
        return Err(diagnostic(at, "HTTP server requires language 1.9.25"));
    }
    if matches!(
        n,
        "stdExternalHttpServerListenTls" | "stdExternalHttpServerTlsCredential"
    ) && !language_at_least(&p.language, "1.9.29")
    {
        return Err(diagnostic(at, "HTTP server TLS requires language 1.9.29"));
    }
    if matches!(
        n,
        "stdExternalHttpServerBearerCredential" | "stdExternalHttpServerListenTlsAuthenticated"
    ) && !language_at_least(&p.language, "1.9.30")
    {
        return Err(diagnostic(
            at,
            "HTTP server authentication requires language 1.9.30",
        ));
    }
    if n == "stdExternalTcpTls" && !language_at_least(&p.language, "1.9.24") {
        return Err(diagnostic(at, "TCP TLS requires language 1.9.24"));
    }
    if n.starts_with("stdExternalTcp") && !language_at_least(&p.language, "1.9.23") {
        return Err(diagnostic(at, "TCP requires language 1.9.23"));
    }
    if n.starts_with("stdExternalHttp") && !language_at_least(&p.language, "1.6.0") {
        return Err(diagnostic(at, "HTTP requires language 1.6.0"));
    }
    if matches!(n, "stdExternalDbPostgres" | "stdExternalDbCredentials")
        && !language_at_least(&p.language, "1.7.1")
    {
        return Err(diagnostic(
            at,
            "PostgreSQL primitives require language 1.7.1",
        ));
    }
    if n.starts_with("stdExternalDb") && !language_at_least(&p.language, "1.7.0") {
        return Err(diagnostic(at, "database primitives require language 1.7.0"));
    }
    if n == "stdGuiScene" && !language_at_least(&p.language, "1.9.13") {
        return Err(diagnostic(
            at,
            "GUI scene serialization requires language 1.9.13",
        ));
    }
    if matches!(
        n,
        "stdGuiWindowNextAnyAsync" | "stdGuiWindowNextAnyLiveAsync"
    ) {
        if n == "stdGuiWindowNextAnyLiveAsync" && !language_at_least(&p.language, "1.9.22") {
            return Err(diagnostic(at, "live GUI input requires language 1.9.22"));
        }
        if !language_at_least(&p.language, "1.9.19") {
            return Err(diagnostic(
                at,
                "asynchronous GUI input requires language 1.9.19",
            ));
        }
        let Some((params, ret)) = args.first().and_then(|ty| function_signature(ty)) else {
            return Err(diagnostic(at, "GUI wait requires a pure event decoder"));
        };
        if args.len() != 1
            || params != ["String"]
            || !ret.starts_with("Result<")
            || !ret.ends_with(",StdError>")
            || !v06::type_effects(&args[0]).is_empty()
        {
            return Err(diagnostic(
                at,
                "GUI wait requires fn(String)->Result<T,StdError> effects {}",
            ));
        }
        return Ok(Some(format!("Task<{ret}>")));
    }
    if n == "stdTaskYieldNow" && !language_at_least(&p.language, "1.9.10") {
        return Err(diagnostic(at, "task yield requires language 1.9.10"));
    }
    if n.starts_with("stdGuiWindow") && !language_at_least(&p.language, "1.9.9") {
        return Err(diagnostic(
            at,
            "named GUI primitives require language 1.9.9",
        ));
    }
    if n == "stdGuiEditGrapheme" && !language_at_least(&p.language, "1.9.7") {
        return Err(diagnostic(at, "grapheme editing requires language 1.9.7"));
    }
    if n.starts_with("stdGui") && !language_at_least(&p.language, "1.3.0") {
        return Err(diagnostic(at, "GUI primitives require language 1.3.0"));
    }
    if matches!(n, "stdGuiEdit" | "stdGuiPollEvent") && !language_at_least(&p.language, "1.4.0") {
        return Err(diagnostic(
            at,
            "GUI editing and polling require language 1.4.0",
        ));
    }
    if matches!(
        n,
        "stdExternalHttpDownload"
            | "stdExternalHttpRead"
            | "stdExternalHttpClose"
            | "stdExternalHttpUpload"
            | "stdExternalHttpWrite"
            | "stdExternalHttpFinish"
            | "stdExternalHttpCloseUpload"
    ) && !language_at_least(&p.language, "1.6.1")
    {
        return Err(diagnostic(at, "HTTP streaming requires language 1.6.1"));
    }
    let (params, ret): (&[&str], &str) = match n {
        "stdExternalHttpServerBearerCredential" => (
            &["String", "Secret<String>"],
            "Result<Unit,HttpServerError>",
        ),
        "stdExternalHttpServerListenTlsAuthenticated" => (
            &[
                "String", "String", "String", "Int", "Int", "Int", "Int", "Int",
            ],
            "Task<Result<HttpServer,HttpServerError>>",
        ),
        "stdExternalHttpServerTlsCredential" => (
            &["String", "Bytes", "Secret<String>"],
            "Result<Unit,HttpServerError>",
        ),
        "stdExternalHttpServerListenTls" => (
            &["String", "String", "Int", "Int", "Int", "Int", "Int"],
            "Task<Result<HttpServer,HttpServerError>>",
        ),
        "stdExternalHttpServerListen" => (
            &["String", "Int", "Int", "Int", "Int", "Int"],
            "Task<Result<HttpServer,HttpServerError>>",
        ),
        "stdExternalHttpServerNext" => (
            &["&mut HttpServer", "Int"],
            "Task<Result<HttpServerRequest,HttpServerError>>",
        ),
        "stdExternalHttpServerRespond" => (
            &[
                "&mut HttpServerRequest",
                "Int",
                "Frozen<List<HttpHeader>>",
                "Bytes",
                "Int",
            ],
            "Task<Result<Unit,HttpServerError>>",
        ),
        "stdExternalHttpServerClose" => (
            &["&mut HttpServer", "Int"],
            "Task<Result<Unit,HttpServerError>>",
        ),
        "stdExternalHttpServerCloseRequest" => (
            &["&mut HttpServerRequest", "Int"],
            "Task<Result<Unit,HttpServerError>>",
        ),
        "stdExternalTcpTls" => (
            &["String", "Int", "Bytes", "Int"],
            "Task<Result<TcpSocket,TcpError>>",
        ),
        "stdExternalTcpConnect" => (
            &["String", "Int", "Int"],
            "Task<Result<TcpSocket,TcpError>>",
        ),
        "stdExternalTcpRead" => (
            &["&mut TcpSocket", "Int", "Int"],
            "Task<Result<Option<Bytes>,TcpError>>",
        ),
        "stdExternalTcpWrite" => (
            &["&mut TcpSocket", "Bytes", "Int"],
            "Task<Result<Int,TcpError>>",
        ),
        "stdExternalTcpShutdownWrite" | "stdExternalTcpClose" => {
            (&["&mut TcpSocket", "Int"], "Task<Result<Unit,TcpError>>")
        }
        "stdExternalDbExecuteMany" => (
            &[
                "&mut DbConnection",
                "String",
                "Frozen<List<Frozen<List<DbValue>>>>",
                "Int",
            ],
            "Task<Result<Int,DbError>>",
        ),
        "stdExternalDbCredentials" => (
            &["String", "Secret<String>", "Bytes"],
            "Result<Unit,DbError>",
        ),
        "stdExternalDbPostgres" => (&["String", "Int"], "Task<Result<DbConnection,DbError>>"),
        "stdExternalDbCleanup" => (&["Int"], "Task<Result<Unit,DbError>>"),
        "stdExternalDbPrivateParameter" => {
            (&["String", "Secret<DbValue>"], "Result<DbValue,DbError>")
        }
        "stdExternalDbSqlite" => (
            &["String", "Bool", "Int"],
            "Task<Result<DbConnection,DbError>>",
        ),
        "stdExternalDbPrepare" => (
            &["&mut DbConnection", "String", "Int"],
            "Task<Result<DbStatement,DbError>>",
        ),
        "stdExternalDbExecuteStatement" => (
            &["&mut DbStatement", "Frozen<List<DbValue>>", "Int"],
            "Task<Result<Int,DbError>>",
        ),
        "stdExternalDbQueryStatement" => (
            &["&mut DbStatement", "Frozen<List<DbValue>>", "Int"],
            "Task<Result<DbCursor,DbError>>",
        ),
        "stdExternalDbCloseStatement" => {
            (&["&mut DbStatement", "Int"], "Task<Result<Unit,DbError>>")
        }
        "stdExternalDbExecute" => (
            &[
                "&mut DbConnection",
                "String",
                "Frozen<List<DbValue>>",
                "Int",
            ],
            "Task<Result<Int,DbError>>",
        ),
        "stdExternalDbQuery" => (
            &[
                "&mut DbConnection",
                "String",
                "Frozen<List<DbValue>>",
                "Int",
            ],
            "Task<Result<DbCursor,DbError>>",
        ),
        "stdExternalDbNext" => (
            &["&mut DbCursor", "Int", "Int", "Int"],
            "Task<Result<DbBatch,DbError>>",
        ),
        "stdExternalDbCloseCursor" => (&["&mut DbCursor", "Int"], "Task<Result<Unit,DbError>>"),
        "stdExternalDbBegin"
        | "stdExternalDbCommit"
        | "stdExternalDbRollback"
        | "stdExternalDbClose" => (&["&mut DbConnection", "Int"], "Task<Result<Unit,DbError>>"),
        "stdHttpComponent" => (&["String"], "Result<String,StdError>"),
        "stdExternalClock" => (&[], "Result<Int,StdError>"),
        "stdExternalHttpUpload" => (
            &[
                "String",
                "String",
                "Bytes",
                "Int",
                "Int",
                "Frozen<List<HttpHeader>>",
                "Bytes",
                "String",
                "Int",
            ],
            "Task<Result<HttpUpload,HttpError>>",
        ),
        "stdExternalHttpWrite" => (
            &["&mut HttpUpload", "Bytes"],
            "Task<Result<Unit,HttpError>>",
        ),
        "stdExternalHttpFinish" => (&["&mut HttpUpload"], "Task<Result<HttpResponse,HttpError>>"),
        "stdExternalHttpCloseUpload" => (&["&mut HttpUpload"], "Task<Result<Unit,HttpError>>"),
        "stdExternalHttpDownload" => (
            &[
                "String",
                "String",
                "Bytes",
                "Int",
                "Int",
                "Frozen<List<HttpHeader>>",
                "Bytes",
                "String",
            ],
            "Task<Result<HttpDownload,HttpError>>",
        ),
        "stdExternalHttpRead" => (
            &["&mut HttpDownload", "Int"],
            "Task<Result<Option<Bytes>,HttpError>>",
        ),
        "stdExternalHttpClose" => (&["&mut HttpDownload"], "Task<Result<Unit,HttpError>>"),
        "stdExternalHttpCredential" => (&["String", "Secret<String>"], "Result<Unit,StdError>"),
        "stdExternalHttpAuthenticated" => (
            &[
                "String",
                "String",
                "Bytes",
                "Int",
                "Int",
                "Frozen<List<HttpHeader>>",
                "Bytes",
                "String",
            ],
            "Task<Result<HttpResponse,HttpError>>",
        ),
        "stdExternalHttpConfigured" => (
            &[
                "String",
                "String",
                "Bytes",
                "Int",
                "Int",
                "Frozen<List<HttpHeader>>",
                "Bytes",
            ],
            "Task<Result<HttpResponse,HttpError>>",
        ),
        "stdExternalHttpStart" => (
            &["String", "String", "Bytes", "Int", "Int"],
            "Task<Result<HttpResponse,HttpError>>",
        ),
        "stdTaskYieldNow" => (&[], "Unit"),
        "stdGuiScene" => (
            &[
                "String",
                "Int",
                "Int",
                "Int",
                "&List<Unknown>",
                "Int",
                "&Map<String,Int>",
            ],
            "Result<String,StdError>",
        ),
        "stdGuiWindowStage" => (&["String", "String"], "Result<Unit,StdError>"),
        "stdGuiWindowClose" => (&["String"], "Result<Unit,StdError>"),
        "stdGuiWindowPoll" => (&["String"], "Result<Option<String>,StdError>"),
        "stdGuiWindowNext" => (&["String"], "Result<String,StdError>"),
        "stdGuiWindowPollAny" => (&[], "Result<Option<String>,StdError>"),
        "stdGuiWindowNextAny" => (&[], "Result<String,StdError>"),

        "stdGuiWindowContinueInput" => (&[], "Unit"),
        "stdGuiEdit" | "stdGuiEditGrapheme" => (
            &["String", "Int", "Int", "String", "String", "Bool"],
            "Result<String,StdError>",
        ),
        "stdGuiPollEvent" => (&[], "Result<Option<String>,StdError>"),
        "stdGuiStage" => (&["String"], "Result<Unit,StdError>"),
        "stdGuiNextEvent" => (&[], "Result<String,StdError>"),
        "stdGuiClose" => (&[], "Result<Unit,StdError>"),
        "stdGuiContinueInput" => (&[], "Unit"),
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
        "stdBytesFromList" => (&["&List<Int>"], "Result<Bytes,StdError>"),
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
        values.push(Value::Text(s.into()));
    }
    Ok(Value::TypedList("String".into(), values.into()))
}
pub(super) fn call(n: &str, args: &[Value], runtime: &mut Runtime) -> Result<Option<Value>> {
    if let Some(value) = gui_scene::call(n, args, runtime)? {
        return Ok(Some(value));
    }
    if let Some(value) = json_stream::call(n, args, runtime)? {
        return Ok(Some(value));
    }
    if let Some(value) = csv_stream::call(n, args, runtime)? {
        return Ok(Some(value));
    }
    if let Some(value) = regex::call(n, args, runtime)? {
        return Ok(Some(value));
    }
    if let Some(value) = unicode::call(n, args, runtime)? {
        return Ok(Some(value));
    }
    if let Some(value) = datetime::call(n, args, runtime)? {
        return Ok(Some(value));
    }
    if let Some(value) = decimal::call(n, args, runtime)? {
        return Ok(Some(value));
    }
    if let Some(value) = bigint::call(n, args, runtime)? {
        return Ok(Some(value));
    }
    if let Some(value) = numeric::call(n, args, runtime)? {
        return Ok(Some(value));
    }
    if n == "stdExternalHttpServerBearerCredential" {
        let [Value::Text(alias), secret] = args else {
            return Err(Error::InvalidOperation(
                "invalid HTTP server bearer arguments".into(),
            ));
        };
        let Some(Value::Text(token)) = v05::unsecret(secret) else {
            return Err(Error::InvalidOperation(
                "HTTP server bearer token requires Secret<String>".into(),
            ));
        };
        let result = runtime.register_http_server_bearer(alias, token)?;
        return Ok(Some(Value::Result(
            result.map(|_| Box::new(Value::Null)).map_err(|code| {
                Box::new(Value::Struct(
                    "HttpServerError".into(),
                    BTreeMap::from([
                        ("code".into(), Value::Text(code.into())),
                        ("phase".into(), Value::Text("NotSent".into())),
                        ("status".into(), Value::Int(0)),
                    ]),
                ))
            }),
        )));
    }
    if n == "stdExternalHttpServerTlsCredential" {
        let [Value::Text(alias), Value::Bytes(certificate), secret] = args else {
            return Err(Error::InvalidOperation(
                "invalid HTTP server TLS credential arguments".into(),
            ));
        };
        let Some(Value::Text(key)) = v05::unsecret(secret) else {
            return Err(Error::InvalidOperation(
                "HTTP server key requires Secret<String>".into(),
            ));
        };
        let result = runtime.register_http_server_tls(alias, certificate, key.as_bytes())?;
        return Ok(Some(Value::Result(
            result.map(|_| Box::new(Value::Null)).map_err(|code| {
                Box::new(Value::Struct(
                    "HttpServerError".into(),
                    BTreeMap::from([
                        ("code".into(), Value::Text(code.into())),
                        ("phase".into(), Value::Text("NotSent".into())),
                        ("status".into(), Value::Int(0)),
                    ]),
                ))
            }),
        )));
    }
    if n == "stdExternalDbCredentials" {
        let [Value::Text(alias), secret, Value::Bytes(certificate)] = args else {
            return Err(Error::InvalidOperation(
                "invalid DB credentials arguments".into(),
            ));
        };
        let Some(Value::Text(dsn)) = v05::unsecret(secret) else {
            return Err(Error::InvalidOperation(
                "DB connection string requires Secret<String>".into(),
            ));
        };
        let result = runtime.register_postgres_credentials(alias, dsn, certificate)?;
        return Ok(Some(Value::Result(
            result.map(|_| Box::new(Value::Null)).map_err(|code| {
                Box::new(Value::Struct(
                    "DbError".into(),
                    BTreeMap::from([
                        ("code".into(), Value::Text(code.into())),
                        ("phase".into(), Value::Text("NotSent".into())),
                        ("sqlCode".into(), Value::Option(None)),
                        ("sqlState".into(), Value::Option(None)),
                    ]),
                ))
            }),
        )));
    }
    if n == "stdExternalDbPrivateParameter" {
        use rewind::database::Parameter;
        let [Value::Text(alias), secret] = args else {
            return Err(Error::InvalidOperation(
                "invalid DB private parameter arguments".into(),
            ));
        };
        let Some(Value::Enum(ty, variant, fields)) = v05::unsecret(secret) else {
            return Err(Error::InvalidOperation(
                "DB parameter requires Secret<DbValue>".into(),
            ));
        };
        if ty != "DbValue" {
            return Err(Error::InvalidOperation("invalid DB value type".into()));
        }
        let value = match (variant.as_str(), fields.first().map(|(_, v)| v)) {
            ("Null", None) => Parameter::Null,
            ("Bool", Some(Value::Bool(v))) => Parameter::Bool(*v),
            ("Int", Some(Value::Int(v))) => Parameter::Int(*v),
            ("Float", Some(Value::Float(v))) => Parameter::Float(f64::from_bits(*v)),
            ("Text", Some(Value::Text(v))) => Parameter::Text(v.to_string()),
            ("Bytes", Some(Value::Bytes(v))) => Parameter::Bytes(v.as_ref().clone()),
            _ => return Err(Error::InvalidOperation("invalid private DB value".into())),
        };
        let result = runtime.register_database_parameter(alias, value)?;
        let result = result
            .map(|_| {
                Box::new(Value::Enum(
                    "DbValue".into(),
                    "Private".into(),
                    vec![("0".into(), Value::Text(alias.clone()))],
                ))
            })
            .map_err(|code| {
                Box::new(Value::Struct(
                    "DbError".into(),
                    BTreeMap::from([
                        ("code".into(), Value::Text(code.into())),
                        ("phase".into(), Value::Text("NotSent".into())),
                        ("sqlCode".into(), Value::Option(None)),
                        ("sqlState".into(), Value::Option(None)),
                    ]),
                ))
            });
        return Ok(Some(Value::Result(result)));
    }
    if n == "stdHttpComponent" {
        let [Value::Text(value)] = args else {
            return Err(Error::InvalidOperation("invalid HTTP component".into()));
        };
        if value.len() > LIMIT / 3 {
            return Ok(Some(outcome(Err(("Limit", 0)))));
        }
        let mut output = String::with_capacity(value.len() * 3);
        const HEX: &[u8] = b"0123456789ABCDEF";
        for byte in value.bytes() {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                output.push(byte as char);
            } else {
                output.push('%');
                output.push(HEX[(byte >> 4) as usize] as char);
                output.push(HEX[(byte & 15) as usize] as char);
            }
        }
        return Ok(Some(outcome(Ok(Value::Text(output.into())))));
    }
    if n == "stdExternalHttpCredential" {
        let [Value::Text(alias), secret] = args else {
            return Err(Error::InvalidOperation(
                "invalid credential arguments".into(),
            ));
        };
        let Some(Value::Text(value)) = v05::unsecret(secret) else {
            return Err(Error::InvalidOperation(
                "credential requires Secret<String>".into(),
            ));
        };
        let result = runtime.register_http_credential(alias, value)?;
        return Ok(Some(outcome(
            result.map(|_| Value::Null).map_err(|e| (e, 0)),
        )));
    }
    if n == "stdExternalClock" && args.is_empty() {
        let result = runtime.external_operation("clock.millis", b"", 128, || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|t| serde_json::json!(t.as_millis() as u64))
                .map_err(|_| "ClockBeforeEpoch".into())
        })?;
        return Ok(Some(match result {
            Ok(v) => outcome(v.as_i64().map(Value::Int).ok_or(("ClockRange", 0))),
            Err(_) => outcome(Err(("ExternalFailure", 0))),
        }));
    }
    if n == "stdGuiContinueInput" && args.is_empty() {
        runtime.gui_continue_input();
        return Ok(Some(Value::Null));
    }
    if n == "stdGuiWindowContinueInput" && args.is_empty() {
        runtime.gui_window_continue_input();
        return Ok(Some(Value::Null));
    }
    if n.starts_with("stdGui") {
        fn gui_string<T: serde::Serialize>(event: T) -> Result<Value> {
            serde_json::to_string(&event)
                .map(|text| Value::Text(text.into()))
                .map_err(|e| Error::InvalidOperation(e.to_string()))
        }
        fn gui_optional<T: serde::Serialize>(event: Option<T>) -> Result<Value> {
            match event {
                None => Ok(Value::Option(None)),
                Some(e) => gui_string(e).map(|v| Value::Option(Some(Box::new(v)))),
            }
        }
        let result: Result<Value> = match (n, args) {
            ("stdGuiWindowStage",[Value::Text(id),Value::Text(scene)])=>{
                if scene.len()>LIMIT{Err(Error::InvalidOperation("GuiSceneLimit".into()))}else{serde_json::from_str::<rewind::gui::Frame>(scene).map_err(|_|Error::InvalidOperation("GuiInvalidScene".into())).and_then(|f|runtime.gui_window_stage(id,Some(f))).map(|_|Value::Null)}
            },
            ("stdGuiWindowClose",[Value::Text(id)])=>runtime.gui_window_stage(id,None).map(|_|Value::Null),
            ("stdGuiWindowPoll",[Value::Text(id)])=>runtime.gui_window_poll(id).and_then(gui_optional),
            ("stdGuiWindowNext",[Value::Text(id)])=>runtime.gui_window_next_event(id).and_then(gui_string),
            ("stdGuiWindowPollAny",[])=>runtime.gui_window_poll_any().and_then(gui_optional),
            ("stdGuiWindowNextAny",[])=>runtime.gui_window_next_any().and_then(gui_string),
            (
                "stdGuiEdit" | "stdGuiEditGrapheme",
                [Value::Text(text), Value::Int(cursor), Value::Int(anchor), Value::Text(key), Value::Text(typed), Value::Bool(multiline)],
            ) => (if n == "stdGuiEditGrapheme" { rewind::gui::edit::apply_grapheme } else { rewind::gui::edit::apply })(text, *cursor, *anchor, key, typed, *multiline)
                .map_err(|e| Error::InvalidOperation(e.into()))
                .and_then(|v| {
                    serde_json::to_string(&serde_json::json!({"text":v.text,"cursor":v.cursor,"anchor":v.anchor,"line":v.text.chars().take(v.cursor).filter(|c|*c=='\n').count()}))
                        .map(|text| Value::Text(text.into()))
                        .map_err(|e| Error::InvalidOperation(e.to_string()))
                }),
            ("stdGuiPollEvent", []) => runtime.gui_poll_event().and_then(|e| match e {
                None => Ok(Value::Option(None)),
                Some(e) => serde_json::to_string(&e)
                    .map(|s| Value::Option(Some(Box::new(Value::Text(s.into())))))
                    .map_err(|e| Error::InvalidOperation(e.to_string())),
            }),
            ("stdGuiStage", [Value::Text(s)]) => {
                if s.len() > LIMIT {
                    Err(Error::InvalidOperation("GuiSceneLimit".into()))
                } else {
                    serde_json::from_str::<rewind::gui::Frame>(s)
                        .map_err(|_| Error::InvalidOperation("GuiInvalidScene".into()))
                        .and_then(|f| runtime.gui_stage(Some(f)))
                        .map(|_| Value::Null)
                }
            }
            ("stdGuiClose", []) => runtime.gui_stage(None).map(|_| Value::Null),
            ("stdGuiNextEvent", []) => runtime.gui_next_event().and_then(|event| {
                serde_json::to_string(&event)
                    .map(|text| Value::Text(text.into()))
                    .map_err(|e| Error::InvalidOperation(e.to_string()))
            }),
            _ => {
                return Err(Error::InvalidOperation(
                    "invalid GUI primitive arguments".into(),
                ))
            }
        };
        return Ok(Some(match result {
            Ok(v) => Value::Result(Ok(Box::new(v))),
            Err(e) => {
                if matches!(e, Error::HistoryBudgetExceeded)
                    || matches!(&e,Error::InvalidOperation(s) if s.starts_with("ReplayMismatch:"))
                {
                    return Err(e);
                }
                let message = match &e {
                    Error::InvalidOperation(s) => s.clone(),
                    _ => e.to_string(),
                };
                let prefix = message.split(':').next().unwrap_or("GuiBackendFailure");
                let code = if prefix.starts_with("Gui") {
                    prefix
                } else {
                    "GuiBackendFailure"
                };
                Value::Result(Err(Box::new(err(code, 0))))
            }
        }));
    }
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
                        // Locate only the two scalar boundaries; avoid a full O(n)
                        // offset table and stop as soon as the requested end is found.
                        let mut begin = None;
                        let mut finish = None;
                        for (index, byte) in s
                            .char_indices()
                            .map(|(i, _)| i)
                            .chain(std::iter::once(s.len()))
                            .enumerate()
                        {
                            if index == start as usize {
                                begin = Some(byte);
                            }
                            if index == end as usize {
                                finish = Some(byte);
                                break;
                            }
                        }
                        match (begin, finish) {
                            (Some(a), Some(b)) => Ok(Value::Text(s[a..b].into())),
                            _ => Err(("Range", 0)),
                        }
                    }
                }
                "stdEncode" => {
                    let s = text(0)?;
                    if s.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        Ok(Value::Bytes(s.as_bytes().to_vec().into()))
                    }
                }
                "stdBytesFromList" => {
                    let target = match args.first() {
                        Some(Value::HeapRef(id)) => runtime.heap_get(*id),
                        other => other,
                    };
                    match target {
                        Some(Value::TypedList(_, values)) if values.len() <= ITEMS => {
                            let mut output = Vec::with_capacity(values.len());
                            let mut failure = None;
                            for (index, value) in values.iter().enumerate() {
                                match value {
                                    Value::Int(v) if (0..=255).contains(v) => output.push(*v as u8),
                                    _ => {
                                        failure = Some(("ByteRange", index));
                                        break;
                                    }
                                }
                            }
                            match failure {
                                Some(e) => Err(e),
                                None => Ok(Value::Bytes(output.into())),
                            }
                        }
                        Some(Value::TypedList(_, _)) => Err(("Limit", 0)),
                        _ => {
                            return Err(Error::InvalidOperation(
                                "expected borrowed List<Int>".into(),
                            ))
                        }
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
                        Ok(Value::Bytes(
                            b[start as usize..end as usize].to_vec().into(),
                        ))
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
                        Ok(Value::Text(n.to_string().into()))
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
                        Ok(Value::Text(String::from_utf8(digits).unwrap().into()))
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
