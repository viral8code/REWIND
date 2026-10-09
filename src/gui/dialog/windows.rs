//! The Win32 modal chooser owns one bounded worker; cancellation never joins a live thread.
use super::{Request, Slot};
use std::sync::{
    atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering},
    mpsc, Arc,
};
type Handle = isize;
#[repr(C)]
struct OpenFileName {
    size: u32,
    owner: Handle,
    instance: Handle,
    filter: *const u16,
    custom_filter: *mut u16,
    max_custom_filter: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_file_title: u32,
    initial_dir: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    file_extension: u16,
    default_extension: *const u16,
    data: isize,
    hook: Option<unsafe extern "system" fn(Handle, u32, usize, isize) -> usize>,
    template: *const u16,
    reserved: *mut std::ffi::c_void,
    reserved_flags: u32,
    flags_ex: u32,
}
#[link(name = "comdlg32")]
unsafe extern "system" {
    fn GetOpenFileNameW(p: *mut OpenFileName) -> i32;
    fn GetSaveFileNameW(p: *mut OpenFileName) -> i32;
    fn CommDlgExtendedError() -> u32;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn GetParent(h: Handle) -> Handle;
    fn IsWindowEnabled(h: Handle) -> i32;
    #[cfg(test)]
    fn AttachThreadInput(a: u32, b: u32, attach: i32) -> i32;
    #[cfg(test)]
    fn SetActiveWindow(h: Handle) -> Handle;
    #[cfg(test)]
    fn EnumChildWindows(
        h: Handle,
        callback: unsafe extern "system" fn(Handle, isize) -> i32,
        data: isize,
    ) -> i32;
    #[cfg(test)]
    fn GetWindowTextW(h: Handle, text: *mut u16, len: i32) -> i32;
    #[cfg(test)]
    fn GetClassNameW(h: Handle, text: *mut u16, len: i32) -> i32;
    #[cfg(test)]
    fn GetDlgCtrlID(h: Handle) -> i32;
    #[cfg(test)]
    fn SendMessageTimeoutW(
        h: Handle,
        message: u32,
        wparam: usize,
        lparam: isize,
        flags: u32,
        timeout: u32,
        result: *mut usize,
    ) -> isize;
    fn PostMessageW(h: Handle, m: u32, w: usize, l: isize) -> i32;
    fn GetWindowThreadProcessId(h: Handle, p: *mut u32) -> u32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
}
struct Cancel {
    requested: AtomicBool,
    ready: AtomicBool,
    window: AtomicIsize,
    thread: AtomicU32,
}
impl Cancel {
    fn post(&self) {
        let window = self.window.load(Ordering::Acquire);
        if self.ready.load(Ordering::Acquire)
            && window != 0
            && unsafe { GetWindowThreadProcessId(window, std::ptr::null_mut()) }
                == self.thread.load(Ordering::Acquire)
        {
            unsafe {
                PostMessageW(window, 0x10, 0, 0);
            }
        }
    }
    fn cancel(&self) {
        self.requested.store(true, Ordering::Release);
        self.post();
    }
}
#[repr(C)]
struct NotifyHeader {
    window: Handle,
    id: usize,
    code: u32,
}
#[repr(C)]
struct FileNotify {
    header: NotifyHeader,
    request: *const OpenFileName,
    file: *const u16,
}
unsafe extern "system" fn hook(window: Handle, message: u32, _: usize, lparam: isize) -> usize {
    if message == 0x110 {
        let request = unsafe { &*(lparam as *const OpenFileName) };
        let cancel = unsafe { &*(request.data as *const Cancel) };
        cancel
            .window
            .store(unsafe { GetParent(window) }, Ordering::Release);
    } else if message == 0x4e && lparam != 0 {
        let header = unsafe { &*(lparam as *const NotifyHeader) };
        // Explorer-style common dialogs notify only after their controls and
        // initial directory are initialized. Earlier close/accept messages can
        // be lost while the modal chooser is still constructing its UI.
        if header.code == (-601i32) as u32 {
            let notification = unsafe { &*(lparam as *const FileNotify) };
            if notification.request.is_null() {
                return 0;
            }
            let request = unsafe { &*notification.request };
            let cancel = unsafe { &*(request.data as *const Cancel) };
            cancel.ready.store(true, Ordering::Release);
            if cancel.requested.load(Ordering::Acquire) {
                cancel.post();
            }
        }
    }
    0
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
pub(crate) struct Session {
    result: mpsc::Receiver<Result<Option<String>, &'static str>>,
    cancel: Arc<Cancel>,
    worker: Option<std::thread::JoinHandle<()>>,
    pending: Option<Result<Option<String>, &'static str>>,
    #[cfg(test)]
    test_path: Vec<u16>,
}
impl Session {
    pub fn start(request: &Request) -> Result<Self, &'static str> {
        request.validate()?;
        let slot = Slot::acquire()?;
        #[cfg(test)]
        let test_path = wide(&request.initial);
        let request = request.clone();
        let cancel = Arc::new(Cancel {
            requested: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            window: AtomicIsize::new(0),
            thread: AtomicU32::new(0),
        });
        let worker = cancel.clone();
        let (send, result) = mpsc::sync_channel(1);
        let worker_handle = std::thread::Builder::new()
            .name("rewind-file-dialog".into())
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                let _slot = slot;
                worker
                    .thread
                    .store(unsafe { GetCurrentThreadId() }, Ordering::Release);
                if worker.requested.load(Ordering::Acquire) {
                    let _ = send.send(Ok(None));
                    return;
                }
                let title = wide(&request.title);
                // The common chooser can otherwise reuse its shell directory
                // even when lpstrFile contains a full initial path.
                let path = std::path::Path::new(&request.initial);
                let directory = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(|p| wide(&p.to_string_lossy()));
                let initial = wide(
                    &path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| request.initial.clone()),
                );
                let mut file = vec![0u16; 4097];
                let count = initial.len().min(file.len());
                file[..count].copy_from_slice(&initial[..count]);
                let mut p = OpenFileName {
                    size: std::mem::size_of::<OpenFileName>() as u32,
                    owner: 0,
                    instance: 0,
                    filter: std::ptr::null(),
                    custom_filter: std::ptr::null_mut(),
                    max_custom_filter: 0,
                    filter_index: 0,
                    file: file.as_mut_ptr(),
                    max_file: file.len() as u32,
                    file_title: std::ptr::null_mut(),
                    max_file_title: 0,
                    initial_dir: directory.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
                    title: title.as_ptr(),
                    flags: 0x80000 | 0x20 | 0x8 | 0x800 | if request.save { 2 } else { 0x1000 },
                    file_offset: 0,
                    file_extension: 0,
                    default_extension: std::ptr::null(),
                    data: Arc::as_ptr(&worker) as isize,
                    hook: Some(hook),
                    template: std::ptr::null(),
                    reserved: std::ptr::null_mut(),
                    reserved_flags: 0,
                    flags_ex: 0,
                };
                let ok = unsafe {
                    if request.save {
                        GetSaveFileNameW(&mut p)
                    } else {
                        GetOpenFileNameW(&mut p)
                    }
                };
                worker.window.store(0, Ordering::Release);
                let outcome = if ok == 0 {
                    match unsafe { CommDlgExtendedError() } {
                        0 => Ok(None),
                        0x3003 => Err("GuiDialogPathLimit"),
                        0x3002 => Err("GuiDialogPath"),
                        _ => Err("GuiDialogUnavailable"),
                    }
                } else {
                    let length = file.iter().position(|c| *c == 0).unwrap_or(file.len());
                    String::from_utf16(&file[..length])
                        .map_err(|_| "GuiDialogPathEncoding")
                        .and_then(|s| {
                            if s.len() > 4096 {
                                Err("GuiDialogPathLimit")
                            } else {
                                Ok(Some(s))
                            }
                        })
                };
                let _ = send.send(outcome);
            })
            .map_err(|_| "GuiDialogUnavailable")?;
        Ok(Self {
            result,
            cancel,
            worker: Some(worker_handle),
            pending: None,
            #[cfg(test)]
            test_path,
        })
    }
    pub fn cancel(&mut self) {
        self.cancel.cancel();
    }
    pub fn poll(&mut self) -> Option<Result<Option<String>, &'static str>> {
        if self.pending.is_none() {
            self.pending = match self.result.try_recv() {
                Ok(v) => Some(v),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(_) => Some(Err("GuiDialogUnavailable")),
            };
        }
        if self.worker.as_ref().is_some_and(|w| w.is_finished()) {
            let _ = self.worker.take().unwrap().join();
            return self.pending.take();
        }
        None
    }
    #[cfg(test)]
    pub fn test_response(&mut self, accept: bool) -> bool {
        let window = self.cancel.window.load(Ordering::Acquire);
        if window == 0 || !self.cancel.ready.load(Ordering::Acquire) {
            return false;
        }
        unsafe extern "system" fn find(window: Handle, data: isize) -> i32 {
            let (id, expected_class, found) = unsafe { &mut *(data as *mut (i32, &str, Handle)) };
            let mut class = [0u16; 16];
            let count = unsafe { GetClassNameW(window, class.as_mut_ptr(), class.len() as i32) }
                .max(0) as usize;
            if unsafe { GetDlgCtrlID(window) } == *id
                && String::from_utf16_lossy(&class[..count]) == *expected_class
            {
                *found = window;
                return 0;
            }
            1
        }
        let mut target_button = (if accept { 1 } else { 2 }, "Button", 0);
        unsafe {
            EnumChildWindows(
                window,
                find,
                &mut target_button as *mut (i32, &str, Handle) as isize,
            );
        }
        let button = target_button.2;
        if accept {
            // Explorer may display a selected filename without its extension.
            // Enter the intended absolute path through the real filename edit
            // control instead of relying on asynchronous initial selection.
            let mut filename = (1148, "Edit", 0);
            unsafe {
                EnumChildWindows(
                    window,
                    find,
                    &mut filename as *mut (i32, &str, Handle) as isize,
                );
            }
            let mut delivered = 0;
            if filename.2 == 0
                || unsafe {
                    SendMessageTimeoutW(
                        filename.2,
                        0xc,
                        0,
                        self.test_path.as_ptr() as isize,
                        3,
                        2000,
                        &mut delivered,
                    )
                } == 0
            {
                return false;
            }
        }
        if button == 0 || unsafe { IsWindowEnabled(button) } == 0 {
            return false;
        }
        let sender = unsafe { GetCurrentThreadId() };
        let target = self.cancel.thread.load(Ordering::Acquire);
        let attached = sender != target && unsafe { AttachThreadInput(sender, target, 1) } != 0;
        unsafe {
            SetActiveWindow(window);
        }
        let delivered = unsafe { PostMessageW(button, 0xf5, 0, 0) } != 0;
        if attached {
            unsafe {
                AttachThreadInput(sender, target, 0);
            }
        }
        delivered
    }
    #[cfg(test)]
    pub fn test_diagnostics(&self) -> String {
        unsafe extern "system" fn child(window: Handle, data: isize) -> i32 {
            let lines = unsafe { &mut *(data as *mut Vec<String>) };
            let mut text = [0u16; 512];
            let mut class = [0u16; 128];
            let t = unsafe { GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32) }.max(0)
                as usize;
            let c = unsafe { GetClassNameW(window, class.as_mut_ptr(), class.len() as i32) }.max(0)
                as usize;
            lines.push(format!(
                "id={} class={} enabled={} text={}",
                unsafe { GetDlgCtrlID(window) },
                String::from_utf16_lossy(&class[..c]),
                unsafe { IsWindowEnabled(window) },
                String::from_utf16_lossy(&text[..t])
            ));
            1
        }
        let mut lines = Vec::new();
        let window = self.cancel.window.load(Ordering::Acquire);
        unsafe {
            EnumChildWindows(window, child, &mut lines as *mut Vec<String> as isize);
        }
        format!(
            "ready={} window={} worker_finished={} controls={:?}",
            self.cancel.ready.load(Ordering::Acquire),
            window,
            self.worker.as_ref().is_some_and(|w| w.is_finished()),
            lines
        )
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
