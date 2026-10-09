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
pub(in crate::v2) fn load(root: &Path) -> Result<Program> {
    let config = project::ProjectConfig::load(root)?
        .ok_or_else(|| Error::InvalidOperation("API snapshot requires a manifest".into()))?;
    config.lock(root, false)?;
    if !matches!(
        config.language.as_str(),
        "0.7"
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
            | "2.0.0"
    ) {
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
    let value = document(root)?;
    write(&value, output)
}
fn write(value: &Json, output: Option<&Path>) -> Result<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?
        + "\n";
    if text.len() > 8 * 1024 * 1024 {
        return Err(Error::InvalidOperation("API snapshot exceeds 8 MiB".into()));
    }
    if let Some(path) = output {
        fs::write(path, text)?;
    } else {
        print!("{text}");
    }
    Ok(())
}
fn document(root: &Path) -> Result<Json> {
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
            symbols.insert(format!("type:{n}"),json!({"kind":if d.immutable{"record"}else{"struct"},"generics":d.type_params,"constructor":d.private_fields.is_empty(),"fields":d.fields.iter().filter(|(n,_)| !d.private_fields.contains(n)).collect::<Vec<_>>()}));
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
    if matches!(
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
            | "1.9.53"
            | "1.9.54"
            | "1.9.55"
            | "2.0.0"
    ) {
        let source = fs::read_to_string(&p.root_origin)?;
        for line in source.lines() {
            let Some(rest) = line.trim().strip_prefix("// @api ") else {
                continue;
            };
            let parts = rest.splitn(3, ' ').collect::<Vec<_>>();
            if parts.len() != 3
                || !matches!(parts[1], "cost" | "failure")
                || parts[2].is_empty()
                || parts[2].len() > 1024
            {
                return Err(Error::InvalidOperation(
                    "invalid @api contract annotation".into(),
                ));
            }
            let Some(contract) = symbols.get_mut(&format!("fn:{}", parts[0])) else {
                return Err(Error::InvalidOperation(
                    "@api annotation must name a public function".into(),
                ));
            };
            if !contract[parts[1]].is_null() {
                return Err(Error::InvalidOperation("duplicate @api annotation".into()));
            }
            contract[parts[1]] = json!(parts[2]);
        }
    }
    if program_v09(&p) {
        let values = v07::constant_values(&p)?;
        for (name, value) in &values {
            if let Some(contract) = symbols.get_mut(&format!("const:{name}")) {
                contract["value_sha256"] = value.clone();
            }
        }
        for (name, d) in &p.structs {
            if let Some(contract) = symbols.get_mut(&format!("type:{name}")) {
                if !d.bounds.is_empty() {
                    contract["bounds"] = json!(d.bounds);
                }
            }
        }
        for ((tr, target), methods) in &p.impls {
            let exposed = p.traits.get(tr).is_some_and(|d| d.public)
                || p.structs
                    .get(target.split('<').next().unwrap_or(target))
                    .is_some_and(|d| d.public)
                || p.enums
                    .get(target.split('<').next().unwrap_or(target))
                    .is_some_and(|d| d.public);
            if exposed
                && p.impl_origins
                    .get(&(tr.clone(), target.clone()))
                    .is_some_and(|o| o == &p.root_origin)
            {
                let methods=methods.iter().map(|(name,symbol)|{let f=&p.functions[symbol];(name.clone(),json!({"parameters":f.params.iter().map(|(_,t)|t).collect::<Vec<_>>(),"return":f.ret,"effects":f.effects}))}).collect::<BTreeMap<_,_>>();
                symbols.insert(format!("impl:{tr}:{target}"),json!({"associated":p.impl_associated.get(&(tr.clone(),target.clone())),"generics":p.impl_generics.get(&(tr.clone(),target.clone())),"methods":methods}));
            }
        }
        // Include loaded dependency interfaces conservatively; body-only changes remain ignored.
        for origin in p.included_modules.iter().filter(|o| *o != &p.root_origin) {
            let mut exports = BTreeMap::new();
            for (n, f) in &p.functions {
                if f.public && &f.origin == origin {
                    exports.insert(format!("fn:{n}"),json!({"generics":f.type_params,"async":f.asynchronous,"parameters":f.params.iter().map(|(_,t)|t).collect::<Vec<_>>(),"return":f.ret,"effects":f.effects}));
                }
            }
            for (n, d) in &p.structs {
                if d.public && &d.origin == origin {
                    exports.insert(format!("type:{n}"),json!({"immutable":d.immutable,"generics":d.type_params,"bounds":d.bounds,"fields":d.fields.iter().filter(|(n,_)| !d.private_fields.contains(n)).collect::<Vec<_>>()}));
                }
            }
            for (n, d) in &p.enums {
                if d.public && &d.origin == origin {
                    exports.insert(
                        format!("type:{n}"),
                        json!({"generics":d.type_params,"variants":d.variants}),
                    );
                }
            }
            for (n, d) in &p.traits {
                if d.public && &d.origin == origin {
                    exports.insert(format!("trait:{n}"), semantic(d)?);
                }
            }
            for (n, d) in &p.aliases {
                if d.public && &d.origin == origin {
                    exports.insert(
                        format!("alias:{n}"),
                        json!({"params":d.params,"target":d.ty}),
                    );
                }
            }
            for (n, t) in &p.consts {
                if p.const_origins
                    .get(n)
                    .is_some_and(|(public, o)| *public && o == origin)
                {
                    exports.insert(
                        format!("const:{n}"),
                        json!({"type":t,"value_sha256":values.get(n)}),
                    );
                }
            }
            for ((tr, target), methods) in &p.impls {
                let exposed = p.traits.get(tr).is_some_and(|d| d.public)
                    || p.structs
                        .get(target.split('<').next().unwrap_or(target))
                        .is_some_and(|d| d.public)
                    || p.enums
                        .get(target.split('<').next().unwrap_or(target))
                        .is_some_and(|d| d.public);
                if exposed
                    && p.impl_origins
                        .get(&(tr.clone(), target.clone()))
                        .is_some_and(|o| o == origin)
                {
                    let signatures=methods.iter().map(|(name,symbol)|{let f=&p.functions[symbol];(name.clone(),json!({"parameters":f.params.iter().map(|(_,ty)|ty).collect::<Vec<_>>(),"return":f.ret,"effects":f.effects}))}).collect::<BTreeMap<_,_>>();
                    exports.insert(format!("impl:{tr}:{target}"),json!({"associated":p.impl_associated.get(&(tr.clone(),target.clone())),"generics":p.impl_generics.get(&(tr.clone(),target.clone())),"methods":signatures}));
                }
            }
            if !exports.is_empty() {
                let relative = origin
                    .strip_prefix(fs::canonicalize(root)?)
                    .map_err(|_| Error::InvalidPath(origin.display().to_string()))?;
                symbols.insert(
                    format!("dependency:{}", relative.to_string_lossy()),
                    json!(exports),
                );
            }
        }
    }
    Ok(
        json!({"format":if program_v09(&p){2}else{1},"kind":"rewind-api","language":p.language,"symbols":symbols}),
    )
}

