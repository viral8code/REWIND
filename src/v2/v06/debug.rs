use super::*;
use serde_json::{json, Value as Json};
pub(in crate::v2) fn delta(before: &Json, after: &Json) -> Vec<Json> {
    fn walk(
        before: Option<&Json>,
        after: Option<&Json>,
        path: &mut Vec<String>,
        out: &mut Vec<Json>,
    ) {
        if before == after {
            return;
        }
        if let (Some(Json::Object(a)), Some(Json::Object(b))) = (before, after) {
            for key in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
                path.push(key.clone());
                walk(a.get(key), b.get(key), path, out);
                path.pop();
            }
        } else {
            out.push(json!({"path":path,"before":before,"after":after,"before_exists":before.is_some(),"after_exists":after.is_some()}));
        }
    }
    let mut out = Vec::new();
    walk(Some(before), Some(after), &mut Vec::new(), &mut out);
    out
}
pub(in crate::v2) fn apply(state: &mut Json, patch: &Json, forward: bool) -> Result<()> {
    let path = patch["path"]
        .as_array()
        .ok_or_else(|| Error::InvalidOperation("invalid debug index path".into()))?;
    if path.len() > 128 {
        return Err(Error::InvalidOperation(
            "debug index path budget exceeded".into(),
        ));
    }
    let field = if forward { "after" } else { "before" };
    let exists = if forward {
        "after_exists"
    } else {
        "before_exists"
    };
    if path.is_empty() {
        *state = patch[field].clone();
        return Ok(());
    }
    let mut target = state;
    for key in &path[..path.len() - 1] {
        let key = key
            .as_str()
            .ok_or_else(|| Error::InvalidOperation("invalid debug path".into()))?;
        target = target
            .get_mut(key)
            .ok_or_else(|| Error::InvalidOperation("missing debug index parent".into()))?;
    }
    let key = path
        .last()
        .and_then(Json::as_str)
        .ok_or_else(|| Error::InvalidOperation("invalid debug index key".into()))?;
    let object = target
        .as_object_mut()
        .ok_or_else(|| Error::InvalidOperation("invalid debug index object".into()))?;
    if patch[exists] == true {
        object.insert(key.into(), patch[field].clone());
    } else {
        object.remove(key);
    }
    Ok(())
}
pub(in crate::v2) fn view(trace: &Json, index: usize) -> Result<Option<Json>> {
    let data = &trace["debug"]["index"];
    if data.is_null() {
        return Ok(None);
    }
    if data["format"] != 1 {
        return Err(Error::InvalidOperation(
            "unsupported debug index format".into(),
        ));
    }
    let deltas = data["deltas"]
        .as_array()
        .ok_or_else(|| Error::InvalidOperation("missing debug index deltas".into()))?;
    if index > deltas.len() {
        return Err(Error::InvalidOperation("debug index out of range".into()));
    }
    let anchor = data["anchors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["event"].as_u64().is_some_and(|n| n as usize <= index))
        .max_by_key(|a| a["event"].as_u64());
    let (mut value, start) = if let Some(anchor) = anchor {
        (
            anchor["state"].clone(),
            anchor["event"].as_u64().unwrap() as usize,
        )
    } else {
        (data["initial"].clone(), 0)
    };
    for patches in &deltas[start..index] {
        for patch in patches
            .as_array()
            .ok_or_else(|| Error::InvalidOperation("invalid debug delta".into()))?
        {
            apply(&mut value, patch, true)?;
        }
    }
    Ok(Some(value))
}
