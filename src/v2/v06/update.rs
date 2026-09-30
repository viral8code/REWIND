use super::*;
use serde_json::{json, Value as Json};
fn old_hash(root: &Path) -> Option<String> {
    fs::read(root.join("rewind.lock")).ok().map(packages::hash)
}
fn proposal(root: &Path) -> Result<Json> {
    let config = project::ProjectConfig::load_for_update(root)?
        .ok_or_else(|| Error::InvalidOperation("rewind.toml is required".into()))?;
    if !matches!(config.language.as_str(), "0.6" | "0.7") {
        return Err(Error::InvalidOperation(
            "update preview/apply requires language 0.6".into(),
        ));
    }
    let lock = config.lock_document(root)?;
    let next: Json =
        serde_json::from_str(&lock).map_err(|e| Error::InvalidOperation(e.to_string()))?;
    let old: Json = fs::read(root.join("rewind.lock"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Json::Null);
    let names = old["dependencies"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.keys())
        .chain(
            next["dependencies"]
                .as_object()
                .into_iter()
                .flat_map(|m| m.keys()),
        )
        .cloned()
        .collect::<BTreeSet<_>>();
    let changes = names.into_iter().filter_map(|n| {
        let before = &old["dependencies"][&n]; let after = &next["dependencies"][&n];
        (before != after).then(|| json!({"package":n,"from":before,"to":after,"reason":"highest trusted local candidate satisfying the complete dependency graph","trust_changed":before["signer"]!=after["signer"]||before["public_key"]!=after["public_key"],"effects_changed":before["effects"]!=after["effects"]}))
    }).collect::<Vec<_>>();
    Ok(
        json!({"format":1,"kind":"rewind-update","manifest_sha256":packages::hash(fs::read(root.join("rewind.toml"))?),"previous_lock_sha256":old_hash(root),"changes":changes,"proposed_lock":lock}),
    )
}
pub(in crate::v2) fn preview(root: &Path, output: Option<&Path>) -> Result<()> {
    let bytes = serde_json::to_string_pretty(&proposal(root)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?
        + "\n";
    if let Some(path) = output {
        let lock = root.join("rewind.lock");
        if fs::canonicalize(path)
            .ok()
            .is_some_and(|p| fs::canonicalize(&lock).ok().as_ref() == Some(&p))
            || path == lock
        {
            return Err(Error::InvalidOperation(
                "preview output must differ from rewind.lock".into(),
            ));
        }
        fs::write(path, bytes)?;
    } else {
        print!("{bytes}");
    }
    Ok(())
}
pub(in crate::v2) fn apply(root: &Path, path: &Path) -> Result<()> {
    if fs::metadata(path)?.len() > 16 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "update proposal exceeds budget".into(),
        ));
    }
    let plan: Json = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if plan != proposal(root)? {
        return Err(Error::InvalidOperation("StaleUpdateProposal: manifest, lock, mirrors, signatures, or trust changed; preview again".into()));
    }
    let lock = root.join("rewind.lock");
    if fs::symlink_metadata(&lock).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::InvalidPath("rewind.lock symlink".into()));
    }
    let bytes = plan["proposed_lock"]
        .as_str()
        .ok_or_else(|| Error::InvalidOperation("missing proposed lock".into()))?;
    let mut nonce = [0u8; 8];
    getrandom::getrandom(&mut nonce).map_err(|e| Error::InvalidOperation(e.to_string()))?;
    let temp = root.join(format!(
        ".rewind-update-{:x}.tmp",
        u64::from_le_bytes(nonce)
    ));
    let result = (|| {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, lock)?;
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}
pub(in crate::v2) fn compatibility(path: &Path) -> Result<()> {
    if fs::metadata(path)?.len() > 128 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "compatibility input exceeds budget".into(),
        ));
    }
    let value: Json = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    let (kind, expected) = if value.get("payload").is_some() {
        ("artifact", 2)
    } else if value.get("events").is_some() {
        ("trace", 1)
    } else if value.get("dependencies").is_some() {
        ("lock", 2)
    } else {
        ("unknown", 0)
    };
    let readable = expected != 0
        && value["format"] == expected
        && value["compiler"] == env!("CARGO_PKG_VERSION");
    println!(
        "{}",
        json!({"kind":kind,"format":value["format"],"compiler":value["compiler"],"current_compiler":env!("CARGO_PKG_VERSION"),"readable":readable,"conversion_available":false,"reason":if readable {"format and compiler supported; normal validation is still required"} else {"unsupported format/compiler; rebuild or record with this compiler"}})
    );
    Ok(())
}
