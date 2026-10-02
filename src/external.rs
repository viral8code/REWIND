//! Irreversible host observations; checkpointed cursor, non-checkpointed ledger.
use crate::{Error, Result, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
const LIMIT: usize = 16 * 1024 * 1024;
const ENTRIES: usize = 1_000_000;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Entry {
    fingerprint: String,
    // None denotes an operation whose outcome could not be recorded. Never retry it.
    outcome: Option<String>,
    pub(crate) reservation: usize,
    #[serde(default)]
    pub(crate) pending: bool,
}
impl Entry {
    pub(crate) fn bytes(&self) -> usize {
        self.reservation + 192
    }
    pub(crate) fn validate(&self) -> bool {
        !self.pending
            && self.fingerprint.len() == 64
            && self.fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
            && self.reservation <= LIMIT
            && self.outcome.as_ref().is_none_or(|s| {
                s.len() <= self.reservation
                    && serde_json::from_str::<std::result::Result<Value, String>>(s).is_ok()
            })
    }
}
fn invalid(s: &str) -> Error {
    Error::InvalidOperation(s.into())
}
impl Runtime {
    pub(crate) fn export_external_entries(&self) -> Result<&[Entry]> {
        let bytes = self.external_memory_bytes;
        if bytes > 16 * 1024 * 1024 {
            return Err(invalid(
                "TraceBudgetExceeded: external observations (16 MiB)",
            ));
        }
        let patterns = self
            .http_secret_patterns()
            .map_err(|_| invalid("SecretObservationUnrecordable: secret validation limit"))?;
        let automaton = if patterns.is_empty() {
            None
        } else {
            Some(
                crate::network::SecretAutomaton::new(&patterns).map_err(|_| {
                    invalid("SecretObservationUnrecordable: secret validation limit")
                })?,
            )
        };
        let mut streams = std::collections::BTreeMap::new();
        for entry in &self.external_entries {
            if entry.pending {
                return Err(invalid(
                    "TraceIncompleteExternal: await or cancel pending operations before exporting",
                ));
            }
            if let Some(s) = &entry.outcome {
                let value: Value = serde_json::from_str(s)
                    .map_err(|_| invalid("ReplayMismatch: invalid external result"))?;
                if let (Some(automaton), Some(response)) = (&automaton, value.get("Ok")) {
                    if crate::network::protect_with_automaton(response, automaton) {
                        return Err(invalid("SecretObservationUnrecordable: private HTTP bytes"));
                    }
                    if let Some(stream) = response.get("stream").and_then(Value::as_u64) {
                        if let Some(body) = response.get("body").and_then(Value::as_str) {
                            use base64::Engine;
                            let bytes = base64::engine::general_purpose::STANDARD
                                .decode(body)
                                .map_err(|_| invalid("ReplayMismatch: invalid HTTP chunk"))?;
                            if (streams
                                .entry(stream)
                                .or_insert_with(|| automaton.matcher())
                                .scan)(&bytes)
                            {
                                return Err(invalid(
                                    "SecretObservationUnrecordable: private HTTP stream",
                                ));
                            }
                        } else {
                            streams.remove(&stream);
                        }
                    }
                    if let Some(stream) = response.get("closed").and_then(Value::as_u64) {
                        streams.remove(&stream);
                    }
                }
                if self.mask_debug_json(&value) != value {
                    return Err(invalid("SecretObservationUnrecordable: external result contains a registered secret"));
                }
            }
        }
        Ok(&self.external_entries)
    }
    pub fn require_internal(&self) -> Result<()> {
        if self.external_depth != 0 {
            Err(invalid(
                "ExternalBoundary: checkpoints, branches and publish require the VM region",
            ))
        } else {
            Ok(())
        }
    }
    pub fn enter_external(&mut self, fresh: bool) -> Result<()> {
        self.require_internal()?;
        if fresh {
            self.state.external_cursor = self.external_high_water;
            self.state.external_poll_cursor = self.external_poll_high_water;
        }
        self.external_depth = 1;
        self.external_owner = 0;
        Ok(())
    }
    pub fn enter_external_task(&mut self, fresh: bool, owner: u64) -> Result<()> {
        self.enter_external(fresh)?;
        self.external_owner = owner;
        Ok(())
    }
    pub fn exit_external(&mut self) -> Result<()> {
        if self.external_depth != 1 {
            return Err(invalid("ExternalBoundary: no active external region"));
        }
        self.external_depth = 0;
        self.external_owner = 0;
        Ok(())
    }
    pub fn external_operation(
        &mut self,
        kind: &str,
        request: &[u8],
        max_result: usize,
        host: impl FnOnce() -> std::result::Result<Value, String>,
    ) -> Result<std::result::Result<Value, String>> {
        if self.external_depth != 1 {
            return Err(invalid(
                "ExternalBoundary: operation requires external { ... }",
            ));
        }
        if max_result == 0 || max_result > LIMIT || request.len() > LIMIT {
            return Err(invalid("ExternalLimit"));
        }
        if let Ok(text) = std::str::from_utf8(request) {
            if self.masked_value(&crate::Value::Text(text.into())) != text {
                return Err(invalid(
                    "ExternalSecretRequest: use an opaque credential alias",
                ));
            }
        }
        let mut hash = Sha256::new();
        if self.external_owner != 0 {
            hash.update(b"task-external-v1");
            hash.update(self.external_owner.to_le_bytes());
        }
        hash.update((kind.len() as u64).to_le_bytes());
        hash.update(kind.as_bytes());
        hash.update(request);
        let fingerprint = format!("{:x}", hash.finalize());
        let cursor = self.state.external_cursor;
        if let Some(entry) = self.external_entries.get(cursor) {
            if entry.fingerprint != fingerprint {
                return Err(invalid(
                    "ExternalRequestMismatch: use external fresh for a new operation",
                ));
            }
            let text = entry
                .outcome
                .as_ref()
                .ok_or_else(|| invalid("ExternalOutcomeUnknown: automatic retry is forbidden"))?;
            let value = serde_json::from_str(text)
                .map_err(|_| invalid("ReplayMismatch: invalid external result"))?;
            self.state.external_cursor += 1;
            self.external_high_water = self.external_high_water.max(self.state.external_cursor);
            return Ok(value);
        }
        if self.replaying {
            return Err(invalid("ReplayMismatch: external journal exhausted"));
        }
        if cursor != self.external_entries.len() || cursor >= ENTRIES {
            return Err(invalid("ExternalLimit"));
        }
        self.external_entries
            .try_reserve(1)
            .map_err(|_| invalid("ExternalAllocation"))?;
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(max_result)
            .map_err(|_| invalid("ExternalAllocation"))?;
        self.external_entries.push(Entry {
            fingerprint,
            outcome: None,
            reservation: max_result,
            pending: false,
        });
        self.external_memory_bytes = self.external_memory_bytes.saturating_add(max_result + 192);
        if let Err(e) = self.enforce_budget() {
            self.external_entries.pop();
            self.external_memory_bytes =
                self.external_memory_bytes.saturating_sub(max_result + 192);
            return Err(e);
        }
        let result = host();
        struct Bounded<'a> {
            buffer: &'a mut Vec<u8>,
            limit: usize,
        }
        impl std::io::Write for Bounded<'_> {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                if self.buffer.len().saturating_add(b.len()) > self.limit {
                    return Err(std::io::Error::other("external result exceeds reservation"));
                }
                self.buffer.extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        if serde_json::to_writer(
            Bounded {
                buffer: &mut buffer,
                limit: max_result,
            },
            &result,
        )
        .is_err()
        {
            return Err(invalid("ExternalOutcomeUnknown: operation finished but result recording failed; automatic retry is forbidden"));
        }
        buffer.shrink_to_fit();
        let text = String::from_utf8(buffer).map_err(|_| invalid("ExternalOutcomeUnknown"))?;
        let entry = &mut self.external_entries[cursor];
        self.external_memory_bytes = self
            .external_memory_bytes
            .saturating_sub(entry.reservation)
            .saturating_add(text.capacity());
        entry.reservation = text.capacity();
        entry.outcome = Some(text);
        self.state.external_cursor += 1;
        self.external_high_water = self.external_high_water.max(self.state.external_cursor);
        Ok(result)
    }
}

