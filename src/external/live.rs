//! Live observations retained only while a VM future/checkpoint owns its lease.
use super::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
pub(crate) const METADATA: usize = 512;
pub struct Lease {
    id: usize,
    releases: Weak<Mutex<VecDeque<usize>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leases_keep_private_outcomes_until_the_last_checkpoint_owner_drops() {
        let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
        runtime.configure_live_external(true);
        runtime.enter_external_live_task(0).unwrap();
        let (id, fresh) = runtime.begin_async_external("test", b"", 128).unwrap();
        assert!(fresh);
        let lease = runtime.live_external_lease(id).unwrap();
        let checkpoint = lease.clone();
        runtime
            .finish_async_external(id, Ok(serde_json::json!(7)))
            .unwrap();
        runtime.exit_external().unwrap();
        drop(lease);
        runtime.reclaim_live_external().unwrap();
        assert_eq!(runtime.external_live_entries.len(), 1);
        assert_eq!(runtime.poll_external(id).unwrap().unwrap().unwrap(), 7);
        drop(checkpoint);
        runtime.reclaim_live_external().unwrap();
        assert!(runtime.external_live_entries.is_empty());
        assert_eq!(runtime.external_memory_bytes, 0);
        assert_eq!(runtime.external_pending, 0);
        assert!(runtime.external_entries.is_empty());
        assert!(runtime.external_polls.is_empty());
        assert_eq!(runtime.state.external_cursor, 0);
        assert_eq!(runtime.state.external_poll_cursor, 0);
    }

    #[test]
    fn cleanup_budget_failure_preserves_every_release_for_a_later_attempt() {
        let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
        runtime.configure_live_external(true);
        runtime.enter_external_live_task(0).unwrap();
        let mut leases = Vec::new();
        for _ in 0..3 {
            let (id, _) = runtime.begin_async_external("test", b"", 128).unwrap();
            leases.push(runtime.live_external_lease(id).unwrap());
            runtime
                .finish_async_external(id, Ok(serde_json::json!(1)))
                .unwrap();
        }
        runtime.exit_external().unwrap();
        drop(leases);
        runtime.configure_native_work(0);
        assert!(runtime.reclaim_live_external().is_err());
        assert_eq!(runtime.external_live_entries.len(), 3);
        runtime.configure_native_work(1000);
        runtime.reclaim_live_external().unwrap();
        assert!(runtime.external_live_entries.is_empty());
        assert!(runtime.external_buffers.is_empty());
        assert_eq!(runtime.external_memory_bytes, 0);
    }