pub(in crate::v2) fn convert(root: &Path, input: &Path, output: &Path) -> Result<()> {
    let old = read(input)?;
    if old["format"] != 1 {
        return Err(Error::InvalidOperation(
            "API conversion requires format 1".into(),
        ));
    }
    let new = document(root)?;
    if new["format"] != 2 {
        return Err(Error::InvalidOperation(
            "API conversion requires language 0.9".into(),
        ));
    }
    let mut legacy = new["symbols"].as_object().unwrap().clone();
    legacy.retain(|n, _| !n.starts_with("dependency:") && !n.starts_with("impl:"));
    for contract in legacy.values_mut() {
        if let Some(o) = contract.as_object_mut() {
            o.remove("value_sha256");
            o.remove("bounds");
        }
    }
    if json!(legacy) != old["symbols"] {
        return Err(Error::InvalidOperation(
            "ApiConversionMismatch: old signatures do not match current checked source".into(),
        ));
    }
    write(&new, Some(output))
}

fn read(path: &Path) -> Result<Json> {
    if fs::metadata(path)?.len() > 8 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "API snapshot budget exceeded".into(),
        ));
    }
    let v: Json = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if !matches!(v["format"].as_u64(), Some(1 | 2))
        || v["kind"] != "rewind-api"
        || !v["symbols"].is_object()
    {
        return Err(Error::InvalidOperation("unsupported API snapshot".into()));
    }
    Ok(v)
}
pub(in crate::v2) fn diff(before: &Path, after: &Path, deny: bool) -> Result<()> {
    let old = read(before)?;
    let new = read(after)?;
    if old["format"] != new["format"] {
        return Err(Error::InvalidOperation(
            "API schema generations differ; use api-convert with checked source".into(),
        ));
    }
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
        let equivalent_const = n.starts_with("const:")
            && a.get(n).is_some_and(|v| !v["value_sha256"].is_null())
            && b.get(n).is_some_and(|v| {
                v["value_sha256"] == a[n]["value_sha256"] && v["type"] == a[n]["type"]
            });
        let incompatible = !added && !equivalent_const;
        breaking |= incompatible;
        changes.push(json!({"symbol":n,"kind":if added{"added"}else if removed{"removed"}else if equivalent_const {"initializerChanged"}else{"contractChanged"},"breaking":incompatible,"before":a.get(n),"after":b.get(n)}));
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
