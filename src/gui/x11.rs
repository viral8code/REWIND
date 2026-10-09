use super::*;
use libc::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::sync::{Arc, Mutex, Once};
use std::{cell::UnsafeCell, ffi::CString};
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
#[repr(C)]
struct ImeStyles {
    count: u16,
    styles: *mut c_ulong,
}
#[repr(C)]
struct ImeCallback {
    data: *mut c_void,
    function: *mut c_void,
}
#[repr(C)]
union ImeString {
    bytes: *const u8,
    wide: *const i32,
}
#[repr(C)]
struct ImeText {
    length: u16,
    feedback: *const c_ulong,
    wide: c_int,
    string: ImeString,
}
#[repr(C)]
struct ImeDraw {
    caret: c_int,
    first: c_int,
    count: c_int,
    text: *const ImeText,
}
#[repr(C)]
struct ImeCaret {
    position: c_int,
    direction: c_int,
    style: c_int,
}
#[repr(C)]
struct ImePoint {
    x: i16,
    y: i16,
}
unsafe fn preedit_state<'a>(data: *mut c_void) -> Option<&'a mut composition::Composition> {
    if data.is_null() {
        return None;
    }
    Some(unsafe { &mut *(*data.cast::<UnsafeCell<composition::Composition>>()).get() })
}
unsafe extern "C" fn preedit_start(_: *mut c_void, data: *mut c_void, _: *mut c_void) -> c_int {
    if let Some(state) = unsafe { preedit_state(data) } {
        if state.accepting {
            state.start();
        }
    }
    composition::MAX_BYTES as c_int
}
unsafe extern "C" fn preedit_done(_: *mut c_void, data: *mut c_void, _: *mut c_void) {
    if let Some(state) = unsafe { preedit_state(data) } {
        state.clear();
    }
}
unsafe fn preedit_text(text: &ImeText) -> Option<String> {
    if text.length as usize > composition::MAX_BYTES {
        return None;
    }
    if text.length == 0 {
        return Some(String::new());
    }
    if text.wide != 0 {
        let ptr = unsafe { text.string.wide };
        if ptr.is_null() {
            return None;
        }
        let mut result = String::new();
        for i in 0..text.length as usize {
            let value = unsafe { *ptr.add(i) };
            let character = char::from_u32(value as u32)?;
            if character == '\0' || result.len() + character.len_utf8() > composition::MAX_BYTES {
                return None;
            }
            result.push(character);
        }
        Some(result)
    } else {
        let ptr = unsafe { text.string.bytes };
        if ptr.is_null() {
            return None;
        }
        let mut bytes = Vec::new();
        // XIM length is characters, not bytes; do not scan beyond those characters for a NUL.
        for _ in 0..text.length {
            let first = unsafe { *ptr.add(bytes.len()) };
            let count = match first {
                1..=0x7f => 1,
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => return None,
            };
            if bytes.len() + count > composition::MAX_BYTES {
                return None;
            }
            let begin = bytes.len();
            for _ in 0..count {
                bytes.push(unsafe { *ptr.add(bytes.len()) });
            }
            std::str::from_utf8(&bytes[begin..]).ok()?;
        }
        String::from_utf8(bytes).ok()
    }
}
unsafe extern "C" fn preedit_draw(_: *mut c_void, data: *mut c_void, call: *mut c_void) {
    let Some(state) = (unsafe { preedit_state(data) }) else {
        return;
    };
    if !state.accepting || !state.active {
        return;
    }
    if call.is_null() {
        state.invalid = true;
        return;
    }
    let change = unsafe { &*call.cast::<ImeDraw>() };
    // XIM can change feedback without supplying a replacement string.
    if !change.text.is_null() && unsafe { (*change.text).string.bytes.is_null() } {
        let length = state.text.chars().count();
        if change.first < 0
            || change.count < 0
            || change.caret < 0
            || change.first as usize + change.count as usize > length
            || change.caret as usize > length
        {
            state.invalid = true;
            return;
        }
        state.cursor = change.caret as usize;
        state.dirty = true;
        return;
    }
    let value = if change.text.is_null() {
        Some(String::new())
    } else {
        unsafe { preedit_text(&*change.text) }
    };
    if let Some(value) = value {
        state.replace(change.first, change.count, &value, change.caret);
    } else {
        state.invalid = true;
    }
}
unsafe extern "C" fn preedit_caret(_: *mut c_void, data: *mut c_void, call: *mut c_void) {
    let Some(state) = (unsafe { preedit_state(data) }) else {
        return;
    };
    if !state.accepting || !state.active {
        return;
    }
    if call.is_null() {
        state.invalid = true;
        return;
    }
    let change = unsafe { &mut *call.cast::<ImeCaret>() };
    let length = state.text.chars().count();
    let cursor = match change.direction {
        0 => state.cursor.saturating_add(1).min(length),
        1 => state.cursor.saturating_sub(1),
        8 => 0,
        9 => length,
        10 => {
            if change.position < 0 || change.position as usize > length {
                state.invalid = true;
                state.cursor
            } else {
                change.position as usize
            }
        }
        _ => state.cursor,
    };
    state.cursor = cursor;
    state.dirty = true;
    change.position = cursor as c_int;
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
        #[allow(non_snake_case,dead_code)] struct Api {_library:Library,create_ic:unsafe extern "C" fn(*mut c_void,...)->*mut c_void,get_im_values:unsafe extern "C" fn(*mut c_void,...)->*mut c_char,set_ic_values:unsafe extern "C" fn(*mut c_void,...)->*mut c_char,nested_list:unsafe extern "C" fn(c_int,...)->*mut c_void,$($name:unsafe extern "C" fn($($arg),*)->$ret,)*}
        impl Api {fn load()->io::Result<Self> {unsafe {
            let lib=Library(libc::dlopen(c"libX11.so.6".as_ptr(),libc::RTLD_NOW|libc::RTLD_LOCAL));
            if lib.0.is_null(){return Err(invalid("GuiUnavailable: libX11.so.6 is required for native GUI"));}
            for name in [c"XGetIMValues",c"XSetICValues",c"XVaCreateNestedList"] {
                if libc::dlsym(lib.0,name.as_ptr()).is_null(){return Err(invalid("GuiUnavailable: X11 IME symbol missing"));}
            }
            Ok(Self {get_im_values:std::mem::transmute::<*mut c_void,unsafe extern "C" fn(*mut c_void,...)->*mut c_char>(libc::dlsym(lib.0,c"XGetIMValues".as_ptr())),set_ic_values:std::mem::transmute::<*mut c_void,unsafe extern "C" fn(*mut c_void,...)->*mut c_char>(libc::dlsym(lib.0,c"XSetICValues".as_ptr())),nested_list:std::mem::transmute::<*mut c_void,unsafe extern "C" fn(c_int,...)->*mut c_void>(libc::dlsym(lib.0,c"XVaCreateNestedList".as_ptr())),create_ic:{let symbol=libc::dlsym(lib.0,c"XCreateIC".as_ptr());if symbol.is_null(){return Err(invalid("GuiUnavailable: XCreateIC missing"));}std::mem::transmute::<*mut c_void,unsafe extern "C" fn(*mut c_void,...)->*mut c_void>(symbol)},$($name:{let symbol=libc::dlsym(lib.0,concat!(stringify!($name),"\0").as_ptr().cast());if symbol.is_null(){return Err(invalid("GuiUnavailable: X11 symbol missing"));}std::mem::transmute::<*mut c_void,unsafe extern "C" fn($($arg),*)->$ret>(symbol)},)*_library:lib})
        }}}
    }
}
xapi! {
fn XInitThreads()->c_int;
fn XOpenIM(*mut c_void,*mut c_void,*mut c_char,*mut c_char)->*mut c_void;
fn XCloseIM(*mut c_void)->c_int;
fn XDestroyIC(*mut c_void)->();
fn XSetICFocus(*mut c_void)->();
fn XUnsetICFocus(*mut c_void)->();
fn Xutf8ResetIC(*mut c_void)->*mut c_char;
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
fn XConnectionNumber(*mut c_void)->c_int;
fn XSync(*mut c_void,c_int)->c_int;
fn XGetImage(*mut c_void,Window,c_int,c_int,c_uint,c_uint,c_ulong,c_int)->*mut c_void;
fn XGetPixel(*mut c_void,c_int,c_int)->c_ulong;
fn XDestroyImage(*mut c_void)->c_int;
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
    frame: Option<Arc<Frame>>,
    accessibility_enabled: bool,
    accessibility_attempted: bool,
    accessibility_tree: accessibility::Tree,
    accessibility_bridge: Option<linux_accessibility::Bridge>,
    im: *mut c_void,
    ic: *mut c_void,
    ime_enabled: bool,
    ime_callbacks: bool,
    composition: Box<UnsafeCell<composition::Composition>>,
    callbacks: Box<[ImeCallback; 4]>,
    clipboard_enabled: bool,
    command_keys_enabled: bool,
    clipboard_text: Option<String>,
    clipboard_pending: Option<(String, c_ulong, std::time::Instant, c_ulong)>,
}
impl Surface {
    pub fn configure_accessibility(&mut self, enabled: bool) {
        self.accessibility_enabled = enabled;
    }
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
            let composition = Box::new(UnsafeCell::new(composition::Composition::default()));
            let data = (&*composition as *const UnsafeCell<composition::Composition>)
                .cast_mut()
                .cast();
            let callbacks = Box::new([
                ImeCallback {
                    data,
                    function: preedit_start as *const () as *mut c_void,
                },
                ImeCallback {
                    data,
                    function: preedit_done as *const () as *mut c_void,
                },
                ImeCallback {
                    data,
                    function: preedit_draw as *const () as *mut c_void,
                },
                ImeCallback {
                    data,
                    function: preedit_caret as *const () as *mut c_void,
                },
            ]);
            Ok(Self {
                ime_enabled: false,
                ime_callbacks: false,
                composition,
                callbacks,
                api,
                display,
                window: 0,
                gc: std::ptr::null_mut(),
                font,
                delete,
                frame: None,
                accessibility_enabled: false,
                accessibility_attempted: false,
                accessibility_tree: accessibility::Tree::default(),
                accessibility_bridge: None,
                im,
                ic: std::ptr::null_mut(),
                clipboard_enabled: false,
                command_keys_enabled: false,
                clipboard_text: None,
                clipboard_pending: None,
            })
        }
    }
    #[cfg(test)]
    pub fn inject_command_key(
        &mut self,
        letter: Option<char>,
        function: Option<u8>,
        control: bool,
        alt: bool,
        shift: bool,
    ) {
        unsafe {
            let symbol = function
                .map(|n| 0xffbe + n as c_ulong - 1)
                .unwrap_or_else(|| letter.unwrap().to_ascii_lowercase() as c_ulong);
            let mut event: XEvent = std::mem::zeroed();
            event.button.kind = 2;
            event.button.display = self.display;
            event.button.window = self.window;
            event.button.state = (if control { 4 } else { 0 })
                | (if alt { 8 } else { 0 })
                | (if shift { 1 } else { 0 });
            event.button.button = (self.api.XKeysymToKeycode)(self.display, symbol) as u32;
            event.button.same = 1;
            (self.api.XSendEvent)(self.display, self.window, 0, 1, &mut event);
            (self.api.XFlush)(self.display);
        }
    }
    pub fn configure_command_keys(&mut self, enabled: bool) {
        self.command_keys_enabled = enabled;
    }
    pub fn configure_ime(&mut self, enabled: bool) {
        self.ime_enabled = enabled;
    }
    fn create_input_context(&mut self) {
        if self.im.is_null() {
            return;
        }
        unsafe {
            if self.ime_enabled {
                let mut styles: *mut ImeStyles = std::ptr::null_mut();
                let error = (self.api.get_im_values)(
                    self.im,
                    c"queryInputStyle".as_ptr(),
                    &mut styles,
                    std::ptr::null::<c_char>(),
                );
                let callbacks = error.is_null()
                    && !styles.is_null()
                    && (*styles).count <= 1024
                    && !(*styles).styles.is_null()
                    && std::slice::from_raw_parts((*styles).styles, (*styles).count as usize)
                        .contains(&0x402);
                if !styles.is_null() {
                    (self.api.XFree)(styles.cast());
                }
                if callbacks {
                    let attributes = (self.api.nested_list)(
                        0,
                        c"preeditStartCallback".as_ptr(),
                        &self.callbacks[0],
                        c"preeditDoneCallback".as_ptr(),
                        &self.callbacks[1],
                        c"preeditDrawCallback".as_ptr(),
                        &self.callbacks[2],
                        c"preeditCaretCallback".as_ptr(),
                        &self.callbacks[3],
                        std::ptr::null::<c_char>(),
                    );
                    if !attributes.is_null() {
                        self.ic = (self.api.create_ic)(
                            self.im,
                            c"inputStyle".as_ptr(),
                            0x402 as c_ulong,
                            c"clientWindow".as_ptr(),
                            self.window,
                            c"focusWindow".as_ptr(),
                            self.window,
                            c"preeditAttributes".as_ptr(),
                            attributes,
                            std::ptr::null::<c_char>(),
                        );
                        (self.api.XFree)(attributes);
                        self.ime_callbacks = !self.ic.is_null();
                    }
                }
            }
            if self.ic.is_null() {
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
            }
            if !self.ic.is_null() {
                (self.api.XSetICFocus)(self.ic);
            }
        }
    }
    fn reset_composition(&mut self) {
        unsafe {
            if !self.ic.is_null() {
                let text = (self.api.Xutf8ResetIC)(self.ic);
                if !text.is_null() {
                    (self.api.XFree)(text.cast());
                }
            }
            (*self.composition.get()).clear();
        }
    }
    fn refresh_composition(&mut self) -> io::Result<()> {
        unsafe {
            if (*self.composition.get()).invalid {
                return Err(invalid("GuiCompositionInvalid"));
            }
            if (*self.composition.get()).dirty {
                (*self.composition.get()).dirty = false;
                self.paint();
                (self.api.XFlush)(self.display);
            }
        }
        Ok(())
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
                .as_deref()
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
                self.create_input_context();
                self.gc = (self.api.XCreateGC)(self.display, self.window, 0, std::ptr::null_mut());
                if self.gc.is_null() {
                    return Err(invalid("GuiUnavailable: graphics context creation failed"));
                }
                (self.api.XSelectInput)(
                    self.display,
                    self.window,
                    (1 << 0)
                        | (1 << 2)
                        | (1 << 15)
                        | (1 << 17)
                        | if self.ime_enabled || self.accessibility_enabled {
                            (1 << 1) | (1 << 21)
                        } else {
                            0
                        },
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
            if self.ime_enabled {
                let before = self.frame.as_deref().and_then(clipboard::focused_input);
                let after = clipboard::focused_input(frame);
                if !composition::same_input(before, after) && (*self.composition.get()).active {
                    // Destroying the old IC prevents queued feedback for its old
                    // text/cursor from being applied to the newly published input.
                    (*self.composition.get()).accepting = false;
                    if !self.ic.is_null() {
                        (self.api.XDestroyIC)(self.ic);
                        self.ic = std::ptr::null_mut();
                    }
                    self.ime_callbacks = false;
                    (*self.composition.get()).clear();
                    self.create_input_context();
                }
            }
            let previous = self.frame.clone();
            let published = Arc::new(frame.clone());
            self.frame = Some(published.clone());
            if self.accessibility_enabled {
                self.accessibility_tree.publish(published);
                if self
                    .accessibility_bridge
                    .as_ref()
                    .is_some_and(|bridge| bridge.needs_rebind())
                {
                    self.accessibility_bridge = None;
                    self.accessibility_bridge =
                        linux_accessibility::Bridge::start(self.accessibility_tree.clone())?;
                } else if let Some(bridge) = &mut self.accessibility_bridge {
                    bridge.sync()?;
                    bridge.changed(previous.as_deref(), frame);
                } else if !self.accessibility_attempted {
                    self.accessibility_attempted = true;
                    self.accessibility_bridge =
                        linux_accessibility::Bridge::start(self.accessibility_tree.clone())?;
                }
            }
            (*self.composition.get()).accepting = clipboard::focused_input(frame).is_some();
            if self.ime_callbacks {
                if let Some(item) = clipboard::focused_input(frame) {
                    let (x, y) = composition::anchor(item, |value| {
                        (self.api.Xutf8TextEscapement)(
                            self.font,
                            value.as_ptr().cast(),
                            value.len() as c_int,
                        )
                    });
                    let mut spot = ImePoint {
                        x: x as i16,
                        y: (y + 13) as i16,
                    };
                    let attrs = (self.api.nested_list)(
                        0,
                        c"spotLocation".as_ptr(),
                        &mut spot,
                        std::ptr::null::<c_char>(),
                    );
                    if !attrs.is_null() {
                        (self.api.set_ic_values)(
                            self.ic,
                            c"preeditAttributes".as_ptr(),
                            attrs,
                            std::ptr::null::<c_char>(),
                        );
                        (self.api.XFree)(attrs);
                    }
                }
            }
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
            self.paint_preedit();
        }
    }
    unsafe fn paint_preedit(&self) {
        unsafe {
            let state = &*self.composition.get();
            if !state.active || state.text.is_empty() {
                return;
            }
            let Some(item) = self.frame.as_deref().and_then(clipboard::focused_input) else {
                return;
            };
            let measure = |value: &str| {
                (self.api.Xutf8TextEscapement)(
                    self.font,
                    value.as_ptr().cast(),
                    value.len() as c_int,
                )
            };
            let (x, y) = composition::anchor(item, measure);
            let width = measure(&state.text);
            let mut clip = Rectangle {
                x: item.x as i16,
                y: item.y as i16,
                width: item.width as u16,
                height: item.height as u16,
            };
            (self.api.XSetClipRectangles)(self.display, self.gc, 0, 0, &mut clip, 1, 0);
            (self.api.XSetForeground)(self.display, self.gc, item.background as c_ulong);
            (self.api.XFillRectangle)(
                self.display,
                self.window,
                self.gc,
                x,
                y,
                width.max(1) as u32,
                18,
            );
            (self.api.XSetForeground)(self.display, self.gc, item.foreground as c_ulong);
            (self.api.Xutf8DrawString)(
                self.display,
                self.window,
                self.font,
                self.gc,
                x,
                y + 13,
                state.text.as_ptr().cast(),
                state.text.len() as c_int,
            );
            (self.api.XDrawLine)(
                self.display,
                self.window,
                self.gc,
                x,
                y + 17,
                x + width,
                y + 17,
            );
            let prefix: String = state.text.chars().take(state.cursor).collect();
            let cursor = x + measure(&prefix);
            (self.api.XDrawLine)(
                self.display,
                self.window,
                self.gc,
                cursor,
                y,
                cursor,
                y + 16,
            );
            (self.api.XSetClipMask)(self.display, self.gc, 0);
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
        let mut processed = 0;
        loop {
            if (self.ime_enabled || self.accessibility_enabled) && !blocking && processed >= 64 {
                return Ok(None);
            }
            processed += 1;
            if let Some(bridge) = &mut self.accessibility_bridge {
                if let Some(event) = bridge.pump() {
                    return Ok(Some(event));
                }
                if blocking && unsafe { (self.api.XPending)(self.display) } == 0 {
                    bridge.wait(unsafe { (self.api.XConnectionNumber)(self.display) })?;
                    continue;
                }
            }
            unsafe {
                if !blocking && (self.api.XPending)(self.display) == 0 {
                    return Ok(None);
                }
                let mut e = XEvent { pad: [0; 24] };
                (self.api.XNextEvent)(self.display, &mut e);
                if e.kind == 9 || e.kind == 10 {
                    self.accessibility_tree.focus(e.kind == 9);
                }
                let filtered = !self.ic.is_null() && (self.api.XFilterEvent)(&mut e, 0) != 0;
                self.refresh_composition()?;
                if filtered {
                    continue;
                }
                match e.kind {
                    9 if self.ime_enabled => {
                        (*self.composition.get()).accepting = self
                            .frame
                            .as_deref()
                            .and_then(clipboard::focused_input)
                            .is_some();
                        if !self.ic.is_null() {
                            (self.api.XSetICFocus)(self.ic);
                        }
                    }
                    10 if self.ime_enabled => {
                        (*self.composition.get()).accepting = false;
                        self.reset_composition();
                        if !self.ic.is_null() {
                            (self.api.XUnsetICFocus)(self.ic);
                        }
                        self.refresh_composition()?;
                    }
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
                            && (!self.command_keys_enabled || b.state & (1 | 8 | 128) == 0)
                            && b.state & 4 != 0
                            && matches!(key, 0x63 | 0x43 | 0x76 | 0x56 | 0x78 | 0x58)
                        {
                            let focused = self
                                .frame
                                .as_deref()
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
                                    self.frame.as_deref().and_then(clipboard::selected_text)
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
                            if self
                                .frame
                                .as_deref()
                                .and_then(clipboard::focused_input)
                                .is_some()
                                || !self.command_keys_enabled
                            {
                                continue;
                            }
                        }
                        if self.command_keys_enabled {
                            let letter = u32::try_from(key).ok().and_then(char::from_u32);
                            let function = (0xffbe..=0xffd5)
                                .contains(&key)
                                .then(|| (key - 0xffbe + 1) as u8);
                            if let Some(name) = command_keys::command_key(
                                letter,
                                function,
                                b.state & 4 != 0,
                                b.state & 8 != 0,
                                b.state & 1 != 0,
                                b.state & 128 != 0,
                            ) {
                                let mut event = Event::simple("key");
                                event.key = name;
                                return Ok(Some(event));
                            }
                        }
                        if !self.ic.is_null()
                            && (b.state & 4 == 0
                                || (self.command_keys_enabled
                                    && (b.state & 128 != 0 || b.state & (4 | 8) == (4 | 8))))
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
                            self.refresh_composition()?;
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
        self.accessibility_tree.close();
        self.accessibility_bridge = None;
        self.accessibility_attempted = false;
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

#[cfg(test)]
mod ime_tests {
    use super::*;
    #[test]
    fn native_ime_uses_real_anthy_preedit_and_commits_only_final_text() {
        if std::env::var_os("REWIND_TEST_IME_SERVICE").is_none() {
            return;
        }
        let title = format!("REWIND native IME {}", std::process::id());
        let mut frame = Frame {
            title: title.clone(),
            width: 320,
            height: 160,
            background: 0xffffff,
            items: vec![Item {
                id: "input".into(),
                kind: "textbox".into(),
                x: 10,
                y: 10,
                width: 280,
                height: 40,
                text: String::new(),
                foreground: 0,
                background: 0xffffff,
                enabled: true,
                checked: false,
                focused: true,
                cursor: 0,
                anchor: 0,
                scroll: 0,
            }],
        };
        let mut surface = Surface::new().unwrap();
        surface.configure_ime(true);
        surface.present(&frame).unwrap();
        assert!(
            surface.ime_callbacks,
            "The required real XIM server must support preedit callbacks"
        );
        let inject = |keys: &str| {
            std::process::Command::new("python3").arg("-c").arg("import sys,time;sys.path.insert(0,sys.argv[1]);from native_gui_fixture import click;\nfor key in sys.argv[3].split(','):parts=key.split('+');click(sys.argv[2],_key=parts[-1],_modifiers=tuple(parts[:-1]),_focus=True,_physical=True);time.sleep(.1)")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"),"/scripts")).arg(&title).arg(keys).spawn().unwrap()
        };
        let mut child = inject("k,a");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(event) = surface.poll().unwrap() {
                assert_ne!(
                    event.kind, "text",
                    "preedit must not escape as committed VM text: {}",
                    event.key
                );
            }
            if unsafe { (&*surface.composition.get()).text == "か" } {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "real Anthy preedit was not delivered; state={}",
                unsafe { (&*surface.composition.get()).text.clone() }
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(child.wait().unwrap().success());
        frame.items[0].x = 20;
        frame.items[0].foreground = 0x123456;
        surface.present(&frame).unwrap();
        assert_eq!(unsafe { (&*surface.composition.get()).text.clone() }, "か");
        assert_eq!(surface.frame.as_ref().unwrap().items[0].text, "");
        let mut child = inject("Enter");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(event) = surface.poll().unwrap() {
                if event.kind == "text" {
                    assert_eq!(event.key, "か");
                    break;
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "committed text was not delivered"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(child.wait().unwrap().success());
        let mut child = inject("k,i");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while unsafe { (&*surface.composition.get()).text != "き" } {
            if let Some(event) = surface.poll().unwrap() {
                assert_ne!(event.kind, "text");
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(child.wait().unwrap().success());
        frame.items[0].focused = false;
        surface.present(&frame).unwrap();
        assert!(!unsafe { (&*surface.composition.get()).active });
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(200);
        while std::time::Instant::now() < deadline {
            if let Some(event) = surface.poll().unwrap() {
                assert_ne!(
                    event.kind, "text",
                    "focus cancellation must not commit preedit"
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        frame.items[0].focused = true;
        surface.present(&frame).unwrap();
        let mut child = inject("k,a,n,a,Space");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(event) = surface.poll().unwrap() {
                assert_ne!(event.kind, "text", "conversion must remain preedit");
            }
            if child.try_wait().unwrap().is_some()
                && unsafe { !(&*surface.composition.get()).text.is_empty() }
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(child.wait().unwrap().success());
        // Pump the service's conversion response after the final injected key.
        for _ in 0..100 {
            if let Some(event) = surface.poll().unwrap() {
                assert_ne!(event.kind, "text");
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let converted = unsafe { (&*surface.composition.get()).text.clone() };
        assert!(!converted.is_empty());
        let mut child = inject("Enter");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(event) = surface.poll().unwrap() {
                if event.kind == "text" {
                    assert_eq!(event.key, converted);
                    break;
                }
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(child.wait().unwrap().success());
        surface.close();
    }
    #[test]
    fn xim_feedback_only_updates_preserve_native_text_and_reject_invalid_offsets() {
        let mut state = UnsafeCell::new(composition::Composition::default());
        let state_ref = state.get_mut();
        state_ref.accepting = true;
        state_ref.start();
        assert!(state_ref.replace(0, 0, "かな", 2));
        let feedback = [2, 2];
        let text = ImeText {
            length: 2,
            feedback: feedback.as_ptr(),
            wide: 0,
            string: ImeString {
                bytes: std::ptr::null(),
            },
        };
        let mut draw = ImeDraw {
            caret: 1,
            first: 0,
            count: 2,
            text: &text,
        };
        unsafe {
            preedit_draw(
                std::ptr::null_mut(),
                (&mut state as *mut UnsafeCell<composition::Composition>).cast(),
                (&mut draw as *mut ImeDraw).cast(),
            );
        }
        assert_eq!(state.get_mut().text, "かな");
        assert_eq!(state.get_mut().cursor, 1);
        assert!(!state.get_mut().invalid);
        draw.count = 3;
        unsafe {
            preedit_draw(
                std::ptr::null_mut(),
                (&mut state as *mut UnsafeCell<composition::Composition>).cast(),
                (&mut draw as *mut ImeDraw).cast(),
            );
        }
        assert!(state.get_mut().invalid);
        assert_eq!(state.get_mut().text, "かな");
    }
    #[test]
    fn xim_unicode_decoding_does_not_require_an_extra_terminator() {
        let bytes = "界🙂".as_bytes();
        let text = ImeText {
            length: 2,
            feedback: std::ptr::null(),
            wide: 0,
            string: ImeString {
                bytes: bytes.as_ptr(),
            },
        };
        assert_eq!(unsafe { preedit_text(&text) }, Some("界🙂".into()));
        let wide = [0x754c, 0x1f642];
        let text = ImeText {
            length: 2,
            feedback: std::ptr::null(),
            wide: 1,
            string: ImeString {
                wide: wide.as_ptr(),
            },
        };
        assert_eq!(unsafe { preedit_text(&text) }, Some("界🙂".into()));
        let invalid = [0xc0];
        let text = ImeText {
            length: 1,
            feedback: std::ptr::null(),
            wide: 0,
            string: ImeString {
                bytes: invalid.as_ptr(),
            },
        };
        assert_eq!(unsafe { preedit_text(&text) }, None);
    }
}

#[cfg(test)]
mod accessibility_tests {
    use super::*;
    // Both tests share a real desktop registry. Keep application churn out of
    // the independent client's discovery/cache initialization phase.
    static DESKTOP_REGISTRY: std::sync::Mutex<()> = std::sync::Mutex::new(());
    #[test]
    fn real_atspi_multiple_windows_close_with_queued_registry_properties_and_rebind_bounded_paths()
    {
        if std::env::var("REWIND_TEST_A11Y_SERVICE").as_deref() != Ok("1") {
            return;
        }
        let _desktop = DESKTOP_REGISTRY.lock().unwrap();
        let mut frame = Frame {
            title: "REWIND accessibility lifecycle".into(),
            width: 320,
            height: 160,
            background: 0xffffff,
            items: vec![],
        };
        for _ in 0..16 {
            let mut first = Surface::new().unwrap();
            first.configure_accessibility(true);
            let mut second = Surface::new().unwrap();
            second.configure_accessibility(true);
            first.present(&frame).unwrap();
            second.present(&frame).unwrap();
            assert!(first.accessibility_bridge.is_some());
            assert!(second.accessibility_bridge.is_some());
            first.close();
            second.close();
        }
        let mut surface = Surface::new().unwrap();
        surface.configure_accessibility(true);
        for epoch in 0..3 {
            frame.items = (0..2048)
                .map(|i| Item {
                    id: format!("{epoch}-{i}"),
                    kind: "button".into(),
                    x: i % 100,
                    y: i / 100,
                    width: 1,
                    height: 1,
                    text: "Activate".into(),
                    foreground: 0,
                    background: 0xffffff,
                    enabled: true,
                    checked: false,
                    focused: false,
                    cursor: 0,
                    anchor: 0,
                    scroll: 0,
                })
                .collect();
            surface.present(&frame).unwrap();
            assert!(surface.accessibility_bridge.as_ref().unwrap().path_count() <= 4099);
        }
        surface.close();
        assert!(surface.accessibility_tree.node(None).is_none());
    }
    #[test]
    fn real_atspi_registry_client_reads_published_controls_and_dispatches_action() {
        if std::env::var("REWIND_TEST_A11Y_SERVICE").as_deref() != Ok("1") {
            return;
        }
        let _desktop = DESKTOP_REGISTRY.lock().unwrap();
        let mut surface = Surface::new().unwrap();
        surface.configure_accessibility(true);
        let mut frame = Frame {
            title: "REWIND accessibility acceptance".into(),
            width: 320,
            height: 160,
            background: 0xffffff,
            items: vec![Item {
                id: "save".into(),
                kind: "button".into(),
                x: 10,
                y: 10,
                width: 100,
                height: 30,
                text: "保存".into(),
                foreground: 0,
                background: 0xffffff,
                enabled: true,
                checked: false,
                focused: false,
                cursor: 0,
                anchor: 0,
                scroll: 0,
            }],
        };
        frame.items.push(Item {
            id: "Email address".into(),
            kind: "textbox".into(),
            x: 10,
            y: 60,
            width: 180,
            height: 24,
            text: "界🙂abc".into(),
            foreground: 0,
            background: 0xffffff,
            enabled: true,
            checked: false,
            focused: false,
            cursor: 2,
            anchor: 1,
            scroll: 0,
        });
        surface.present(&frame).unwrap();
        let bridge = surface
            .accessibility_bridge
            .as_ref()
            .expect("actual AT-SPI session required");
        let mut client = std::process::Command::new("/usr/bin/python3")
            .arg("scripts/native-accessibility-client.py")
            .arg(bridge.bus_name())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut actions = 0;
        loop {
            if let Some(status) = client.try_wait().unwrap() {
                assert!(status.success(), "actual accessibility client failed");
                break;
            }
            if std::time::Instant::now() > deadline {
                client.kill().unwrap();
                client.wait().unwrap();
                panic!("actual accessibility client timed out");
            }
            if let Some(event) = surface.poll().unwrap() {
                if event.kind == "pointer" {
                    actions += 1;
                    assert_eq!((event.x, event.y), (60, 25));
                    frame.items[0].enabled = false;
                    frame.items[0].text = "Disabled".into();
                    surface.present(&frame).unwrap();
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(actions, 1);
        surface.close();
        assert!(surface.accessibility_bridge.is_none());
        assert!(surface.accessibility_tree.node(None).is_none());
    }
}

#[cfg(test)]
mod font_tests {
    use super::*;
    #[test]
    fn native_font_service_draws_distinct_latin_and_japanese_glyphs() {
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        let mut surface = Surface::new().unwrap();
        let mut frame = Frame {
            title: "REWIND native font acceptance".into(),
            width: 240,
            height: 160,
            background: 0xffffff,
            items: vec![Item {
                id: "glyph".into(),
                kind: "label".into(),
                x: 10,
                y: 10,
                width: 100,
                height: 30,
                text: "I".into(),
                foreground: 0,
                background: 0xffffff,
                enabled: true,
                checked: false,
                focused: false,
                cursor: 0,
                anchor: 0,
                scroll: 0,
            }],
        };
        let mut capture = |text: &str| {
            frame.items[0].text = text.into();
            surface.present(&frame).unwrap();
            surface.paint();
            unsafe {
                (surface.api.XSync)(surface.display, 0);
                let image =
                    (surface.api.XGetImage)(surface.display, surface.window, 10, 10, 64, 30, !0, 2);
                assert!(!image.is_null());
                let mut pixels = Vec::new();
                for y in 0..30 {
                    for x in 0..64 {
                        pixels.push((surface.api.XGetPixel)(image, x, y));
                    }
                }
                (surface.api.XDestroyImage)(image);
                pixels
            }
        };
        assert_ne!(
            capture("I"),
            capture("W"),
            "native capture must distinguish glyphs"
        );
        assert_ne!(
            capture("界"),
            capture("語"),
            "Japanese text must not render as identical missing-glyph placeholders"
        );
    }
}
