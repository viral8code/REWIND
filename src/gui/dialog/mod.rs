//! A bounded native dialog; the VM journals responses, never native windows.
use std::sync::atomic::{AtomicBool, Ordering};
mod runtime;
static ACTIVE: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
pub(crate) const RESERVATION: usize = 2 * 1024 * 1024 + 64 * 1024;
#[cfg(not(windows))]
pub(crate) const RESERVATION: usize = 64 * 1024;
#[derive(Clone, Debug, serde::Serialize)]
pub struct Request {
    pub save: bool,
    pub title: String,
    pub initial: String,
}
impl Request {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.title.is_empty() || self.title.len() > 256 || self.title.contains('\0') {
            return Err("GuiDialogTitle");
        }
        if self.initial.len() > 4096 || self.initial.contains('\0') {
            return Err("GuiDialogPath");
        }
        Ok(())
    }
}
pub(super) struct Slot;
impl Slot {
    fn acquire() -> Result<Self, &'static str> {
        ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| "GuiDialogBusy")
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
pub(crate) use linux::Session;
#[cfg(windows)]
pub(crate) use windows::Session;
#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) struct Session;
#[cfg(not(any(target_os = "linux", windows)))]
impl Session {
    pub fn start(_: &Request) -> Result<Self, &'static str> {
        Err("GuiUnsupportedPlatform")
    }
    pub fn cancel(&mut self) {}
    pub fn poll(&mut self) -> Option<Result<Option<String>, &'static str>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dialog_requests_are_bounded_before_native_admission() {
        for request in [
            Request {
                save: false,
                title: "".into(),
                initial: "".into(),
            },
            Request {
                save: false,
                title: "x".repeat(257),
                initial: "".into(),
            },
            Request {
                save: false,
                title: "Open".into(),
                initial: "x\0y".into(),
            },
            Request {
                save: false,
                title: "Open".into(),
                initial: "x".repeat(4097),
            },
        ] {
            assert!(request.validate().is_err());
        }
        assert!(Request {
            save: true,
            title: "保存 界".into(),
            initial: "界.txt".into()
        }
        .validate()
        .is_ok());
    }
    #[test]
    fn native_file_dialog_selection_cancel_and_revert_keep_one_physical_operation() {
        #[cfg(target_os = "linux")]
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        #[cfg(any(target_os = "linux", windows))]
        {
            use crate::Runtime;
            use std::time::{Duration, Instant};
            let root =
                std::env::temp_dir().join(format!("rewind-native-dialog-{}", std::process::id()));
            std::fs::create_dir_all(&root).unwrap();
            let file = root.join("selection-界.txt");
            std::fs::write(&file, b"unchanged").unwrap();
            let mut rt = Runtime::new(&root).unwrap();
            for (save, accept) in [(false, true), (true, true), (false, false)] {
                let selected = if save {
                    root.join("new-界.txt")
                } else {
                    file.clone()
                };
                let request = Request {
                    save,
                    title: format!("REWIND native dialog {save} {accept}"),
                    initial: selected.to_string_lossy().into_owned(),
                };
                rt.commit("before-dialog").unwrap();
                rt.enter_external(true).unwrap();
                let id = rt.start_gui_dialog(request.clone()).unwrap();
                rt.exit_external().unwrap();
                assert!(
                    rt.gui_dialogs.contains_key(&id),
                    "native dialog failed to open"
                );
                let deadline = Instant::now() + Duration::from_secs(15);
                let mut result = None;
                let mut sent = false;
                while Instant::now() < deadline {
                    if let Some(session) = rt.gui_dialogs.get_mut(&id) {
                        // Let the native chooser finish loading its initial directory.
                        if !sent
                            && deadline.duration_since(Instant::now()) < Duration::from_secs(14)
                        {
                            sent = session.test_response(accept);
                        }
                    }
                    if let Some(value) = rt.poll_gui_dialog(id).unwrap() {
                        result = Some(value);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert!(sent, "native response not delivered");
                let expected = if accept {
                    serde_json::Value::String(selected.to_string_lossy().into_owned())
                } else {
                    serde_json::Value::Null
                };
                #[cfg(windows)]
                let diagnostics = rt
                    .gui_dialogs
                    .get(&id)
                    .map(|session| session.test_diagnostics());
                #[cfg(windows)]
                assert_eq!(
                    result,
                    Some(Ok(expected.clone())),
                    "native chooser diagnostic: {diagnostics:?}"
                );
                #[cfg(not(windows))]
                assert_eq!(result, Some(Ok(expected.clone())));
                assert!(rt.gui_dialogs.is_empty());
                rt.revert("before-dialog").unwrap();
                rt.enter_external(false).unwrap();
                let reused = rt.start_gui_dialog(request).unwrap();
                rt.exit_external().unwrap();
                assert_eq!(reused, id);
                assert!(rt.gui_dialogs.is_empty());
                let mut replayed = None;
                for _ in 0..10000 {
                    if let Some(value) = rt.poll_gui_dialog(id).unwrap() {
                        replayed = Some(value);
                        break;
                    }
                    assert!(rt.gui_dialogs.is_empty());
                }
                assert_eq!(replayed, Some(Ok(expected)));
                assert_eq!(std::fs::read(&file).unwrap(), b"unchanged");
                assert!(!root.join("new-界.txt").exists());
                rt.drop_checkpoint("before-dialog").unwrap();
            }
            // Retain an in-flight operation across revert and cancel it without opening another chooser.
            rt.commit("pending").unwrap();
            rt.enter_external(false).unwrap();
            let request = Request {
                save: false,
                title: "REWIND cancel dialog".into(),
                initial: file.to_string_lossy().into_owned(),
            };
            let id = rt.start_gui_dialog(request.clone()).unwrap();
            rt.exit_external().unwrap();
            rt.revert("pending").unwrap();
            rt.enter_external(false).unwrap();
            let same = rt.start_gui_dialog(request).unwrap();
            rt.exit_external().unwrap();
            assert_eq!(id, same);
            assert_eq!(rt.gui_dialogs.len(), 1);
            rt.cancel_gui_dialog(id).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while !rt.gui_dialogs.is_empty() && Instant::now() < deadline {
                rt.reap_cancelled_gui_dialogs();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(rt.gui_dialogs.is_empty());
            assert_eq!(
                rt.poll_gui_dialog(id).unwrap(),
                Some(Err("GuiDialogCancelled".into()))
            );
            rt.configure_live_external(true);
            for _ in 0..16 {
                rt.enter_external_live_task(0).unwrap();
                let id = rt
                    .start_gui_dialog(Request {
                        save: false,
                        title: "REWIND live cancellation".into(),
                        initial: file.to_string_lossy().into_owned(),
                    })
                    .unwrap();
                let lease = rt.live_external_lease(id).unwrap();
                let checkpoint_lease = lease.clone();
                rt.exit_external().unwrap();
                assert_eq!(rt.gui_dialogs.len(), 1);
                rt.cancel_gui_dialog(id).unwrap();
                drop(lease);
                rt.reclaim_live_external().unwrap();
                assert!(rt.external_live_entries.contains_key(&id));
                assert_eq!(
                    rt.poll_gui_dialog(id).unwrap(),
                    Some(Err("GuiDialogCancelled".into()))
                );
                let deadline = Instant::now() + Duration::from_secs(10);
                while !rt.gui_dialogs.is_empty() && Instant::now() < deadline {
                    rt.reap_cancelled_gui_dialogs();
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert!(rt.gui_dialogs.is_empty());
                drop(checkpoint_lease);
                rt.reclaim_live_external().unwrap();
                assert!(rt.external_live_entries.is_empty());
                assert!(rt.external_buffers.is_empty());
                assert_eq!(rt.external_pending, 0);
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
