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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WindowInput {
    pub request: Option<String>,
    pub poll: bool,
    pub result: Option<WindowEvent>,
}
impl WindowInput {
    pub(crate) fn validate(&self) -> std::io::Result<()> {
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
            + 96
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
        self.gui_windows_read(Some(id), false)?
            .map(|e| e.event)
            .ok_or_else(|| Error::InvalidOperation("GuiWindowEventTapeEnd".into()))
    }
    pub fn gui_window_poll(&mut self, id: &str) -> Result<Option<Event>> {
        Ok(self.gui_windows_read(Some(id), true)?.map(|e| e.event))
    }
    pub fn gui_window_next_any(&mut self) -> Result<WindowEvent> {
        self.gui_windows_read(None, false)?
            .ok_or_else(|| Error::InvalidOperation("GuiWindowNotPublished".into()))
    }
    pub fn gui_window_poll_any(&mut self) -> Result<Option<WindowEvent>> {
        self.gui_windows_read(None, true)
    }
    pub fn gui_window_continue_input(&mut self) {
        self.state.gui_windows_cursor = self.gui_window_high_water;
    }
    fn gui_windows_read(
        &mut self,
        request: Option<&str>,
        poll: bool,
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
        if cursor >= 1_000_000 {
            return Err(Error::InvalidOperation("GuiWindowEventLimit".into()));
        }
        let result = if let Some(record) = self.gui_window_observations.get(cursor) {
            if record.request.as_deref() != request || record.poll != poll {
                return Err(Error::InvalidOperation(
                    "ReplayMismatch: named GUI request or polling mode".into(),
                ));
            }
            record.result.clone()
        } else {
            if self.replaying {
                return Err(Error::InvalidOperation(
                    "ReplayMismatch: named GUI journal exhausted".into(),
                ));
            }
            // Reserve the largest admitted event before consuming any native/scripted input.
            self.check_native_allocation(8192)?;
            let result = if let Some(tape) = self.gui_window_scripted.as_mut() {
                match tape.front() {
                    Some(e) if request.is_none_or(|id| id == e.window) => {
                        if !self.gui_window_frames.contains_key(&e.window) {
                            return Err(Error::InvalidOperation(
                                "GuiWindowEventTapeUnpublished".into(),
                            ));
                        }
                        let event = tape.pop_front();
                        self.gui_window_scripted_bytes -=
                            event.as_ref().map_or(0, WindowEvent::bytes);
                        event
                    }
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
                                Err(e) if e.to_string() == "GuiClosed" => {
                                    closed += 1;
                                }
                                Err(e) => {
                                    return Err(e.into());
                                }
                            }
                        }
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
            let record = WindowInput {
                request: request.map(str::to_owned),
                poll,
                result: result.clone(),
            };
            record.validate()?;
            self.gui_window_observation_bytes += record.bytes();
            self.gui_window_observations.push(record);
            self.enforce_budget()?;
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
        self.state.gui_windows_cursor += 1;
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
                prepared.insert(
                    id.clone(),
                    Host::prepare()
                        .map_err(|e| Error::InvalidOperation(format!("GuiUnavailable: {e}")))?,
                );
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
        low.history_memory = 2 * 1024 * 1024;
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
