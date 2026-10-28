use super::{Event, Frame, Host};
use crate::{Error, Result, Runtime};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub(crate) struct Request {
    pub id: u64,
    pub frame: Option<Arc<Frame>>,
}
impl Runtime {
    pub fn configure_gui_events(&mut self, events: Vec<Event>) -> Result<()> {
        if events.len() > 1_000_000 {
            return Err(Error::InvalidOperation("GuiEventLimit".into()));
        }
        for e in &events {
            e.validate()?;
        }
        let previous = self.gui_scripted.replace(events.into());
        if let Err(error) = self.enforce_budget() {
            *self.gui_scripted = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn gui_stage(&mut self, frame: Option<Frame>) -> Result<()> {
        if let Some(f) = &frame {
            f.validate()?;
            if f.bytes() > 1024 * 1024 {
                return Err(Error::InvalidOperation("GuiSceneLimit".into()));
            }
        }
        self.pending_transaction(|rt| {
            let id = rt.operation()?;
            rt.state.gui_pending = Some(Request {
                id,
                frame: frame.map(Arc::new),
            });
            Ok(())
        })
    }
    pub fn gui_next_event(&mut self) -> Result<Event> {
        self.gui_read_event(false, 0)
    }
    pub fn gui_poll_event(&mut self) -> Result<Option<Event>> {
        self.gui_wait_event(0)
    }
    /// Waits up to `milliseconds` for input. A timeout is journaled as an idle event, so a replay
    /// returns at once and sees the same result.
    pub fn gui_wait_event(&mut self, milliseconds: u64) -> Result<Option<Event>> {
        let e = self.gui_read_event(true, milliseconds)?;
        Ok(if e.kind == "idle" { None } else { Some(e) })
    }
    fn gui_read_event(&mut self, poll: bool, wait: u64) -> Result<Event> {
        if self.gui_displayed.is_none() {
            return Err(Error::InvalidOperation(
                "GuiNotPublished: publish a GUI scene before reading input".into(),
            ));
        }
        let cursor = self.state.gui_cursor;
        if cursor >= 1_000_000 {
            return Err(Error::InvalidOperation("GuiEventLimit".into()));
        }
        let event = if let Some(e) = self.gui_observations.get(cursor) {
            e.clone()
        } else {
            if self.replaying {
                return Err(Error::InvalidOperation(
                    "ReplayMismatch: GUI event journal exhausted".into(),
                ));
            }
            // Bound the next journal entry before taking an event from the host.
            self.check_native_allocation(8192)?;
            let e = if let Some(tape) = self.gui_scripted.as_mut() {
                tape.pop_front()
                    .ok_or_else(|| Error::InvalidOperation("GuiEventTapeEnd".into()))?
            } else {
                let host = self
                    .gui_host
                    .as_mut()
                    .ok_or_else(|| Error::InvalidOperation("GuiNotPublished".into()))?;
                if poll {
                    let deadline =
                        std::time::Instant::now() + std::time::Duration::from_millis(wait);
                    loop {
                        if let Some(e) = host.poll()? {
                            break e;
                        }
                        let left = deadline.saturating_duration_since(std::time::Instant::now());
                        if left.is_zero() {
                            break Event::simple("idle");
                        }
                        std::thread::sleep(left.min(std::time::Duration::from_millis(4)));
                    }
                } else {
                    host.event()?
                }
            };
            e.validate()?;
            self.gui_observations.push(e.clone());
            if let Err(error) = self.enforce_budget() {
                self.gui_observations.pop();
                return Err(error);
            }
            e
        };
        self.state.gui_cursor += 1;
        self.gui_high_water = self.gui_high_water.max(self.state.gui_cursor);
        if !poll && event.kind == "idle" {
            // A wait that timed out recorded an idle event. `nextEvent` has no timeout, so the
            // idle entry means nothing to it: read the next real input.
            return self.gui_read_event(false, 0);
        }
        Ok(event)
    }
    pub fn gui_continue_input(&mut self) {
        self.state.gui_cursor = self.gui_high_water;
    }
    pub(crate) fn gui_prepare(&mut self) -> Result<()> {
        if self
            .state
            .gui_pending
            .as_ref()
            .is_some_and(|r| r.frame.is_some())
            && !self.replaying
            && !self.virtual_publish
            && self.gui_scripted.is_none()
            && self.gui_host.is_none()
        {
            if self.gui_clipboard_enabled || self.gui_ime_enabled || self.gui_accessibility_enabled
            {
                self.check_native_allocation(self.gui_host_reservation())?;
            }
            let mut host = Host::prepare()
                .map_err(|e| Error::InvalidOperation(format!("GuiUnavailable: {e}")))?;
            host.configure_clipboard(self.gui_clipboard_enabled);
            host.configure_ime(self.gui_ime_enabled);
            host.configure_accessibility(self.gui_accessibility_enabled);
            host.configure_command_keys(self.gui_command_keys_enabled);
            self.gui_host = Some(host);
        }
        Ok(())
    }
    pub(crate) fn gui_apply(&mut self) -> std::io::Result<()> {
        let Some(request) = &self.state.gui_pending else {
            return Ok(());
        };
        if self.gui_scripted.is_none() && !self.replaying && !self.virtual_publish {
            if let Some(frame) = &request.frame {
                self.gui_host
                    .as_mut()
                    .ok_or_else(|| super::invalid("GuiUnavailable"))?
                    .present(frame)?;
            } else if let Some(host) = self.gui_host.as_mut() {
                host.close();
            }
        }
        *self.gui_displayed = request.frame.clone();
        Ok(())
    }
    pub fn gui_displayed_frame(&self) -> Option<&Frame> {
        self.gui_displayed.as_deref()
    }
}
