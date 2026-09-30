use super::*;
fn clean(value: &mut Json) {
    match value {
        Json::Object(m) => {
            m.remove("at");
            m.remove("origin");
            for v in m.values_mut() {
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
fn semantic(value: &impl serde::Serialize) -> Result<Json> {
    let mut value =
        serde_json::to_value(value).map_err(|e| Error::InvalidOperation(e.to_string()))?;
    clean(&mut value);
    Ok(value)
}
fn load(root: &Path) -> Result<Program> {
    let config = project::ProjectConfig::load(root)?
        .ok_or_else(|| Error::InvalidOperation("API snapshot requires a manifest".into()))?;
    config.lock(root, false)?;
    if !matches!(config.language.as_str(), "0.7" | "0.8") {
        return Err(Error::InvalidOperation(
            "API snapshot requires language 0.7".into(),
        ));
    }
    let mut p = load_program(
        &config.entry,
        root,
        &config.imports,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    p.language = config.language.clone();
    p.strict_visibility = true;
    v05::prepare(&mut p)?;
    check_program(&p)?;
    v05::validate(&p, &config)?;
    Ok(p)
}
pub(in crate::v2) fn snapshot(root: &Path, output: Option<&Path>) -> Result<()> {
    let p = load(root)?;
    let mut symbols = BTreeMap::new();
    for (n, f) in &p.functions {
        if f.public && f.origin == p.root_origin {
            symbols.insert(format!("fn:{n}"),json!({"async":f.asynchronous,"generics":f.type_params,"parameters":f.params.iter().map(|(_,t)|t).collect::<Vec<_>>(),"return":f.ret,"effects":f.effects}));
        }
    }
    for (n, d) in &p.structs {
        if d.public
            && d.origin == p.root_origin
            && !matches!(
                n.as_str(),
                "Diagnostic" | "WaitGraph" | "WaitEdge" | "PropertyFailure" | "PropertyCase"
            )
        {
            symbols.insert(format!("type:{n}"),json!({"kind":if d.immutable{"record"}else{"struct"},"generics":d.type_params,"fields":d.fields}));
        }
    }
    for (n, d) in &p.enums {
        if d.public
            && d.origin == p.root_origin
            && !matches!(n.as_str(), "TaskError" | "BudgetKind" | "WaitTarget")
        {
            symbols.insert(
                format!("type:{n}"),
                json!({"kind":"enum","generics":d.type_params,"variants":d.variants}),
            );
        }
    }
    for (n, d) in &p.traits {
        if d.public && d.origin == p.root_origin {
            let defaults = d
                .defaults
                .iter()
                .map(|(n, f)| Ok((n.clone(), semantic(&f.body)?)))
                .collect::<Result<BTreeMap<_, _>>>()?;
            symbols.insert(format!("trait:{n}"),json!({"associated":d.associated,"methods":d.methods,"effects":d.method_effects,"defaults":defaults}));
        }
    }
    for (n, d) in &p.aliases {
        if d.public && d.origin == p.root_origin {
            symbols.insert(
                format!("alias:{n}"),
                json!({"generics":d.params,"target":d.ty}),
            );
        }
    }
    for (n, t) in &p.consts {
        if p.const_origins
            .get(n)
            .is_some_and(|(public, origin)| *public && origin == &p.root_origin)
        {
            let initializer = p.stmts.iter().find_map(|s| match &s.kind {
                StmtKind::Let(name, _, _, e) if name == n => Some(e),
                _ => None,
            });
            symbols.insert(
                format!("const:{n}"),
                json!({"type":t,"initializer":initializer.map(semantic).transpose()?}),
            );
        }
    }
    let value = json!({"format":1,"kind":"rewind-api","language":p.language,"symbols":symbols});
    let text = serde_json::to_string_pretty(&value)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?
        + "\n";
    if let Some(path) = output {
        fs::write(path, text)?;
    } else {
        print!("{text}");
    }
    Ok(())
}
fn read(path: &Path) -> Result<Json> {
    if fs::metadata(path)?.len() > 8 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "API snapshot budget exceeded".into(),
        ));
    }
    let v: Json = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if v["format"] != 1 || v["kind"] != "rewind-api" || !v["symbols"].is_object() {
        return Err(Error::InvalidOperation("unsupported API snapshot".into()));
    }
    Ok(v)
}
pub(in crate::v2) fn diff(before: &Path, after: &Path, deny: bool) -> Result<()> {
    let old = read(before)?;
    let new = read(after)?;
    let a = old["symbols"].as_object().unwrap();
    let b = new["symbols"].as_object().unwrap();
    let mut changes = Vec::new();
    let mut breaking = false;
    for n in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
        if a.get(n) == b.get(n) {
            continue;
        }
        let removed = !b.contains_key(n);
        let added = !a.contains_key(n);
        let incompatible = !added;
        breaking |= incompatible;
        changes.push(json!({"symbol":n,"kind":if added{"added"}else if removed{"removed"}else{"contractChanged"},"breaking":incompatible,"before":a.get(n),"after":b.get(n)}));
    }
    println!(
        "{}",
        json!({"format":1,"compatible":!breaking,"changes":changes})
    );
    if deny && breaking {
        return Err(Error::InvalidOperation("BreakingApiChange".into()));
    }
    Ok(())
}
