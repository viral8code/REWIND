use super::*;
use serde_json::{json, Value as Json};
fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidOperation(format!("ReplayMismatch: {}", message.into()))
}
// Feed the existing Debug wire representation to the digest without a full String copy.
struct DigestFormatter<'a>(&'a mut sha2::Sha256);
impl std::fmt::Write for DigestFormatter<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        use sha2::Digest;
        self.0.update(text.as_bytes());
        Ok(())
    }
}
impl std::io::Write for DigestFormatter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        use sha2::Digest;
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod digest_tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn streaming_formatter_and_spilled_journal_preserve_digest_wire_bytes() {
        let value = Value::TypedList(
            "String".into(),
            vec![
                Value::Text("a\n😀\"\\".into()),
                Value::Text("x".repeat(32768)),
            ]
            .into(),
        );
        let expected = Sha256::digest(format!("{:?}", value).as_bytes());
        let mut hash = Sha256::new();
        std::fmt::write(&mut DigestFormatter(&mut hash), format_args!("{:?}", value)).unwrap();
        assert_eq!(hash.finalize(), expected);
        let mut journal = Journal::default();
        journal.append(b"first\n");
        journal.append(&vec![b'x'; 32768]);
        journal.append("😀".as_bytes());
        for segment in journal.segments() {
            segment.spill().unwrap();
        }
        let expected = Sha256::digest(journal.bytes().unwrap());
        let mut hash = Sha256::new();
        journal
            .write_since(&Journal::default(), &mut DigestFormatter(&mut hash))
            .unwrap();
        assert_eq!(hash.finalize(), expected);
    }
}
fn array(value: &Json) -> Result<&Vec<Json>> {
    value
        .as_array()
        .ok_or_else(|| invalid("expected journal array"))
}
fn string(value: &Json) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| invalid("expected journal string"))
}
fn number(value: &Json) -> Result<u64> {
    value
        .as_u64()
        .ok_or_else(|| invalid("expected journal integer"))
}
fn bytes(value: &Json) -> Result<Vec<u8>> {
    array(value)?
        .iter()
        .map(|b| u8::try_from(number(b)?).map_err(|_| invalid("invalid byte")))
        .collect()
}
impl Runtime {
    pub fn enable_virtual_publish(&mut self) {
        self.virtual_publish = true;
    }
    pub fn export_observations(&self) -> Result<Json> {
        let external_entries = self.export_external_entries()?;
        let raw_bytes = self.byte_input.iter().fold(0usize, |n, (_, s)| {
            let (a, b) = s.usage();
            n.saturating_add(a).saturating_add(b)
        });
        let raw_bytes = self
            .observations
            .values()
            .flat_map(|o| o.blocks.values())
            .fold(raw_bytes, |n, s| {
                let (a, b) = s.usage();
                n.saturating_add(a).saturating_add(b)
            });
        if raw_bytes > 16 * 1024 * 1024 {
            return Err(Error::InvalidOperation(
                "TraceBudgetExceeded: observation byte payload (16 MiB)".into(),
            ));
        }
        let mut files = Vec::new();
        for ((epoch, path), observation) in &self.observations {
            let mut blocks = Vec::new();
            for (index, block) in &observation.blocks {
                blocks.push(json!({"index":index,"bytes":block.bytes()?}));
            }
            files.push(json!({"epoch":epoch,"path":path,"version":observation.version.as_ref().map(|v| json!({"len":v.len,"hash":v.content_hash.to_string()})),"blocks":blocks}));
        }
        let env = self
            .env_observations
            .iter()
            .map(|(name, value)| {
                if self.env_secrets.contains(name) {
                    json!({"name":name,"secret":true})
                } else {
                    json!({"name":name,"value":value,"secret":false})
                }
            })
            .collect::<Vec<_>>();
        let directories = self
            .directory_observations
            .iter()
            .map(|((epoch, path), entries)| json!({"epoch":epoch,"path":path,"entries":entries}))
            .collect::<Vec<_>>();
        let byte_input = self
            .byte_input
            .iter()
            .map(|(limit, segment)| Ok((*limit, segment.bytes()?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(
            json!({"format":1,"external_format":2,"external_entries":external_entries,"external_polls":self.external_polls,"gui_events":self.gui_observations,"gui_window_events":self.gui_window_observations,"byte_input":byte_input,"byte_eof":self.byte_eof,"input":self.input.iter().enumerate().map(|(i,s)|if self.secret_input_indices.contains(&i){json!({"secret":true})}else{json!(s)}).collect::<Vec<_>>(),"input_eof":self.input_eof,"times":self.times.iter().map(u128::to_string).collect::<Vec<_>>(),"args":self.arguments,"locale":self.locale,"env":env,"entry_observations":self.entry_observations,"files":files,"directories":directories}),
        )
    }
    pub fn import_observations(&mut self, data: &Json) -> Result<()> {
        if !self.external_entries.is_empty() {
            return Err(invalid("external observations require a fresh runtime"));
        }
        if data["format"] != 1 {
            return Err(invalid("unsupported observation format"));
        }
        if data
            .get("external_format")
            .is_some_and(|v| v != 1 && v != 2)
        {
            return Err(invalid("unsupported external observation format"));
        }
        let external_entries: Vec<external::Entry> = data
            .get("external_entries")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(|_| invalid("invalid external ledger"))?
            .unwrap_or_default();
        if external_entries.len() > 1_000_000 || external_entries.iter().any(|e| !e.validate()) {
            return Err(invalid("invalid bounded external ledger"));
        }
        self.external_memory_bytes = external_entries
            .iter()
            .map(external::Entry::bytes)
            .fold(0usize, usize::saturating_add);
        self.external_polls = data
            .get("external_polls")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(|_| invalid("invalid external completion tape"))?
            .unwrap_or_default();
        if self.external_polls.len() > 1_000_000
            || self
                .external_polls
                .iter()
                .any(|p| p.operation >= external_entries.len())
        {
            return Err(invalid("invalid external completion tape"));
        }
        self.state.external_poll_cursor = 0;
        self.external_entries = external_entries;
        self.external_high_water = 0;
        self.state.external_cursor = 0;
        if data["format"] != 1 {
            return Err(invalid("unsupported observation format"));
        }
        let byte_input: Vec<(usize, Vec<u8>)> = match data.get("byte_input") {
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|_| invalid("invalid byte input journal"))?,
            None => Vec::new(),
        };
        if byte_input.iter().any(|(limit, bytes)| {
            !(1..=65536).contains(limit) || bytes.is_empty() || bytes.len() > *limit
        }) {
            return Err(invalid("invalid bounded byte observation"));
        }
        self.byte_input = byte_input
            .into_iter()
            .map(|(limit, bytes)| (limit, Arc::new(Segment::new(bytes))))
            .collect();
        self.byte_eof = data["byte_eof"] == true;
        self.secret_input_indices.clear();
        let mut secret_index = 0usize;
        self.input = array(&data["input"])?
            .iter()
            .enumerate()
            .map(|(index, value)| {
                if value["secret"] == true {
                    let text = self
                        .supplied_secret_input
                        .as_ref()
                        .and_then(|v| v.get(secret_index))
                        .cloned()
                        .ok_or_else(|| invalid("secret input requires --secret-input FILE"))?;
                    secret_index += 1;
                    self.secret_input_indices.insert(index);
                    self.sensitive_values.insert(text.clone());
                    Ok(format!("{text}\n"))
                } else {
                    string(value).map(str::to_string)
                }
            })
            .collect::<Result<_>>()?;
        self.input_eof = data["input_eof"]
            .as_bool()
            .ok_or_else(|| invalid("missing EOF observation"))?;
        self.times = array(&data["times"])?
            .iter()
            .map(|v| {
                string(v)?
                    .parse::<u128>()
                    .map_err(|_| invalid("invalid clock value"))
            })
            .collect::<Result<_>>()?;
        self.arguments = array(&data["args"])?
            .iter()
            .map(|v| string(v).map(str::to_string))
            .collect::<Result<_>>()?;
        self.locale = string(&data["locale"])?.into();
        self.env_observations.clear();
        for e in array(&data["env"])? {
            let name = string(&e["name"])?;
            if e["secret"] != true {
                self.env_allowed.insert(name.into());
            }
            let value = if e["secret"] == true {
                if !self.env_secrets.contains(name) {
                    return Err(invalid(format!("secret {name} requires --secret-env")));
                }
                std::env::var(name).ok()
            } else if e["value"].is_null() {
                None
            } else {
                Some(string(&e["value"])?.into())
            };
            self.env_observations.push((name.into(), value));
        }
        self.entry_observations = serde_json::from_value(data["entry_observations"].clone())
            .map_err(|e| invalid(e.to_string()))?;
        self.directory_observations.clear();
        for directory in array(&data["directories"])? {
            let path = string(&directory["path"])?;
            if path != "." {
                self.checked_path(path)?;
            }
            let entries = if directory["entries"].is_null() {
                None
            } else {
                Some(
                    array(&directory["entries"])?
                        .iter()
                        .map(|v| string(v).map(str::to_string))
                        .collect::<Result<BTreeSet<_>>>()?,
                )
            };
            self.directory_observations
                .insert((number(&directory["epoch"])?, path.into()), entries);
        }
        self.observations.clear();
        for file in array(&data["files"])? {
            let path = string(&file["path"])?;
            self.checked_path(path)?;
            let version = if file["version"].is_null() {
                None
            } else {
                Some(HostVersion {
                    len: number(&file["version"]["len"])?,
                    modified: None,
                    content_hash: string(&file["version"]["hash"])?
                        .parse()
                        .map_err(|_| invalid("invalid file hash"))?,
                })
            };
            let mut blocks = BTreeMap::new();
            for block in array(&file["blocks"])? {
                let index = usize::try_from(number(&block["index"])?)
                    .map_err(|_| invalid("block index too large"))?;
                let data = bytes(&block["bytes"])?;
                if data.len() > FILE_PAGE_SIZE {
                    return Err(invalid("block exceeds page size"));
                }
                blocks.insert(index, Arc::new(Segment::new(data)));
            }
            self.observations.insert(
                (number(&file["epoch"])?, path.into()),
                Observation { version, blocks },
            );
        }
        self.gui_observations = match data.get("gui_events") {
            Some(v) => serde_json::from_value(v.clone())
                .map_err(|_| invalid("invalid GUI event journal"))?,
            None => Vec::new(),
        };
        if self.gui_observations.len() > 1_000_000 {
            return Err(invalid("GUI event journal too large"));
        }
        for e in &self.gui_observations {
            e.validate().map_err(|_| invalid("invalid GUI event"))?;
        }
        self.gui_window_observations = match data.get("gui_window_events") {
            Some(v) => serde_json::from_value(v.clone())
                .map_err(|_| invalid("invalid named GUI event journal"))?,
            None => Vec::new(),
        };
        if self.gui_window_observations.len() > 1_000_000 {
            return Err(invalid("named GUI event journal too large"));
        }
        self.gui_window_observation_ends.clear();
        let mut end = 0usize;
        for e in &self.gui_window_observations {
            e.validate()
                .map_err(|_| invalid("invalid named GUI event"))?;
            end = end
                .checked_add(e.repeat)
                .ok_or_else(|| invalid("named GUI journal cursor overflow"))?;
            self.gui_window_observation_ends.push(end);
        }
        self.gui_window_observation_bytes = self
            .gui_window_observations
            .iter()
            .map(crate::gui::WindowInput::bytes)
            .sum();
        self.enforce_budget()?;
        self.replaying = true;
        Ok(())
    }
    /// Reuse a trusted in-process observation tape while allowing new observations.
    pub fn import_session_observations(&mut self, data: &Json) -> Result<()> {
        self.import_observations(data)?;
        self.replaying = false;
        Ok(())
    }
    pub fn masked_value(&self, value: &Value) -> String {
        let mut text = self.display_value(value);
        for (name, secret) in &self.env_observations {
            if self.env_secrets.contains(name) {
                if let Some(secret) = secret {
                    if !secret.is_empty() {
                        text = text.replace(secret, "<redacted>");
                    }
                }
            }
        }
        for secret in &self.sensitive_values {
            text = text.replace(secret, "<redacted>");
        }
        text
    }
    pub fn state_digest(&self) -> Result<String> {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        if self.incremental_publish {
            std::fmt::write(
                &mut DigestFormatter(&mut hash),
                format_args!(
                    "{:?}{:?}{:?}{:?}{:?}{}{}",
                    self.published_operations,
                    self.state.stdout.operations(),
                    self.state.stderr.operations(),
                    self.state.file_operations,
                    self.state.directory_operations,
                    self.next_operation,
                    self.published_epoch
                ),
            )
            .expect("digest formatting is infallible");
        }
        if !self.state.gui_windows_pending.is_empty()
            || !self.gui_window_frames.is_empty()
            || self.gui_window_high_water != 0
        {
            hash.update(b"gui.windows.v1");
            std::fmt::write(
                &mut DigestFormatter(&mut hash),
                format_args!(
                    "{:?}{:?}{}{}",
                    self.state.gui_windows_pending,
                    self.gui_window_frames,
                    self.state.gui_windows_cursor,
                    self.gui_window_high_water
                ),
            )
            .expect("digest formatting is infallible");
        }
        if self.external_high_water != 0 {
            hash.update(b"external.v1");
            hash.update((self.state.external_cursor as u64).to_le_bytes());
            hash.update((self.external_high_water as u64).to_le_bytes());
            hash.update((self.external_depth as u64).to_le_bytes());
            hash.update(self.external_owner.to_le_bytes());
            hash.update((self.state.external_poll_cursor as u64).to_le_bytes());
        }
        if self.incremental_publish {
            hash.update((self.state.byte_cursor as u64).to_le_bytes());
        }
        self.state
            .stdout
            .write_since(&Journal::default(), &mut DigestFormatter(&mut hash))?;
        hash.update([0]);
        self.state
            .stderr
            .write_since(&Journal::default(), &mut DigestFormatter(&mut hash))?;
        for (path, value) in self.state.files.iter() {
            hash.update(path.as_bytes());
            hash.update([0]);
            if let Some(file) = value {
                hash.update([1]);
                hash.update((file.len as u64).to_le_bytes());
                for (index, page) in file.pages.iter() {
                    hash.update((*index as u64).to_le_bytes());
                    page.write_to(&mut DigestFormatter(&mut hash))?;
                }
            } else {
                hash.update([2]);
            }
        }
        std::fmt::write(
            &mut DigestFormatter(&mut hash),
            format_args!("{:?}", self.state.directories),
        )
        .expect("digest formatting is infallible");
        std::fmt::write(
            &mut DigestFormatter(&mut hash),
            format_args!(
                "{:?}{:?}{:?}{:?}",
                self.state.heap, self.state.globals, self.state.stack, self.state.call_frames
            ),
        )
        .expect("digest formatting is infallible");
        if !self.state.native_owners.is_empty() {
            std::fmt::write(
                &mut DigestFormatter(&mut hash),
                format_args!("{:?}", self.state.native_owners),
            )
            .expect("digest formatting is infallible");
        }
        Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
    }
    pub fn debug_state(&self) -> Json {
        let deltas=self.state.files.iter().map(|(path,file)| {
            let delta=if let Some(file)=file {json!({"operation":"write","length":file.len,"changed_pages":file.pages.keys().collect::<Vec<_>>(),"content":if file.len<=65536 {file.to_vec().ok().map(|bytes|self.masked_value(&String::from_utf8(bytes.clone()).map(Value::Text).unwrap_or(Value::Bytes(bytes.into()))))}else{None}})}else{json!({"operation":"delete"})};(path.clone(),delta)
        }).collect::<BTreeMap<_,_>>();
        let states = std::iter::once(&self.state)
            .chain(self.checkpoints.values().map(|c| &c.state))
            .collect::<Vec<_>>();
        let heap_roots = states
            .iter()
            .map(|s| (Arc::as_ptr(&s.heap) as usize, &s.heap))
            .collect::<BTreeMap<_, _>>();
        let heap_bytes = heap_roots
            .values()
            .map(|h| h.logical_bytes())
            .sum::<usize>();
        let observed_bytes = self
            .observations
            .values()
            .flat_map(|o| o.blocks.values())
            .map(|b| {
                let (m, s) = b.usage();
                m + s
            })
            .sum::<usize>();
        let mut state = json!({"pc":self.state.program_counter,"heap_objects":self.state.heap.len(),"published_epoch":self.published_epoch,"published_operation_count":self.published_operations.len(),"byte_input_cursor":self.state.byte_cursor,"checkpoint_heap_roots":heap_roots.len(),"shared_checkpoint_heap_roots":states.len()-heap_roots.len(),"retained_heap_logical_bytes":heap_bytes,"observed_file_bytes":observed_bytes,"checkpoints":self.checkpoints.keys().collect::<Vec<_>>(),"stdout_bytes":self.state.stdout.len(),"stderr_bytes":self.state.stderr.len(),"journal_storage_bytes":self.state.stdout.storage_bytes()+self.state.stderr.storage_bytes(),"globals":self.state.globals.iter().map(|(n,v)|(n.clone(),self.masked_value(v))).collect::<BTreeMap<_,_>>(),"heap":self.state.heap.iter().map(|(id,v)|(id.to_string(),self.masked_value(v))).collect::<BTreeMap<_,_>>(),"files":self.state.files.keys().collect::<Vec<_>>(),"file_deltas":deltas,"directories":*self.state.directories,"cursors":{"input":self.state.stdin_cursor,"time":self.state.time_cursor,"env":self.state.env_cursor,"directory":self.state.directory_cursor}});
        if self.state.gui_pending.is_some()
            || self.gui_displayed.is_some()
            || !self.gui_observations.is_empty()
        {
            state["gui"] = json!({"cursor":self.state.gui_cursor,"pending":self.state.gui_pending.is_some(),"published":self.gui_displayed.is_some(),"widgets":self.gui_displayed.as_ref().map(|f|f.items.len())});
        }
        if !self.state.gui_windows_pending.is_empty()
            || !self.gui_window_frames.is_empty()
            || !self.gui_window_observations.is_empty()
        {
            state["gui_windows"] = json!({"cursor":self.state.gui_windows_cursor,"observed":self.gui_window_observations.len(),"pending":self.state.gui_windows_pending.keys().collect::<Vec<_>>(),"published":self.gui_window_frames.iter().map(|(id,f)|json!({"id":id,"title":f.title,"width":f.width,"height":f.height,"widgets":f.items.len()})).collect::<Vec<_>>()});
        }
        if self.external_live_used {
            state["external_live"] = json!({
                "retained_operations":self.external_live_entries.len(),
                "pending_operations":self.external_live_entries.values().filter(|v|v.entry.pending).count(),
                "recorded_operations":self.external_entries.len(),
                "recorded_polls":self.external_polls.len(),
                "native_resources":self.native_resource_count(),
                "reservations_and_metadata_bytes":self.external_memory_bytes,
                "outcome_bytes":self.external_live_entries.values().filter_map(|v|v.entry.outcome.as_ref()).map(|v|v.len()).sum::<usize>()
            });
        }
        state
    }
}
