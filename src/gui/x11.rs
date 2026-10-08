use super::*;
use libc::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::ffi::CString;
use std::sync::{Mutex, Once};
// Locale/font/input-method caches are shared by Xlib across displays.
static SETUP: Mutex<()> = Mutex::new(());
static LOCALE: Once = Once::new();
type Window = c_ulong;
#[repr(C)]
struct Rectangle {
    x: i16,
    y: i16,
    width: u16,
    height: u16,
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Button {
    kind: c_int,
    serial: c_ulong,
    send: c_int,
    display: *mut c_void,
    window: Window,
    root: Window,
    subwindow: Window,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    button: c_uint,
    same: c_int,
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Configure {
    kind: c_int,
    serial: c_ulong,
    send: c_int,
    display: *mut c_void,
    event: Window,
    window: Window,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border: c_int,
    above: Window,
    override_redirect: c_int,
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Client {
    kind: c_int,
    serial: c_ulong,
    send: c_int,
    display: *mut c_void,
    window: Window,
    message_type: c_ulong,
    format: c_int,
    data: [c_long; 5],
}
#[repr(C)]
union XEvent {
    kind: c_int,
    button: Button,
    configure: Configure,
    client: Client,
    selection_request: SelectionRequest,
    selection: Selection,
    pad: [c_long; 24],
}
#[derive(Clone, Copy)]
#[repr(C)]
struct SelectionRequest {
    kind: c_int,
    serial: c_ulong,
    send: c_int,
    display: *mut c_void,
    owner: Window,
    requestor: Window,
    selection: c_ulong,
    target: c_ulong,
    property: c_ulong,
    time: c_ulong,
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Selection {
    kind: c_int,
    serial: c_ulong,
    send: c_int,
    display: *mut c_void,
    requestor: Window,
    selection: c_ulong,
    target: c_ulong,
    property: c_ulong,
    time: c_ulong,
}
struct Library(*mut c_void);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.0);
        }
    }
}
macro_rules! xapi {
    ($(fn $name:ident($($arg:ty),*) -> $ret:ty;)*) => {
        #[allow(non_snake_case,dead_code)] struct Api {_library:Library,create_ic:unsafe extern "C" fn(*mut c_void,...)->*mut c_void,$($name:unsafe extern "C" fn($($arg),*)->$ret,)*}
        impl Api {fn load()->io::Result<Self> {unsafe {
            let lib=Library(libc::dlopen(c"libX11.so.6".as_ptr(),libc::RTLD_NOW|libc::RTLD_LOCAL));
            if lib.0.is_null(){return Err(invalid("GuiUnavailable: libX11.so.6 is required for native GUI"));}
            Ok(Self {create_ic:{let symbol=libc::dlsym(lib.0,c"XCreateIC".as_ptr());if symbol.is_null(){return Err(invalid("GuiUnavailable: XCreateIC missing"));}std::mem::transmute::<*mut c_void,unsafe extern "C" fn(*mut c_void,...)->*mut c_void>(symbol)},$($name:{let symbol=libc::dlsym(lib.0,concat!(stringify!($name),"\0").as_ptr().cast());if symbol.is_null(){return Err(invalid("GuiUnavailable: X11 symbol missing"));}std::mem::transmute::<*mut c_void,unsafe extern "C" fn($($arg),*)->$ret>(symbol)},)*_library:lib})
        }}}
    }
}
xapi! {
fn XInitThreads()->c_int;
fn XOpenIM(*mut c_void,*mut c_void,*mut c_char,*mut c_char)->*mut c_void;
fn XCloseIM(*mut c_void)->c_int;
fn XDestroyIC(*mut c_void)->();
fn XSetICFocus(*mut c_void)->();
fn XFilterEvent(*mut XEvent,Window)->c_int;
fn Xutf8LookupString(*mut c_void,*mut Button,*mut c_char,c_int,*mut c_ulong,*mut c_int)->c_int;
fn Xutf8TextEscapement(*mut c_void,*const c_char,c_int)->c_int;
fn XDrawLine(*mut c_void,Window,*mut c_void,c_int,c_int,c_int,c_int)->c_int;
fn XSetLocaleModifiers(*const c_char)->*mut c_char;
fn XOpenDisplay(*const c_char)->*mut c_void;
fn XCloseDisplay(*mut c_void)->c_int;
fn XDefaultScreen(*mut c_void)->c_int;
fn XDefaultDepth(*mut c_void,c_int)->c_int;
fn XRootWindow(*mut c_void,c_int)->Window;
fn XCreateSimpleWindow(*mut c_void,Window,c_int,c_int,c_uint,c_uint,c_uint,c_ulong,c_ulong)->Window;
fn XDestroyWindow(*mut c_void,Window)->c_int;
fn XSetClipRectangles(*mut c_void,*mut c_void,c_int,c_int,*mut Rectangle,c_int,c_int)->c_int;
fn XSetClipMask(*mut c_void,*mut c_void,c_ulong)->c_int;
fn XChangeProperty(*mut c_void,Window,c_ulong,c_ulong,c_int,c_int,*const u8,c_int)->c_int;
fn XSetSelectionOwner(*mut c_void,c_ulong,Window,c_ulong)->c_int;
fn XGetSelectionOwner(*mut c_void,c_ulong)->Window;
fn XConvertSelection(*mut c_void,c_ulong,c_ulong,c_ulong,Window,c_ulong)->c_int;
fn XGetWindowProperty(*mut c_void,Window,c_ulong,c_long,c_long,c_int,c_ulong,*mut c_ulong,*mut c_int,*mut c_ulong,*mut c_ulong,*mut *mut u8)->c_int;
fn XFree(*mut c_void)->c_int;
fn XDeleteProperty(*mut c_void,Window,c_ulong)->c_int;
fn XStoreName(*mut c_void,Window,*const c_char)->c_int;
fn XResizeWindow(*mut c_void,Window,c_uint,c_uint)->c_int;
fn XSelectInput(*mut c_void,Window,c_long)->c_int;
fn XMapWindow(*mut c_void,Window)->c_int;
fn XCreateGC(*mut c_void,Window,c_ulong,*mut c_void)->*mut c_void;
fn XFreeGC(*mut c_void,*mut c_void)->c_int;
fn XSetForeground(*mut c_void,*mut c_void,c_ulong)->c_int;
fn XFillRectangle(*mut c_void,Window,*mut c_void,c_int,c_int,c_uint,c_uint)->c_int;
fn XDrawRectangle(*mut c_void,Window,*mut c_void,c_int,c_int,c_uint,c_uint)->c_int;
fn XCreateFontSet(*mut c_void,*const c_char,*mut *mut *mut c_char,*mut c_int,*mut *mut c_char)->*mut c_void;
fn XFreeStringList(*mut *mut c_char)->();
fn XFreeFontSet(*mut c_void,*mut c_void)->();
fn Xutf8DrawString(*mut c_void,Window,*mut c_void,*mut c_void,c_int,c_int,*const c_char,c_int)->();
fn XInternAtom(*mut c_void,*const c_char,c_int)->c_ulong;
fn XSetWMProtocols(*mut c_void,Window,*mut c_ulong,c_int)->c_int;
fn XPending(*mut c_void)->c_int;
fn XNextEvent(*mut c_void,*mut XEvent)->c_int;
fn XLookupKeysym(*mut Button,c_int)->c_ulong;
fn XKeysymToKeycode(*mut c_void,c_ulong)->u8;
fn XFlush(*mut c_void)->c_int;
fn XSendEvent(*mut c_void,Window,c_int,c_long,*mut XEvent)->c_int;
}
pub(super) struct Surface {
    api: Api,
    display: *mut c_void,
    window: Window,
    gc: *mut c_void,
    font: *mut c_void,
    delete: c_ulong,
    frame: Option<Frame>,
    im: *mut c_void,
    ic: *mut c_void,
    clipboard_enabled: bool,
    clipboard_text: Option<String>,
    clipboard_pending: Option<(String, c_ulong, std::time::Instant, c_ulong)>,
}
impl Surface {
    #[cfg(test)]
    pub fn inject_clipboard_key(&mut self, key: u8) {
        unsafe {
            let mut event: XEvent = std::mem::zeroed();
            event.button.kind = 2;
            event.button.display = self.display;
            event.button.window = self.window;
            event.button.state = 4;
            event.button.button =
                (self.api.XKeysymToKeycode)(self.display, key.to_ascii_lowercase() as c_ulong)
                    as u32;
            event.button.same = 1;
            (self.api.XSendEvent)(self.display, self.window, 0, 1, &mut event);
            (self.api.XFlush)(self.display);
        }
    }
    pub fn new() -> io::Result<Self> {
        let _setup = SETUP.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            LOCALE.call_once(|| {
                libc::setlocale(libc::LC_CTYPE, c"C.UTF-8".as_ptr());
            });
            let api = Api::load()?;
            // This precedes every other Xlib call on the first loaded library,
            // and is harmless if another display already initialized its locks.
            if (api.XInitThreads)() == 0 {
                return Err(invalid("GuiUnavailable: X11 thread initialization failed"));
            }
            (api.XSetLocaleModifiers)(c"".as_ptr());
            let display = (api.XOpenDisplay)(std::ptr::null());
            if display.is_null() {
                return Err(invalid("GuiUnavailable: no X11 display; set DISPLAY or use --gui-events for headless testing"));
            }
            if (api.XDefaultDepth)(display, (api.XDefaultScreen)(display)) < 24 {
                (api.XCloseDisplay)(display);
                return Err(invalid(
                    "GuiUnavailable: a 24-bit TrueColor X11 display is required",
                ));
            }
            let mut missing = std::ptr::null_mut();
            let mut count = 0;
            let mut default = std::ptr::null_mut();
            let font = (api.XCreateFontSet)(
                display,
                c"fixed".as_ptr(),
                &mut missing,
                &mut count,
                &mut default,
            );
            if !missing.is_null() {
                (api.XFreeStringList)(missing);
            }
            if font.is_null() {
                (api.XCloseDisplay)(display);
                return Err(invalid("GuiUnavailable: install X11 core fonts (fixed)"));
            }
            let delete = (api.XInternAtom)(display, c"WM_DELETE_WINDOW".as_ptr(), 0);
            let im = (api.XOpenIM)(
                display,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            Ok(Self {
                api,
                display,
                window: 0,
                gc: std::ptr::null_mut(),
                font,
                delete,
                frame: None,
                im,
                ic: std::ptr::null_mut(),
                clipboard_enabled: false,
                clipboard_text: None,
                clipboard_pending: None,
            })
        }
    }
    pub fn configure_clipboard(&mut self, enabled: bool) {
        self.clipboard_enabled = enabled;
    }
    unsafe fn clipboard_atom(&self, name: &std::ffi::CStr) -> c_ulong {
        unsafe { (self.api.XInternAtom)(self.display, name.as_ptr(), 0) }
    }
    unsafe fn answer_selection(&self, request: SelectionRequest) {
        unsafe {
            let clipboard = self.clipboard_atom(c"CLIPBOARD");
            let utf8 = self.clipboard_atom(c"UTF8_STRING");
            let targets = self.clipboard_atom(c"TARGETS");
            let property = if request.property == 0 {
                request.target
            } else {
                request.property
            };
            let mut accepted = false;
            if request.selection == clipboard && request.owner == self.window {
                if let Some(text) = &self.clipboard_text {
                    if request.target == targets {
                        let supported = [targets, utf8];
                        (self.api.XChangeProperty)(
                            self.display,
                            request.requestor,
                            property,
                            4,
                            32,
                            0,
                            supported.as_ptr().cast(),
                            supported.len() as c_int,
                        );
                        accepted = true;
                    } else if request.target == utf8 {
                        (self.api.XChangeProperty)(
                            self.display,
                            request.requestor,
                            property,
                            utf8,
                            8,
                            0,
                            text.as_ptr(),
                            text.len() as c_int,
                        );
                        accepted = true;
                    }
                }
            }
            let mut reply = XEvent {
                selection: Selection {
                    kind: 31,
                    serial: 0,
                    send: 1,
                    display: self.display,
                    requestor: request.requestor,
                    selection: request.selection,
                    target: request.target,
                    property: if accepted { property } else { 0 },
                    time: request.time,
                },
            };
            (self.api.XSendEvent)(self.display, request.requestor, 0, 0, &mut reply);
            (self.api.XFlush)(self.display);
        }
    }
    unsafe fn receive_selection(&mut self, selection: Selection) -> Option<Event> {
        unsafe {
            let utf8 = self.clipboard_atom(c"UTF8_STRING");
            if selection.requestor != self.window
                || selection.selection != self.clipboard_atom(c"CLIPBOARD")
                || selection.target != utf8
            {
                return None;
            }
            let (_, property, _, time) = self.clipboard_pending.as_ref()?;
            if selection.time != *time {
                return None;
            }
            let property = *property;
            if selection.property != 0 && selection.property != property {
                return None;
            }
            let (focused, _, deadline, _) = self.clipboard_pending.take()?;
            if selection.property == 0 || std::time::Instant::now() > deadline {
                return None;
            }
            let mut actual_type = 0;
            let mut format = 0;
            let mut count = 0;
            let mut remaining = 0;
            let mut data = std::ptr::null_mut();
            let status = (self.api.XGetWindowProperty)(
                self.display,
                self.window,
                property,
                0,
                1025,
                1,
                utf8,
                &mut actual_type,
                &mut format,
                &mut count,
                &mut remaining,
                &mut data,
            );
            let result = if status == 0
                && actual_type == utf8
                && format == 8
                && remaining == 0
                && count <= clipboard::MAX_TEXT_BYTES as c_ulong
                && !data.is_null()
            {
                std::str::from_utf8(std::slice::from_raw_parts(data, count as usize))
                    .ok()
                    .map(str::to_owned)
                    .and_then(clipboard::text_event)
            } else {
                None
            };
            if !data.is_null() {
                (self.api.XFree)(data.cast());
            }
            (self.api.XDeleteProperty)(self.display, self.window, property);
            if self
                .frame
                .as_ref()
                .and_then(clipboard::focused_input)
                .is_some_and(|item| item.id == focused)
            {
                result
            } else {
                None
            }
        }
    }
    pub fn present(&mut self, frame: &Frame) -> io::Result<()> {
        unsafe {
            if self.window == 0 {
                let screen = (self.api.XDefaultScreen)(self.display);
                let root = (self.api.XRootWindow)(self.display, screen);
                self.window = (self.api.XCreateSimpleWindow)(
                    self.display,
                    root,
                    0,
                    0,
                    frame.width as u32,
                    frame.height as u32,
                    0,
                    0,
                    frame.background as c_ulong,
                );
                if self.window == 0 {
                    return Err(invalid("GuiUnavailable: window creation failed"));
                }
                if !self.im.is_null() {
                    self.ic = (self.api.create_ic)(
                        self.im,
                        c"inputStyle".as_ptr(),
                        0x408 as c_ulong,
                        c"clientWindow".as_ptr(),
                        self.window,
                        c"focusWindow".as_ptr(),
                        self.window,
                        std::ptr::null::<c_char>(),
                    );
                    if !self.ic.is_null() {
                        (self.api.XSetICFocus)(self.ic);
                    }
                }
                self.gc = (self.api.XCreateGC)(self.display, self.window, 0, std::ptr::null_mut());
                if self.gc.is_null() {
                    return Err(invalid("GuiUnavailable: graphics context creation failed"));
                }
                (self.api.XSelectInput)(
                    self.display,
                    self.window,
                    (1 << 0) | (1 << 2) | (1 << 15) | (1 << 17),
                );
                (self.api.XSetWMProtocols)(self.display, self.window, &mut self.delete, 1);
                (self.api.XMapWindow)(self.display, self.window);
            }
            let title =
                CString::new(frame.title.as_bytes()).map_err(|_| invalid("GuiInvalidScene"))?;
            (self.api.XStoreName)(self.display, self.window, title.as_ptr());
            let name = (self.api.XInternAtom)(self.display, c"_NET_WM_NAME".as_ptr(), 0);
            let utf8 = (self.api.XInternAtom)(self.display, c"UTF8_STRING".as_ptr(), 0);
            (self.api.XChangeProperty)(
                self.display,
                self.window,
                name,
                utf8,
                8,
                0,
                frame.title.as_ptr(),
                frame.title.len() as i32,
            );
            (self.api.XResizeWindow)(
                self.display,
                self.window,
                frame.width as u32,
                frame.height as u32,
            );
            self.frame = Some(frame.clone());
            self.paint();
            (self.api.XFlush)(self.display);
            Ok(())
        }
    }
    unsafe fn draw_edit(&self, item: &Item) {
        unsafe {
            let mut offset = 0usize;
            let start = item.cursor.min(item.anchor);
            let end = item.cursor.max(item.anchor);
            for (row, line) in item.text.split('\n').enumerate() {
                let count = line.chars().count();
                if row >= item.scroll && row < item.scroll + (item.height as usize / 18).max(1) {
                    let y = item.y + 6 + ((row - item.scroll) * 18) as i32;
                    let mut x = item.x + 6;
                    let prefix = |n: usize| line.chars().take(n).collect::<String>();
                    let width = |s: &str| {
                        (self.api.Xutf8TextEscapement)(self.font, s.as_ptr().cast(), s.len() as i32)
                    };
                    if item.cursor >= offset && item.cursor <= offset + count {
                        let caret = width(&prefix(item.cursor - offset));
                        x -= (caret - (item.width - 14)).max(0);
                    }
                    if start < end && end > offset && start < offset + count {
                        let a = prefix(start.saturating_sub(offset).min(count));
                        let b = prefix(end.saturating_sub(offset).min(count));
                        let left = width(&a);
                        let right = width(&b);
                        (self.api.XSetForeground)(self.display, self.gc, 0xbfdbfe);
                        (self.api.XFillRectangle)(
                            self.display,
                            self.window,
                            self.gc,
                            x + left,
                            y,
                            (right - left).max(1) as u32,
                            18,
                        );
                    }
                    (self.api.XSetForeground)(self.display, self.gc, item.foreground as c_ulong);
                    (self.api.Xutf8DrawString)(
                        self.display,
                        self.window,
                        self.font,
                        self.gc,
                        x,
                        y + 13,
                        line.as_ptr().cast(),
                        line.len() as i32,
                    );
                    if item.focused && item.cursor >= offset && item.cursor <= offset + count {
                        let caret = x + width(&prefix(item.cursor - offset));
                        (self.api.XDrawLine)(
                            self.display,
                            self.window,
                            self.gc,
                            caret,
                            y,
                            caret,
                            y + 16,
                        );
                    }
                }
                offset += count + 1;
            }
        }
    }
    fn paint(&self) {
        unsafe {
            let Some(frame) = &self.frame else {
                return;
            };
            (self.api.XSetForeground)(self.display, self.gc, frame.background as c_ulong);
            (self.api.XFillRectangle)(
                self.display,
                self.window,
                self.gc,
                0,
                0,
                frame.width as u32,
                frame.height as u32,
            );
            for item in &frame.items {
                if item.kind != "label" {
                    (self.api.XSetForeground)(self.display, self.gc, item.background as c_ulong);
                    (self.api.XFillRectangle)(
                        self.display,
                        self.window,
                        self.gc,
                        item.x,
                        item.y,
                        item.width as u32,
                        item.height as u32,
                    );
                    (self.api.XSetForeground)(
                        self.display,
                        self.gc,
                        if item.focused { 0x2563eb } else { 0x888888 },
                    );
                    (self.api.XDrawRectangle)(
                        self.display,
                        self.window,
                        self.gc,
                        item.x,
                        item.y,
                        (item.width - 1) as u32,
                        (item.height - 1) as u32,
                    );
                }
                if item.kind == "rect" {
                    continue;
                }
                let text = if item.kind == "checkbox" {
                    format!("[{}] {}", if item.checked { "x" } else { " " }, item.text)
                } else {
                    item.text.clone()
                };
                (self.api.XSetForeground)(
                    self.display,
                    self.gc,
                    if item.enabled {
                        item.foreground as c_ulong
                    } else {
                        0x888888
                    },
                );
                let mut clip = Rectangle {
                    x: item.x as i16,
                    y: item.y as i16,
                    width: item.width as u16,
                    height: item.height as u16,
                };
                (self.api.XSetClipRectangles)(self.display, self.gc, 0, 0, &mut clip, 1, 0);
                if matches!(item.kind.as_str(), "textbox" | "textarea") {
                    self.draw_edit(item);
                    (self.api.XSetClipMask)(self.display, self.gc, 0);
                    continue;
                }
                (self.api.Xutf8DrawString)(
                    self.display,
                    self.window,
                    self.font,
                    self.gc,
                    item.x + 6,
                    item.y + (item.height / 2) + 5,
                    text.as_ptr().cast(),
                    text.len() as i32,
                );
                (self.api.XSetClipMask)(self.display, self.gc, 0);
            }
        }
    }
    pub fn event(&mut self) -> io::Result<Event> {
        self.read(true)?.ok_or_else(|| invalid("GuiClosed"))
    }
    pub fn poll(&mut self) -> io::Result<Option<Event>> {
        self.read(false)
    }
    fn read(&mut self, blocking: bool) -> io::Result<Option<Event>> {
        if self.window == 0 {
            return Err(invalid("GuiNotPublished"));
        }
        loop {
            unsafe {
                if !blocking && (self.api.XPending)(self.display) == 0 {
                    return Ok(None);
                }
                let mut e = XEvent { pad: [0; 24] };
                (self.api.XNextEvent)(self.display, &mut e);
                if !self.ic.is_null() && (self.api.XFilterEvent)(&mut e, 0) != 0 {
                    continue;
                }
                match e.kind {
                    29 if self.clipboard_enabled => {
                        self.clipboard_text = None;
                    }
                    30 if self.clipboard_enabled => {
                        self.answer_selection(e.selection_request);
                    }
                    31 if self.clipboard_enabled => {
                        if let Some(event) = self.receive_selection(e.selection) {
                            return Ok(Some(event));
                        }
                    }
                    12 => {
                        self.paint();
                        (self.api.XFlush)(self.display);
                    }
                    4 => {
                        let b = e.button;
                        if b.button == 4 || b.button == 5 {
                            let mut out = Event::simple("wheel");
                            out.y = if b.button == 4 { -1 } else { 1 };
                            return Ok(Some(out));
                        }
                        if b.button == 1 {
                            let mut out = Event::simple("pointer");
                            out.x = b.x;
                            out.y = b.y;
                            return Ok(Some(out));
                        }
                    }
                    2 => {
                        let mut b = e.button;
                        let key =
                            (self.api.XLookupKeysym)(&mut b, if b.state & 1 != 0 { 1 } else { 0 });
                        if self.clipboard_enabled
                            && b.state & 4 != 0
                            && matches!(key, 0x63 | 0x43 | 0x76 | 0x56 | 0x78 | 0x58)
                        {
                            let focused = self
                                .frame
                                .as_ref()
                                .and_then(clipboard::focused_input)
                                .map(|item| item.id.clone());
                            if let Some(focused) = focused {
                                let atom = self.clipboard_atom(c"CLIPBOARD");
                                if matches!(key, 0x76 | 0x56) {
                                    if (self.api.XGetSelectionOwner)(self.display, atom)
                                        == self.window
                                    {
                                        if let Some(event) = self
                                            .clipboard_text
                                            .clone()
                                            .and_then(clipboard::text_event)
                                        {
                                            return Ok(Some(event));
                                        }
                                    } else {
                                        if self.clipboard_pending.as_ref().is_some_and(
                                            |(_, _, deadline, _)| {
                                                std::time::Instant::now() > *deadline
                                            },
                                        ) {
                                            self.clipboard_pending = None;
                                        }
                                        if self.clipboard_pending.is_some() {
                                            continue;
                                        }
                                        let property =
                                            self.clipboard_atom(c"REWIND_CLIPBOARD_TEXT");
                                        self.clipboard_pending = Some((
                                            focused,
                                            property,
                                            std::time::Instant::now()
                                                + std::time::Duration::from_secs(2),
                                            b.time,
                                        ));
                                        (self.api.XConvertSelection)(
                                            self.display,
                                            atom,
                                            self.clipboard_atom(c"UTF8_STRING"),
                                            property,
                                            self.window,
                                            b.time,
                                        );
                                        (self.api.XFlush)(self.display);
                                    }
                                } else if let Some(text) =
                                    self.frame.as_ref().and_then(clipboard::selected_text)
                                {
                                    self.clipboard_text = Some(text);
                                    (self.api.XSetSelectionOwner)(
                                        self.display,
                                        atom,
                                        self.window,
                                        b.time,
                                    );
                                    (self.api.XFlush)(self.display);
                                    if matches!(key, 0x78 | 0x58)
                                        && (self.api.XGetSelectionOwner)(self.display, atom)
                                            == self.window
                                    {
                                        let mut event = Event::simple("key");
                                        event.key = "Delete".into();
                                        return Ok(Some(event));
                                    }
                                }
                            }
                            continue;
                        }
                        if !self.ic.is_null()
                            && b.state & 4 == 0
                            && key != 0x20
                            && !(0xff00..=0xffff).contains(&key)
                        {
                            let mut buffer = vec![0u8; 4096];
                            let mut symbol = 0;
                            let mut status = 0;
                            let count = (self.api.Xutf8LookupString)(
                                self.ic,
                                &mut b,
                                buffer.as_mut_ptr().cast(),
                                buffer.len() as i32,
                                &mut symbol,
                                &mut status,
                            );
                            if count > 0 && count <= 4096 && status != -1 {
                                if let Ok(text) = std::str::from_utf8(&buffer[..count as usize]) {
                                    if !text.chars().any(char::is_control) {
                                        let mut out = Event::simple("text");
                                        out.key = text.into();
                                        return Ok(Some(out));
                                    }
                                }
                            }
                        }
                        let name = match key {
                            0xff0d => "Enter".into(),
                            0xff09 => "Tab".into(),
                            0xff1b => "Escape".into(),
                            0xff08 => "Backspace".into(),
                            0xffff => "Delete".into(),
                            0xff50 => "Home".into(),
                            0xff57 => "End".into(),
                            0x61 | 0x41 if b.state & 4 != 0 => "Ctrl+A".into(),
                            0xff51 => "Left".into(),
                            0xff52 => "Up".into(),
                            0xff53 => "Right".into(),
                            0xff54 => "Down".into(),
                            0x20 => "Space".into(),
                            0x21..=0x7e | 0xa0..=0xff => {
                                char::from_u32(key as u32).unwrap().to_string()
                            }
                            0x1000000..=0x110ffff => char::from_u32((key - 0x1000000) as u32)
                                .map(|c| c.to_string())
                                .unwrap_or_default(),
                            _ => continue,
                        };
                        let mut out = Event::simple("key");
                        if name.chars().count() == 1 {
                            out.kind = "text".into();
                        }
                        out.key = if b.state & 1 != 0
                            && matches!(
                                name.as_str(),
                                "Left" | "Right" | "Home" | "End" | "Up" | "Down"
                            ) {
                            format!("Shift+{name}")
                        } else {
                            name
                        };
                        return Ok(Some(out));
                    }
                    22 => {
                        let c = e.configure;
                        let mut out = Event::simple("resize");
                        out.width = c.width;
                        out.height = c.height;
                        return Ok(Some(out));
                    }
                    33 if e.client.data[0] as c_ulong == self.delete => {
                        self.close();
                        return Ok(Some(Event::simple("close")));
                    }
                    _ => {}
                }
            }
        }
    }
    pub fn close(&mut self) {
        unsafe {
            if !self.ic.is_null() {
                (self.api.XDestroyIC)(self.ic);
                self.ic = std::ptr::null_mut();
            }
            if self.window != 0 {
                if !self.gc.is_null() {
                    (self.api.XFreeGC)(self.display, self.gc);
                    self.gc = std::ptr::null_mut();
                }
                (self.api.XDestroyWindow)(self.display, self.window);
                self.window = 0;
                (self.api.XFlush)(self.display);
            }
            self.frame = None;
        }
    }
    #[cfg(test)]
    pub fn inject_key_and_close(&mut self) {
        unsafe {
            let mut key: XEvent = std::mem::zeroed();
            key.button.kind = 2;
            key.button.display = self.display;
            key.button.window = self.window;
            key.button.button = (self.api.XKeysymToKeycode)(self.display, 0xff0d) as u32;
            key.button.same = 1;
            (self.api.XSendEvent)(self.display, self.window, 0, 1, &mut key);
            let mut close = XEvent {
                client: Client {
                    kind: 33,
                    serial: 0,
                    send: 1,
                    display: self.display,
                    window: self.window,
                    message_type: (self.api.XInternAtom)(self.display, c"WM_PROTOCOLS".as_ptr(), 0),
                    format: 32,
                    data: [self.delete as c_long, 0, 0, 0, 0],
                },
            };
            (self.api.XSendEvent)(self.display, self.window, 0, 0, &mut close);
            (self.api.XFlush)(self.display);
        }
    }
    #[cfg(test)]
    pub fn inject_pointer(&mut self, x: i32, y: i32) {
        unsafe {
            let root =
                (self.api.XRootWindow)(self.display, (self.api.XDefaultScreen)(self.display));
            let mut e = XEvent {
                button: Button {
                    kind: 4,
                    serial: 0,
                    send: 1,
                    display: self.display,
                    window: self.window,
                    root,
                    subwindow: 0,
                    time: 0,
                    x,
                    y,
                    x_root: x,
                    y_root: y,
                    state: 0,
                    button: 1,
                    same: 1,
                },
            };
            (self.api.XSendEvent)(self.display, self.window, 0, 1 << 2, &mut e);
            (self.api.XFlush)(self.display);
        }
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        let _setup = SETUP.lock().unwrap_or_else(|e| e.into_inner());
        self.close();
        unsafe {
            if !self.im.is_null() {
                (self.api.XCloseIM)(self.im);
            }
            (self.api.XFreeFontSet)(self.display, self.font);
            (self.api.XCloseDisplay)(self.display);
        }
    }
}
