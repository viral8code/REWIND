use super::*;
use serde_json::{json, Value as Json};
mod session;
pub(super) use session::{repl, session_replay};
pub(in crate::v2) fn record_arguments(
    p: &Program,
    s: &StructDef,
    sub: &BTreeMap<String, String>,
    bounds: &BTreeMap<String, String>,
    at: &Tok,
) -> Result<()> {
    for (param, bound) in s.bounds.iter().filter(|_| {
        matches!(
            p.language.as_str(),
            "1.0.0"
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
    }) {
        if let Some(ty) = sub.get(param) {
            if !trait_satisfied(p, bound, ty) && !bound_provided(bounds, ty, bound) {
                return Err(diagnostic(at, format!("{ty} does not implement {bound}")));
            }
        }
    }
    if program_v09(p) && s.immutable {
        for param in &s.type_params {
            if !sub
                .get(param)
                .is_some_and(|ty| v05::transfer_bounded(p, ty, true, bounds))
            {
                return Err(diagnostic(at, "record type arguments must be Share"));
            }
        }
    }
    Ok(())
}

pub(super) fn install_production(root: &Path, output: &Path) -> Result<()> {
    let config = project::ProjectConfig::load(root)?
        .ok_or_else(|| Error::InvalidOperation("install requires a manifest".into()))?;
    config.lock(root, false)?;
    let mut p = load_program(
        &config.entry,
        root,
        &config.imports,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    p.language = config.language.clone();
    p.strict_visibility = true;
    if !program_v09(&p) {
        return Err(Error::InvalidOperation(
            "production install requires language 0.9".into(),
        ));
    }
    v05::prepare(&mut p)?;
    check_program(&p)?;
    v05::validate(&p, &config)?;
    let root = fs::canonicalize(root)?;
    fn files(dir: &Path, out: &mut BTreeSet<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                return Err(Error::InvalidPath("package symlink".into()));
            }
            if meta.is_dir() {
                files(&path, out)?;
            } else if path.extension().is_some_and(|s| s == "rw")
                || path
                    .file_name()
                    .is_some_and(|s| s == "rewind.package.json" || s == "rewind.signature")
            {
                out.insert(path);
            }
        }
        Ok(())
    }
    let mut selected = p.included_modules.clone();
    for (name, path) in &config.imports {
        if !name.is_empty() {
            files(path, &mut selected)?;
        }
    }
    let mut copies = Vec::new();
    let mut total = 0usize;
    for path in selected {
        let relative = path
            .strip_prefix(&root)
            .map_err(|_| Error::InvalidPath(path.display().to_string()))?
            .to_path_buf();
        let bytes = fs::read(&path)?;
        total += bytes.len();
        if total > 32 * 1024 * 1024 {
            return Err(Error::InvalidOperation(
                "install source budget exceeded".into(),
            ));
        }
        copies.push((relative, bytes));
    }
    if !config.assets.is_empty() {
        v091::asset_inventory(&root, &config.assets)?;
        for path in config.assets.values() {
            let bytes = fs::read(root.join(path))?;
            copies.push((PathBuf::from(path), bytes));
        }
    }
    // Only a new directory is populated, so existing installations are never changed.
    fs::create_dir(output)?;
    let result = (|| {
        for (relative, bytes) in copies {
            let target = output.join(relative);
            fs::create_dir_all(target.parent().unwrap())?;
            fs::write(target, bytes)?;
        }
        let manifest = fs::read_to_string(root.join("rewind.toml"))?;
        let manifest = manifest
            .lines()
            .filter(|l| !l.trim_start().starts_with("dependency_mode"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(
            output.join("rewind.toml"),
            format!("dependency_mode = \"production\"\n{manifest}\n"),
        )?;
        fs::write(
            output.join("rewind.lock"),
            fs::read(root.join("rewind.lock"))?,
        )?;
        let installed = project::ProjectConfig::load(output)?
            .ok_or_else(|| Error::InvalidOperation("missing installed manifest".into()))?;
        installed.lock(output, false)?;
        if matches!(
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
            v091::write_release(output, &installed)?;
        }
        println!(
            "{}",
            json!({"mode":"production","packages":config.imports.len().saturating_sub(1),"development_graph_verified":false})
        );
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(output);
    }
    result
}

pub(in crate::v2) fn constant_digest<R: BufRead>(
    engine: &Engine<R>,
    value: &Value,
) -> Result<Json> {
    fn encode(rt: &Runtime, v: &Value, depth: usize, seen: &mut BTreeSet<u64>) -> Result<Json> {
        if depth > 128 {
            return Err(Error::InvalidOperation(
                "const value snapshot depth exceeded".into(),
            ));
        }
        let mut child = |v: &Value| encode(rt, v, depth + 1, seen);
        Ok(match v {
            Value::HeapRef(id) | Value::CellRef(id) => {
                if !seen.insert(*id) {
                    return Err(Error::InvalidOperation("cyclic const snapshot".into()));
                }
                let value = rt
                    .heap_get(*id)
                    .ok_or_else(|| Error::InvalidOperation("missing const object".into()))?;
                let encoded = encode(rt, value, depth + 1, seen)?;
                seen.remove(id);
                encoded
            }
            Value::List(xs) => {
                json!({"List": xs.iter().map(&mut child).collect::<Result<Vec<_>>>()?})
            }
            Value::TypedList(t, xs) => {
                json!({"TypedList": [json!(t), json!(xs.iter().map(&mut child).collect::<Result<Vec<_>>>()?)]})
            }
            Value::Map(xs) | Value::TypedMap(_, _, xs) => {
                // JSON object keys cannot represent the tagged MapKey enum. Encode
                // entries in their stable BTreeMap order, retaining the value tag.
                let entries = xs
                    .iter()
                    .map(|(k, v)| Ok(json!([k, child(v)?])))
                    .collect::<Result<Vec<_>>>()?;
                match v {
                    Value::TypedMap(k, t, _) => {
                        json!({"TypedMap":[json!(k),json!(t),json!(entries)]})
                    }
                    _ => json!({"Map":entries}),
                }
            }
            Value::OrderedMap(k, t, xs) => {
                let entries = xs
                    .iter()
                    .map(|(k, v)| Ok(json!([child(k)?, child(v)?])))
                    .collect::<Result<Vec<_>>>()?;
                json!({"OrderedMap":[json!(k),json!(t),json!(entries)]})
            }
            Value::Struct(t, fields) => {
                let fields = fields
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), child(v)?)))
                    .collect::<Result<BTreeMap<_, _>>>()?;
                json!({"Struct":[json!(t),json!(fields)]})
            }
            Value::Enum(t, variant, fields) => {
                let fields = fields
                    .iter()
                    .map(|(k, v)| Ok(json!([k, child(v)?])))
                    .collect::<Result<Vec<_>>>()?;
                json!({"Enum":[json!(t),json!(variant),json!(fields)]})
            }
            Value::Closure(name, t, fields) => {
                let fields = fields
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), child(v)?)))
                    .collect::<Result<BTreeMap<_, _>>>()?;
                json!({"Closure":[json!(name),json!(t),json!(fields)]})
            }
            Value::Option(Some(v)) => json!({"Option":child(v)?}),
            Value::Result(Ok(v)) => json!({"Result":{"Ok":child(v)?}}),
            Value::Result(Err(v)) => json!({"Result":{"Err":child(v)?}}),
            other => {
                serde_json::to_value(other).map_err(|e| Error::InvalidOperation(e.to_string()))?
            }
        })
    }
    let value = encode(&engine.runtime, value, 0, &mut BTreeSet::new())?;
    fn function_names(v: &Json, out: &mut BTreeSet<String>) {
        match v {
            Json::Object(o) => {
                for key in ["Function", "Closure"] {
                    if let Some(name) = o
                        .get(key)
                        .and_then(Json::as_array)
                        .and_then(|a| a.first())
                        .and_then(Json::as_str)
                    {
                        out.insert(name.into());
                    }
                }
                for v in o.values() {
                    function_names(v, out);
                }
            }
            Json::Array(a) => {
                for v in a {
                    function_names(v, out);
                }
            }
            _ => {}
        }
    }
    fn clean(v: &mut Json) {
        match v {
            Json::Object(o) => {
                o.remove("at");
                o.remove("origin");
                for v in o.values_mut() {
                    clean(v);
                }
            }
            Json::Array(a) => {
                for v in a {
                    clean(v);
                }
            }
            _ => {}
        }
    }
    let mut pending = BTreeSet::new();
    function_names(&value, &mut pending);
    let mut functions = BTreeMap::new();
    while let Some(name) = pending.pop_first() {
        if functions.contains_key(&name) {
            continue;
        }
        if let Some(f) = engine.program.functions.get(&name) {
            let mut body =
                serde_json::to_value(f).map_err(|e| Error::InvalidOperation(e.to_string()))?;
            clean(&mut body);
            functions.insert(name, body);
            v05::expressions(&f.body, &mut |e| {
                if let ExprKind::Name(n) = &e.kind {
                    let n = resolve_alias(&engine.program, n);
                    if engine.program.functions.contains_key(&n) && !functions.contains_key(&n) {
                        pending.insert(n);
                    }
                }
            });
        }
    }
    Ok(json!(packages::hash(
        serde_json::to_vec(&json!({"value":value,"functions":functions}))
            .map_err(|e| Error::InvalidOperation(e.to_string()))?
    )))
}

