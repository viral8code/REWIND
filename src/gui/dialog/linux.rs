//! GTK's native file chooser is driven on its owning UI thread without gtk_dialog_run.
use super::{Request, Slot};
use libc::{c_char, c_int, c_ulong, c_void};
use std::ffi::{CStr, CString};
use std::sync::OnceLock;
use std::thread::ThreadId;
type Widget = *mut c_void;
struct Api {
    owner: ThreadId,
    dialog: unsafe extern "C" fn(*const c_char, Widget, c_int, *const c_char, ...) -> Widget,
    add_button: unsafe extern "C" fn(Widget, *const c_char, c_int) -> Widget,
    show: unsafe extern "C" fn(Widget),
    default_response: unsafe extern "C" fn(Widget, c_int),
    ref_sink: unsafe extern "C" fn(Widget) -> Widget,
    unref: unsafe extern "C" fn(Widget),
    destroy: unsafe extern "C" fn(Widget),
    pending: unsafe extern "C" fn() -> c_int,
    iterate: unsafe extern "C" fn(c_int) -> c_int,
    filename: unsafe extern "C" fn(Widget) -> *mut c_char,
    set_filename: unsafe extern "C" fn(Widget, *const c_char) -> c_int,
    set_folder: unsafe extern "C" fn(Widget, *const c_char) -> c_int,
    set_name: unsafe extern "C" fn(Widget, *const c_char),
    overwrite: unsafe extern "C" fn(Widget, c_int),
    signal: unsafe extern "C" fn(
        Widget,
        *const c_char,
        *const c_void,
        *mut c_void,
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        c_int,
    ) -> c_ulong,
    free: unsafe extern "C" fn(*mut c_void),
}
static API: OnceLock<Result<Api, &'static str>> = OnceLock::new();
unsafe fn load() -> Result<Api, &'static str> {
    // Keep the library loaded: GTK owns process-lifetime caches and callbacks.
    let h = unsafe { libc::dlopen(c"libgtk-3.so.0".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if h.is_null() {
        return Err("GuiDialogUnavailable");
    }
    macro_rules! symbol {
        ($name:literal,$ty:ty) => {{
            let p = unsafe { libc::dlsym(h, concat!($name, "\0").as_ptr().cast()) };
            if p.is_null() {
                return Err("GuiDialogUnavailable");
            }
            unsafe { std::mem::transmute::<*mut c_void, $ty>(p) }
        }};
    }
    let init = symbol!(
        "gtk_init_check",
        unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int
    );
    if unsafe { init(std::ptr::null_mut(), std::ptr::null_mut()) } == 0 {
        return Err("GuiDialogUnavailable");
    }
    Ok(Api {
        owner: std::thread::current().id(),
        dialog: symbol!(
            "gtk_file_chooser_dialog_new",
            unsafe extern "C" fn(*const c_char, Widget, c_int, *const c_char, ...) -> Widget
        ),
        add_button: symbol!(
            "gtk_dialog_add_button",
            unsafe extern "C" fn(Widget, *const c_char, c_int) -> Widget
        ),
        show: symbol!("gtk_widget_show_all", unsafe extern "C" fn(Widget)),
        default_response: symbol!(
            "gtk_dialog_set_default_response",
            unsafe extern "C" fn(Widget, c_int)
        ),
        ref_sink: symbol!("g_object_ref_sink", unsafe extern "C" fn(Widget) -> Widget),
        unref: symbol!("g_object_unref", unsafe extern "C" fn(Widget)),
        destroy: symbol!("gtk_widget_destroy", unsafe extern "C" fn(Widget)),
        pending: symbol!("gtk_events_pending", unsafe extern "C" fn() -> c_int),
        iterate: symbol!(
            "gtk_main_iteration_do",
            unsafe extern "C" fn(c_int) -> c_int
        ),
        filename: symbol!(
            "gtk_file_chooser_get_filename",
            unsafe extern "C" fn(Widget) -> *mut c_char
        ),
        set_filename: symbol!(
            "gtk_file_chooser_set_filename",
            unsafe extern "C" fn(Widget, *const c_char) -> c_int
        ),
        set_folder: symbol!(
            "gtk_file_chooser_set_current_folder",
            unsafe extern "C" fn(Widget, *const c_char) -> c_int
        ),
        set_name: symbol!(
            "gtk_file_chooser_set_current_name",
            unsafe extern "C" fn(Widget, *const c_char)
        ),
        overwrite: symbol!(
            "gtk_file_chooser_set_do_overwrite_confirmation",
            unsafe extern "C" fn(Widget, c_int)
        ),
        signal: symbol!(
            "g_signal_connect_data",
            unsafe extern "C" fn(
                Widget,
                *const c_char,
                *const c_void,
                *mut c_void,
                Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
                c_int,
            ) -> c_ulong
        ),
        free: symbol!("g_free", unsafe extern "C" fn(*mut c_void)),
    })
}
pub(crate) struct Session {
    api: &'static Api,
    widget: Widget,
    response: Box<std::cell::Cell<c_int>>,
    _slot: Slot,
}
unsafe extern "C" fn response(_: Widget, code: c_int, data: *mut c_void) {
    if !data.is_null() {
        unsafe { &*(data.cast::<std::cell::Cell<c_int>>()) }.set(code);
    }
}
unsafe extern "C" fn close(_: Widget, _: *mut c_void, data: *mut c_void) -> c_int {
    if !data.is_null() {
        unsafe { &*(data.cast::<std::cell::Cell<c_int>>()) }.set(-6);
    }
    1
}
impl Session {
    pub fn start(request: &Request) -> Result<Self, &'static str> {
        request.validate()?;
        let slot = Slot::acquire()?;
        let api = API
            .get_or_init(|| unsafe { load() })
            .as_ref()
            .map_err(|e| *e)?;
        if api.owner != std::thread::current().id() {
            return Err("GuiDialogThread");
        }
        let title = CString::new(request.title.as_str()).map_err(|_| "GuiDialogTitle")?;
        let widget = unsafe {
            (api.dialog)(
                title.as_ptr(),
                std::ptr::null_mut(),
                if request.save { 1 } else { 0 },
                std::ptr::null::<c_char>(),
            )
        };
        if widget.is_null() {
            return Err("GuiDialogUnavailable");
        }
        unsafe {
            (api.ref_sink)(widget);
        }
        let mut session = Self {
            api,
            widget,
            response: Box::new(std::cell::Cell::new(0)),
            _slot: slot,
        };
        unsafe {
            (api.add_button)(widget, c"Cancel".as_ptr(), -6);
            (api.add_button)(
                widget,
                if request.save { c"Save" } else { c"Open" }.as_ptr(),
                -3,
            );
            (api.signal)(
                widget,
                c"response".as_ptr(),
                response as *const c_void,
                (&mut *session.response as *mut std::cell::Cell<c_int>).cast(),
                None,
                0,
            );
            (api.signal)(
                widget,
                c"delete-event".as_ptr(),
                close as *const c_void,
                (&mut *session.response as *mut std::cell::Cell<c_int>).cast(),
                None,
                0,
            );
            (api.default_response)(widget, -3);
            if !request.initial.is_empty() {
                let path = std::path::Path::new(&request.initial);
                if request.save {
                    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                        let p = CString::new(parent.as_os_str().as_encoded_bytes())
                            .map_err(|_| "GuiDialogPath")?;
                        (api.set_folder)(widget, p.as_ptr());
                    }
                    if let Some(name) = path.file_name() {
                        let n =
                            CString::new(name.as_encoded_bytes()).map_err(|_| "GuiDialogPath")?;
                        (api.set_name)(widget, n.as_ptr());
                    }
                } else {
                    let p = CString::new(request.initial.as_str()).map_err(|_| "GuiDialogPath")?;
                    (api.set_filename)(widget, p.as_ptr());
                }
            }
            (api.overwrite)(widget, i32::from(request.save));
            (api.show)(widget);
        }
        Ok(session)
    }
    pub fn cancel(&mut self) {
        if !self.widget.is_null() {
            unsafe {
                (self.api.destroy)(self.widget);
                (self.api.unref)(self.widget);
            }
            self.widget = std::ptr::null_mut();
        }
    }
    #[cfg(test)]
    pub fn test_response(&mut self, accept: bool) -> bool {
        unsafe {
            let h = libc::dlopen(c"libgtk-3.so.0".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if h.is_null() {
                return false;
            }
            let p = libc::dlsym(h, c"gtk_dialog_response".as_ptr());
            if !p.is_null() {
                let respond =
                    std::mem::transmute::<*mut c_void, unsafe extern "C" fn(Widget, c_int)>(p);
                respond(self.widget, if accept { -3 } else { -6 });
            }
            libc::dlclose(h);
            !p.is_null()
        }
    }
    pub fn poll(&mut self) -> Option<Result<Option<String>, &'static str>> {
        if self.widget.is_null() {
            return Some(Ok(None));
        }
        for _ in 0..32 {
            if unsafe { (self.api.pending)() } == 0 {
                break;
            }
            unsafe {
                (self.api.iterate)(0);
            }
        }
        let code = self.response.get();
        if code == 0 {
            return None;
        }
        if code != -3 {
            return Some(Ok(None));
        }
        let p = unsafe { (self.api.filename)(self.widget) };
        if p.is_null() {
            return Some(Err("GuiDialogPath"));
        }
        let result = unsafe {
            let len = libc::strnlen(p, 4097);
            if len > 4096 {
                Err("GuiDialogPathLimit")
            } else {
                CStr::from_ptr(p)
                    .to_str()
                    .map(|s| Some(s.to_owned()))
                    .map_err(|_| "GuiDialogPathEncoding")
            }
        };
        unsafe {
            (self.api.free)(p.cast());
        }
        Some(result)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel();
    }
}
