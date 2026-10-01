use super::*;
use std::{cell::UnsafeCell, collections::VecDeque, ffi::c_void, sync::Arc};
type Handle = isize;
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Message {
    hwnd: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}
#[repr(C)]
struct Paint {
    dc: Handle,
    erase: i32,
    rect: Rect,
    restore: i32,
    inc: i32,
    reserved: [u8; 32],
}
#[repr(C)]
struct Class {
    style: u32,
    proc: Option<unsafe extern "system" fn(Handle, u32, usize, isize) -> isize>,
    class_extra: i32,
    window_extra: i32,
    instance: Handle,
    icon: Handle,
    cursor: Handle,
    background: Handle,
    menu: *const u16,
    name: *const u16,
}
#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(class: *const Class) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Handle,
        menu: Handle,
        instance: Handle,
        param: *mut c_void,
    ) -> Handle;
    fn DefWindowProcW(hwnd: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
    fn DestroyWindow(hwnd: Handle) -> i32;
    fn ShowWindow(hwnd: Handle, command: i32) -> i32;
    fn SetWindowTextW(hwnd: Handle, text: *const u16) -> i32;
    fn SetWindowLongPtrW(hwnd: Handle, index: i32, value: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: Handle, index: i32) -> isize;
    fn AdjustWindowRect(rect: *mut Rect, style: u32, menu: i32) -> i32;
    fn SetWindowPos(
        hwnd: Handle,
        after: Handle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn InvalidateRect(hwnd: Handle, rect: *const Rect, erase: i32) -> i32;
    fn UpdateWindow(hwnd: Handle) -> i32;
    fn PeekMessageW(message: *mut Message, hwnd: Handle, min: u32, max: u32, remove: u32) -> i32;
    fn GetKeyState(key: i32) -> i16;
    fn GetMessageW(message: *mut Message, hwnd: Handle, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn BeginPaint(hwnd: Handle, paint: *mut Paint) -> Handle;
    fn EndPaint(hwnd: Handle, paint: *const Paint) -> i32;
    fn FillRect(dc: Handle, rect: *const Rect, brush: Handle) -> i32;
    fn FrameRect(dc: Handle, rect: *const Rect, brush: Handle) -> i32;
    fn LoadCursorW(instance: Handle, name: *const u16) -> Handle;
    #[cfg(test)]
    fn PostMessageW(hwnd: Handle, message: u32, wparam: usize, lparam: isize) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn SaveDC(dc: Handle) -> i32;
    fn RestoreDC(dc: Handle, saved: i32) -> i32;
    fn IntersectClipRect(dc: Handle, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn GetTextExtentPoint32W(dc: Handle, text: *const u16, len: i32, size: *mut Point) -> i32;
    fn MoveToEx(dc: Handle, x: i32, y: i32, old: *mut Point) -> i32;
    fn LineTo(dc: Handle, x: i32, y: i32) -> i32;
    fn CreateSolidBrush(color: u32) -> Handle;
    fn DeleteObject(object: Handle) -> i32;
    fn SetTextColor(dc: Handle, color: u32) -> u32;
    fn SetBkMode(dc: Handle, mode: i32) -> i32;
    fn TextOutW(dc: Handle, x: i32, y: i32, text: *const u16, len: i32) -> i32;
    fn SelectObject(dc: Handle, object: Handle) -> Handle;
    fn GetStockObject(id: i32) -> Handle;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn color(rgb: u32) -> u32 {
    ((rgb & 0xff) << 16) | (rgb & 0xff00) | (rgb >> 16)
}
struct State {
    frame: Option<Arc<Frame>>,
    events: VecDeque<Event>,
    closed: bool,
    overflow: bool,
    surrogate: Option<u16>,
}
impl State {
    fn push(&mut self, event: Event) {
        if self.events.len() >= 4096 {
            self.overflow = true;
        } else {
            self.events.push_back(event);
        }
    }
}
unsafe extern "system" fn procedure(
    hwnd: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    // The pointer is owned by Surface and remains valid until DestroyWindow returns.
    let ptr = unsafe { GetWindowLongPtrW(hwnd, -21) } as *mut State;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    match message {
        0x0f => {
            let frame = unsafe { (&*ptr).frame.clone() };
            unsafe {
                draw(hwnd, frame.as_deref());
            }
            0
        }
        0x14 => 1,
        0x201 => {
            let state = unsafe { &mut *ptr };
            let mut e = Event::simple("pointer");
            e.x = (lparam as u16 as i16) as i32;
            e.y = ((lparam as usize >> 16) as u16 as i16) as i32;
            state.push(e);
            0
        }
        0x05 => {
            let state = unsafe { &mut *ptr };
            let mut e = Event::simple("resize");
            e.width = (lparam as u16) as i32;
            e.height = ((lparam as usize >> 16) as u16) as i32;
            state.push(e);
            0
        }
        0x10 => {
            let state = unsafe { &mut *ptr };
            state.closed = true;
            state.push(Event::simple("close"));
            unsafe {
                ShowWindow(hwnd, 0);
            }
            0
        }
        0x100 => {
            let key = match wparam {
                0x0d => "Enter",
                0x09 => "Tab",
                0x1b => "Escape",
                0x08 => "Backspace",
                0x2e => "Delete",
                0x24 => "Home",
                0x23 => "End",
                0x41 if unsafe { GetKeyState(0x11) } < 0 => "Ctrl+A",
                0x20 => "Space",
                0x25 => "Left",
                0x26 => "Up",
                0x27 => "Right",
                0x28 => "Down",
                _ => return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
            };
            let state = unsafe { &mut *ptr };
            let mut e = Event::simple("key");
            e.key = if unsafe { GetKeyState(0x10) } < 0
                && matches!(key, "Left" | "Right" | "Home" | "End" | "Up" | "Down")
            {
                format!("Shift+{key}")
            } else {
                key.into()
            };
            state.push(e);
            0
        }
        0x20a => {
            let state = unsafe { &mut *ptr };
            let mut e = Event::simple("wheel");
            e.y = if ((wparam >> 16) as u16 as i16) > 0 {
                -1
            } else {
                1
            };
            state.push(e);
            0
        }
        0x102 => {
            let state = unsafe { &mut *ptr };
            let unit = wparam as u16;
            if (0xd800..=0xdbff).contains(&unit) {
                state.surrogate = Some(unit);
                return 0;
            }
            let c = if (0xdc00..=0xdfff).contains(&unit) {
                state.surrogate.take().and_then(|high| {
                    char::from_u32(
                        0x10000 + (((high as u32 - 0xd800) << 10) | (unit as u32 - 0xdc00)),
                    )
                })
            } else {
                state.surrogate = None;
                char::from_u32(unit as u32)
            };
            if let Some(c) = c.filter(|c| !c.is_control() && *c != ' ') {
                let mut e = Event::simple("text");
                e.key = c.to_string();
                state.push(e);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}
unsafe fn fill(dc: Handle, r: &Rect, rgb: u32, border: bool) {
    unsafe {
        let brush = CreateSolidBrush(color(rgb));
        if border {
            FrameRect(dc, r, brush);
        } else {
            FillRect(dc, r, brush);
        }
        DeleteObject(brush);
    }
}
unsafe fn draw_edit(dc: Handle, item: &Item) {
    unsafe {
        let mut offset = 0usize;
        let start = item.cursor.min(item.anchor);
        let end = item.cursor.max(item.anchor);
        for (row, line) in item.text.split('\n').enumerate() {
            let count = line.chars().count();
            if row >= item.scroll && row < item.scroll + (item.height as usize / 18).max(1) {
                let mut x = item.x + 6;
                let y = item.y + 6 + ((row - item.scroll) * 18) as i32;
                let width = |n: usize| {
                    let text = wide(&line.chars().take(n).collect::<String>());
                    let mut size = Point { x: 0, y: 0 };
                    GetTextExtentPoint32W(dc, text.as_ptr(), (text.len() - 1) as i32, &mut size);
                    size.x
                };
                if item.cursor >= offset && item.cursor <= offset + count {
                    let caret = width(item.cursor - offset);
                    x -= (caret - (item.width - 14)).max(0);
                }
                if start < end && end > offset && start < offset + count {
                    let left = width(start.saturating_sub(offset).min(count));
                    let right = width(end.saturating_sub(offset).min(count));
                    fill(
                        dc,
                        &Rect {
                            left: x + left,
                            top: y,
                            right: x + right,
                            bottom: y + 18,
                        },
                        0xbfdbfe,
                        false,
                    );
                }
                let text = wide(line);
                SetTextColor(dc, color(item.foreground));
                TextOutW(dc, x, y, text.as_ptr(), (text.len() - 1) as i32);
                if item.focused && item.cursor >= offset && item.cursor <= offset + count {
                    let caret = x + width(item.cursor - offset);
                    MoveToEx(dc, caret, y, std::ptr::null_mut());
                    LineTo(dc, caret, y + 16);
                }
            }
            offset += count + 1;
        }
    }
}
unsafe fn draw(hwnd: Handle, frame: Option<&Frame>) {
    unsafe {
        let mut paint: Paint = std::mem::zeroed();
        let dc = BeginPaint(hwnd, &mut paint);
        if let Some(frame) = frame {
            fill(
                dc,
                &Rect {
                    left: 0,
                    top: 0,
                    right: frame.width,
                    bottom: frame.height,
                },
                frame.background,
                false,
            );
            SetBkMode(dc, 2);
            let old = SelectObject(dc, GetStockObject(17));
            for item in &frame.items {
                let rect = Rect {
                    left: item.x,
                    top: item.y,
                    right: item.x + item.width,
                    bottom: item.y + item.height,
                };
                if item.kind != "label" {
                    fill(dc, &rect, item.background, false);
                    fill(
                        dc,
                        &rect,
                        if item.focused { 0x2563eb } else { 0x888888 },
                        true,
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
                let text = wide(&text);
                let saved = SaveDC(dc);
                IntersectClipRect(dc, rect.left, rect.top, rect.right, rect.bottom);
                if matches!(item.kind.as_str(), "textbox" | "textarea") {
                    draw_edit(dc, item);
                    RestoreDC(dc, saved);
                    continue;
                }
                SetTextColor(
                    dc,
                    color(if item.enabled {
                        item.foreground
                    } else {
                        0x888888
                    }),
                );
                TextOutW(
                    dc,
                    item.x + 6,
                    item.y + (item.height / 2) - 8,
                    text.as_ptr(),
                    (text.len() - 1) as i32,
                );
                RestoreDC(dc, saved);
            }
            SelectObject(dc, old);
        }
        EndPaint(hwnd, &paint);
    }
}
pub(super) struct Surface {
    window: Handle,
    state: Box<UnsafeCell<State>>,
    instance: Handle,
}
impl Surface {
    fn state(&self) -> &State {
        unsafe { &*self.state.get() }
    }
    fn state_mut(&mut self) -> &mut State {
        unsafe { &mut *self.state.get() }
    }
    pub fn new() -> io::Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let name = wide("REWIND.Native.GUI.1");
            let class = Class {
                style: 3,
                proc: Some(procedure),
                class_extra: 0,
                window_extra: 0,
                instance,
                icon: 0,
                cursor: LoadCursorW(0, 32512usize as *const u16),
                background: 0,
                menu: std::ptr::null(),
                name: name.as_ptr(),
            };
            if RegisterClassW(&class) == 0
                && io::Error::last_os_error().raw_os_error() != Some(1410)
            {
                return Err(io::Error::last_os_error());
            }
            Ok(Self {
                window: 0,
                state: Box::new(UnsafeCell::new(State {
                    frame: None,
                    events: VecDeque::new(),
                    closed: false,
                    overflow: false,
                    surrogate: None,
                })),
                instance,
            })
        }
    }
    pub fn present(&mut self, frame: &Frame) -> io::Result<()> {
        unsafe {
            let style = 0x00cf0000;
            let mut rect = Rect {
                left: 0,
                top: 0,
                right: frame.width,
                bottom: frame.height,
            };
            if AdjustWindowRect(&mut rect, style, 0) == 0 {
                return Err(io::Error::last_os_error());
            }
            if self.window == 0 {
                let class = wide("REWIND.Native.GUI.1");
                let title = wide(&frame.title);
                self.window = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    title.as_ptr(),
                    style,
                    0x80000000u32 as i32,
                    0x80000000u32 as i32,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    0,
                    0,
                    self.instance,
                    std::ptr::null_mut(),
                );
                if self.window == 0 {
                    return Err(io::Error::last_os_error());
                }
                SetWindowLongPtrW(self.window, -21, self.state.get() as isize);
            }
            self.state_mut().frame = Some(Arc::new(frame.clone()));
            self.state_mut().closed = false;
            let title = wide(&frame.title);
            if SetWindowTextW(self.window, title.as_ptr()) == 0 {
                return Err(io::Error::last_os_error());
            }
            if SetWindowPos(
                self.window,
                0,
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                0x16,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            ShowWindow(self.window, 5);
            InvalidateRect(self.window, std::ptr::null(), 0);
            UpdateWindow(self.window);
            Ok(())
        }
    }
    pub fn event(&mut self) -> io::Result<Event> {
        loop {
            if self.state().overflow {
                return Err(invalid("GuiEventQueueLimit"));
            }
            if let Some(event) = self.state_mut().events.pop_front() {
                return Ok(event);
            }
            if self.window == 0 || self.state().closed {
                return Err(invalid("GuiClosed"));
            }
            unsafe {
                let mut message: Message = std::mem::zeroed();
                let result = GetMessageW(&mut message, 0, 0, 0);
                if result < 0 {
                    return Err(io::Error::last_os_error());
                }
                if result == 0 {
                    return Ok(Event::simple("close"));
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    pub fn poll(&mut self) -> io::Result<Option<Event>> {
        if self.state().overflow {
            return Err(invalid("GuiEventQueueLimit"));
        }
        if let Some(event) = self.state_mut().events.pop_front() {
            return Ok(Some(event));
        }
        if self.window == 0 || self.state().closed {
            return Err(invalid("GuiClosed"));
        }
        unsafe {
            let mut message: Message = std::mem::zeroed();
            for _ in 0..4096 {
                if PeekMessageW(&mut message, 0, 0, 0, 1) == 0 {
                    return Ok(None);
                }
                if message.message == 0x12 {
                    return Ok(Some(Event::simple("close")));
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
                if let Some(event) = self.state_mut().events.pop_front() {
                    return Ok(Some(event));
                }
            }
        }
        Ok(None)
    }
    pub fn close(&mut self) {
        if self.window != 0 {
            unsafe {
                DestroyWindow(self.window);
            }
            self.window = 0;
        }
        self.state_mut().frame = None;
        self.state_mut().events.clear();
    }
    #[cfg(test)]
    pub fn inject_key_and_close(&mut self) {
        unsafe {
            PostMessageW(self.window, 0x100, 0x0d, 0);
            PostMessageW(self.window, 0x10, 0, 0);
        }
    }
    #[cfg(test)]
    pub fn inject_pointer(&mut self, x: i32, y: i32) {
        unsafe {
            PostMessageW(
                self.window,
                0x201,
                1,
                (((y as u16 as usize) << 16) | (x as u16 as usize)) as isize,
            );
        }
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        self.close();
    }
}
