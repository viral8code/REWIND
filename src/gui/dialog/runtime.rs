use super::{Request, Session, RESERVATION};
use crate::{Error, Result, Runtime};
impl Runtime {
    pub(crate) fn reap_cancelled_gui_dialogs(&mut self) {
        let ids = self
            .gui_dialogs
            .keys()
            .copied()
            .filter(|id| !self.external_operation_pending(*id))
            .collect::<Vec<_>>();
        for id in ids {
            if self
                .gui_dialogs
                .get_mut(&id)
                .and_then(Session::poll)
                .is_some()
            {
                self.gui_dialogs.remove(&id);
            }
        }
    }
    pub fn start_gui_dialog(&mut self, request: Request) -> Result<usize> {
        self.reap_cancelled_gui_dialogs();
        request
            .validate()
            .map_err(|code| Error::InvalidOperation(code.into()))?;
        let bytes = serde_json::to_vec(&request)
            .map_err(|_| Error::InvalidOperation("GuiDialogEncoding".into()))?;
        let (id, fresh) = self.begin_async_external("gui.file-dialog.v1", &bytes, 32 * 1024)?;
        if !fresh {
            return Ok(id);
        }
        if !self.gui_dialogs.is_empty() {
            self.finish_async_external(id, Err("GuiDialogBusy".into()))?;
            return Ok(id);
        }
        if let Err(error) = self.check_native_allocation(RESERVATION) {
            self.finish_async_external(id, Err("GuiDialogMemoryLimit".into()))?;
            return Err(error);
        }
        match Session::start(&request) {
            Ok(session) => {
                self.gui_dialogs.insert(id, session);
            }
            Err(code) => self.finish_async_external(id, Err(code.into()))?,
        }
        Ok(id)
    }
    pub fn poll_gui_dialog(
        &mut self,
        id: usize,
    ) -> Result<Option<std::result::Result<serde_json::Value, String>>> {
        self.charge_native_work(32)?;
        if !self.replaying && self.external_operation_pending(id) {
            if let Some(value) = self.gui_dialogs.get_mut(&id).and_then(Session::poll) {
                self.gui_dialogs.remove(&id);
                let outcome = value
                    .map(|p| p.map_or(serde_json::Value::Null, serde_json::Value::String))
                    .map_err(str::to_owned);
                self.finish_async_external(id, outcome)?;
            }
        }
        self.poll_external(id)
    }
    pub fn cancel_gui_dialog(&mut self, id: usize) -> Result<()> {
        if let Some(session) = self.gui_dialogs.get_mut(&id) {
            session.cancel();
            if session.poll().is_some() {
                self.gui_dialogs.remove(&id);
            }
        }
        if self.external_operation_pending(id) {
            self.finish_async_external(id, Err("GuiDialogCancelled".into()))?;
        }
        Ok(())
    }
}
