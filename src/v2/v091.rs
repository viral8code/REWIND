//! v0.9.1 primitives. Application policy lives in the REWIND libraries.
use super::*;
use serde_json::{json, Value as J};
const BYTE_LIMIT: usize = 1024 * 1024;
const ITEM_LIMIT: usize = 65_536;
const DEPTH_LIMIT: usize = 64;
const NAMES: &[&str] = &[
    "jsonParse",
    "jsonParseBytes",
    "jsonStringify",
    "jsonGet",
    "jsonKeys",
    "jsonKind",
    "jsonInt",
    "jsonText",
    "jsonBool",
    "appStatus",
];

pub(super) fn prepare(p: &mut Program) -> Result<()> {
    if !matches!(
        p.language.as_str(),
        "0.9.1"
            | "0.9.2"
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
            | "2.0.0"
    ) {
        return Ok(());
    }
    for n in NAMES {
        if p.functions.contains_key(*n) || p.structs.contains_key(*n) || p.enums.contains_key(*n) {
            return Err(Error::InvalidOperation(format!(
                "reserved standard function {n}"
            )));
        }
    }
    for n in ["Json", "JsonError"] {
        if p.structs.contains_key(n) || p.enums.contains_key(n) {
            return Err(Error::InvalidOperation(format!(
                "reserved standard type {n}"
            )));
        }
    }
    p.enums.insert(
        "Json".into(),
        EnumDef {
            type_params: vec![],
            public: true,
            origin: p.root_origin.clone(),
            variants: BTreeMap::from([
                ("Null".into(), vec![]),
                ("Bool".into(), vec![("0".into(), "Bool".into())]),
                ("Int".into(), vec![("0".into(), "Int".into())]),
                ("Float".into(), vec![("0".into(), "Float".into())]),
                ("Text".into(), vec![("0".into(), "String".into())]),
                (
                    "Array".into(),
                    vec![("0".into(), "Frozen<List<Json>>".into())],
                ),
                (
                    "Object".into(),
                    vec![("0".into(), "Frozen<Map<String,Json>>".into())],
                ),
            ]),
        },
    );
    p.structs.insert(
        "JsonError".into(),
        StructDef {
            private_fields: BTreeSet::new(),
            type_params: vec![],
            bounds: BTreeMap::new(),
            immutable: true,
            public: true,
            origin: p.root_origin.clone(),
            fields: vec![
                ("code".into(), "String".into()),
                ("line".into(), "Int".into()),
                ("column".into(), "Int".into()),
                ("offset".into(), "Int".into()),
            ],
        },
    );
    // A main function is an opt-in application entry; scripts retain their statements.
    if let Some(f) = p
        .functions
        .get("main")
        .filter(|f| f.origin == p.root_origin)
    {
        if f.ret != "Int" || !f.params.is_empty() || !f.type_params.is_empty() || f.asynchronous {
            return Err(diagnostic(&f.at, "application main must be fn main()->Int"));
        }
        let mut at = f.at.clone();
        at.text = "@application-entry".into();
        let call = Expr {
            at: at.clone(),
            kind: ExprKind::Call(
                Box::new(Expr {
                    at: at.clone(),
                    kind: ExprKind::Name("main".into()),
                }),
                vec![],
            ),
        };
        let status = Expr {
            at: at.clone(),
            kind: ExprKind::Call(
                Box::new(Expr {
                    at: at.clone(),
                    kind: ExprKind::Name("appStatus".into()),
                }),
                vec![
                    call,
                    Expr {
                        at: at.clone(),
                        kind: ExprKind::Value(Value::Text(String::new())),
                    },
                ],
            ),
        };
        p.stmts.push(Stmt {
            at,
            kind: StmtKind::Expr(status),
        });
        p.stmt_origins.push(p.root_origin.clone());
    }
    Ok(())
}
pub(super) fn call_type(p: &Program, n: &str, args: &[String], at: &Tok) -> Result<Option<String>> {
    if !matches!(
        p.language.as_str(),
        "0.9.1"
            | "0.9.2"
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
            | "2.0.0"
    ) || !NAMES.contains(&n)
    {
        return Ok(None);
    }
    let (params, ret): (&[&str], &str) = match n {
        "jsonParse" => (&["String"], "Result<Json,JsonError>"),
        "jsonParseBytes" => (&["Bytes"], "Result<Json,JsonError>"),
        "jsonStringify" => (&["Json"], "Result<String,JsonError>"),
        "jsonGet" => (&["Json", "String"], "Option<Json>"),
        "jsonKeys" => (&["Json"], "List<String>"),
        "jsonKind" => (&["Json"], "String"),
        "jsonInt" => (&["Json"], "Option<Int>"),
        "jsonText" => (&["Json"], "Option<String>"),
        "jsonBool" => (&["Json"], "Option<Bool>"),
        _ => (&["Int", "String"], "Unit"),
    };
    if params.len() != args.len() || params.iter().zip(args).any(|(p, a)| !compatible(p, a)) {
        return Err(diagnostic(
            at,
            format!("{n} requires ({})", params.join(",")),
        ));
    }
    Ok(Some(ret.into()))
}
fn error(code: &str, text: &str, offset: usize) -> Value {
    let prefix = &text[..offset.min(text.len())];
    Value::Struct(
        "JsonError".into(),
        BTreeMap::from([
            ("code".into(), Value::Text(code.into())),
            ("offset".into(), Value::Int(offset as i64)),
            (
                "line".into(),
                Value::Int(prefix.bytes().filter(|b| *b == b'\n').count() as i64 + 1),
            ),
            (
                "column".into(),
                Value::Int(prefix.rsplit('\n').next().unwrap_or("").chars().count() as i64 + 1),
            ),
        ]),
    )
}
fn variant(n: &str, v: Option<Value>) -> Value {
    Value::Enum(
        "Json".into(),
        n.into(),
        v.into_iter().map(|v| ("0".into(), v)).collect(),
    )
}
fn frozen(ty: &str, v: Value) -> Value {
    Value::Struct(
        format!("Frozen<{ty}>"),
        BTreeMap::from([("$value".into(), v)]),
    )
}
struct Parser<'a> {
    text: &'a str,
    pos: usize,
    items: usize,
}
type Parse<T> = std::result::Result<T, (&'static str, usize)>;
impl Parser<'_> {
    fn ws(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.pos)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.pos += 1;
        }
    }
    fn take(&mut self, b: u8) -> bool {
        self.ws();
        if self.text.as_bytes().get(self.pos) == Some(&b) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Parse<String> {
        self.ws();
        let start = self.pos;
        if !self.take(b'"') {
            return Err(("Syntax", self.pos));
        }
        let mut escaped = false;
        while let Some(&b) = self.text.as_bytes().get(self.pos) {
            self.pos += 1;
            if b == b'"' && !escaped {
                return serde_json::from_str(&self.text[start..self.pos])
                    .map_err(|_| ("Syntax", start));
            }
            if b == b'\\' && !escaped {
                escaped = true;
            } else {
                escaped = false;
            }
        }
        Err(("Syntax", start))
    }
    fn value(&mut self, depth: usize) -> Parse<Value> {
        self.ws();
        let start = self.pos;
        self.items += 1;
        if depth > DEPTH_LIMIT {
            return Err(("DepthLimit", start));
        }
        if self.items > ITEM_LIMIT {
            return Err(("ItemLimit", start));
        }
        match self.text.as_bytes().get(start).copied() {
            Some(b'"') => Ok(variant("Text", Some(Value::Text(self.string()?)))),
            Some(b'{') => {
                self.pos += 1;
                let mut fields = BTreeMap::new();
                if !self.take(b'}') {
                    loop {
                        let key_at = self.pos;
                        let key = self.string()?;
                        if fields.contains_key(&MapKey::Text(key.clone())) {
                            return Err(("DuplicateKey", key_at));
                        }
                        if !self.take(b':') {
                            return Err(("Syntax", self.pos));
                        }
                        fields.insert(MapKey::Text(key), self.value(depth + 1)?);
                        if self.take(b'}') {
                            break;
                        }
                        if !self.take(b',') {
                            return Err(("Syntax", self.pos));
                        }
                    }
                }
                Ok(variant(
                    "Object",
                    Some(frozen(
                        "Map<String,Json>",
                        Value::TypedMap("String".into(), "Json".into(), fields.into()),
                    )),
                ))
            }
            Some(b'[') => {
                self.pos += 1;
                let mut values = vec![];
                if !self.take(b']') {
                    loop {
                        values.push(self.value(depth + 1)?);
                        if self.take(b']') {
                            break;
                        }
                        if !self.take(b',') {
                            return Err(("Syntax", self.pos));
                        }
                    }
                }
                Ok(variant(
                    "Array",
                    Some(frozen(
                        "List<Json>",
                        Value::TypedList("Json".into(), values.into()),
                    )),
                ))
            }
            Some(b't' | b'f' | b'n') => {
                for (literal, v) in [
                    ("true", variant("Bool", Some(Value::Bool(true)))),
                    ("false", variant("Bool", Some(Value::Bool(false)))),
                    ("null", variant("Null", None)),
                ] {
                    if self.text[start..].starts_with(literal) {
                        self.pos += literal.len();
                        return Ok(v);
                    }
                }
                Err(("Syntax", start))
            }
            Some(b'-' | b'0'..=b'9') => {
                while self
                    .text
                    .as_bytes()
                    .get(self.pos)
                    .is_some_and(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
                {
                    self.pos += 1;
                }
                let raw = &self.text[start..self.pos];
                if !number_syntax(raw) {
                    return Err(("Syntax", start));
                }
                let n: serde_json::Number =
                    serde_json::from_str(raw).map_err(|_| ("NumberRange", start))?;
                if !raw.contains(['.', 'e', 'E']) {
                    let n = raw.parse::<i64>().map_err(|_| ("NumberRange", start))?;
                    Ok(variant("Int", Some(Value::Int(n))))
                } else {
                    let n = n
                        .as_f64()
                        .filter(|n| n.is_finite())
                        .ok_or(("NumberRange", start))?;
                    Ok(variant("Float", Some(Value::Float(n.to_bits()))))
                }
            }
            _ => Err(("Syntax", start)),
        }
    }
}
fn parse(bytes: &[u8]) -> Value {
    let result = (|| {
        if bytes.len() > BYTE_LIMIT {
            return Err(error("ByteLimit", "", 0));
        }
        let text = std::str::from_utf8(bytes).map_err(|e| {
            error(
                "Utf8",
                std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
                e.valid_up_to(),
            )
        })?;
        let mut p = Parser {
            text,
            pos: 0,
            items: 0,
        };
        let v = p.value(0).map_err(|(code, pos)| error(code, text, pos))?;
        p.ws();
        if p.pos != text.len() {
            return Err(error("Syntax", text, p.pos));
        }
        Ok(v)
    })();
    Value::Result(result.map(Box::new).map_err(Box::new))
}
fn resolve<'a>(rt: &'a Runtime, v: &'a Value) -> Result<&'a Value> {
    match v {
        Value::HeapRef(id) | Value::CellRef(id) => rt
            .heap_get(*id)
            .ok_or_else(|| Error::InvalidOperation("missing JSON object".into())),
        _ => Ok(v),
    }
}
fn to_json(
    rt: &Runtime,
    v: &Value,
    depth: usize,
    items: &mut usize,
    bytes: &mut usize,
) -> Parse<J> {
    *items += 1;
    if depth > DEPTH_LIMIT {
        return Err(("DepthLimit", 0));
    }
    if *items > ITEM_LIMIT {
        return Err(("ItemLimit", 0));
    }
    let v = resolve(rt, v).map_err(|_| ("InvalidValue", 0))?;
    let Value::Enum(t, n, fields) = v else {
        return Err(("InvalidValue", 0));
    };
    if t != "Json" {
        return Err(("InvalidValue", 0));
    }
    let value = fields.first().map(|(_, v)| v);
    match (n.as_str(), value) {
        ("Null", None) => Ok(J::Null),
        ("Bool", Some(Value::Bool(b))) => Ok(json!(b)),
        ("Int", Some(Value::Int(n))) => Ok(json!(n)),
        ("Text", Some(Value::Text(s))) => {
            *bytes += s.len();
            if *bytes > BYTE_LIMIT {
                return Err(("ByteLimit", 0));
            }
            Ok(json!(s))
        }
        ("Float", Some(Value::Float(b))) => {
            let n = f64::from_bits(*b);
            if !n.is_finite() {
                return Err(("NumberRange", 0));
            }
            Ok(json!(n))
        }
        ("Array", Some(v)) => {
            let v = v05::unfrozen(v).ok_or(("InvalidValue", 0))?;
            let Value::TypedList(_, xs) = v else {
                return Err(("InvalidValue", 0));
            };
            Ok(J::Array(
                xs.iter()
                    .map(|v| to_json(rt, v, depth + 1, items, bytes))
                    .collect::<Parse<_>>()?,
            ))
        }
        ("Object", Some(v)) => {
            let v = v05::unfrozen(v).ok_or(("InvalidValue", 0))?;
            let Value::TypedMap(_, _, xs) = v else {
                return Err(("InvalidValue", 0));
            };
            let mut out = serde_json::Map::new();
            for (k, v) in xs {
                let MapKey::Text(k) = k else {
                    return Err(("InvalidValue", 0));
                };
                *bytes += k.len();
                if *bytes > BYTE_LIMIT {
                    return Err(("ByteLimit", 0));
                }
                out.insert(k.clone(), to_json(rt, v, depth + 1, items, bytes)?);
            }
            Ok(J::Object(out))
        }
        _ => Err(("InvalidValue", 0)),
    }
}
pub(super) fn call(rt: &Runtime, n: &str, args: &[Value]) -> Result<Option<Value>> {
    if !NAMES.contains(&n) {
        return Ok(None);
    }
    let a = args
        .first()
        .ok_or_else(|| Error::InvalidOperation("missing application argument".into()))?;
    let a = resolve(rt, a)?;
    let value = match n {
        "jsonParse" => {
            let Value::Text(s) = a else {
                return Err(Error::InvalidOperation("jsonParse expects String".into()));
            };
            parse(s.as_bytes())
        }
        "jsonParseBytes" => {
            let Value::Bytes(b) = a else {
                return Err(Error::InvalidOperation(
                    "jsonParseBytes expects Bytes".into(),
                ));
            };
            parse(b)
        }
        "jsonStringify" => {
            let result = to_json(rt, a, 0, &mut 0, &mut 0)
                .and_then(|j| serde_json::to_string(&j).map_err(|_| ("InvalidValue", 0)))
                .and_then(|s| {
                    if s.len() > BYTE_LIMIT {
                        Err(("ByteLimit", 0))
                    } else {
                        Ok(Value::Text(s))
                    }
                });
            Value::Result(
                result
                    .map(Box::new)
                    .map_err(|(c, p)| Box::new(error(c, "", p))),
            )
        }
        "appStatus" => {
            let Value::Int(code) = a else {
                return Err(Error::InvalidOperation("appStatus expects Int".into()));
            };
            if !(0..=63).contains(code) {
                return Err(Error::InvalidOperation(
                    "application status must be 0..63".into(),
                ));
            }
            if *code != 0 {
                let message = match args.get(1) {
                    Some(Value::Text(s)) => rt.masked_value(&Value::Text(s.clone())),
                    _ => String::new(),
                };
                return Err(Error::Diagnostic(Box::new(rewind::DiagnosticRecord {
                    code: format!("ApplicationExit{code}"),
                    message,
                    source: String::new(),
                    line: 0,
                    column: 0,
                    task_id: None,
                    frames: vec![],
                    hints: vec![],
                    frames_truncated: false,
                    causes: vec![],
                    wait_edges: vec![],
                })));
            }
            Value::Null
        }
        _ => {
            let Value::Enum(_, kind, fields) = a else {
                return Err(Error::InvalidOperation("expected Json".into()));
            };
            let inner = fields.first().map(|(_, v)| v);
            match n {
                "jsonKind" => Value::Text(kind.clone()),
                "jsonInt" | "jsonText" | "jsonBool" => {
                    let expected = match n {
                        "jsonInt" => "Int",
                        "jsonText" => "Text",
                        _ => "Bool",
                    };
                    Value::Option(if kind == expected {
                        inner.cloned().map(Box::new)
                    } else {
                        None
                    })
                }
                "jsonGet" | "jsonKeys" => {
                    let map = inner.and_then(v05::unfrozen).and_then(|v| {
                        if let Value::TypedMap(_, _, xs) = v {
                            Some(xs)
                        } else {
                            None
                        }
                    });
                    if n == "jsonKeys" {
                        Value::TypedList(
                            "String".into(),
                            map.into_iter()
                                .flat_map(|m| m.keys())
                                .filter_map(|k| {
                                    if let MapKey::Text(s) = k {
                                        Some(Value::Text(s.clone()))
                                    } else {
                                        None
                                    }
                                })
                                .collect(),
                        )
                    } else {
                        let key = match args.get(1) {
                            Some(Value::Text(s)) => s,
                            _ => return Err(Error::InvalidOperation("expected JSON key".into())),
                        };
                        Value::Option(
                            map.and_then(|m| m.get(&MapKey::Text(key.clone())))
                                .cloned()
                                .map(Box::new),
                        )
                    }
                }
                _ => unreachable!(),
            }
        }
    };
    Ok(Some(value))
}

pub(super) fn reserved_functions() -> &'static [&'static str] {
    NAMES
}

