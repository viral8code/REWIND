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
    fn GetDC(window: Handle) -> Handle;
    fn IsWindowVisible(window: Handle) -> i32;
    fn ReleaseDC(window: Handle, dc: Handle) -> i32;
    #[cfg(test)]
    fn GetKeyboardState(state: *mut u8) -> i32;
    #[cfg(test)]
    fn SetKeyboardState(state: *const u8) -> i32;
    #[cfg(test)]
    fn SendMessageW(window: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
    fn OpenClipboard(window: Handle) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, data: Handle) -> Handle;
    fn GetClipboardData(format: u32) -> Handle;
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
    #[cfg(test)]
    fn GetPixel(dc: Handle, x: i32, y: i32) -> u32;
    fn SelectObject(dc: Handle, object: Handle) -> Handle;
    fn GetStockObject(id: i32) -> Handle;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalFree(memory: Handle) -> Handle;
    fn GlobalLock(memory: Handle) -> *mut c_void;
    fn GlobalUnlock(memory: Handle) -> i32;
    fn GlobalSize(memory: Handle) -> usize;
}
#[repr(C)]
struct CompositionForm {
    style: u32,
    position: Point,
    area: Rect,
}
#[repr(C)]
struct CandidateForm {
    index: u32,
    style: u32,
    position: Point,
    area: Rect,
}
#[link(name = "imm32")]
unsafe extern "system" {
    fn ImmGetContext(window: Handle) -> Handle;
    fn ImmCreateContext() -> Handle;
    fn ImmDestroyContext(context: Handle) -> i32;
    fn ImmAssociateContext(window: Handle, context: Handle) -> Handle;
    fn ImmReleaseContext(window: Handle, context: Handle) -> i32;
    fn ImmGetCompositionStringW(context: Handle, index: u32, buffer: *mut c_void, len: u32) -> i32;
    fn ImmSetCompositionWindow(context: Handle, form: *const CompositionForm) -> i32;
    fn ImmSetCandidateWindow(context: Handle, form: *const CandidateForm) -> i32;
    fn ImmNotifyIME(context: Handle, action: u32, index: u32, value: u32) -> i32;
}
struct ImeContext {
    window: Handle,
    context: Handle,
}
impl ImeContext {
    fn get(window: Handle) -> Option<Self> {
        let context = unsafe { ImmGetContext(window) };
        (context != 0).then_some(Self { window, context })
    }
    fn text(&self, index: u32) -> Option<Vec<u16>> {
        let bytes =
            unsafe { ImmGetCompositionStringW(self.context, index, std::ptr::null_mut(), 0) };
        if bytes < 0 || bytes as usize > composition::MAX_BYTES * 2 || bytes % 2 != 0 {
            return None;
        }
        let mut units = vec![0u16; bytes as usize / 2];
        let received = unsafe {
            ImmGetCompositionStringW(self.context, index, units.as_mut_ptr().cast(), bytes as u32)
        };
        (received == bytes).then_some(units)
    }
}
impl Drop for ImeContext {
    fn drop(&mut self) {
        unsafe {
            ImmReleaseContext(self.window, self.context);
        }
    }
}
fn cancel_composition(window: Handle) {
    if let Some(context) = ImeContext::get(window) {
        unsafe {
            ImmNotifyIME(context.context, 0x15, 4, 0);
        }
    }
}
fn position_composition(window: Handle, frame: &Frame) {
    let Some(item) = clipboard::focused_input(frame) else {
        return;
    };
    let Some(context) = ImeContext::get(window) else {
        return;
    };
    unsafe {
        let dc = GetDC(window);
        if dc == 0 {
            return;
        }
        let old = SelectObject(dc, GetStockObject(17));
        let (x, y) = composition::anchor(item, |text| {
            let text = wide(text);
            let mut size = Point { x: 0, y: 0 };
            GetTextExtentPoint32W(dc, text.as_ptr(), (text.len() - 1) as i32, &mut size);
            size.x
        });
        SelectObject(dc, old);
        ReleaseDC(window, dc);
        let form = CompositionForm {
            style: 2,
            position: Point { x, y },
            area: Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        };
        ImmSetCompositionWindow(context.context, &form);
        let form = CandidateForm {
            index: 0,
            style: 0x40,
            position: Point { x, y: y + 18 },
            area: Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        };
        ImmSetCandidateWindow(context.context, &form);
    }
}
struct ClipboardGuard;
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}
fn copy_text(window: Handle, text: &str) -> bool {
    if !clipboard::valid_text(text) {
        return false;
    }
    unsafe {
        if OpenClipboard(window) == 0 {
            return false;
        }
        let _guard = ClipboardGuard;
        let data = wide(text);
        let memory = GlobalAlloc(2, data.len() * 2);
        if memory == 0 {
            return false;
        }
        let pointer = GlobalLock(memory).cast::<u16>();
        if pointer.is_null() {
            GlobalFree(memory);
            return false;
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), pointer, data.len());
        GlobalUnlock(memory);
        if EmptyClipboard() == 0 || SetClipboardData(13, memory) == 0 {
            GlobalFree(memory);
            return false;
        }
        true
    }
}
fn paste_text(window: Handle) -> Option<String> {
    unsafe {
        if OpenClipboard(window) == 0 {
            return None;
        }
        let _guard = ClipboardGuard;
        let memory = GetClipboardData(13);
        if memory == 0 {
            return None;
        }
        let bytes = GlobalSize(memory);
        // Never scan an unbounded OS allocation, even if its terminator is missing.
        if bytes < 2 || bytes > (clipboard::MAX_TEXT_BYTES + 1) * 2 || bytes % 2 != 0 {
            return None;
        }
        let pointer = GlobalLock(memory).cast::<u16>();
        if pointer.is_null() {
            return None;
        }
        let units = std::slice::from_raw_parts(pointer, bytes / 2);
        let text = units
            .iter()
            .position(|unit| *unit == 0)
            .and_then(|end| String::from_utf16(&units[..end]).ok());
        GlobalUnlock(memory);
        text.filter(|text| clipboard::valid_text(text))
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn color(rgb: u32) -> u32 {
    ((rgb & 0xff) << 16) | (rgb & 0xff00) | (rgb >> 16)
}
struct State {
    accessibility_enabled: bool,
    accessibility: super::accessibility::Tree,
    ime_enabled: bool,
    composition: composition::Composition,
    clipboard_enabled: bool,
    command_keys_enabled: bool,
    suppress_syschar: Option<u16>,
    frame: Option<Arc<Frame>>,
    events: VecDeque<Event>,
    queued_bytes: usize,
    closed: bool,
    overflow: bool,
    surrogate: Option<u16>,
}
impl State {
    fn pop(&mut self) -> Option<Event> {
        let event = self.events.pop_front()?;
        self.queued_bytes = self
            .queued_bytes
            .saturating_sub(std::mem::size_of::<Event>() + event.key.len() + event.kind.len());
        Some(event)
    }

    fn push(&mut self, event: Event) {
        let bytes = std::mem::size_of::<Event>() + event.key.len() + event.kind.len();
        if self.events.len() >= 4096
            || ((self.ime_enabled || self.accessibility_enabled)
                && self.queued_bytes.saturating_add(bytes) > 512 * 1024)
        {
            self.overflow = true;
        } else {
            self.queued_bytes += bytes;
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
    if message == 0x07 || message == 0x08 {
        unsafe {
            (*ptr).accessibility.focus(message == 0x07);
            if message == 0x07 && (*ptr).accessibility_enabled {
                super::windows_accessibility::announce_focus(hwnd, &(*ptr).accessibility);
            }
        }
    }
    if unsafe { (&*ptr).ime_enabled } {
        match message {
            0x07 => unsafe {
                (&mut *ptr).composition.accepting = (&*ptr)
                    .frame
                    .as_deref()
                    .and_then(clipboard::focused_input)
                    .is_some();
            },
            0x286 => return 0,

            0x281 => {
                return unsafe {
                    DefWindowProcW(hwnd, message, wparam, lparam & !(0x80000000u32 as isize))
                }
            }
            0x10d => {
                if !unsafe { (&*ptr).composition.accepting } {
                    return 0;
                }
                unsafe {
                    (&mut *ptr).composition.start();
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                return 0;
            }
            0x10e => {
                unsafe {
                    (&mut *ptr).composition.clear();
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                return 0;
            }
            0x10f => {
                if !unsafe { (&*ptr).composition.accepting } {
                    return 0;
                }
                let Some(context) = ImeContext::get(hwnd) else {
                    return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
                };
                if lparam & 0x800 != 0 {
                    let committed = context
                        .text(0x800)
                        .and_then(|units| String::from_utf16(&units).ok());
                    let state = unsafe { &mut *ptr };
                    state.composition.clear();
                    match committed {
                        Some(text) if clipboard::valid_text(&text) => {
                            if let Some(event) = clipboard::text_event(text) {
                                state.push(event);
                            }
                        }
                        _ => state.composition.invalid = true,
                    }
                }
                if lparam & 8 != 0 {
                    let text = context.text(8);
                    let cursor = unsafe {
                        ImmGetCompositionStringW(context.context, 0x80, std::ptr::null_mut(), 0)
                    };
                    let state = unsafe { &mut *ptr };
                    if let Some(text) = text.filter(|_| cursor >= 0) {
                        state.composition.set_utf16(&text, cursor as usize);
                    } else {
                        state.composition.invalid = true;
                    }
                }
                unsafe {
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                return 0;
            }
            0x08 => {
                unsafe {
                    (&mut *ptr).composition.accepting = false;
                    (&mut *ptr).composition.clear();
                }
                cancel_composition(hwnd);
            }
            _ => {}
        }
    }
    if message == 0x3d && unsafe { (*ptr).accessibility_enabled } {
        return super::windows_accessibility::get_object(hwnd, wparam, lparam, unsafe {
            (*ptr).accessibility.clone()
        });
    }
    match message {
        0x0f => {
            let frame = unsafe { (&*ptr).frame.clone() };
            let preedit = unsafe {
                (&*ptr)
                    .composition
                    .active
                    .then(|| ((&*ptr).composition.text.clone(), (&*ptr).composition.cursor))
            };
            unsafe {
                draw(hwnd, frame.as_deref(), preedit.as_ref());
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
            state.accessibility.close();
            state.push(Event::simple("close"));
            unsafe {
                ShowWindow(hwnd, 0);
            }
            0
        }
        0x100 | 0x104 => {
            let state = unsafe { &mut *ptr };
            state.suppress_syschar = None;
            let control = unsafe { GetKeyState(0x11) } < 0;
            let alt = unsafe { GetKeyState(0x12) } < 0;
            let shift = unsafe { GetKeyState(0x10) } < 0;
            if message == 0x104 && !state.command_keys_enabled {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            }
            if state.clipboard_enabled
                && (!state.command_keys_enabled || (!alt && !shift))
                && control
                && matches!(wparam, 0x43 | 0x56 | 0x58)
            {
                let focused = state
                    .frame
                    .as_deref()
                    .and_then(clipboard::focused_input)
                    .is_some();
                if focused {
                    if wparam == 0x56 {
                        if let Some(event) = paste_text(hwnd).and_then(clipboard::text_event) {
                            state.push(event);
                        }
                    } else if let Some(text) =
                        state.frame.as_deref().and_then(clipboard::selected_text)
                    {
                        if copy_text(hwnd, &text) && wparam == 0x58 {
                            let mut event = Event::simple("key");
                            event.key = "Delete".into();
                            state.push(event);
                        }
                    }
                }
                if focused || !state.command_keys_enabled {
                    return 0;
                }
            }
            if state.command_keys_enabled {
                let letter = (0x41..=0x5a)
                    .contains(&wparam)
                    .then(|| char::from_u32(wparam as u32))
                    .flatten();
                let function = (0x70..=0x87)
                    .contains(&wparam)
                    .then(|| (wparam - 0x70 + 1) as u8);
                if let Some(key) =
                    command_keys::command_key(letter, function, control, alt, shift, false)
                {
                    let mut event = Event::simple("key");
                    event.key = key;
                    if alt {
                        state.suppress_syschar = letter.map(|c| c.to_ascii_uppercase() as u16);
                    }
                    state.push(event);
                    return 0;
                }
            }
            if message == 0x104 {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            }
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
        0x106 => {
            let state = unsafe { &mut *ptr };
            if state.command_keys_enabled
                && state.suppress_syschar.take().is_some_and(|c| {
                    char::from_u32(wparam as u32)
                        .is_some_and(|key| key.to_ascii_uppercase() as u32 == c as u32)
                })
            {
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
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
unsafe fn draw(hwnd: Handle, frame: Option<&Frame>, preedit: Option<&(String, usize)>) {
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
        if let (Some(frame), Some((text, cursor))) = (frame, preedit) {
            if let Some(item) = clipboard::focused_input(frame) {
                let saved = SaveDC(dc);
                SelectObject(dc, GetStockObject(17));
                SetBkMode(dc, 1);
                IntersectClipRect(
                    dc,
                    item.x,
                    item.y,
                    item.x + item.width,
                    item.y + item.height,
                );
                let measure = |value: &str| {
                    let units = wide(value);
                    let mut size = Point { x: 0, y: 0 };
                    GetTextExtentPoint32W(dc, units.as_ptr(), (units.len() - 1) as i32, &mut size);
                    size.x
                };
                let (x, y) = composition::anchor(item, measure);
                let width = measure(text);
                fill(
                    dc,
                    &Rect {
                        left: x,
                        top: y,
                        right: x + width.max(1),
                        bottom: y + 18,
                    },
                    item.background,
                    false,
                );
                SetTextColor(dc, color(item.foreground));
                let units = wide(text);
                TextOutW(dc, x, y, units.as_ptr(), (units.len() - 1) as i32);
                MoveToEx(dc, x, y + 17, std::ptr::null_mut());
                LineTo(dc, x + width, y + 17);
                let prefix: String = text.chars().take(*cursor).collect();
                let caret = x + measure(&prefix);
                MoveToEx(dc, caret, y, std::ptr::null_mut());
                LineTo(dc, caret, y + 16);
                RestoreDC(dc, saved);
            }
        }
        EndPaint(hwnd, &paint);
    }
}
pub(super) struct Surface {
    ime_context: Handle,
    apartment: Option<super::windows_accessibility::Apartment>,
    window: Handle,
    state: Box<UnsafeCell<State>>,
    instance: Handle,
}
impl Surface {
    #[cfg(test)]
    pub fn inject_clipboard_key(&mut self, key: u8) {
        unsafe {
            let mut saved = [0u8; 256];
            assert_ne!(GetKeyboardState(saved.as_mut_ptr()), 0);
            let mut pressed = saved;
            pressed[0x11] = 0x80;
            assert_ne!(SetKeyboardState(pressed.as_ptr()), 0);
            SendMessageW(self.window, 0x100, key as usize, 0);
            assert_ne!(SetKeyboardState(saved.as_ptr()), 0);
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
            let mut saved = [0u8; 256];
            assert_ne!(GetKeyboardState(saved.as_mut_ptr()), 0);
            let mut pressed = saved;
            pressed[0x11] = if control { 0x80 } else { 0 };
            pressed[0x12] = if alt { 0x80 } else { 0 };
            pressed[0x10] = if shift { 0x80 } else { 0 };
            assert_ne!(SetKeyboardState(pressed.as_ptr()), 0);
            let key = function
                .map(|n| 0x70 + n as usize - 1)
                .unwrap_or_else(|| letter.unwrap().to_ascii_uppercase() as usize);
            SendMessageW(self.window, if alt { 0x104 } else { 0x100 }, key, 0);
            assert_ne!(SetKeyboardState(saved.as_ptr()), 0);
        }
    }
    pub fn configure_command_keys(&mut self, enabled: bool) {
        unsafe {
            (*self.state.get()).command_keys_enabled = enabled;
        }
    }
    pub fn configure_ime(&mut self, enabled: bool) {
        self.state_mut().ime_enabled = enabled;
    }
    pub fn configure_clipboard(&mut self, enabled: bool) {
        unsafe {
            (*self.state.get()).clipboard_enabled = enabled;
        }
    }
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
                ime_context: 0,
                apartment: None,
                state: Box::new(UnsafeCell::new(State {
                    accessibility_enabled: false,
                    accessibility: super::accessibility::Tree::default(),
                    ime_enabled: false,
                    composition: composition::Composition::default(),
                    clipboard_enabled: false,
                    command_keys_enabled: false,
                    suppress_syschar: None,
                    frame: None,
                    events: VecDeque::new(),
                    queued_bytes: 0,
                    closed: false,
                    overflow: false,
                    surrogate: None,
                })),
                instance,
            })
        }
    }
    pub fn configure_accessibility(&mut self, enabled: bool) {
        self.state_mut().accessibility_enabled = enabled;
    }
    pub fn present(&mut self, frame: &Frame) -> io::Result<()> {
        // Modern IMM-compatible input methods use the thread's COM/TSF apartment.
        // Keep it alive through window/context destruction, alongside accessibility.
        if (self.state().accessibility_enabled || self.state().ime_enabled)
            && self.apartment.is_none()
        {
            self.apartment = Some(super::windows_accessibility::Apartment::new().map_err(
                |error| {
                    if self.state().ime_enabled && !self.state().accessibility_enabled {
                        invalid("GuiCompositionUnavailable: COM apartment")
                    } else {
                        error
                    }
                },
            )?);
        }
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
                if self.state().ime_enabled {
                    self.ime_context = ImmCreateContext();
                    if self.ime_context == 0 {
                        return Err(invalid("GuiCompositionUnavailable"));
                    }
                    ImmAssociateContext(self.window, self.ime_context);
                }
            }
            if self.state().ime_enabled && self.state().composition.active {
                let previous = self
                    .state()
                    .frame
                    .as_deref()
                    .and_then(clipboard::focused_input);
                let next = clipboard::focused_input(frame);
                if !composition::same_input(previous, next) {
                    let fresh = ImmCreateContext();
                    if fresh == 0 {
                        return Err(invalid("GuiCompositionUnavailable"));
                    }
                    self.state_mut().composition.accepting = false;
                    self.state_mut().composition.clear();
                    cancel_composition(self.window);
                    ImmAssociateContext(self.window, fresh);
                    if self.ime_context != 0 {
                        ImmDestroyContext(self.ime_context);
                    }
                    self.ime_context = fresh;
                }
            }
            let previous = self.state().frame.clone();
            let published = Arc::new(frame.clone());
            if self.state().accessibility_enabled {
                self.state().accessibility.publish(published.clone());
            }
            self.state_mut().frame = Some(published);
            self.state_mut().composition.accepting = clipboard::focused_input(frame).is_some();
            if self.state().ime_enabled {
                position_composition(self.window, frame);
            }
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
            if IsWindowVisible(self.window) == 0 {
                ShowWindow(self.window, 5);
            }
            InvalidateRect(self.window, std::ptr::null(), 0);
            UpdateWindow(self.window);
            if self.state().accessibility_enabled {
                super::windows_accessibility::changed(self.window, previous.as_deref(), frame);
            }

            Ok(())
        }
    }
    pub fn event(&mut self) -> io::Result<Event> {
        loop {
            if self.state().composition.invalid {
                return Err(invalid("GuiCompositionInvalid"));
            }
            if self.state().overflow {
                return Err(invalid("GuiEventQueueLimit"));
            }
            if let Some(event) = self.state_mut().pop() {
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
        if self.state().composition.invalid {
            return Err(invalid("GuiCompositionInvalid"));
        }
        if self.state().overflow {
            return Err(invalid("GuiEventQueueLimit"));
        }
        if let Some(event) = self.state_mut().pop() {
            return Ok(Some(event));
        }
        if self.window == 0 || self.state().closed {
            return Err(invalid("GuiClosed"));
        }
        unsafe {
            let mut message: Message = std::mem::zeroed();
            let limit = if self.state().ime_enabled || self.state().accessibility_enabled {
                64
            } else {
                4096
            };
            for _ in 0..limit {
                if PeekMessageW(&mut message, 0, 0, 0, 1) == 0 {
                    return Ok(None);
                }
                if message.message == 0x12 {
                    return Ok(Some(Event::simple("close")));
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
                if let Some(event) = self.state_mut().pop() {
                    return Ok(Some(event));
                }
            }
        }
        Ok(None)
    }
    pub fn close(&mut self) {
        self.state().accessibility.close();
        if self.window != 0 {
            unsafe {
                DestroyWindow(self.window);
            }
            self.window = 0;
        }
        if self.ime_context != 0 {
            unsafe {
                ImmDestroyContext(self.ime_context);
            }
            self.ime_context = 0;
        }
        self.state_mut().composition.accepting = false;
        self.state_mut().composition.clear();
        self.state_mut().frame = None;
        self.state_mut().events.clear();
        self.state_mut().queued_bytes = 0;
    }
    #[cfg(test)]
    pub fn native_window_handle(&self) -> isize {
        self.window
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

#[cfg(test)]
mod ime_tests {
    use super::*;
    #[test]
    fn native_ime_context_lifetime_and_cancelled_feedback_follow_published_focus() {
        let mut surface = Surface::new().unwrap();
        surface.configure_ime(true);
        let mut frame = Frame {
            title: "REWIND IME context acceptance".into(),
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
        surface.present(&frame).unwrap();
        assert_ne!(surface.ime_context, 0);
        {
            let native = ImeContext::get(surface.window).unwrap();
            assert_eq!(native.context, surface.ime_context);
        }
        // Supply preedit state separately: this verifies native context lifetime,
        // not the presence of a Japanese conversion engine on the test desktop.
        surface.state_mut().composition.start();
        assert!(surface.state_mut().composition.replace(0, 0, "かな", 2));
        let old = surface.ime_context;
        frame.items[0].x = 20;
        frame.items[0].foreground = 0x123456;
        surface.present(&frame).unwrap();
        assert_eq!(surface.ime_context, old);
        assert_eq!(surface.state().composition.text, "かな");
        frame.items[0].focused = false;
        surface.present(&frame).unwrap();
        assert_ne!(surface.ime_context, old);
        assert!(!surface.state().composition.active);
        assert!(!surface.state().composition.accepting);
        assert!(surface.state().composition.text.is_empty());
        // A queued old result notification must not read/commit the new context.
        unsafe {
            SendMessageW(surface.window, 0x10f, 0, 0x800);
        }
        assert!(!surface.state().events.iter().any(|e| e.kind == "text"));
        surface.close();
        assert_eq!(surface.ime_context, 0);
    }
    #[test]
    fn real_japanese_ime_service_preedit_convert_commit_focus_and_close() {
        if std::env::var("REWIND_TEST_WINDOWS_IME_SERVICE").as_deref() != Ok("1") {
            return;
        }
        #[link(name = "user32")]
        unsafe extern "system" {
            fn LoadKeyboardLayoutW(name: *const u16, flags: u32) -> Handle;
            fn ActivateKeyboardLayout(layout: Handle, flags: u32) -> Handle;
            fn GetKeyboardLayout(thread: u32) -> Handle;
            fn GetKeyboardLayoutList(count: i32, layouts: *mut Handle) -> i32;
            fn SetFocus(window: Handle) -> Handle;
            fn SetActiveWindow(window: Handle) -> Handle;
            fn SetForegroundWindow(window: Handle) -> i32;
            fn GetForegroundWindow() -> Handle;
            fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
        }
        #[link(name = "imm32")]
        unsafe extern "system" {
            fn ImmIsIME(layout: Handle) -> i32;
            fn ImmSetOpenStatus(context: Handle, open: i32) -> i32;
            fn ImmSetConversionStatus(context: Handle, conversion: u32, sentence: u32) -> i32;
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct KeyboardInput {
            key: u16,
            scan: u16,
            flags: u32,
            time: u32,
            extra: usize,
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct MouseInput {
            x: i32,
            y: i32,
            data: u32,
            flags: u32,
            time: u32,
            extra: usize,
        }
        #[repr(C)]
        union InputBody {
            keyboard: KeyboardInput,
            mouse: MouseInput,
        }
        #[repr(C)]
        struct Input {
            kind: u32,
            body: InputBody,
        }
        fn key(key: u16) {
            let input = |flags| Input {
                kind: 1,
                body: InputBody {
                    keyboard: KeyboardInput {
                        key,
                        scan: 0,
                        flags,
                        time: 0,
                        extra: 0,
                    },
                },
            };
            let events = [input(0), input(2)];
            assert_eq!(
                unsafe { SendInput(2, events.as_ptr(), std::mem::size_of::<Input>() as i32) },
                2,
                "Real keyboard injection must deliver both key-down and key-up"
            );
        }
        struct Layout(Handle);
        impl Drop for Layout {
            fn drop(&mut self) {
                unsafe {
                    ActivateKeyboardLayout(self.0, 0);
                }
            }
        }
        let _restore = Layout(unsafe { GetKeyboardLayout(0) });
        let mut layout = 0;
        for name in ["E0010411", "00000411"] {
            let name = wide(name);
            let candidate = unsafe { LoadKeyboardLayoutW(name.as_ptr(), 0) };
            if candidate != 0 && unsafe { ImmIsIME(candidate) } != 0 {
                layout = candidate;
                break;
            }
        }
        if layout == 0 {
            let count = unsafe { GetKeyboardLayoutList(0, std::ptr::null_mut()) };
            assert!(
                (0..=256).contains(&count),
                "Native keyboard-layout count is bounded"
            );
            let mut layouts = vec![0; count as usize];
            let read = unsafe { GetKeyboardLayoutList(count, layouts.as_mut_ptr()) };
            for candidate in layouts.into_iter().take(read.max(0) as usize) {
                if candidate as usize & 0xffff == 0x0411 && unsafe { ImmIsIME(candidate) } != 0 {
                    layout = candidate;
                    break;
                }
            }
        }
        assert_ne!(layout, 0, "A real Japanese IMM-compatible conversion service is required; keyboard layout alone is insufficient");
        let mut surface = Surface::new().unwrap();
        surface.configure_ime(true);
        let mut frame = Frame {
            title: "REWIND real Microsoft IME acceptance".into(),
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
        surface.present(&frame).unwrap();
        unsafe {
            SetActiveWindow(surface.window);
            SetFocus(surface.window);
            SetForegroundWindow(surface.window);
            // Activate after the native window and its COM/TSF apartment exist.
            ActivateKeyboardLayout(layout, 0);
        }
        assert_eq!(
            unsafe { GetForegroundWindow() },
            surface.window,
            "Real IME input must target the REWIND foreground window"
        );
        fn pump(surface: &mut Surface, phase: &str, mut ready: impl FnMut(&Surface) -> bool) {
            let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !ready(surface) && std::time::Instant::now() < end {
                // Preserve committed text for inspection while servicing real native messages.
                unsafe {
                    let mut message: Message = std::mem::zeroed();
                    while PeekMessageW(&mut message, 0, 0, 0, 1) != 0 {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            if !ready(surface) {
                let native = ImeContext::get(surface.window).and_then(|c| c.text(8));
                let events: Vec<_> = surface
                    .state()
                    .events
                    .iter()
                    .map(|e| (&e.kind, &e.key))
                    .collect();
                panic!("Real IME {phase} notification did not reach the REWIND surface (accepting={}, active={}, preedit={:?}, invalid={}, native={native:?}, events={events:?})",
                    surface.state().composition.accepting, surface.state().composition.active,
                    surface.state().composition.text, surface.state().composition.invalid);
            }
        }
        fn start(surface: &mut Surface) {
            let context = ImeContext::get(surface.window).unwrap();
            assert_ne!(unsafe { ImmSetOpenStatus(context.context, 1) }, 0);
            // Use the real keyboard path; setting a composition buffer does not
            // establish that the service delivers native notifications to this window.
            assert_ne!(
                unsafe { ImmSetConversionStatus(context.context, 0x19, 0) },
                0,
                "Japanese native/full-width/roman conversion must be available"
            );
            drop(context);
            for letter in b"NIHONN" {
                key(u16::from(*letter));
                // Service keyboard messages before injecting the next syllable.
                unsafe {
                    let mut message: Message = std::mem::zeroed();
                    while PeekMessageW(&mut message, 0, 0, 0, 1) != 0 {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
            pump(surface, "preedit", |s| {
                s.state().composition.active && s.state().composition.text == "にほん"
            });
        }
        for _ in 0..8 {
            start(&mut surface);
            assert_eq!(surface.state().composition.text, "にほん");
            assert!(!surface.state().events.iter().any(|e| e.kind == "text"));
            key(0x20); // Space asks the actual Japanese service to convert.
            pump(&mut surface, "convert", |s| {
                s.state().composition.text != "にほん" && !s.state().composition.text.is_empty()
            });
            let converted = surface.state().composition.text.clone();
            key(0x0d); // Enter commits the service's converted result.
            pump(&mut surface, "commit", |s| {
                s.state().events.iter().any(|e| e.kind == "text")
            });
            let committed = surface
                .state_mut()
                .events
                .iter()
                .filter(|e| e.kind == "text")
                .map(|e| e.key.as_str())
                .collect::<String>();
            assert_eq!(committed, converted);
            while surface.state_mut().pop().is_some() {}
        }
        start(&mut surface);
        let old = surface.ime_context;
        frame.items[0].focused = false;
        surface.present(&frame).unwrap();
        assert_ne!(surface.ime_context, old);
        assert!(!surface.state().composition.active);
        assert!(!surface.state().events.iter().any(|e| e.kind == "text"));
        frame.items[0].focused = true;
        surface.present(&frame).unwrap();
        start(&mut surface);
        surface.close();
        assert_eq!(surface.ime_context, 0);
        assert!(!surface.state().composition.active);
    }
    #[test]
    fn native_ime_event_queue_has_a_byte_bound_and_releases_drained_entries() {
        let mut surface = Surface::new().unwrap();
        surface.configure_ime(true);
        for _ in 0..4096 {
            let mut event = Event::simple("text");
            event.key = "x".repeat(4096);
            surface.state_mut().push(event);
        }
        assert!(surface.state().overflow);
        assert!(surface.state().queued_bytes <= 512 * 1024);
        assert!(surface.state().events.len() < 4096);
        while surface.state_mut().pop().is_some() {}
        assert_eq!(surface.state().queued_bytes, 0);
        surface.close();
    }
}

#[cfg(test)]
mod font_tests {
    use super::*;
    #[test]
    fn native_font_service_draws_distinct_latin_and_japanese_glyphs() {
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
            unsafe {
                let dc = GetDC(surface.window);
                assert_ne!(dc, 0);
                let mut pixels = Vec::new();
                for y in 10..40 {
                    for x in 10..74 {
                        pixels.push(GetPixel(dc, x, y));
                    }
                }
                ReleaseDC(surface.window, dc);
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
