use super::*;
use libc::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::ffi::CString;
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
    pad: [c_long; 24],
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
        #[allow(non_snake_case,dead_code)] struct Api {_library:Library,$($name:unsafe extern "C" fn($($arg),*)->$ret,)*}
        impl Api {fn load()->io::Result<Self> {unsafe {
            let lib=Library(libc::dlopen(c"libX11.so.6".as_ptr(),libc::RTLD_NOW|libc::RTLD_LOCAL));
            if lib.0.is_null(){return Err(invalid("GuiUnavailable: libX11.so.6 is required for native GUI"));}
            Ok(Self {$($name:{let symbol=libc::dlsym(lib.0,concat!(stringify!($name),"\0").as_ptr().cast());if symbol.is_null(){return Err(invalid("GuiUnavailable: X11 symbol missing"));}std::mem::transmute::<*mut c_void,unsafe extern "C" fn($($arg),*)->$ret>(symbol)},)*_library:lib})
        }}}
    }
}
xapi! {
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
}
impl Surface {
    pub fn new() -> io::Result<Self> {
        unsafe {
            let api = Api::load()?;
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
            Ok(Self {
                api,
                display,
                window: 0,
                gc: std::ptr::null_mut(),
                font,
                delete,
                frame: None,
            })
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
        if self.window == 0 {
            return Err(invalid("GuiNotPublished"));
        }
        loop {
            unsafe {
                let mut e = XEvent { pad: [0; 24] };
                (self.api.XNextEvent)(self.display, &mut e);
                match e.kind {
                    12 => {
                        self.paint();
                        (self.api.XFlush)(self.display);
                    }
                    4 => {
                        let b = e.button;
                        if b.button == 1 {
                            let mut out = Event::simple("pointer");
                            out.x = b.x;
                            out.y = b.y;
                            return Ok(out);
                        }
                    }
                    2 => {
                        let mut b = e.button;
                        let key =
                            (self.api.XLookupKeysym)(&mut b, if b.state & 1 != 0 { 1 } else { 0 });
                        let name = match key {
                            0xff0d => "Enter".into(),
                            0xff09 => "Tab".into(),
                            0xff1b => "Escape".into(),
                            0xff08 => "Backspace".into(),
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
                        out.key = name;
                        return Ok(out);
                    }
                    22 => {
                        let c = e.configure;
                        let mut out = Event::simple("resize");
                        out.width = c.width;
                        out.height = c.height;
                        return Ok(out);
                    }
                    33 if e.client.data[0] as c_ulong == self.delete => {
                        self.close();
                        return Ok(Event::simple("close"));
                    }
                    _ => {}
                }
            }
        }
    }
    pub fn close(&mut self) {
        unsafe {
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
        self.close();
        unsafe {
            (self.api.XFreeFontSet)(self.display, self.font);
            (self.api.XCloseDisplay)(self.display);
        }
    }
}
