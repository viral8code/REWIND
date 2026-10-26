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
        || name == "stdExternalInstant"
        || name == "stdExternalGuiDialog")
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
        .or_else(|| (name == "stdBytesFromInts" && index == 0).then_some("&List<Int>"))
        .or_else(|| (name == "stdTextFromCodePoints" && index == 0).then_some("&List<Int>"))
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
        "stdExternalGuiDialog",
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
        "stdNumericFiniteStep",
        "stdNumericIdentityInit",
        "stdNumericIdentityStep",
        "stdNumericIdentityDone",
        "stdNumericIdentityKey",
        "stdNumericIdentityValue",
        "stdNumericSparseInit",
        "stdNumericEigenInit",
        "stdNumericEigenStep",
        "stdNumericEigenDone",
        "stdNumericEigenResult",
        "stdNumericReshapeLogicalInit",
        "stdNumericReshapeLogicalStep",
        "stdNumericSumShapeInit",
        "stdNumericSumShapeStep",
        "stdNumericGraphBfsInit",
        "stdNumericGraphBfsStep",
        "stdNumericGraphBfsDone",
        "stdNumericGraphBfsResult",
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
        "stdNumericFromFloatInit",
        "stdNumericFromFloatStep",
        "stdNumericFromIntInit",
        "stdNumericFromIntStep",
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
        "stdNumericSumStep",
        "stdNumericMomentsStep",
        "stdNumericVectorInit",
        "stdNumericUnaryInit",
        "stdNumericUnaryStep",
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
        "stdTextCodePoints",
        "stdTextFromCodePoints",
        "stdExternalLineSeparator",
        "stdDecode",
        "stdEncode",
        "stdBytesGet",
        "stdBytesSlice",
        "stdBytesLength",
        "stdBytesFromList",
        "stdDeflate",
        "stdInflate",
        "stdZlibCompress",
        "stdZlibDecompress",
        "stdCrc32",
        "stdAdler32",
        "stdSha256",
        "stdSha512",
        "stdHmacSha256",
        "stdPbkdf2Sha256",
        "stdConstantTimeEq",
        "stdBytesToHex",
        "stdBytesFromHex",
        "stdBytesConcat",
        "stdBytesFromInts",
        "stdBytesToInts",
        "stdBytesIndexOf",
        "stdBytesEqual",
        "stdBytesReadInt",
        "stdPngDecode",
        "stdPngEncode",
        "stdGzipCompress",
        "stdGzipDecompress",
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
        "stdBitLeadingZeros",
        "stdBitTrailingZeros",
        "stdBitRotateLeft",
        "stdBitRotateRight",
        "stdBitReverse",
        "stdBitByteSwap",
        "stdBitExtract",
        "stdBitDeposit",
        "stdMulMod",
        "stdHttpComponent",
        "stdExternalClock",
        "stdExternalRandomBytes",
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
            | "1.9.53"
            | "1.9.54"
            | "1.9.55"
            | "1.9.56"
            | "1.9.57"
            | "1.9.58"
            | "1.9.59"
            | "1.9.60"
            | "1.9.61"
            | "1.9.62"
            | "1.9.63"
            | "1.9.64"
            | "1.9.65"
            | "2.0.0"
            | "3.0.0"
            | "3.1.0"
            | "3.2.0"
            | "3.3.0"
            | "3.4.0"
            | "3.5.0"
            | "3.6.0"
            | "4.0.0"
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
        | "stdCountBits" | "stdShiftLeft" | "stdShiftRight" | "stdShiftUnsigned"
        | "stdBitLeadingZeros" | "stdBitTrailingZeros" | "stdBitRotateLeft" | "stdBitRotateRight"
        | "stdBitReverse" | "stdBitByteSwap" | "stdBitExtract" | "stdBitDeposit" => 1,
        // 先頭から一致を探すので、最悪ケースは入力長 × 針の長さになる。
        "stdBytesIndexOf" => match args {
            [Value::Bytes(data), Value::Bytes(needle), ..] => data
                .len()
                .saturating_mul(needle.len().max(1))
                .saturating_add(16),
            _ => 16,
        },
        "stdBytesReadInt" => 16,
        "stdBytesSlice" => match args {
            [Value::Bytes(bytes), Value::Int(a), Value::Int(b)]
                if *a >= 0 && *b >= *a && (*b as u64) <= bytes.len() as u64 =>
            {
                (*b - *a) as usize
            }
            _ => 1,
        },
        "stdMulMod" => 64,
        // Compression is a few tens of MB/s: charge 8 units per input byte. Decompression is
        // bounded by the caller's output limit and by the maximum expansion, charged up front.
        "stdDeflate" | "stdZlibCompress" | "stdGzipCompress" => match args {
            [Value::Bytes(bytes), _] => bytes.len().saturating_mul(8).saturating_add(256),
            _ => 256,
        },
        "stdInflate" | "stdZlibDecompress" | "stdGzipDecompress" => match args {
            // DEFLATE cannot expand more than 1032:1, so the output (and the work to produce it)
            // is bounded by both the caller's limit and the input size.
            [Value::Bytes(bytes), Value::Int(max)] => bytes
                .len()
                .saturating_mul(4)
                .saturating_add(
                    ((*max).clamp(0, LIMIT as i64) as usize).min(bytes.len().saturating_mul(1032)),
                )
                .saturating_add(256),
            _ => 256,
        },
        "stdCrc32" | "stdAdler32" => match args {
            [Value::Bytes(bytes)] => bytes.len().saturating_mul(2).saturating_add(16),
            _ => 16,
        },
        "stdSha256" | "stdSha512" | "stdBytesToHex" => match args {
            [Value::Bytes(bytes)] => bytes.len().saturating_mul(4).saturating_add(256),
            _ => 256,
        },
        "stdHmacSha256" => match args {
            [Value::Bytes(key), Value::Bytes(message)] => key
                .len()
                .saturating_add(message.len())
                .saturating_mul(4)
                .saturating_add(256),
            _ => 256,
        },
        // PBKDF2 は反復ごとに HMAC を回すので、コストは反復回数に比例させる。
        // 範囲外の引数は実行前に Range で返すので、仕事量を見積もらず 256 とする。
        "stdPbkdf2Sha256" => match args {
            [Value::Bytes(_), Value::Bytes(salt), Value::Int(iterations), Value::Int(length)]
                if (1..=1_000_000).contains(iterations)
                    && (1..=1024).contains(length)
                    && (1..=1024).contains(&salt.len()) =>
            {
                (*iterations as usize)
                    .saturating_mul(64)
                    .saturating_add((*length as usize).saturating_mul(4))
                    .saturating_add(256)
            }
            _ => 256,
        },
        "stdConstantTimeEq" => match args {
            [Value::Bytes(a), Value::Bytes(b)] => a.len().min(b.len()).saturating_mul(2).saturating_add(16),
            _ => 16,
        },
        // 乱数は要求された長さに比例して作る。範囲外の引数は実行前に Range で返すので、仕事量を見積もらず 256 とする。
        "stdExternalRandomBytes" => match args {
            [Value::Int(length)] if (1..=65_536).contains(length) => {
                (*length as usize).saturating_mul(4).saturating_add(256)
            }
            _ => 256,
        },
        "stdBytesFromHex" => match args {
            [Value::Text(text)] => text.len().saturating_mul(2).saturating_add(16),
            _ => 16,
        },
        // Decoding is bounded by the pixel limit the caller passes and by the input size (a PNG
        // cannot inflate to more than ~1032 times its data); encoding filters five ways per row.
        "stdPngDecode" => match args {
            [Value::Bytes(bytes), Value::Int(max)] => bytes
                .len()
                .saturating_mul(4)
                .saturating_add(
                    ((*max).clamp(0, LIMIT as i64) as usize)
                        .min(bytes.len().saturating_mul(344))
                        .saturating_mul(8),
                )
                .saturating_add(256),
            _ => 256,
        },
        "stdPngEncode" => match args {
            [Value::Bytes(bytes), ..] => bytes.len().saturating_mul(24).saturating_add(256),
            _ => 256,
        },
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
            | "1.9.53"
            | "1.9.54"
            | "1.9.55"
            | "1.9.56"
            | "1.9.57"
            | "1.9.58"
            | "1.9.59"
            | "1.9.60"
            | "1.9.61"
            | "1.9.62"
            | "1.9.63"
            | "1.9.64"
            | "1.9.65"
            | "2.0.0"
            | "3.0.0"
            | "3.1.0"
            | "3.2.0"
            | "3.3.0"
            | "3.4.0"
            | "3.5.0"
            | "3.6.0"
            | "4.0.0"
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
                | "1.9.53"
                | "1.9.54"
                | "1.9.55"
                | "1.9.56"
                | "1.9.57"
                | "1.9.58"
                | "1.9.59"
                | "1.9.60"
                | "1.9.61"
                | "1.9.62"
                | "1.9.63"
                | "1.9.64"
                | "1.9.65"
                | "2.0.0"
                | "3.0.0"
                | "3.1.0"
                | "3.2.0"
                | "3.3.0"
                | "3.4.0"
                | "3.5.0"
                | "3.6.0"
                | "4.0.0"
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
    if n == "stdExternalGuiDialog" && !language_at_least(&p.language, "1.9.60") {
        return Err(diagnostic(
            at,
            "native file dialogs require language 1.9.60",
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
        "stdExternalGuiDialog" => (
            &["Bool", "String", "String"],
            "Task<Result<Option<String>,StdError>>",
        ),
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
        "stdExternalRandomBytes" => (&["Int"], "Result<Bytes,StdError>"),
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
        "stdTextCodePoints" => (&["String"], "List<Int>"),
        "stdTextFromCodePoints" => (&["&List<Int>"], "Result<String,StdError>"),
        "stdExternalLineSeparator" => (&[], "String"),
        "stdDecode" => (&["Bytes"], "Result<String,StdError>"),
        "stdEncode" => (&["String"], "Result<Bytes,StdError>"),
        "stdBytesGet" => (&["Bytes", "Int"], "Result<Int,StdError>"),
        "stdBytesSlice" => (&["Bytes", "Int", "Int"], "Result<Bytes,StdError>"),
        "stdBytesLength" => (&["Bytes"], "Int"),
        "stdBytesFromList" => (&["&List<Int>"], "Result<Bytes,StdError>"),
        "stdDeflate" | "stdZlibCompress" => (&["Bytes", "Int"], "Result<Bytes,StdError>"),
        "stdInflate" | "stdZlibDecompress" => (&["Bytes", "Int"], "Result<Bytes,StdError>"),
        "stdCrc32" | "stdAdler32" => (&["Bytes"], "Int"),
        "stdSha256" | "stdSha512" => (&["Bytes"], "Result<Bytes,StdError>"),
        "stdHmacSha256" => (&["Bytes", "Bytes"], "Result<Bytes,StdError>"),
        "stdPbkdf2Sha256" => (&["Bytes", "Bytes", "Int", "Int"], "Result<Bytes,StdError>"),
        "stdConstantTimeEq" => (&["Bytes", "Bytes"], "Bool"),
        "stdBytesToHex" => (&["Bytes"], "String"),
        "stdBytesFromHex" => (&["String"], "Result<Bytes,StdError>"),
        "stdPngDecode" => (&["Bytes", "Int"], "Result<Bytes,StdError>"),
        "stdGzipCompress" => (&["Bytes", "Int"], "Result<Bytes,StdError>"),
        "stdGzipDecompress" => (&["Bytes", "Int"], "Result<Bytes,StdError>"),
        "stdPngEncode" => (&["Bytes", "Int", "Int", "Int"], "Result<Bytes,StdError>"),
        "stdParseInt" => (&["String", "Int"], "Result<Int,StdError>"),
        "stdParseFloat" => (&["String"], "Result<Float,StdError>"),
        "stdFormatInt" => (&["Int", "Int"], "Result<String,StdError>"),
        "stdFormatFloat" => (&["Float"], "Result<String,StdError>"),
        "stdBitAnd" | "stdBitOr" | "stdBitXor" => (&["Int", "Int"], "Int"),
        "stdBitNot" | "stdCountBits" => (&["Int"], "Int"),
        "stdShiftLeft" | "stdShiftRight" | "stdShiftUnsigned" => {
            (&["Int", "Int"], "Result<Int,StdError>")
        }
        "stdBitLeadingZeros" | "stdBitTrailingZeros" | "stdBitReverse" | "stdBitByteSwap" => {
            (&["Int"], "Int")
        }
        "stdBitRotateLeft" | "stdBitRotateRight" => {
            (&["Int", "Int"], "Result<Int,StdError>")
        }
        "stdBitExtract" => (&["Int", "Int", "Int"], "Result<Int,StdError>"),
        "stdBitDeposit" => (&["Int", "Int", "Int", "Int"], "Result<Int,StdError>"),
        "stdBytesConcat" => (&["Bytes", "Bytes"], "Result<Bytes,StdError>"),
        "stdBytesFromInts" => (&["&List<Int>"], "Result<Bytes,StdError>"),
        "stdBytesToInts" => (&["Bytes"], "Result<List<Int>,StdError>"),
        "stdBytesIndexOf" => (&["Bytes", "Bytes", "Int"], "Result<Int,StdError>"),
        "stdBytesEqual" => (&["Bytes", "Bytes"], "Bool"),
        "stdBytesReadInt" => (&["Bytes", "Int", "Int", "Bool"], "Result<Int,StdError>"),
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
fn png_error(error: rewind::png::PngError) -> (&'static str, usize) {
    use rewind::png::PngError;
    match error {
        PngError::Range => ("Range", 0),
        PngError::Limit => ("Limit", 0),
        PngError::Unsupported => ("Unsupported", 0),
        PngError::Truncated(offset) => ("Truncated", offset),
        PngError::Corrupt(offset) => ("Corrupt", offset),
    }
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
    // 実行環境の改行。クロックや乱数と同じ外部の観測として記録し、replay は記録した値を返す。
    if n == "stdExternalLineSeparator" && args.is_empty() {
        let result = runtime.external_operation("env.lineSeparator", b"", 64, || {
            Ok(serde_json::json!(if cfg!(windows) { "\r\n" } else { "\n" }))
        })?;
        return match result {
            Ok(v) => match v.as_str() {
                Some(sep @ ("\n" | "\r\n")) => Ok(Some(Value::Text(sep.into()))),
                _ => Err(Error::InvalidOperation("ExternalFailure: line separator".into())),
            },
            Err(_) => Err(Error::InvalidOperation("ExternalFailure: line separator".into())),
        };
    }
    // 範囲外の長さは外部操作を記録せずに Range で返す。記録する値は 16 進文字列で、上限は長さの 2 倍と JSON の包みの分。
    if n == "stdExternalRandomBytes" && args.len() == 1 {
        if let Value::Int(length) = &args[0] {
            let length = *length;
            if !(1..=65_536).contains(&length) {
                return Ok(Some(outcome(Err(("Range", 0)))));
            }
            let request = length.to_string();
            let result = runtime.external_operation("entropy.bytes", request.as_bytes(), 131_136, || {
                let mut buffer = vec![0u8; length as usize];
                getrandom::getrandom(&mut buffer)
                    .map(|()| serde_json::json!(rewind::crypto::to_hex(&buffer)))
                    .map_err(|_| "EntropyFailure".into())
            })?;
            return Ok(Some(match result {
                Ok(v) => outcome(
                    v.as_str()
                        .and_then(|hex| rewind::crypto::from_hex(hex).ok())
                        .map(|out| Value::Bytes(out.into()))
                        .ok_or(("ExternalFailure", 0)),
                ),
                Err(_) => outcome(Err(("ExternalFailure", 0))),
            }));
        }
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
        // 戻り値が List なので、上限を超える入力は Err にできない。実行時エラーで止める（stdTextFind と同じ）。
        "stdTextCodePoints" => {
            let s = text(0)?;
            if s.len() > LIMIT {
                return Err(Error::InvalidOperation("text code point limit exceeded".into()));
            }
            let values: Vec<Value> = s.chars().map(|c| Value::Int(i64::from(u32::from(c)))).collect();
            Value::TypedList("Int".into(), values.into())
        }
        "stdBytesLength" => Value::Int(bytes(0)?.len() as i64),
        "stdCrc32" => {
            let mut crc = 0xffff_ffffu32;
            for byte in bytes(0)? {
                crc ^= u32::from(*byte);
                for _ in 0..8 {
                    crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
                }
            }
            Value::Int(i64::from(!crc))
        }
        "stdAdler32" => {
            let (mut a, mut c) = (1u32, 0u32);
            for chunk in bytes(0)?.chunks(5552) {
                for byte in chunk {
                    a += u32::from(*byte);
                    c += a;
                }
                a %= 65521;
                c %= 65521;
            }
            Value::Int(i64::from((c << 16) | a))
        }
        "stdConstantTimeEq" => Value::Bool(rewind::crypto::constant_time_eq(bytes(0)?, bytes(1)?)),
        "stdBytesToHex" => Value::Text(rewind::crypto::to_hex(bytes(0)?).into()),
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
        "stdBitLeadingZeros" => Value::Int(int(0)?.leading_zeros() as i64),
        "stdBitTrailingZeros" => Value::Int(int(0)?.trailing_zeros() as i64),
        "stdBitReverse" => Value::Int(int(0)?.reverse_bits()),
        "stdBitByteSwap" => Value::Int(int(0)?.swap_bytes()),
        "stdBytesEqual" => Value::Bool(bytes(0)? == bytes(1)?),
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
                    } else if s.is_ascii() {
                        // ASCII では文字位置とバイト位置が一致するので、走査せずに切り出す。
                        if (end as u128) <= s.len() as u128 {
                            Ok(Value::Text(s[start as usize..end as usize].into()))
                        } else {
                            Err(("Range", 0))
                        }
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
                "stdSha256" | "stdSha512" => {
                    let b = bytes(0)?;
                    if b.len() > LIMIT {
                        Err(("Limit", 0))
                    } else if n == "stdSha256" {
                        Ok(Value::Bytes(rewind::crypto::sha256(b).to_vec().into()))
                    } else {
                        Ok(Value::Bytes(rewind::crypto::sha512(b).to_vec().into()))
                    }
                }
                "stdHmacSha256" => {
                    let key = bytes(0)?;
                    let message = bytes(1)?;
                    if key.len() > LIMIT || message.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        Ok(Value::Bytes(
                            rewind::crypto::hmac_sha256(key, message).to_vec().into(),
                        ))
                    }
                }
                "stdPbkdf2Sha256" => {
                    let password = bytes(0)?;
                    let salt = bytes(1)?;
                    let iterations = int(2)?;
                    let length = int(3)?;
                    if !(1..=1_000_000).contains(&iterations)
                        || !(1..=1024).contains(&length)
                        || !(1..=1024).contains(&salt.len())
                    {
                        Err(("Range", 0))
                    } else if password.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        Ok(Value::Bytes(
                            rewind::crypto::pbkdf2_sha256(
                                password,
                                salt,
                                iterations as u32,
                                length as usize,
                            )
                            .into(),
                        ))
                    }
                }
                "stdBytesFromHex" => {
                    let s = text(0)?;
                    if s.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        match rewind::crypto::from_hex(s) {
                            Ok(out) => Ok(Value::Bytes(out.into())),
                            Err(offset) => Err(("Corrupt", offset)),
                        }
                    }
                }
                "stdDeflate" | "stdZlibCompress" => {
                    let b = bytes(0)?;
                    let level = int(1)?;
                    if !(0..=10).contains(&level) {
                        Err(("Range", 0))
                    } else if b.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        let out = if n == "stdDeflate" {
                            miniz_oxide::deflate::compress_to_vec(b, level as u8)
                        } else {
                            miniz_oxide::deflate::compress_to_vec_zlib(b, level as u8)
                        };
                        if out.len() > LIMIT {
                            Err(("Limit", 0))
                        } else {
                            Ok(Value::Bytes(out.into()))
                        }
                    }
                }
                "stdInflate" | "stdZlibDecompress" => {
                    let b = bytes(0)?;
                    let max = int(1)?;
                    if max < 0 || max as usize > LIMIT {
                        Err(("Range", 0))
                    } else if b.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        let result = if n == "stdInflate" {
                            miniz_oxide::inflate::decompress_to_vec_with_limit(b, max as usize)
                        } else {
                            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(b, max as usize)
                        };
                        use miniz_oxide::inflate::TINFLStatus;
                        match result {
                            Ok(out) => Ok(Value::Bytes(out.into())),
                            // More output than the caller allowed: refuse, never truncate.
                            Err(e) if e.status == TINFLStatus::HasMoreOutput => Err(("Limit", 0)),
                            // The stream ended before its final block / checksum.
                            Err(e)
                                if matches!(
                                    e.status,
                                    TINFLStatus::FailedCannotMakeProgress
                                        | TINFLStatus::NeedsMoreInput
                                ) =>
                            {
                                Err(("Truncated", e.output.len()))
                            }
                            Err(e) => Err(("Corrupt", e.output.len())),
                        }
                    }
                }
                "stdGzipCompress" => {
                    let b = bytes(0)?;
                    let level = int(1)?;
                    if !(0..=10).contains(&level) {
                        Err(("Range", 0))
                    } else if b.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        let out = rewind::gzip::compress(b, level as u8);
                        if out.len() > LIMIT {
                            Err(("Limit", 0))
                        } else {
                            Ok(Value::Bytes(out.into()))
                        }
                    }
                }
                "stdGzipDecompress" => {
                    let b = bytes(0)?;
                    let max = int(1)?;
                    if max < 0 || max as usize > LIMIT {
                        Err(("Range", 0))
                    } else if b.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        use rewind::gzip::GzipError;
                        match rewind::gzip::decompress(b, max as usize) {
                            Ok(out) => Ok(Value::Bytes(out.into())),
                            Err(GzipError::Range) => Err(("Range", 0)),
                            Err(GzipError::Limit) => Err(("Limit", 0)),
                            Err(GzipError::Unsupported) => Err(("Unsupported", 0)),
                            Err(GzipError::Truncated(at)) => Err(("Truncated", at)),
                            Err(GzipError::Corrupt(at)) => Err(("Corrupt", at)),
                        }
                    }
                }
                "stdPngDecode" => {
                    let data = bytes(0)?;
                    let max_pixels = int(1)?;
                    if max_pixels < 0 || max_pixels as usize > LIMIT {
                        Err(("Range", 0))
                    } else if data.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        match rewind::png::decode(data, max_pixels as usize, LIMIT) {
                            Ok(image) => {
                                // 9-byte header (width u32 LE, height u32 LE, channels u8), then pixels.
                                let mut out = Vec::with_capacity(9 + image.pixels.len());
                                out.extend_from_slice(&(image.width as u32).to_le_bytes());
                                out.extend_from_slice(&(image.height as u32).to_le_bytes());
                                out.push(image.channels as u8);
                                out.extend_from_slice(&image.pixels);
                                Ok(Value::Bytes(out.into()))
                            }
                            Err(e) => Err(png_error(e)),
                        }
                    }
                }
                "stdPngEncode" => {
                    let (width, height, channels) = (int(1)?, int(2)?, int(3)?);
                    if width < 1 || height < 1 || !(1..=4).contains(&channels) {
                        Err(("Range", 0))
                    } else {
                        match rewind::png::encode(
                            bytes(0)?,
                            width as usize,
                            height as usize,
                            channels as usize,
                            LIMIT,
                        ) {
                            Ok(out) if out.len() > LIMIT => Err(("Limit", 0)),
                            Ok(out) => Ok(Value::Bytes(out.into())),
                            Err(e) => Err(png_error(e)),
                        }
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
                "stdBitRotateLeft" | "stdBitRotateRight" => {
                    let a = int(0)?;
                    let count = int(1)?;
                    if !(0..64).contains(&count) {
                        Err(("Range", 0))
                    } else {
                        let u = a as u64;
                        let r = if n == "stdBitRotateLeft" {
                            u.rotate_left(count as u32)
                        } else {
                            u.rotate_right(count as u32)
                        };
                        Ok(Value::Int(r as i64))
                    }
                }
                "stdBitExtract" | "stdBitDeposit" => {
                    let a = int(0)?;
                    let start = int(1)?;
                    let width = int(2)?;
                    if !(0..64).contains(&start) || !(1..=64).contains(&width) || start + width > 64 {
                        Err(("Range", 0))
                    } else {
                        let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
                        if n == "stdBitExtract" {
                            Ok(Value::Int((((a as u64) >> start) & mask) as i64))
                        } else {
                            let v = int(3)?;
                            if v < 0 || (v as u64) & !mask != 0 {
                                Err(("Range", 0))
                            } else {
                                let cleared = (a as u64) & !(mask << start);
                                Ok(Value::Int((cleared | ((v as u64) << start)) as i64))
                            }
                        }
                    }
                }
                "stdBytesConcat" => {
                    let a = bytes(0)?;
                    let b = bytes(1)?;
                    if a.len() + b.len() > LIMIT {
                        Err(("Limit", 0))
                    } else {
                        let mut output = a.to_vec();
                        output.extend_from_slice(b);
                        Ok(Value::Bytes(output.into()))
                    }
                }
                "stdBytesFromInts" => {
                    let target = match args.first() {
                        Some(Value::HeapRef(id)) => runtime.heap_get(*id),
                        other => other,
                    };
                    match target {
                        Some(Value::TypedList(_, values)) if values.len() <= ITEMS => {
                            let mut output = Vec::with_capacity(values.len());
                            let mut failure = None;
                            for value in values.iter() {
                                match value {
                                    Value::Int(v) if (0..=255).contains(v) => output.push(*v as u8),
                                    _ => {
                                        failure = Some(("Range", 0));
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
                "stdTextFromCodePoints" => {
                    let target = match args.first() {
                        Some(Value::HeapRef(id)) => runtime.heap_get(*id),
                        other => other,
                    };
                    match target {
                        Some(Value::TypedList(_, values)) if values.len() <= LIMIT => {
                            let mut output = String::with_capacity(values.len());
                            let mut failure = None;
                            for (index, value) in values.iter().enumerate() {
                                match value {
                                    Value::Int(v) if (0xD800..=0xDFFF).contains(v) => {
                                        failure = Some(("Surrogate", index));
                                        break;
                                    }
                                    Value::Int(v) => match u32::try_from(*v).ok().and_then(char::from_u32) {
                                        Some(c) => output.push(c),
                                        None => {
                                            failure = Some(("Range", index));
                                            break;
                                        }
                                    },
                                    _ => {
                                        failure = Some(("Range", index));
                                        break;
                                    }
                                }
                                if output.len() > LIMIT {
                                    failure = Some(("Limit", index));
                                    break;
                                }
                            }
                            match failure {
                                Some(e) => Err(e),
                                None => Ok(Value::Text(output.into())),
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
                "stdBytesToInts" => {
                    let b = bytes(0)?;
                    if b.len() > ITEMS {
                        Err(("Limit", 0))
                    } else {
                        let values: Vec<Value> =
                            b.iter().map(|v| Value::Int(i64::from(*v))).collect();
                        Ok(Value::TypedList("Int".into(), values.into()))
                    }
                }
                "stdBytesIndexOf" => {
                    let data = bytes(0)?;
                    let needle = bytes(1)?;
                    let start = int(2)?;
                    if needle.is_empty() || start < 0 || start as u64 > data.len() as u64 {
                        Err(("Range", 0))
                    } else {
                        let from = start as usize;
                        let found = data[from..]
                            .windows(needle.len())
                            .position(|window| window == needle)
                            .map_or(-1, |i| (i + from) as i64);
                        Ok(Value::Int(found))
                    }
                }
                "stdBytesReadInt" => {
                    let data = bytes(0)?;
                    let offset = int(1)?;
                    let width = int(2)?;
                    let big = matches!(args.get(3), Some(Value::Bool(true)));
                    if !matches!(width, 1 | 2 | 4 | 8)
                        || offset < 0
                        || offset as u64 + width as u64 > data.len() as u64
                    {
                        Err(("Range", 0))
                    } else {
                        let field = &data[offset as usize..(offset + width) as usize];
                        let mut value: u64 = 0;
                        if big {
                            for byte in field {
                                value = (value << 8) | u64::from(*byte);
                            }
                        } else {
                            for byte in field.iter().rev() {
                                value = (value << 8) | u64::from(*byte);
                            }
                        }
                        Ok(Value::Int(value as i64))
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
