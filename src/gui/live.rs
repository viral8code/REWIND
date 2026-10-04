//! Explicit live named-window input. Results belong to the waiting future.
use super::WindowEvent;
use crate::{Error, Result, Runtime};
impl Runtime {
    pub fn start_live_gui_input(&mut self) -> Result<usize> {
        if !self.external_live {
            return Err(Error::InvalidOperation(
                "ExternalBoundary: live GUI input requires external live".into(),
            ));
        }
        self.begin_async_external("gui.wait.live.v1", b"", 65536)
            .map(|(id, _)| id)
    }
    pub fn cancel_live_gui_input(&mut self, id: usize) -> Result<()> {
        if self.external_operation_pending(id) {
            self.finish_async_external(id, Err("GuiCancelled".into()))?;
        }
        Ok(())
    }
    pub fn poll_live_gui_input(&mut self, id: usize, busy: bool) -> Result<Option<WindowEvent>> {
        self.charge_native_work(1)?;
        if !self.external_live_entries.contains_key(&id) {
            return Err(Error::InvalidOperation("ExternalOperationUnknown".into()));
        }
        if self.external_operation_pending(id) {
            let captured = if busy {
                Err(Error::InvalidOperation("GuiWaitBusy".into()))
            } else if self.gui_window_frames.is_empty() {
                Err(Error::InvalidOperation("GuiWindowNotPublished".into()))
            } else {
                // The factory already reserved the maximum event before native consumption.
                self.gui_windows_capture(None, true, true)
            };
            let result = match captured {
                Ok(None) => return Ok(None),
                Ok(Some(event)) => {
                    event.validate()?;
                    Ok(serde_json::to_value(event)
                        .map_err(|_| Error::InvalidOperation("GuiEventEncoding".into()))?)
                }
                Err(Error::InvalidOperation(code))
                    if matches!(
                        code.as_str(),
                        "GuiWaitBusy"
                            | "GuiClosed"
                            | "GuiWindowNotPublished"
                            | "GuiWindowEventTapeEnd"
                    ) =>
                {
                    Err(code)
                }
                Err(error) => return Err(error),
            };
            self.finish_async_external(id, result)?;
        }
        match self.poll_external(id)? {
            Some(Ok(value)) => {
                let event: WindowEvent = serde_json::from_value(value)
                    .map_err(|_| Error::InvalidOperation("GuiEventEncoding".into()))?;
                event.validate()?;
                Ok(Some(event))
            }
            Some(Err(code)) => Err(Error::InvalidOperation(code)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::{Event, Frame};
    fn runtime() -> Runtime {
        let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
        rt.configure_live_external(true);
        rt.enable_incremental_publish();
        let events = (0..3)
            .map(|n| WindowEvent {
                window: "one".into(),
                event: Event {
                    kind: "key".into(),
                    x: 0,
                    y: 0,
                    key: n.to_string(),
                    width: 0,
                    height: 0,
                },
            })
            .collect();
        rt.configure_gui_window_events(events).unwrap();
        rt.gui_window_stage(
            "one",
            Some(Frame {
                title: "live".into(),
                width: 100,
                height: 100,
                background: 0xffffff,
                items: vec![],
            }),
        )
        .unwrap();
        rt.publish(false, &mut std::io::sink(), &mut std::io::sink())
            .unwrap();
        rt
    }
    #[test]
    fn retained_future_reuses_input_while_new_wait_consumes_the_next_event() {
        let mut rt = runtime();
        rt.commit("before").unwrap();
        for expected in 0..3 {
            rt.enter_external_live_task(0).unwrap();
            let id = rt.start_live_gui_input().unwrap();
            let lease = rt.live_external_lease(id).unwrap();
            rt.exit_external().unwrap();
            assert_eq!(
                rt.poll_live_gui_input(id, false)
                    .unwrap()
                    .unwrap()
                    .event
                    .key,
                expected.to_string()
            );
            rt.revert("before").unwrap();
            assert_eq!(
                rt.poll_live_gui_input(id, true).unwrap().unwrap().event.key,
                expected.to_string()
            );
            drop(lease);
            rt.reclaim_live_external().unwrap();
            assert!(rt.external_live_entries.is_empty());
        }
        assert!(rt.gui_window_observations.is_empty());
        assert_eq!(rt.state.gui_windows_cursor, 0);
    }
    #[test]
    fn cancelled_and_busy_waits_are_cached_without_consuming_input() {
        let mut rt = runtime();
        for busy in [false, true] {
            rt.enter_external_live_task(0).unwrap();
            let id = rt.start_live_gui_input().unwrap();
            let lease = rt.live_external_lease(id).unwrap();
            rt.exit_external().unwrap();
            if !busy {
                rt.cancel_live_gui_input(id).unwrap();
            }
            let expected = if busy { "GuiWaitBusy" } else { "GuiCancelled" };
            assert_eq!(
                rt.poll_live_gui_input(id, busy).unwrap_err().to_string(),
                format!("invalid operation: {expected}")
            );
            assert_eq!(
                rt.poll_live_gui_input(id, false).unwrap_err().to_string(),
                format!("invalid operation: {expected}")
            );
            drop(lease);
            rt.reclaim_live_external().unwrap();
        }
        assert_eq!(rt.gui_window_scripted.as_ref().unwrap().len(), 3);
        assert!(rt.gui_window_observations.is_empty());
    }
}
