use super::*;
use serde_json::{json, Value as Json};
mod repl;
pub(super) use repl::load as session_program;
pub(super) use repl::repl;

// Inspection never executes a program. Execution/replay still require the exact compiler.
pub(in crate::v2) fn readable_trace(trace: &Json) -> bool {
    trace["format"] == 1
        && (trace["compiler"] == env!("CARGO_PKG_VERSION")
            || (matches!(
                trace["compiler"].as_str(),
                Some("0.8.0" | "0.9.0" | "0.9.1")
            ) && trace["debug"]["index"]["format"] == 1))
}
pub(in crate::v2) fn inspection_trace(mut value: Json) -> Result<Json> {
    if value["kind"] != "rewind-inspection" {
        return Ok(value);
    }
    if value["format"] != 1
        || value["executable"] != false
        || !matches!(
            value["source_compiler"].as_str(),
            Some(
                "0.7.0"
                    | "0.8.0"
                    | "0.9.0"
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
                    | "2.0.0"
            )
        )
        || value["debug"]["index"]["format"] != 1
    {
        return Err(Error::InvalidOperation(
            "unsupported inspection bundle".into(),
        ));
    }
    value["compiler"] = json!(env!("CARGO_PKG_VERSION"));
    value["entry"] = json!("inspection.rw");
    value["fingerprint"] = value["source_sha256"].clone();
    value["artifact_sha256"] = value["source_sha256"].clone();
    Ok(value)
}
pub(super) fn trace_export(path: &Path, output: &Path, key: Option<&str>) -> Result<()> {
    if fs::metadata(path)?.len() > 128 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "trace export exceeds budget".into(),
        ));
    }
    if let Some(key) = key {
        packages::verify_file(
            path,
            &PathBuf::from(format!("{}.signature", path.display())),
            key,
            "trace",
        )?;
    }
    let bytes = fs::read(path)?;
    let trace: Json =
        serde_json::from_slice(&bytes).map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if !readable_trace(&trace) {
        return Err(Error::InvalidOperation(
            "unsupported inspection trace format/compiler".into(),
        ));
    }
    let events = trace["events"]
        .as_array()
        .ok_or_else(|| Error::InvalidOperation("missing events".into()))?;
    if trace["debug"]["index"]["deltas"]
        .as_array()
        .is_none_or(|a| a.len() != events.len())
        || trace["artifact_sha256"] != trace["fingerprint"]
    {
        return Err(Error::InvalidOperation("invalid indexed trace".into()));
    }
    // Validate all patches, not just the nearest final anchor.
    let mut state = v06::debug::view(&trace, 0)?
        .ok_or_else(|| Error::InvalidOperation("export requires an indexed trace".into()))?;
    for patches in trace["debug"]["index"]["deltas"].as_array().unwrap() {
        for patch in patches
            .as_array()
            .ok_or_else(|| Error::InvalidOperation("invalid index delta".into()))?
        {
            v06::debug::apply(&mut state, patch, true)?;
        }
    }
    if state["runtime"] != trace["debug"]["final"]["runtime"]
        || state["scheduler"] != trace["debug"]["final"]["scheduler"]
    {
        return Err(Error::InvalidOperation(
            "inspection final state mismatch".into(),
        ));
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let destination = fs::canonicalize(parent)?.join(
        output
            .file_name()
            .ok_or_else(|| Error::InvalidPath(output.display().to_string()))?,
    );
    if destination == fs::canonicalize(path)?
        || fs::symlink_metadata(output).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(Error::InvalidPath(
            "export must not overwrite its source or follow a symlink".into(),
        ));
    }
    let bundle = json!({"format":1,"kind":"rewind-inspection","source_compiler":trace["compiler"],"source_sha256":packages::hash(&bytes),"source_signature_verified":key.is_some(),"executable":false,"events":events,"debug":trace["debug"],"result":trace["result"],"audit":trace["audit"]});
    // No source program, observation journal, executable fingerprint or signature is copied.
    use std::io::Write;
    let bytes =
        serde_json::to_vec_pretty(&bundle).map_err(|e| Error::InvalidOperation(e.to_string()))?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&bytes)?;

    println!(
        "{}",
        json!({"kind":"rewind-inspection","events":events.len(),"source_compiler":trace["compiler"]})
    );
    Ok(())
}