pub(super) fn asset_inventory(root: &Path, assets: &BTreeMap<String, String>) -> Result<J> {
    let root = fs::canonicalize(root)?;
    let mut inventory = serde_json::Map::new();
    let mut total = 0u64;
    if assets.len() > 256 {
        return Err(Error::InvalidOperation(
            "asset count budget exceeded".into(),
        ));
    }
    for path in assets.values() {
        let relative = Path::new(path);
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || path.contains('\\')
        {
            return Err(Error::InvalidPath(path.clone()));
        }
        if relative
            .components()
            .any(|c| matches!(c.as_os_str().to_str(), Some(".git" | ".rewind" | "target")))
            || matches!(
                relative.file_name().and_then(|s| s.to_str()),
                Some(
                    "rewind.toml"
                        | "rewind.lock"
                        | "rewind.release.json"
                        | "rewind.signature"
                        | "rewind.package.json"
                )
            )
            || matches!(
                relative.extension().and_then(|s| s.to_str()),
                Some("rw" | "seed" | "key" | "pem" | "trace")
            )
        {
            return Err(Error::InvalidOperation(
                "reserved distribution asset path".into(),
            ));
        }
        let mut full = root.clone();
        for component in relative.components() {
            full.push(component.as_os_str());
            if fs::symlink_metadata(&full)?.file_type().is_symlink() {
                return Err(Error::InvalidPath("asset symlink".into()));
            }
        }
        let meta = fs::metadata(&full)?;
        if !meta.is_file() {
            return Err(Error::InvalidPath(path.clone()));
        }
        total = total
            .checked_add(meta.len())
            .ok_or_else(|| Error::InvalidOperation("asset budget exceeded".into()))?;
        if total > 32 * 1024 * 1024 {
            return Err(Error::InvalidOperation("asset budget exceeded".into()));
        }
        if inventory
            .insert(
                path.clone(),
                json!({"sha256":packages::hash(fs::read(full)?),"bytes":meta.len()}),
            )
            .is_some()
        {
            return Err(Error::InvalidOperation("duplicate asset path".into()));
        }
    }
    Ok(J::Object(inventory))
}
pub(super) fn verify_assets(root: &Path, expected: &J) -> Result<()> {
    let map = expected
        .as_object()
        .ok_or_else(|| Error::InvalidOperation("invalid asset inventory".into()))?;
    let declared = map
        .keys()
        .enumerate()
        .map(|(i, p)| (i.to_string(), p.clone()))
        .collect();
    if asset_inventory(root, &declared)? != *expected {
        return Err(Error::InvalidOperation("asset digest mismatch".into()));
    }
    Ok(())
}
pub(super) fn write_release(root: &Path, config: &project::ProjectConfig) -> Result<()> {
    let release = json!({"format":1,"kind":"rewind-release","compiler":env!("CARGO_PKG_VERSION"),"language":config.language,"entry":config.entry.strip_prefix(fs::canonicalize(root)?).map_err(|_|Error::InvalidPath("release entry".into()))?.to_string_lossy(),"target":format!("{}-{}",std::env::consts::ARCH,std::env::consts::OS),"effects":config.effects,"budgets":{"execution_steps":100000,"asset_bytes":33554432},"lock_sha256":packages::hash(fs::read(root.join("rewind.lock"))?),"assets":asset_inventory(root,&config.assets)?});
    let mut release = release;
    let p = load_program(
        &config.entry,
        root,
        &config.imports,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    release["sources"] = json!(p
        .included_modules
        .iter()
        .map(|p| Ok((
            p.strip_prefix(fs::canonicalize(root)?)
                .map_err(|_| Error::InvalidPath("release source".into()))?
                .to_string_lossy()
                .into_owned(),
            packages::hash(fs::read(p)?)
        )))
        .collect::<Result<BTreeMap<_, _>>>()?);
    fs::write(
        root.join("rewind.release.json"),
        serde_json::to_vec_pretty(&release).map_err(|e| Error::InvalidOperation(e.to_string()))?,
    )?;
    Ok(())
}

pub(super) fn verify_release(path: &Path) -> Result<()> {
    if fs::metadata(path)?.len() > 1024 * 1024 {
        return Err(Error::InvalidOperation("release budget exceeded".into()));
    }
    let release: J = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    let root = fs::canonicalize(path.parent().unwrap_or(Path::new(".")))?;
    if release["format"] != 1
        || release["kind"] != "rewind-release"
        || release["compiler"] != env!("CARGO_PKG_VERSION")
        || release["lock_sha256"] != packages::hash(fs::read(root.join("rewind.lock"))?)
    {
        return Err(Error::InvalidOperation(
            "release metadata or lock mismatch".into(),
        ));
    }
    verify_assets(&root, &release["assets"])?;
    let config = project::ProjectConfig::load(&root)?
        .ok_or_else(|| Error::InvalidOperation("release manifest missing".into()))?;
    config.lock(&root, false)?;
    let p = load_program(
        &config.entry,
        &root,
        &config.imports,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    let sources = p
        .included_modules
        .iter()
        .map(|p| {
            Ok((
                p.strip_prefix(&root)
                    .map_err(|_| Error::InvalidPath("release source".into()))?
                    .to_string_lossy()
                    .into_owned(),
                packages::hash(fs::read(p)?),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    if release["sources"] != json!(sources)
        || release["language"] != config.language
        || release["effects"] != json!(config.effects)
        || release["entry"]
            != config
                .entry
                .strip_prefix(&root)
                .map_err(|_| Error::InvalidPath("release entry".into()))?
                .to_string_lossy()
                .as_ref()
    {
        return Err(Error::InvalidOperation(
            "release source or manifest mismatch".into(),
        ));
    }
    Ok(())
}

fn number_syntax(raw: &str) -> bool {
    let b = raw.as_bytes();
    let mut i = usize::from(b.first() == Some(&b'-'));
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => {
            i += 1;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        _ => return false,
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return false;
        }
    }
    i == b.len()
}