/// A host resource identity never refers to a different resource after close.
/// VM adapters expose an affine wrapper; Rust tokens may be copied for lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceToken {
    registry: u64,
    id: u64,
}
/// Host-owned values live outside State and checkpoints. Dropping the registry
/// drops all remaining values. Individual close removes and drops the owner.
pub struct Resources<T> {
    identity: u64,
    next: u64,
    limit: usize,
    values: std::collections::BTreeMap<u64, T>,
}
impl<T> Resources<T> {
    pub fn new(limit: usize) -> Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let identity = NEXT
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |n| n.checked_add(1),
            )
            .map_err(|_| invalid("ExternalResourceLimit"))?;
        Ok(Self {
            identity,
            next: 1,
            limit,
            values: std::collections::BTreeMap::new(),
        })
    }
    pub fn insert(&mut self, value: T) -> Result<ResourceToken> {
        if self.values.len() >= self.limit {
            return Err(invalid("ExternalResourceLimit"));
        }
        let id = self.next;
        self.next = id
            .checked_add(1)
            .ok_or_else(|| invalid("ExternalResourceLimit"))?;
        self.values.insert(id, value);
        Ok(ResourceToken {
            registry: self.identity,
            id,
        })
    }
    pub fn get(&self, token: ResourceToken) -> Result<&T> {
        if token.registry != self.identity {
            return Err(invalid("ExternalResourceKind"));
        }
        self.values
            .get(&token.id)
            .ok_or_else(|| invalid("ExternalResourceClosed"))
    }
    pub fn get_mut(&mut self, token: ResourceToken) -> Result<&mut T> {
        if token.registry != self.identity {
            return Err(invalid("ExternalResourceKind"));
        }
        self.values
            .get_mut(&token.id)
            .ok_or_else(|| invalid("ExternalResourceClosed"))
    }
    pub fn close(&mut self, token: ResourceToken) -> Result<()> {
        if token.registry != self.identity {
            return Err(invalid("ExternalResourceKind"));
        }
        self.values
            .remove(&token.id)
            .ok_or_else(|| invalid("ExternalResourceClosed"))?;
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Poll {
    pub(crate) operation: usize,
    pub(crate) ready: bool,
}
impl Runtime {
    pub(crate) fn begin_async_external(
        &mut self,
        kind: &str,
        request: &[u8],
        max_result: usize,
    ) -> Result<(usize, bool)> {
        if self.external_depth != 1 {
            return Err(invalid(
                "ExternalBoundary: operation requires external region",
            ));
        }
        if max_result == 0 || max_result > LIMIT || request.len() > LIMIT {
            return Err(invalid("ExternalLimit"));
        }
        let mut hash = Sha256::new();
        if self.external_owner != 0 {
            hash.update(b"task-external-v1");
            hash.update(self.external_owner.to_le_bytes());
        }
        hash.update((kind.len() as u64).to_le_bytes());
        hash.update(kind.as_bytes());
        hash.update(request);
        let fingerprint = format!("{:x}", hash.finalize());
        let cursor = self.state.external_cursor;
        let fresh = if let Some(e) = self.external_entries.get(cursor) {
            if e.fingerprint != fingerprint {
                return Err(invalid(
                    "ExternalRequestMismatch: use external fresh for a new operation",
                ));
            }
            if !e.pending && e.outcome.is_none() {
                return Err(invalid(
                    "ExternalOutcomeUnknown: automatic retry is forbidden",
                ));
            }
            false
        } else {
            if self.replaying {
                return Err(invalid("ReplayMismatch: external journal exhausted"));
            }
            if cursor != self.external_entries.len() || cursor >= ENTRIES {
                return Err(invalid("ExternalLimit"));
            }
            self.external_entries
                .try_reserve(1)
                .map_err(|_| invalid("ExternalAllocation"))?;
            let mut buffer = Vec::new();
            buffer
                .try_reserve_exact(max_result)
                .map_err(|_| invalid("ExternalAllocation"))?;
            self.external_buffers.insert(cursor, buffer);
            self.external_entries.push(Entry {
                fingerprint,
                outcome: None,
                reservation: max_result,
                pending: true,
            });
            self.external_memory_bytes =
                self.external_memory_bytes.saturating_add(max_result + 192);
            if let Err(e) = self.enforce_budget() {
                self.external_buffers.remove(&cursor);
                self.external_entries.pop();
                self.external_memory_bytes =
                    self.external_memory_bytes.saturating_sub(max_result + 192);
                return Err(e);
            }
            true
        };
        if fresh {
            self.external_pending += 1;
        }
        self.state.external_cursor += 1;
        self.external_high_water = self.external_high_water.max(self.state.external_cursor);
        Ok((cursor, fresh))
    }
    pub(crate) fn finish_async_external(
        &mut self,
        id: usize,
        result: std::result::Result<Value, String>,
    ) -> Result<()> {
        let entry = self
            .external_entries
            .get_mut(id)
            .ok_or_else(|| invalid("ExternalOperationUnknown"))?;
        if !entry.pending {
            return Ok(());
        }
        entry.pending = false;
        self.external_pending = self.external_pending.saturating_sub(1);
        let mut buffer = self
            .external_buffers
            .remove(&id)
            .ok_or_else(|| invalid("ExternalOutcomeUnknown"))?;
        struct Bounded<'a>(&'a mut Vec<u8>, usize);
        impl std::io::Write for Bounded<'_> {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                if self.0.len().saturating_add(b.len()) > self.1 {
                    return Err(std::io::Error::other("external result limit"));
                }
                self.0.extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        serde_json::to_writer(Bounded(&mut buffer, entry.reservation), &result)
            .map_err(|_| invalid("ExternalOutcomeUnknown: result recording failed"))?;
        buffer.shrink_to_fit();
        let text = String::from_utf8(buffer).map_err(|_| invalid("ExternalOutcomeUnknown"))?;
        self.external_memory_bytes = self
            .external_memory_bytes
            .saturating_sub(entry.reservation)
            .saturating_add(text.capacity());
        entry.reservation = text.capacity();
        entry.outcome = Some(text);
        Ok(())
    }
    pub fn poll_external(
        &mut self,
        id: usize,
    ) -> Result<Option<std::result::Result<Value, String>>> {
        if id >= self.external_entries.len() {
            return Err(invalid("ExternalOperationUnknown"));
        }
        let cursor = self.state.external_poll_cursor;
        let ready = if let Some(p) = self.external_polls.get(cursor) {
            if p.operation != id {
                return Err(invalid("ReplayMismatch: external completion order changed"));
            }
            p.ready
        } else {
            if self.replaying {
                return Err(invalid(
                    "ReplayMismatch: external completion tape exhausted",
                ));
            }
            if cursor >= ENTRIES {
                return Err(invalid("ExternalLimit: completion polls"));
            }
            self.external_polls
                .try_reserve(1)
                .map_err(|_| invalid("ExternalAllocation"))?;
            self.external_polls.push(Poll {
                operation: id,
                ready: false,
            });
            if let Err(e) = self.enforce_budget() {
                self.external_polls.pop();
                return Err(e);
            }
            if self.external_entries[id].pending {
                let result = self.network_host.as_mut().and_then(|h| h.poll(id));
                if let Some(mut result) = result {
                    result = self.sanitise_http_result(result);
                    self.finish_async_external(id, Ok(result))?;
                }
            }
            let ready = !self.external_entries[id].pending;
            self.external_polls[cursor].ready = ready;
            ready
        };
        self.state.external_poll_cursor += 1;
        self.external_poll_high_water = self
            .external_poll_high_water
            .max(self.state.external_poll_cursor);
        if ready {
            let work = self.external_entries[id]
                .outcome
                .as_ref()
                .map_or(1, |s| s.len().saturating_add(1));
            self.charge_native_work(work)?;
            let s = self.external_entries[id]
                .outcome
                .as_ref()
                .ok_or_else(|| invalid("ExternalOutcomeUnknown: automatic retry is forbidden"))?;
            Ok(Some(serde_json::from_str(s).map_err(|_| {
                invalid("ReplayMismatch: invalid external result")
            })?))
        } else {
            Ok(None)
        }
    }
    pub fn external_waiting(&self) -> bool {
        self.external_pending != 0
    }
    pub fn wait_external_completion(&self) {
        if !self.replaying {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
