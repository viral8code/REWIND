//! VM ownership leases are checkpointed; physical resource lifetime is not.
use crate::{Error, Result, Runtime, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
pub fn resource_type(ty: &str) -> bool {
    matches!(
        ty,
        "HttpServer"
            | "HttpServerRequest"
            | "TcpSocket"
            | "HttpDownload"
            | "HttpUpload"
            | "DbConnection"
            | "DbStatement"
            | "DbCursor"
    )
}
pub fn token(value: &Value) -> Option<(u64, u64)> {
    if let Value::Struct(ty, fields) = value {
        if resource_type(ty) {
            if let (Some(Value::Int(id)), Some(Value::Int(lease))) =
                (fields.get("$native.id"), fields.get("$native.lease"))
            {
                return Some((u64::try_from(*id).ok()?, u64::try_from(*lease).ok()?));
            }
        }
    }
    None
}
fn error(message: &str) -> Error {
    Error::InvalidOperation(message.into())
}
impl Runtime {
    pub fn native_value(
        &mut self,
        ty: &str,
        id: u64,
        mut fields: BTreeMap<String, Value>,
    ) -> Result<Value> {
        let id_value = i64::try_from(id).map_err(|_| error("NativeResourceIdentityLimit"))?;
        Arc::make_mut(&mut self.state.native_owners).insert(id, 0);
        fields.insert("$native.id".into(), Value::Int(id_value));
        fields.insert("$native.lease".into(), Value::Int(0));
        Ok(Value::Struct(ty.into(), fields))
    }
    pub fn check_native(&self, value: &Value) -> Result<u64> {
        let (id, lease) = token(value).ok_or_else(|| error("NativeResourceInvalid"))?;
        if self.state.native_owners.get(&id) != Some(&lease) {
            return Err(error("NativeResourceMoved"));
        }
        Ok(id)
    }
    /// Claim an unowned returned value. A named alias remains a lexical borrow.
    pub fn claim_native(&mut self, value: &mut Value) -> Result<Option<(u64, u64)>> {
        let Some((id, lease)) = token(value) else {
            return Ok(None);
        };
        self.check_native(value)?;
        if lease != 0 {
            return Ok(None);
        }
        let next = self.next_native_lease;
        self.next_native_lease = next
            .checked_add(1)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or_else(|| error("NativeResourceIdentityLimit"))?;
        Arc::make_mut(&mut self.state.native_owners).insert(id, next);
        if let Value::Struct(_, fields) = value {
            fields.insert("$native.lease".into(), Value::Int(next as i64));
        }
        Ok(Some((id, next)))
    }
    pub fn move_native(&mut self, value: &mut Value) -> Result<()> {
        if let Some((id, _)) = token(value) {
            self.check_native(value)?;
            Arc::make_mut(&mut self.state.native_owners).insert(id, 0);
            if let Value::Struct(_, fields) = value {
                fields.insert("$native.lease".into(), Value::Int(0));
            }
            return Ok(());
        }
        match value {
            Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                self.move_native(v)?
            }
            Value::Struct(_, fields) => {
                for v in fields.values_mut() {
                    self.move_native(v)?;
                }
            }
            Value::Enum(_, _, fields) => {
                for (_, v) in fields {
                    self.move_native(v)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub fn native_owner_matches(&self, id: u64, lease: u64) -> bool {
        self.state.native_owners.get(&id) == Some(&lease)
    }
    pub fn forget_native_owner(&mut self, id: u64) {
        Arc::make_mut(&mut self.state.native_owners).remove(&id);
    }
    pub fn unclaimed_native_ids(&self, roots: &[Value]) -> BTreeSet<u64> {
        self.live_native_ids(roots)
            .into_iter()
            .filter(|id| self.state.native_owners.get(id) == Some(&0))
            .collect()
    }
    /// Checkpoint-only tokens intentionally do not retain a physical connection.
    pub fn live_native_ids(&self, roots: &[Value]) -> BTreeSet<u64> {
        let mut live = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut pending = roots.iter().collect::<Vec<_>>();
        while let Some(value) = pending.pop() {
            if let Some((id, lease)) = token(value) {
                if self.native_owner_matches(id, lease) {
                    live.insert(id);
                }
                continue;
            }
            match value {
                Value::HeapRef(id) | Value::CellRef(id) => {
                    if visited.insert(*id) {
                        if let Some(v) = self.heap_get(*id) {
                            pending.push(v);
                        }
                    }
                }
                Value::Struct(_, f) | Value::Closure(_, _, f) => pending.extend(f.values()),
                Value::Enum(_, _, f) => pending.extend(f.iter().map(|(_, v)| v)),
                Value::List(v) => pending.extend(v.iter()),
                Value::TypedList(_, v) => pending.extend(v.iter()),
                Value::Option(Some(v)) | Value::Result(Ok(v)) | Value::Result(Err(v)) => {
                    pending.push(v)
                }
                Value::Map(v) | Value::TypedMap(_, _, v) => {
                    pending.extend(v.iter().map(|(_, v)| v))
                }
                Value::OrderedMap(_, _, v) => {
                    for (k, v) in v {
                        pending.push(k);
                        pending.push(v);
                    }
                }
                _ => {}
            }
        }
        live
    }
}