pub(in crate::v2) fn record_type(
    p: &Program,
    ty: &str,
    bounds: &BTreeMap<String, String>,
    at: &Tok,
    depth: usize,
) -> Result<()> {
    if !program_v09(p) {
        return Ok(());
    }
    if depth > 64 {
        return Err(diagnostic(at, "record type expansion budget exceeded"));
    }
    let ty = ty.trim_start_matches('&').trim_start_matches("mut ");
    if let Some((params, ret)) = function_signature(ty) {
        for t in params {
            record_type(p, &t, bounds, at, depth + 1)?;
        }
        return record_type(p, &ret, bounds, at, depth + 1);
    }
    let base = ty.split('<').next().unwrap_or(ty);
    let args = ty
        .split_once('<')
        .map(|(_, args)| split_type_args(outer_type_end(args)))
        .unwrap_or_default();
    if let Some(s) = p.structs.get(base).filter(|s| s.immutable) {
        if args.len() != s.type_params.len() {
            return Err(diagnostic(at, "record type argument count mismatch"));
        }
        let sub = s
            .type_params
            .iter()
            .cloned()
            .zip(args.iter().map(|s| s.to_string()))
            .collect();
        record_arguments(p, s, &sub, bounds, at)?;
    }
    for arg in args {
        record_type(p, arg, bounds, at, depth + 1)?;
    }
    Ok(())
}
pub(in crate::v2) fn record_signatures(p: &Program) -> Result<()> {
    if !program_v09(p) {
        return Ok(());
    }
    fn body(p: &Program, stmts: &[Stmt], bounds: &BTreeMap<String, String>) -> Result<()> {
        for s in stmts {
            match &s.kind {
                StmtKind::Let(_, _, Some(ty), _) => record_type(p, ty, bounds, &s.at, 0)?,
                StmtKind::External(_, b)
                | StmtKind::Block(b)
                | StmtKind::Branch(_, b)
                | StmtKind::While(_, b)
                | StmtKind::For(_, _, _, b) => body(p, b, bounds)?,
                StmtKind::If(_, a, b) => {
                    body(p, a, bounds)?;
                    body(p, b, bounds)?;
                }
                StmtKind::Match(_, arms) => {
                    for (_, _, s) in arms {
                        body(p, std::slice::from_ref(s), bounds)?;
                    }
                }
                _ => {}
            }
        }
        let mut error = None;
        v05::expressions(stmts, &mut |e| {
            if let ExprKind::Closure(params, ret, b) = &e.kind {
                let result = (|| {
                    for (_, t) in params {
                        record_type(p, t, bounds, &e.at, 0)?;
                    }
                    record_type(p, ret, bounds, &e.at, 0)?;
                    body(p, b, bounds)
                })();
                if let Err(e) = result {
                    error = Some(e);
                }
            }
        });
        if let Some(e) = error {
            return Err(e);
        }
        Ok(())
    }
    let at = Tok {
        source: p.root_origin.to_string_lossy().into_owned(),
        text: String::new(),
        line: 1,
        col: 1,
    };
    for s in p.structs.values() {
        for (_, ty) in &s.fields {
            record_type(p, ty, &s.bounds, &at, 0)?;
        }
    }
    for e in p.enums.values() {
        let bounds = BTreeMap::new();
        for (_, ty) in e.variants.values().flatten() {
            record_type(p, ty, &bounds, &at, 0)?;
        }
    }
    body(p, &p.stmts, &BTreeMap::new())?;
    for f in p.functions.values() {
        let bounds = f
            .type_params
            .iter()
            .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
            .collect();
        for (_, t) in &f.params {
            record_type(p, t, &bounds, &f.at, 0)?;
        }
        record_type(p, &f.ret, &bounds, &f.at, 0)?;
        body(p, &f.body, &bounds)?;
    }
    Ok(())
}
