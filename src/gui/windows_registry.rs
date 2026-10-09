//! Named native surfaces. Staged scenes are VM state; published surfaces are not.
use super::{Event, Frame, Host, Request};
use crate::{Error, Result, Runtime};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
const MAX_WINDOWS: usize = 16;
const MAX_PIXELS: u64 = 16 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowEvent {
    pub window: String,
    pub event: Event,
}
impl WindowEvent {
    pub fn validate(&self) -> std::io::Result<()> {
        valid_id(&self.window)?;
        if self.event.kind == "idle" {
            return Err(super::invalid("GuiInvalidWindowEvent"));
        }
        self.event.validate()
    }
    pub(crate) fn bytes(&self) -> usize {
        self.window.len() + self.event.key.len() + 192
    }
}
fn one() -> usize {
    1
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WindowInput {
    pub request: Option<String>,
    pub poll: bool,
    pub result: Option<WindowEvent>,
    #[serde(default = "one")]
    pub repeat: usize,
    #[serde(default)]
    pub wait: bool,
    #[serde(default)]
    pub error: Option<String>,
}
impl WindowInput {
    pub(crate) fn validate(&self) -> std::io::Result<()> {
        if self.repeat == 0
            || self.repeat > 1_000_000
            || (self.repeat != 1 && (!self.poll || self.result.is_some()))
        {
            return Err(super::invalid("GuiInvalidWindowJournal"));
        }
        if self.wait && (!self.poll || self.request.is_some()) {
            return Err(super::invalid("GuiInvalidWindowJournal"));
        }
        if let Some(code) = &self.error {
            if !self.wait
                || self.repeat != 1
                || self.result.is_some()
                || !matches!(code.as_str(), "GuiClosed" | "GuiWindowEventTapeEnd")
            {
                return Err(super::invalid("GuiInvalidWindowJournal"));
            }
        }
        if let Some(id) = &self.request {
            valid_id(id)?;
        }
        if let Some(event) = &self.result {
            event.validate()?;
            if event.event.kind == "idle"
                || self.request.as_ref().is_some_and(|id| *id != event.window)
            {
                return Err(super::invalid("GuiInvalidWindowJournal"));
            }
        } else if !self.poll {
            return Err(super::invalid("GuiInvalidWindowJournal"));
        }
        Ok(())
    }
    pub(crate) fn bytes(&self) -> usize {
        self.request.as_ref().map_or(0, String::len)
            + self.result.as_ref().map_or(0, WindowEvent::bytes)
            + self.error.as_ref().map_or(0, String::len)
            + std::mem::size_of::<Self>()
            + std::mem::size_of::<usize>()
    }
}
fn valid_id(id: &str) -> std::io::Result<()> {
    if id.is_empty() || id.len() > 128 || id.contains('\0') {
        Err(super::invalid("GuiInvalidWindowId"))
    } else {
        Ok(())
    }
}
impl Runtime {
    pub fn configure_gui_window_events(&mut self, events: Vec<WindowEvent>) -> Result<()> {
        if events.len() > 1_000_000 {
            return Err(Error::InvalidOperation("GuiWindowEventLimit".into()));
        }
        for e in &events {
            e.validate()?;
        }
        let old_bytes = self.gui_window_scripted_bytes;
        self.gui_window_scripted_bytes = events.iter().map(WindowEvent::bytes).sum();
        let old = self.gui_window_scripted.replace(events.into());
        if let Err(e) = self.enforce_budget() {
            self.gui_window_scripted = old;
            self.gui_window_scripted_bytes = old_bytes;
            return Err(e);
        }
        Ok(())
    }
    fn gui_windows_capacity(&self, extra: Option<(&str, Option<&Frame>)>) -> Result<()> {
        let mut dimensions: BTreeMap<&str, (u64, u64)> = self
            .gui_window_frames
            .iter()
            .map(|(id, f)| (id.as_str(), (f.width as u64, f.height as u64)))
            .collect();
        for (id, r) in self.state.gui_windows_pending.iter() {
            if self.published_operations.contains(&r.id) {
                continue;
            }
            match &r.frame {
                Some(f) => {
                    dimensions.insert(id, (f.width as u64, f.height as u64));
                }
                None => {
                    dimensions.remove(id.as_str());
                }
            }
        }
        if let Some((id, frame)) = extra {
            match frame {
                Some(f) => {
                    dimensions.insert(id, (f.width as u64, f.height as u64));
                }
                None => {
                    dimensions.remove(id);
                }
            }
        }
        let legacy = self
            .state
            .gui_pending
            .as_ref()
            .map(|r| r.frame.as_deref())
            .unwrap_or(self.gui_displayed.as_deref());
        let pixels = dimensions.values().map(|(w, h)| w * h).sum::<u64>()
            + legacy.map_or(0, |f| f.width as u64 * f.height as u64);
        if dimensions.len() > MAX_WINDOWS || pixels > MAX_PIXELS {
            return Err(Error::InvalidOperation("GuiWindowCapacity".into()));
        }
        Ok(())
    }
    pub fn gui_window_stage(&mut self, id: &str, frame: Option<Frame>) -> Result<()> {
        valid_id(id)?;
        if let Some(f) = &frame {
            f.validate()?;
            if f.bytes() > 1024 * 1024 {
                return Err(Error::InvalidOperation("GuiSceneLimit".into()));
            }
        }
        if frame.is_none()
            && !self.gui_window_frames.contains_key(id)
            && !self.state.gui_windows_pending.contains_key(id)
        {
            return Err(Error::InvalidOperation("GuiWindowNotPublished".into()));
        }
        self.gui_windows_capacity(Some((id, frame.as_ref())))?;
        self.pending_transaction(|rt| {
            let operation = rt.operation()?;
            Arc::make_mut(&mut rt.state.gui_windows_pending).insert(
                id.into(),
                Request {
                    id: operation,
                    frame: frame.map(Arc::new),
                },
            );
            Ok(())
        })
    }
    pub fn gui_window_next_event(&mut self, id: &str) -> Result<Event> {
        self.gui_windows_read(Some(id), false, false)?
            .map(|e| e.event)
            .ok_or_else(|| Error::InvalidOperation("GuiWindowEventTapeEnd".into()))
    }
    pub fn gui_window_poll(&mut self, id: &str) -> Result<Option<Event>> {
        Ok(self
            .gui_windows_read(Some(id), true, false)?
            .map(|e| e.event))
    }
    pub fn gui_window_next_any(&mut self) -> Result<WindowEvent> {
        self.gui_windows_read(None, false, false)?
            .ok_or_else(|| Error::InvalidOperation("GuiWindowNotPublished".into()))
    }
    pub fn gui_window_poll_any(&mut self) -> Result<Option<WindowEvent>> {
        self.gui_windows_read(None, true, false)
    }
    /// Nonblocking scheduler poll with deterministic terminal input failures.
    pub fn gui_window_wait_poll_any(&mut self) -> Result<Option<WindowEvent>> {
        self.charge_native_work(1)?;
        if self.gui_window_frames.is_empty() {
            return Err(Error::InvalidOperation("GuiWindowNotPublished".into()));
        }
        self.gui_windows_read(None, true, true)
    }
    pub fn gui_window_continue_input(&mut self) {
        self.state.gui_windows_cursor = self.gui_window_high_water;
    }
    pub(super) fn gui_windows_capture(
        &mut self,
        request: Option<&str>,
        poll: bool,
        wait: bool,
    ) -> Result<Option<WindowEvent>> {
        let result = if let Some(tape) = self.gui_window_scripted.as_mut() {
            match tape.front() {
                Some(e) if request.is_none_or(|id| id == e.window) => {
                    if !self.gui_window_frames.contains_key(&e.window) {
                        return Err(Error::InvalidOperation(
                            "GuiWindowEventTapeUnpublished".into(),
                        ));
                    }
                    let event = tape.pop_front();
                    self.gui_window_scripted_bytes -= event.as_ref().map_or(0, WindowEvent::bytes);
                    event
                }
                _ if wait => return Err(Error::InvalidOperation("GuiWindowEventTapeEnd".into())),
                _ if poll => None,
                _ => {
                    return Err(Error::InvalidOperation("GuiWindowEventTapeEnd".into()));
                }
            }
        } else if let Some(id) = request {
            let host = self
                .gui_window_hosts
                .get_mut(id)
                .ok_or_else(|| Error::InvalidOperation("GuiWindowNotPublished".into()))?;
            let event = if poll {
                host.poll()?
            } else {
                Some(host.event()?)
            };
            event.map(|event| WindowEvent {
                window: id.into(),
                event,
            })
        } else {
            let mut ids: Vec<String> = self.gui_window_frames.keys().cloned().collect();
            if let Some(last) = &self.gui_window_last_polled {
                let start = ids.partition_point(|id| id <= last);
                ids.rotate_left(start);
            }
            let mut picked = None;
            loop {
                let mut closed = 0usize;
                for id in &ids {
                    if let Some(host) = self.gui_window_hosts.get_mut(id) {
                        match host.poll() {
                            Ok(Some(event)) => {
                                picked = Some(WindowEvent {
                                    window: id.clone(),
                                    event,
                                });
                                self.gui_window_last_polled = Some(id.clone());
                                break;
                            }
                            Ok(None) => {}
                            Err(e)
                                if e.to_string() == "GuiClosed"
                                    || (wait && e.to_string() == "GuiNotPublished") =>
                            {
                                closed += 1;
                            }
                            Err(e) => {
                                return Err(e.into());
                            }
                        }
                    }
                }
                if picked.is_none() && wait && closed == ids.len() {
                    return Err(Error::InvalidOperation("GuiClosed".into()));
                }
                if picked.is_some() || poll {
                    break;
                }
                if closed == ids.len() {
                    return Err(Error::InvalidOperation("GuiClosed".into()));
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            picked
        };
        Ok(result)
    }
    fn gui_windows_read(
        &mut self,
        request: Option<&str>,
        poll: bool,
        wait: bool,
    ) -> Result<Option<WindowEvent>> {
        if let Some(id) = request {
            valid_id(id)?;
            if !self.gui_window_frames.contains_key(id) {
                return Err(Error::InvalidOperation("GuiWindowNotPublished".into()));
            }
        } else if self.gui_window_frames.is_empty() {
            return Ok(None);
        }
        let cursor = self.state.gui_windows_cursor;
        let next_cursor = cursor
            .checked_add(1)
            .ok_or_else(|| Error::InvalidOperation("GuiWindowEventLimit".into()))?;
        let index = self
            .gui_window_observation_ends
            .partition_point(|end| *end <= cursor);
        let result = if let Some(record) = self.gui_window_observations.get(index) {
            if record.request.as_deref() != request || record.poll != poll || record.wait != wait {
                return Err(Error::InvalidOperation(
                    "ReplayMismatch: named GUI request or polling mode".into(),
                ));
            }
            if let Some(code) = &record.error {
                self.state.gui_windows_cursor = next_cursor;
                self.gui_window_high_water = self.gui_window_high_water.max(next_cursor);
                return Err(Error::InvalidOperation(code.clone()));
            }
            record.result.clone()
        } else {
            if self.replaying {
                return Err(Error::InvalidOperation(
                    "ReplayMismatch: named GUI journal exhausted".into(),
                ));
            }
            if self.gui_window_observations.len() >= 1_000_000 {
                return Err(Error::InvalidOperation("GuiWindowEventLimit".into()));
            }
            // Reserve the largest admitted event before consuming any native/scripted input.
            self.check_native_allocation(if wait { 65536 } else { 8192 })?;
            let capture = self.gui_windows_capture(request, poll, wait);
            let (result, error) = match capture {
                Ok(result) => (result, None),
                Err(Error::InvalidOperation(code))
                    if wait && matches!(code.as_str(), "GuiClosed" | "GuiWindowEventTapeEnd") =>
                {
                    (None, Some(code))
                }
                Err(error) => return Err(error),
            };
            let record = WindowInput {
                request: request.map(str::to_owned),
                poll,
                result: result.clone(),
                repeat: 1,
                wait,
                error: error.clone(),
            };
            record.validate()?;
            let compress = self.gui_window_observations.last().is_some_and(|previous| {
                poll && result.is_none()
                    && previous.poll
                    && previous.result.is_none()
                    && previous.request == record.request
                    && previous.repeat < 1_000_000
            });
            let compress = compress
                && error.is_none()
                && self
                    .gui_window_observations
                    .last()
                    .is_some_and(|p| p.error.is_none() && p.wait == wait);
            if compress {
                self.gui_window_observations.last_mut().unwrap().repeat += 1;
                *self.gui_window_observation_ends.last_mut().unwrap() = next_cursor;
            } else {
                self.gui_window_observation_bytes += record.bytes();
                self.gui_window_observations.push(record);
                self.gui_window_observation_ends.push(next_cursor);
            }
            self.enforce_budget()?;
            if let Some(code) = error {
                self.state.gui_windows_cursor = next_cursor;
                self.gui_window_high_water = self.gui_window_high_water.max(next_cursor);
                return Err(Error::InvalidOperation(code));
            }
            result
        };
        if result
            .as_ref()
            .is_some_and(|e| !self.gui_window_frames.contains_key(&e.window))
        {
            return Err(Error::InvalidOperation(
                "ReplayMismatch: event window not published".into(),
            ));
        }
        self.state.gui_windows_cursor = next_cursor;
        self.gui_window_high_water = self
            .gui_window_high_water
            .max(self.state.gui_windows_cursor);
        Ok(result)
    }
    pub(crate) fn gui_windows_prepare(&mut self) -> Result<()> {
        self.gui_windows_capacity(None)?;
        let allocation = self
            .state
            .gui_windows_pending
            .values()
            .filter(|r| !self.published_operations.contains(&r.id))
            .filter_map(|r| r.frame.as_ref())
            .fold(0usize, |n, f| {
                n.saturating_add(f.bytes().saturating_mul(2))
                    .saturating_add(
                        (f.width as usize)
                            .saturating_mul(f.height as usize)
                            .saturating_mul(4),
                    )
                    .saturating_add(8192)
            });
        self.check_native_allocation(allocation)?;
        if self.replaying || self.virtual_publish || self.gui_window_scripted.is_some() {
            return Ok(());
        }
        if self.gui_clipboard_enabled || self.gui_ime_enabled {
            let new_hosts = self
                .state
                .gui_windows_pending
                .iter()
                .filter(|(id, r)| {
                    r.frame.is_some()
                        && !self.published_operations.contains(&r.id)
                        && !self.gui_window_hosts.contains_key(*id)
                })
                .count();
            self.check_native_allocation(
                allocation.saturating_add(new_hosts.saturating_mul(self.gui_host_reservation())),
            )?;
        }
        self.gui_window_hosts.retain(|id, _| {
            self.gui_window_frames.contains_key(id)
                || self
                    .state
                    .gui_windows_pending
                    .get(id)
                    .is_some_and(|r| r.frame.is_some())
        });
        let mut prepared = BTreeMap::new();
        for (id, r) in self.state.gui_windows_pending.iter() {
            if r.frame.is_some()
                && !self.published_operations.contains(&r.id)
                && !self.gui_window_hosts.contains_key(id)
            {
                let mut host = Host::prepare()
                    .map_err(|e| Error::InvalidOperation(format!("GuiUnavailable: {e}")))?;
                host.configure_clipboard(self.gui_clipboard_enabled);
                host.configure_ime(self.gui_ime_enabled);
                host.configure_command_keys(self.gui_command_keys_enabled);
                prepared.insert(id.clone(), host);
            }
        }
        self.gui_window_hosts.extend(prepared);
        Ok(())
    }
    pub(crate) fn gui_windows_apply(&mut self, applied: &mut Vec<String>) -> std::io::Result<()> {
        let requests = self.state.gui_windows_pending.clone();
        for (id, r) in requests.iter() {
            if self.published_operations.contains(&r.id) {
                continue;
            }
            if !self.replaying && !self.virtual_publish && self.gui_window_scripted.is_none() {
                match &r.frame {
                    Some(frame) => {
                        self.gui_window_hosts
                            .get_mut(id)
                            .ok_or_else(|| super::invalid("GuiUnavailable"))?
                            .present(frame)?;
                    }
                    None => {
                        if let Some(mut host) = self.gui_window_hosts.remove(id) {
                            host.close();
                        }
                    }
                }
            }
            match &r.frame {
                Some(frame) => {
                    self.gui_window_frames.insert(id.clone(), frame.clone());
                }
                None => {
                    self.gui_window_frames.remove(id);
                }
            }
            self.published_operations.insert(r.id);
            applied.push(format!("gui-window {id}"));
        }
        Ok(())
    }
    pub fn gui_window_displayed_frame(&self, id: &str) -> Option<&Frame> {
        self.gui_window_frames.get(id).map(AsRef::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(title: &str) -> Frame {
        Frame {
            title: title.into(),
            width: 128,
            height: 96,
            background: 0xffffff,
            items: vec![],
        }
    }
    fn runtime() -> Runtime {
        let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
        rt.enable_incremental_publish();
        rt
    }
    fn publish(rt: &mut Runtime) {
        rt.publish(false, &mut Vec::new(), &mut Vec::new()).unwrap();
    }
    #[test]
    fn async_fixture_end_is_recorded_and_replayed_without_host_input() {
        let mut rt = runtime();
        rt.enable_virtual_publish();
        rt.gui_window_stage("one", Some(frame("One"))).unwrap();
        publish(&mut rt);
        rt.configure_gui_window_events(vec![WindowEvent {
            window: "one".into(),
            event: Event::simple("key"),
        }])
        .unwrap();
        rt.commit("waiting").unwrap();
        assert_eq!(
            rt.gui_window_wait_poll_any().unwrap().unwrap().event.kind,
            "key"
        );
        assert!(
            matches!(rt.gui_window_wait_poll_any(),Err(Error::InvalidOperation(s)) if s=="GuiWindowEventTapeEnd")
        );
        rt.revert("waiting").unwrap();
        assert!(rt.gui_window_wait_poll_any().unwrap().is_some());
        assert!(
            matches!(rt.gui_window_wait_poll_any(),Err(Error::InvalidOperation(s)) if s=="GuiWindowEventTapeEnd")
        );
        let data = rt.export_observations().unwrap();
        assert_eq!(
            data["gui_window_events"][1]["error"],
            "GuiWindowEventTapeEnd"
        );
        let mut replay = runtime();
        replay.import_observations(&data).unwrap();
        replay.gui_window_stage("one", Some(frame("One"))).unwrap();
        publish(&mut replay);
        assert!(replay.gui_window_wait_poll_any().unwrap().is_some());
        assert!(
            matches!(replay.gui_window_wait_poll_any(),Err(Error::InvalidOperation(s)) if s=="GuiWindowEventTapeEnd")
        );
        assert!(replay.gui_window_hosts.is_empty());
        // A sync poll cannot consume an async readiness record.
        replay.state.gui_windows_cursor = 0;
        assert!(
            matches!(replay.gui_window_poll_any(),Err(Error::InvalidOperation(s)) if s.starts_with("ReplayMismatch:"))
        );
        let mut corrupt = data.clone();
        corrupt["gui_window_events"][1]["repeat"] = serde_json::json!(2);
        assert!(runtime().import_observations(&corrupt).is_err());
    }
    #[test]
    fn empty_poll_runs_keep_exact_restore_positions_and_bounded_storage() {
        let mut rt = runtime();
        rt.enable_virtual_publish();
        rt.gui_window_stage("one", Some(frame("One"))).unwrap();
        publish(&mut rt);
        rt.configure_gui_window_events(vec![]).unwrap();
        for _ in 0..100 {
            assert!(rt.gui_window_poll_any().unwrap().is_none());
        }
        rt.commit("inside").unwrap();
        for _ in 0..10000 {
            assert!(rt.gui_window_poll_any().unwrap().is_none());
        }
        assert_eq!(rt.gui_window_observations.len(), 1);
        assert_eq!(rt.gui_window_observations[0].repeat, 10100);
        assert!(rt.gui_window_observation_bytes < 256);
        rt.revert("inside").unwrap();
        assert_eq!(rt.state.gui_windows_cursor, 100);
        for _ in 0..10000 {
            assert!(rt.gui_window_poll_any().unwrap().is_none());
        }
        // Extending a consumed run must be visible again after a later restore.
        assert!(rt.gui_window_poll_any().unwrap().is_none());
        assert_eq!(rt.gui_window_observations[0].repeat, 10101);
        rt.revert("inside").unwrap();
        rt.gui_window_continue_input();
        assert_eq!(rt.state.gui_windows_cursor, 10101);
        rt.configure_gui_window_events(vec![WindowEvent {
            window: "one".into(),
            event: Event::simple("close"),
        }])
        .unwrap();
        assert_eq!(
            rt.gui_window_poll_any().unwrap().unwrap().event.kind,
            "close"
        );
        let data = rt.export_observations().unwrap();
        let mut replay = runtime();
        replay.import_observations(&data).unwrap();
        replay.gui_window_stage("one", Some(frame("One"))).unwrap();
        publish(&mut replay);
        for _ in 0..10101 {
            assert!(replay.gui_window_poll_any().unwrap().is_none());
        }
        assert_eq!(
            replay.gui_window_poll_any().unwrap().unwrap().event.kind,
            "close"
        );
        assert!(replay.gui_window_hosts.is_empty());
    }
    #[test]
    fn old_single_poll_tapes_load_and_invalid_repeated_events_are_rejected() {
        let mut rt = runtime();
        rt.enable_virtual_publish();
        rt.gui_window_stage("one", Some(frame("One"))).unwrap();
        publish(&mut rt);
        rt.configure_gui_window_events(vec![]).unwrap();
        rt.gui_window_poll_any().unwrap();
        let mut data = rt.export_observations().unwrap();
        data["gui_window_events"][0]
            .as_object_mut()
            .unwrap()
            .remove("repeat");
        let mut replay = runtime();
        replay.import_observations(&data).unwrap();
        assert_eq!(replay.gui_window_observation_ends, vec![1]);
        for repeat in [0, 1000001] {
            data["gui_window_events"][0]["repeat"] = serde_json::json!(repeat);
            assert!(runtime().import_observations(&data).is_err());
        }
        data["gui_window_events"][0]["repeat"] = serde_json::json!(2);
        data["gui_window_events"][0]["poll"] = serde_json::json!(false);
        assert!(runtime().import_observations(&data).is_err());
        data["gui_window_events"][0]["poll"] = serde_json::json!(true);
        data["gui_window_events"][0]["result"] =
            serde_json::json!({"window":"one","event":Event::simple("close")});
        assert!(runtime().import_observations(&data).is_err());
    }
    #[test]
    fn staged_windows_revert_published_windows_survive_and_journal_replays() {
        let mut rt = runtime();
        rt.enable_virtual_publish();
        rt.commit("begin").unwrap();
        rt.gui_window_stage("one", Some(frame("One"))).unwrap();
        rt.commit("staged").unwrap();
        rt.revert("begin").unwrap();
        publish(&mut rt);
        assert!(rt.gui_window_displayed_frame("one").is_none());
        rt.revert("staged").unwrap();
        rt.gui_window_stage("two", Some(frame("Two"))).unwrap();
        publish(&mut rt);
        rt.revert("begin").unwrap();
        assert_eq!(rt.gui_window_frames.len(), 2);
        rt.configure_gui_window_events(vec![
            WindowEvent {
                window: "two".into(),
                event: Event::simple("close"),
            },
            WindowEvent {
                window: "one".into(),
                event: Event::simple("key"),
            },
        ])
        .unwrap();
        assert!(rt.gui_window_poll("one").unwrap().is_none());
        rt.commit("input").unwrap();
        let event = rt.gui_window_poll_any().unwrap().unwrap();
        assert_eq!(event.window, "two");
        rt.revert("input").unwrap();
        assert_eq!(rt.gui_window_poll_any().unwrap().unwrap().window, "two");
        rt.gui_window_continue_input();
        assert_eq!(rt.gui_window_next_event("one").unwrap().kind, "key");
        let observations = rt.export_observations().unwrap();
        let mut replay = runtime();
        replay.import_observations(&observations).unwrap();
        replay.gui_window_stage("one", Some(frame("One"))).unwrap();
        replay.gui_window_stage("two", Some(frame("Two"))).unwrap();
        publish(&mut replay);
        assert!(replay.gui_window_poll("one").unwrap().is_none());
        assert_eq!(replay.gui_window_poll_any().unwrap().unwrap().window, "two");
        assert_eq!(replay.gui_window_next_event("one").unwrap().kind, "key");
        assert!(replay.gui_window_hosts.is_empty());
        rt.gui_window_stage("one", None).unwrap();
        publish(&mut rt);
        assert!(rt.gui_window_displayed_frame("one").is_none());
        assert!(rt.gui_window_displayed_frame("two").is_some());
        rt.revert("staged").unwrap();
        publish(&mut rt);
        assert!(
            rt.gui_window_displayed_frame("one").is_none(),
            "stale published scene must not reopen a closed surface"
        );
    }
    #[test]
    fn input_admission_does_not_consume_tape_and_replay_checks_mode() {
        let mut rt = runtime();
        rt.enable_virtual_publish();
        rt.gui_window_stage("one", Some(frame("One"))).unwrap();
        publish(&mut rt);
        rt.configure_gui_window_events(vec![WindowEvent {
            window: "one".into(),
            event: Event::simple("close"),
        }])
        .unwrap();
        let old = rt.budget;
        let mut low = old;
        low.history_memory = 128 * 96 * 4 + 1000;
        rt.set_budget(low).unwrap();
        assert!(rt.gui_window_poll("one").is_err());
        assert_eq!(rt.gui_window_scripted.as_ref().unwrap().len(), 1);
        assert!(rt.gui_window_observations.is_empty());
        rt.set_budget(old).unwrap();
        assert_eq!(rt.gui_window_poll("one").unwrap().unwrap().kind, "close");
        let mut replay = runtime();
        replay
            .import_observations(&rt.export_observations().unwrap())
            .unwrap();
        replay.gui_window_stage("one", Some(frame("One"))).unwrap();
        publish(&mut replay);
        assert!(replay
            .gui_window_next_event("one")
            .unwrap_err()
            .to_string()
            .contains("polling mode"));
        assert_eq!(replay.state.gui_windows_cursor, 0);
        assert_eq!(
            replay.gui_window_poll("one").unwrap().unwrap().kind,
            "close"
        );
    }
    #[test]
    fn window_capacity_and_scene_validation_are_preflighted() {
        let mut rt = runtime();
        rt.enable_virtual_publish();
        assert!(rt.gui_window_stage("", Some(frame("bad"))).is_err());
        assert!(rt.gui_window_poll("missing").is_err());
        for i in 0..MAX_WINDOWS {
            rt.gui_window_stage(&format!("w{i}"), Some(frame("window")))
                .unwrap();
        }
        assert!(rt.gui_window_stage("extra", Some(frame("extra"))).is_err());
        assert_eq!(rt.state.gui_windows_pending.len(), MAX_WINDOWS);
        publish(&mut rt);
        assert_eq!(rt.gui_window_frames.len(), MAX_WINDOWS);
        rt.gui_window_stage("w0", None).unwrap();
        rt.gui_window_stage("extra", Some(frame("extra"))).unwrap();
        publish(&mut rt);
        assert_eq!(rt.gui_window_frames.len(), MAX_WINDOWS);
    }
    #[test]
    fn ime_host_reservation_rejects_over_budget_before_contacting_a_native_service() {
        let mut rt = runtime();
        rt.enable_gui_ime();
        assert_eq!(
            rt.gui_host_reservation(),
            super::super::clipboard::HOST_RESERVATION + super::super::composition::HOST_RESERVATION
        );
        rt.gui_window_stage("one", Some(frame("IME budget")))
            .unwrap();
        let mut budget = rt.budget;
        budget.history_memory = 512 * 1024;
        rt.set_budget(budget).unwrap();
        assert!(matches!(
            rt.gui_windows_prepare(),
            Err(Error::HistoryBudgetExceeded)
        ));
        assert!(rt.gui_window_hosts.is_empty());
        assert_eq!(rt.state.gui_windows_pending.len(), 1);
    }
    #[test]
    fn clipboard_host_is_admitted_before_native_window_preparation() {
        let mut rt = runtime();
        rt.enable_gui_clipboard();
        rt.gui_window_stage("one", Some(frame("clipboard budget")))
            .unwrap();
        let mut budget = rt.budget;
        budget.history_memory = 80 * 1024;
        rt.set_budget(budget).unwrap();
        assert!(matches!(
            rt.gui_windows_prepare(),
            Err(Error::HistoryBudgetExceeded)
        ));
        assert!(rt.gui_window_hosts.is_empty());
        assert_eq!(rt.state.gui_windows_pending.len(), 1);
        // The earlier host contract fits; the new clipboard reservation causes rejection.
        rt.gui_clipboard_enabled = false;
        let frame = rt
            .state
            .gui_windows_pending
            .get("one")
            .unwrap()
            .frame
            .as_ref()
            .unwrap();
        let legacy = frame.bytes() * 2 + frame.width as usize * frame.height as usize * 4 + 8192;
        rt.check_native_allocation(legacy).unwrap();
    }
    #[test]
    fn completed_window_effects_are_kept_on_later_publish_failure() {
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("fixture output failure"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut rt = runtime();
        rt.configure_gui_window_events(vec![]).unwrap();
        rt.gui_window_stage("one", Some(frame("One"))).unwrap();
        rt.gui_window_stage("two", Some(frame("Two"))).unwrap();
        rt.commit("staged").unwrap();
        rt.write_output(b"output").unwrap();
        assert!(matches!(
            rt.publish(false, &mut Broken, &mut Vec::new()),
            Err(Error::PublishPartiallyApplied(_))
        ));
        let failure = rt.publish_failure().unwrap();
        assert_eq!(failure.phase, "stdout");
        assert!(failure.applied.contains(&"gui-window one".to_string()));
        assert!(failure.applied.contains(&"gui-window two".to_string()));
        rt.revert("staged").unwrap();
        assert!(rt.state.gui_windows_pending.is_empty());
        assert_eq!(rt.gui_window_frames.len(), 2);
    }
    #[test]
    fn input_accounting_tracks_delivery_import_and_failed_fixture_replacement() {
        let mut rt = runtime();
        rt.enable_virtual_publish();
        rt.gui_window_stage("one", Some(frame("One"))).unwrap();
        publish(&mut rt);
        rt.configure_gui_window_events(vec![WindowEvent {
            window: "one".into(),
            event: Event::simple("close"),
        }])
        .unwrap();
        assert!(rt.gui_window_scripted_bytes > 0);
        rt.gui_window_poll_any().unwrap();
        assert_eq!(rt.gui_window_scripted_bytes, 0);
        for _ in 0..10000 {
            assert!(rt.gui_window_poll_any().unwrap().is_none());
        }
        assert_eq!(
            rt.gui_window_observation_bytes,
            rt.gui_window_observations
                .iter()
                .map(WindowInput::bytes)
                .sum::<usize>()
        );
        let mut replay = runtime();
        replay
            .import_observations(&rt.export_observations().unwrap())
            .unwrap();
        assert_eq!(
            replay.gui_window_observation_bytes,
            rt.gui_window_observation_bytes
        );
        let previous = rt.gui_window_scripted_bytes;
        let mut low = rt.budget;
        low.history_memory = 512 * 1024;
        rt.set_budget(low).unwrap();
        assert!(rt
            .configure_gui_window_events(vec![
                WindowEvent {
                    window: "one".into(),
                    event: Event::simple("close"),
                };
                10000
            ])
            .is_err());
        assert_eq!(rt.gui_window_scripted_bytes, previous);
        assert!(rt.gui_window_scripted.as_ref().unwrap().is_empty());
    }
    #[test]
    fn legacy_input_admission_keeps_unconsumed_events() {
        let mut rt = runtime();
        rt.configure_gui_events(vec![Event::simple("close")])
            .unwrap();
        rt.gui_stage(Some(frame("Legacy"))).unwrap();
        publish(&mut rt);
        let old = rt.budget;
        let mut low = old;
        low.history_memory = 1024;
        rt.set_budget(low).unwrap();
        assert!(rt.gui_poll_event().is_err());
        assert_eq!(rt.gui_scripted.as_ref().unwrap().len(), 1);
        assert!(rt.gui_observations.is_empty());
        rt.set_budget(old).unwrap();
        assert_eq!(rt.gui_poll_event().unwrap().unwrap().kind, "close");
    }
    #[test]
    fn native_gui_surface_async_wait_records_pending_input_and_closed_host() {
        #[cfg(target_os = "linux")]
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        #[cfg(any(target_os = "linux", windows))]
        {
            let mut rt = runtime();
            rt.gui_window_stage("one", Some(frame("Async wait")))
                .unwrap();
            publish(&mut rt);
            for _ in 0..64 {
                if rt.gui_window_wait_poll_any().unwrap().is_none() {
                    break;
                }
            }
            rt.commit("pending").unwrap();
            for _ in 0..1000 {
                assert!(rt.gui_window_wait_poll_any().unwrap().is_none());
            }
            let last = rt.gui_window_observations.last().unwrap();
            assert!(last.wait);
            assert!(last.repeat >= 1000);
            rt.gui_window_hosts
                .get_mut("one")
                .unwrap()
                .backend
                .inject_pointer(17, 23);
            let mut pointer = false;
            for _ in 0..100 {
                if let Some(e) = rt.gui_window_wait_poll_any().unwrap() {
                    if e.event.kind == "pointer" {
                        assert_eq!((e.event.x, e.event.y), (17, 23));
                        pointer = true;
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert!(pointer);
            rt.gui_window_hosts.get_mut("one").unwrap().close();
            let mut closed = false;
            for _ in 0..100 {
                match rt.gui_window_wait_poll_any() {
                    Err(Error::InvalidOperation(code)) if code == "GuiClosed" => {
                        closed = true;
                        break;
                    }
                    Ok(_) => {}
                    other => panic!("unexpected close: {other:?}"),
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert!(closed);
            let data = rt.export_observations().unwrap();
            let mut replay = runtime();
            replay.import_observations(&data).unwrap();
            replay
                .gui_window_stage("one", Some(frame("Async wait")))
                .unwrap();
            publish(&mut replay);
            let mut found = false;
            for _ in 0..2000 {
                match replay.gui_window_wait_poll_any() {
                    Ok(Some(e)) if e.event.kind == "pointer" => found = true,
                    Ok(_) => {}
                    Err(Error::InvalidOperation(s)) if s == "GuiClosed" => break,
                    other => panic!("unexpected replay: {other:?}"),
                }
            }
            assert!(found);
            assert!(replay.gui_window_hosts.is_empty());
        }
    }
    #[test]
    fn native_gui_surface_named_windows_route_pointer_and_close_independently() {
        #[cfg(target_os = "linux")]
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            return;
        }
        #[cfg(any(target_os = "linux", windows))]
        {
            let mut rt = runtime();
            rt.gui_window_stage("one", Some(frame("One"))).unwrap();
            rt.gui_window_stage("two", Some(frame("Two"))).unwrap();
            publish(&mut rt);
            rt.gui_window_hosts
                .get_mut("one")
                .unwrap()
                .backend
                .inject_pointer(12, 13);
            rt.gui_window_hosts
                .get_mut("two")
                .unwrap()
                .backend
                .inject_pointer(22, 23);
            for (id, x, y) in [("two", 22, 23), ("one", 12, 13)] {
                let mut pointer = false;
                for _ in 0..32 {
                    let e = rt.gui_window_next_event(id).unwrap();
                    if e.kind == "pointer" {
                        assert_eq!((e.x, e.y), (x, y));
                        pointer = true;
                        break;
                    }
                }
                assert!(pointer);
            }
            rt.gui_window_hosts
                .get_mut("one")
                .unwrap()
                .backend
                .inject_pointer(41, 42);
            rt.gui_window_hosts
                .get_mut("two")
                .unwrap()
                .backend
                .inject_pointer(51, 52);
            let mut routed = BTreeMap::new();
            for _ in 0..64 {
                let received = if routed.is_empty() {
                    Some(rt.gui_window_next_any().unwrap())
                } else {
                    rt.gui_window_poll_any().unwrap()
                };
                if let Some(e) = received {
                    if e.event.kind == "pointer" {
                        routed.insert(e.window, (e.event.x, e.event.y));
                    }
                }
                if routed.len() == 2 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert_eq!(routed.get("one"), Some(&(41, 42)));
            assert_eq!(routed.get("two"), Some(&(51, 52)));
            rt.gui_window_hosts
                .get_mut("one")
                .unwrap()
                .backend
                .inject_key_and_close();
            let mut closed = false;
            for _ in 0..32 {
                if rt.gui_window_next_event("one").unwrap().kind == "close" {
                    closed = true;
                    break;
                }
            }
            assert!(closed);
            rt.gui_window_stage("one", None).unwrap();
            publish(&mut rt);
            assert_eq!(rt.gui_window_hosts.len(), 1);
            rt.gui_window_hosts
                .get_mut("two")
                .unwrap()
                .backend
                .inject_pointer(31, 32);
            let mut pointer = false;
            for _ in 0..32 {
                let e = rt.gui_window_next_event("two").unwrap();
                if e.kind == "pointer" {
                    assert_eq!((e.x, e.y), (31, 32));
                    pointer = true;
                    break;
                }
            }
            assert!(pointer);
            rt.gui_window_stage("one", Some(frame("Reopened"))).unwrap();
            publish(&mut rt);
            assert_eq!(rt.gui_window_hosts.len(), 2);
            rt.gui_window_stage("one", None).unwrap();
            rt.gui_window_stage("two", None).unwrap();
            publish(&mut rt);
            assert!(rt.gui_window_hosts.is_empty());
        }
    }
}
