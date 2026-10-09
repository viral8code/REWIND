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
    fn PostMessageW(h: Handle, m: u32, w: usize, l: isize) -> i32;
    fn GetWindowThreadProcessId(h: Handle, p: *mut u32) -> u32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
}
struct Cancel {
    requested: AtomicBool,
    window: AtomicIsize,
    thread: AtomicU32,
}
impl Cancel {
    fn post(&self) {
        let window = self.window.load(Ordering::Acquire);
        if window != 0
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
unsafe extern "system" fn hook(window: Handle, message: u32, _: usize, lparam: isize) -> usize {
    if message == 0x110 {
        let request = unsafe { &*(lparam as *const OpenFileName) };
        let cancel = unsafe { &*(request.data as *const Cancel) };
        let owner = unsafe { GetParent(window) };
        cancel.window.store(owner, Ordering::Release);
        if cancel.requested.load(Ordering::Acquire) {
            cancel.post();
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
}
impl Session {
    pub fn start(request: &Request) -> Result<Self, &'static str> {
        request.validate()?;
        let slot = Slot::acquire()?;
        let request = request.clone();
        let cancel = Arc::new(Cancel {
            requested: AtomicBool::new(false),
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
                let initial = wide(&request.initial);
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
                    initial_dir: std::ptr::null(),
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
        if window == 0 {
            return false;
        }
        unsafe { PostMessageW(window, 0x111, if accept { 1 } else { 2 }, 0) != 0 }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
