use super::*;
fn paths(program: &mut Program, mut map: impl FnMut(&Path) -> Result<PathBuf>) -> Result<()> {
    program.root_origin = map(&program.root_origin)?;
    for p in &mut program.stmt_origins {
        *p = map(p)?;
    }
    for f in program.functions.values_mut() {
        f.origin = map(&f.origin)?;
    }
    for s in program.structs.values_mut() {
        s.origin = map(&s.origin)?;
    }
    for e in program.enums.values_mut() {
        e.origin = map(&e.origin)?;
    }
    for t in program.traits.values_mut() {
        t.origin = map(&t.origin)?;
        for f in t.defaults.values_mut() {
            f.origin = map(&f.origin)?;
        }
    }
    for a in program.aliases.values_mut() {
        a.origin = map(&a.origin)?;
    }
    for (_, p) in program.const_origins.values_mut() {
        *p = map(p)?;
    }
    for p in program.impl_origins.values_mut() {
        *p = map(p)?;
    }
    program.included_modules = program
        .included_modules
        .iter()
        .map(|p| map(p))
        .collect::<Result<_>>()?;
    program.module_effects = program
        .module_effects
        .iter()
        .map(|(p, e)| Ok((map(p)?, e.clone())))
        .collect::<Result<_>>()?;
    program.import_exposure = program
        .import_exposure
        .iter()
        .map(|((a, b), e)| Ok(((map(a)?, map(b)?), e.clone())))
        .collect::<Result<_>>()?;
    Ok(())
}
pub fn build(
    program: &Program,
    root: &Path,
    config: &project::ProjectConfig,
) -> Result<serde_json::Value> {
    let build = vm::build_artifact(program, root)?;
    let mut portable = program.clone();
    let root = fs::canonicalize(root)?;
    paths(&mut portable, |p| {
        Ok(PathBuf::from(
            p.strip_prefix(&root)
                .map_err(|_| Error::InvalidPath(p.display().to_string()))?
                .to_string_lossy()
                .replace('\\', "/"),
        ))
    })?;
    let payload = serde_json::json!({"program":portable,"build":build,"effects":config.effects,"assets":v091::asset_inventory(&root,&config.assets)?});
    let payload_bytes =
        serde_json::to_vec(&payload).map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if payload_bytes.len() > 16 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "ArtifactBudgetExceeded: 16 MiB".into(),
        ));
    }
    let digest = packages::hash(&payload_bytes);
    let artifact = serde_json::json!({"format":2,"compiler":env!("CARGO_PKG_VERSION"),"sha256":digest,"payload":payload});
    let cache = root.join(".rewind/cache");
    if cache.exists() && fs::symlink_metadata(&cache)?.file_type().is_symlink() {
        return Err(Error::InvalidPath(cache.display().to_string()));
    }
    if root.join(".rewind").exists()
        && fs::symlink_metadata(root.join(".rewind"))?
            .file_type()
            .is_symlink()
    {
        return Err(Error::InvalidPath(".rewind cache symlink".into()));
    }
    fs::create_dir_all(&cache)?;
    let cached = cache.join(format!("{digest}.json"));
    if fs::symlink_metadata(&cached).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::InvalidPath(cached.display().to_string()));
    }
    let bytes =
        serde_json::to_vec_pretty(&artifact).map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if !cached.exists() || fs::read(&cached)? != bytes {
        fs::write(cached, &bytes)?;
    }
    Ok(artifact)
}
pub fn run(path: &Path, root: &Path, mut options: RunOptions) -> Result<()> {
    if let Some(key) = &options.verify_key {
        packages::verify_file(
            path,
            &PathBuf::from(format!("{}.signature", path.display())),
            key,
            "artifact",
        )?;
    }
    let invalid =
        |message: &str| Error::InvalidOperation(format!("ArtifactVerificationFailed: {message}"));
    if fs::metadata(path)?.len() > 32 * 1024 * 1024 {
        return Err(invalid("file exceeds 32 MiB"));
    }
    let artifact: serde_json::Value =
        serde_json::from_slice(&fs::read(path)?).map_err(|e| invalid(&e.to_string()))?;
    if artifact["format"] != 2 || artifact["compiler"] != env!("CARGO_PKG_VERSION") {
        return Err(invalid("unsupported format/compiler"));
    }
    let payload = &artifact["payload"];
    if artifact["sha256"]
        != packages::hash(serde_json::to_vec(payload).map_err(|e| invalid(&e.to_string()))?)
    {
        return Err(invalid("payload hash mismatch"));
    }
    let mut program: Program =
        serde_json::from_value(payload["program"].clone()).map_err(|e| invalid(&e.to_string()))?;
    if !matches!(
        program.language.as_str(),
        "0.5"
            | "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
    ) || !program.strict_visibility
        || program.stmts.len() != program.stmt_origins.len()
        || program.functions.len() > 4096
        || program.included_modules.len() > 1024
    {
        return Err(invalid("invalid program metadata/budget"));
    }
    let root = fs::canonicalize(root)?;
    if let Some(assets) = payload.get("assets") {
        v091::verify_assets(&root, assets)?;
    }
    paths(&mut program, |p| {
        if p.as_os_str().is_empty()
            || p.is_absolute()
            || p.components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(invalid("non-relative source map path"));
        }
        Ok(root.join(p))
    })?;
    // Standard layouts are part of the verified format, not user supplied IR.
    let mut standard = Program {
        language: program.language.clone(),
        root_origin: program.root_origin.clone(),
        ..Program::default()
    };
    prepare(&mut standard)?;
    if matches!(
        program.language.as_str(),
        "0.9.2" | "0.9.3" | "0.9.4" | "0.9.5" | "0.9.6" | "0.9.7" | "0.9.8" | "0.9.9" | "1.0.0"
    ) && (serde_json::to_value(program.structs.get("StdError")).ok()
        != serde_json::to_value(standard.structs.get("StdError")).ok()
        || program.enums.contains_key("StdError")
        || v092::names()
            .iter()
            .any(|n| program.functions.contains_key(*n)))
    {
        return Err(invalid("invalid standard primitive layout"));
    }

    if matches!(
        program.language.as_str(),
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
    ) {
        for n in ["Json", "JsonError"] {
            if serde_json::to_value(program.structs.get(n)).ok()
                != serde_json::to_value(standard.structs.get(n)).ok()
                || serde_json::to_value(program.enums.get(n)).ok()
                    != serde_json::to_value(standard.enums.get(n)).ok()
            {
                return Err(invalid("invalid JSON standard layout"));
            }
        }
        if v091::reserved_functions()
            .iter()
            .any(|n| program.functions.contains_key(*n))
        {
            return Err(invalid("reserved application function"));
        }
    }
    if matches!(
        program.language.as_str(),
        "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
    ) && (serde_json::to_value(program.structs.get("WaitEdge")).ok()
        != serde_json::to_value(standard.structs.get("WaitEdge")).ok()
        || serde_json::to_value(program.enums.get("WaitTarget")).ok()
            != serde_json::to_value(standard.enums.get("WaitTarget")).ok())
    {
        return Err(invalid("invalid wait graph layout"));
    }
    if serde_json::to_value(program.enums.get("BudgetKind")).ok()
        != serde_json::to_value(standard.enums.get("BudgetKind")).ok()
        || serde_json::to_value(program.structs.get("WaitGraph")).ok()
            != serde_json::to_value(standard.structs.get("WaitGraph")).ok()
    {
        return Err(invalid("invalid error type layout"));
    }
    if serde_json::to_value(program.enums.get("TaskError")).ok()
        != serde_json::to_value(standard.enums.get("TaskError")).ok()
        || serde_json::to_value(program.structs.get("Diagnostic")).ok()
            != serde_json::to_value(standard.structs.get("Diagnostic")).ok()
    {
        return Err(invalid("invalid standard type layout"));
    }
    for n in [
        "Frozen",
        "Secret",
        "Iterator",
        "Task",
        "Channel",
        "TaskGroup",
    ] {
        if program.structs.contains_key(n) || program.enums.contains_key(n) {
            return Err(invalid("reserved standard type"));
        }
    }
    if matches!(
        program.language.as_str(),
        "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
    ) && serde_json::to_value(program.structs.get("PropertyFailure")).ok()
        != serde_json::to_value(standard.structs.get("PropertyFailure")).ok()
    {
        return Err(invalid("invalid property failure layout"));
    }
    if matches!(
        program.language.as_str(),
        "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
    ) && (program.structs.contains_key("Tuple") || program.enums.contains_key("Tuple"))
    {
        return Err(invalid("Tuple is a reserved standard type"));
    }
    for n in ["freeze", "thaw", "secret", "reveal"] {
        if program.functions.contains_key(n) {
            return Err(invalid("reserved standard function"));
        }
    }
    let effects: BTreeSet<String> =
        serde_json::from_value(payload["effects"].clone()).map_err(|e| invalid(&e.to_string()))?;
    if let Some(missing) = effects.difference(&options.allowed_effects).next() {
        return Err(invalid(&format!(
            "effect {missing} requires --allow-effects"
        )));
    }
    for symbol in program.impls.values().flat_map(|m| m.values()) {
        if !program.functions.contains_key(symbol) {
            return Err(invalid("missing method function"));
        }
    }
    for key in program.impls.keys() {
        if !program.impl_origins.contains_key(key) {
            return Err(invalid("missing impl source"));
        }
    }
    if program_v07(&program) {
        let actual = program
            .structs
            .get("PropertyCase")
            .ok_or_else(|| invalid("missing PropertyCase"))?;
        if !actual.bounds.is_empty()
            || actual.immutable
            || actual.type_params != vec!["T".to_string()]
            || actual.fields
                != vec![
                    ("seed".into(), "Int".into()),
                    ("case".into(), "Int".into()),
                    ("input".into(), "T".into()),
                    ("shrinks".into(), "Int".into()),
                ]
        {
            return Err(invalid("invalid PropertyCase layout"));
        }
    }
    let mut nodes = 0usize;
    let mut unsafe_literal = false;
    let mut inspect = |e: &Expr| {
        nodes += 1;
        if let ExprKind::Value(v) = &e.kind {
            if !matches!(
                v,
                Value::Bool(_)
                    | Value::Int(_)
                    | Value::Float(_)
                    | Value::Text(_)
                    | Value::Bytes(_)
                    | Value::Null
                    | Value::Option(None)
            ) {
                unsafe_literal = true;
            }
        }
    };
    expressions(&program.stmts, &mut inspect);
    for f in program.functions.values() {
        expressions(&f.body, &mut inspect);
    }
    if unsafe_literal || nodes > 100_000 {
        return Err(invalid("invalid literal/IR budget"));
    }
    if matches!(
        program.language.as_str(),
        "0.6"
            | "0.7"
            | "0.8"
            | "0.9"
            | "0.9.1"
            | "0.9.2"
            | "0.9.3"
            | "0.9.4"
            | "0.9.5"
            | "0.9.6"
            | "0.9.7"
            | "0.9.8"
            | "0.9.9"
            | "1.0.0"
    ) {
        v06::infer(&mut program)?;
    }
    check_program(&program)?;
    validate(
        &program,
        &project::ProjectConfig::artifact(
            &root,
            program.root_origin.clone(),
            options.allowed_effects.clone(),
        ),
    )?;
    // Recompile the typed IR with the trusted compiler, then compare every
    // instruction and entry. Untrusted operation strings are never executed.
    let code = vm::verified_code(&program)?;
    let functions=program.functions.iter().map(|(name,f)|(name.clone(),serde_json::json!({"async":f.asynchronous,"type_params":f.type_params,"parameters":f.params,"return":f.ret}))).collect::<BTreeMap<_,_>>();
    if payload["build"]["functions"] != serde_json::json!(functions)
        || payload["build"]["compiler"] != env!("CARGO_PKG_VERSION")
    {
        return Err(invalid("function/compiler metadata mismatch"));
    }
    if code["bytecode"] != payload["build"]["bytecode"]
        || code["function_entries"] != payload["build"]["function_entries"]
    {
        return Err(invalid("bytecode/type IR mismatch"));
    }
    options.artifact = Some(payload["build"].clone());
    vm::execute(program, &root, false, "run", options)
}