    #[test]
    fn an_unrecordable_result_is_never_retried_and_releases_its_reservation() {
        let mut runtime = Runtime::new(std::env::temp_dir()).unwrap();
        runtime.configure_live_external(true);
        runtime.enter_external_live_task(0).unwrap();
        let (id, _) = runtime.begin_async_external("test", b"", 4).unwrap();
        let lease = runtime.live_external_lease(id).unwrap();
        assert!(runtime
            .finish_async_external(id, Ok(serde_json::json!("too long")))
            .is_err());
        runtime.exit_external().unwrap();
        assert!(runtime
            .poll_external(id)
            .unwrap_err()
            .to_string()
            .contains("ExternalOutcomeUnknown"));
        drop(lease);
        runtime.reclaim_live_external().unwrap();
        assert_eq!(runtime.external_memory_bytes, 0);
        assert_eq!(runtime.external_pending, 0);
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(queue) = self.releases.upgrade() {
            queue
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push_back(self.id);
        }
    }
}
pub(crate) enum Resource {
    Database(usize),
    Http(usize),
}
pub(crate) struct LiveEntry {
    pub(crate) entry: Entry,
    lease: Weak<Lease>,
    creates_http_stream: bool,
    claimed: bool,
    resources: Vec<Resource>,
}
impl Runtime {
    /// Retain a live operation while its future/checkpoint can still be polled.
    /// Rust embedders must hold this lease; the VM attaches it to the Task.
    pub fn live_external_lease(&mut self, id: usize) -> Option<Arc<Lease>> {
        self.external_live_unattached
            .remove(&id)
            .or_else(|| self.external_live_entries.get(&id)?.lease.upgrade())
    }
    /// Transfer returned affine resources to the caller's ordinary handle lifetime.
    pub fn claim_live_external(&mut self, id: usize) {
        if let Some(live) = self.external_live_entries.get_mut(&id) {
            live.claimed = true;
        }
    }
    /// Cancel abandoned operations and release results whose last lease has dropped.
    pub fn reclaim_live_external(&mut self) -> Result<()> {
        if !self.external_live_used {
            return Ok(());
        }
        // A failed factory can leave a reservation without a VM task.
        self.external_live_unattached.clear();
        let mut released = std::mem::take(
            &mut *self
                .external_live_releases
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
        while let Some(id) = released.pop_front() {
            if !self.external_live_entries.contains_key(&id) {
                continue;
            }
            let cleanup = self.charge_native_work(1).and_then(|_| {
                if self.external_live_entries[&id].entry.pending {
                    self.cancel_http(id)
                } else {
                    Ok(())
                }
            });
            if let Err(error) = cleanup {
                released.push_front(id);
                self.external_live_releases
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .extend(released);
                return Err(error);
            }
            let live = self.external_live_entries.remove(&id).unwrap();
            self.external_buffers.remove(&id);
            self.external_memory_bytes = self.external_memory_bytes.saturating_sub(METADATA);
            if !live.claimed {
                for resource in live.resources {
                    match resource {
                        Resource::Database(id) => {
                            if let Some(host) = &mut self.database_host {
                                host.close_resource(id);
                            }
                        }
                        Resource::Http(id) => {
                            if let Some(host) = &mut self.network_host {
                                host.close(id);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn begin_live_external(
        &mut self,
        kind: &str,
        request: &[u8],
        max_result: usize,
    ) -> Result<(usize, bool)> {
        self.reclaim_live_external()?;
        if self.external_live_entries.len() >= ENTRIES {
            return Err(invalid("ExternalLimit: retained live observations"));
        }
        let id = self.external_live_next;
        let next = id
            .checked_add(1)
            .filter(|id| *id <= i64::MAX as usize)
            .ok_or_else(|| invalid("ExternalResourceLimit"))?;
        self.external_live_next = next;
        self.check_native_allocation(max_result.saturating_add(METADATA))?;
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(max_result)
            .map_err(|_| invalid("ExternalAllocation"))?;
        let lease = Arc::new(Lease {
            id,
            releases: Arc::downgrade(&self.external_live_releases),
        });
        let mut hash = Sha256::new();
        hash.update(kind.as_bytes());
        hash.update(request);
        self.external_live_entries.insert(
            id,
            LiveEntry {
                entry: Entry {
                    fingerprint: format!("{:x}", hash.finalize()),
                    outcome: None,
                    reservation: max_result,
                    pending: true,
                },
                lease: Arc::downgrade(&lease),
                creates_http_stream: matches!(kind, "http.download.v1" | "http.upload.v1"),
                claimed: false,
                resources: Vec::new(),
            },
        );
        self.external_live_unattached.insert(id, lease);
        self.external_buffers.insert(id, buffer);
        self.external_memory_bytes = self
            .external_memory_bytes
            .saturating_add(max_result + METADATA);
        if let Err(error) = self.enforce_budget() {
            self.external_live_entries.remove(&id);
            self.external_live_unattached.remove(&id);
            self.external_buffers.remove(&id);
            self.external_memory_bytes = self
                .external_memory_bytes
                .saturating_sub(max_result + METADATA);
            return Err(error);
        }
        self.external_pending += 1;
        Ok((id, true))
    }
    pub(crate) fn capture_live_resources(&mut self, id: usize, result: &Value) {
        let Some(live) = self.external_live_entries.get_mut(&id) else {
            return;
        };
        if result["adapter"] == "db" {
            for key in ["connection", "statement", "cursor"] {
                if let Some(id) = result[key].as_u64() {
                    live.resources.push(Resource::Database(id as usize));
                }
            }
        } else if live.creates_http_stream {
            if let Some(id) = result["stream"].as_u64() {
                live.resources.push(Resource::Http(id as usize));
            }
        }
    }
    pub(super) fn poll_live_external(
        &mut self,
        id: usize,
    ) -> Result<Option<std::result::Result<Value, String>>> {
        self.charge_native_work(1)?;
        let live = self
            .external_live_entries
            .get(&id)
            .ok_or_else(|| invalid("ExternalOperationUnknown"))?;
        if live.entry.pending {
            let database = self
                .database_host
                .as_ref()
                .is_some_and(|host| host.contains_job(id));
            let result = if database {
                self.database_host.as_mut().and_then(|host| host.poll(id))
            } else {
                self.network_host.as_mut().and_then(|host| host.poll(id))
            };
            if let Some(result) = result {
                self.capture_live_resources(id, &result);
                let result = if database {
                    self.sanitise_database_result(result)
                } else {
                    self.sanitise_http_result(result)
                };
                self.finish_async_external(id, Ok(result))?;
            }
        }
        let live = &self.external_live_entries[&id];
        if live.entry.pending {
            return Ok(None);
        }
        let wire = live.entry.outcome.as_ref().ok_or_else(|| {
            invalid("ExternalOutcomeUnknown: completed live outcome is unavailable")
        })?;
        let work = wire.len().saturating_add(1);
        self.charge_native_work(work)?;
        let bytes = self.external_live_entries[&id]
            .entry
            .outcome
            .as_ref()
            .unwrap()
            .read()?;
        Ok(Some(serde_json::from_slice(&bytes).map_err(|_| {
            invalid("ExternalOutcomeUnknown: invalid live outcome")
        })?))
    }
}
