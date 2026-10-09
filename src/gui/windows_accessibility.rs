//! Standard IAccessible for the current published canvas. COM clients own only
//! bounded interface handles; closing clears the shared scene behind them.
use super::accessibility::{Node, Role, Tree};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicU32, Ordering},
};
type HResult = i32;
type Handle = isize;
const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_FAIL: i32 = 0x80004005u32 as i32;
const E_POINTER: i32 = 0x80004003u32 as i32;
const E_INVALIDARG: i32 = 0x80070057u32 as i32;
const E_NOTIMPL: i32 = 0x80004001u32 as i32;
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}
const UNKNOWN: Guid = Guid {
    a: 0,
    b: 0,
    c: 0,
    d: [0xc0, 0, 0, 0, 0, 0, 0, 0x46],
};
const DISPATCH: Guid = Guid {
    a: 0x20400,
    ..UNKNOWN
};
const ACCESSIBLE: Guid = Guid {
    a: 0x618736e0,
    b: 0x3c3d,
    c: 0x11cf,
    d: [0x81, 0x0c, 0, 0xaa, 0, 0x38, 0x9b, 0x71],
};
#[derive(Clone, Copy)]
#[repr(C)]
union Data {
    number: i32,
    pointer: *mut c_void,
    pair: [usize; 2],
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Variant {
    kind: u16,
    a: u16,
    b: u16,
    c: u16,
    data: Data,
}
impl Variant {
    fn empty() -> Self {
        Self {
            kind: 0,
            a: 0,
            b: 0,
            c: 0,
            data: Data { pair: [0; 2] },
        }
    }
    fn int(n: i32) -> Self {
        let mut value = Self::empty();
        value.kind = 3;
        value.data.number = n;
        value
    }
}
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[link(name = "oleacc")]
unsafe extern "system" {
    fn LresultFromObject(iid: *const Guid, wparam: usize, object: *mut c_void) -> isize;
}
#[link(name = "oleaut32")]
unsafe extern "system" {
    fn SysAllocStringLen(text: *const u16, len: u32) -> *mut u16;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn NotifyWinEvent(event: u32, window: Handle, object: i32, child: i32);
    fn ClientToScreen(window: Handle, point: *mut Point) -> i32;
    fn ScreenToClient(window: Handle, point: *mut Point) -> i32;
    fn PostMessageW(window: Handle, message: u32, wparam: usize, lparam: isize) -> i32;
}
#[repr(C)]
struct Vtable {
    query: unsafe extern "system" fn(*mut Object, *const Guid, *mut *mut c_void) -> HResult,
    add: unsafe extern "system" fn(*mut Object) -> u32,
    release: unsafe extern "system" fn(*mut Object) -> u32,
    type_count: unsafe extern "system" fn(*mut Object, *mut u32) -> HResult,
    type_info: unsafe extern "system" fn(*mut Object, u32, u32, *mut *mut c_void) -> HResult,
    ids: unsafe extern "system" fn(
        *mut Object,
        *const Guid,
        *mut *mut u16,
        u32,
        u32,
        *mut i32,
    ) -> HResult,
    invoke: unsafe extern "system" fn(
        *mut Object,
        i32,
        *const Guid,
        u32,
        u16,
        *mut c_void,
        *mut Variant,
        *mut c_void,
        *mut u32,
    ) -> HResult,
    parent: unsafe extern "system" fn(*mut Object, *mut *mut Object) -> HResult,
    child_count: unsafe extern "system" fn(*mut Object, *mut i32) -> HResult,
    child: unsafe extern "system" fn(*mut Object, Variant, *mut *mut Object) -> HResult,
    name: unsafe extern "system" fn(*mut Object, Variant, *mut *mut u16) -> HResult,
    value: unsafe extern "system" fn(*mut Object, Variant, *mut *mut u16) -> HResult,
    description: unsafe extern "system" fn(*mut Object, Variant, *mut *mut u16) -> HResult,
    role: unsafe extern "system" fn(*mut Object, Variant, *mut Variant) -> HResult,
    state: unsafe extern "system" fn(*mut Object, Variant, *mut Variant) -> HResult,
    help: unsafe extern "system" fn(*mut Object, Variant, *mut *mut u16) -> HResult,
    help_topic: unsafe extern "system" fn(*mut Object, *mut *mut u16, Variant, *mut i32) -> HResult,
    shortcut: unsafe extern "system" fn(*mut Object, Variant, *mut *mut u16) -> HResult,
    focus: unsafe extern "system" fn(*mut Object, *mut Variant) -> HResult,
    selection: unsafe extern "system" fn(*mut Object, *mut Variant) -> HResult,
    action: unsafe extern "system" fn(*mut Object, Variant, *mut *mut u16) -> HResult,
    select: unsafe extern "system" fn(*mut Object, i32, Variant) -> HResult,
    location: unsafe extern "system" fn(
        *mut Object,
        *mut i32,
        *mut i32,
        *mut i32,
        *mut i32,
        Variant,
    ) -> HResult,
    navigate: unsafe extern "system" fn(*mut Object, i32, Variant, *mut Variant) -> HResult,
    hit: unsafe extern "system" fn(*mut Object, i32, i32, *mut Variant) -> HResult,
    do_action: unsafe extern "system" fn(*mut Object, Variant) -> HResult,
    put_name: unsafe extern "system" fn(*mut Object, Variant, *mut u16) -> HResult,
    put_value: unsafe extern "system" fn(*mut Object, Variant, *mut u16) -> HResult,
}
#[repr(C)]
struct Object {
    vtable: *const Vtable,
    refs: AtomicU32,
    tree: Tree,
    window: Handle,
    id: Option<String>,
}
impl Object {
    fn new(tree: Tree, window: Handle, id: Option<String>) -> Option<*mut Self> {
        if !tree.lease() {
            return None;
        }
        Some(Box::into_raw(Box::new(Self {
            vtable: &VTABLE,
            refs: AtomicU32::new(1),
            tree,
            window,
            id,
        })))
    }
    fn node(&self, child: Variant) -> Option<Node> {
        if child.kind != 3 {
            return None;
        }
        let n = unsafe { child.data.number };
        if n == 0 {
            return self.tree.node(self.id.as_deref());
        }
        if n < 1 || self.id.is_some() {
            return None;
        }
        let ids = self.tree.children();
        self.tree.node(Some(ids.get(n as usize - 1)?))
    }
}
unsafe extern "system" fn query(this: *mut Object, iid: *const Guid, out: *mut *mut c_void) -> i32 {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = std::ptr::null_mut();
        if [UNKNOWN, DISPATCH, ACCESSIBLE].contains(&*iid) {
            *out = this.cast();
            add(this);
            S_OK
        } else {
            0x80004002u32 as i32
        }
    }
}
unsafe extern "system" fn add(this: *mut Object) -> u32 {
    unsafe { (*this).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}
unsafe extern "system" fn release(this: *mut Object) -> u32 {
    let left = unsafe { (*this).refs.fetch_sub(1, Ordering::AcqRel) - 1 };
    if left == 0 {
        let object = unsafe { Box::from_raw(this) };
        object.tree.release();
    }
    left
}
unsafe extern "system" fn type_count(_: *mut Object, out: *mut u32) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = 0;
    }
    S_OK
}
unsafe extern "system" fn type_info(_: *mut Object, _: u32, _: u32, out: *mut *mut c_void) -> i32 {
    if !out.is_null() {
        unsafe {
            *out = std::ptr::null_mut();
        }
    }
    E_NOTIMPL
}
unsafe extern "system" fn ids(
    _: *mut Object,
    _: *const Guid,
    _: *mut *mut u16,
    _: u32,
    _: u32,
    _: *mut i32,
) -> i32 {
    E_NOTIMPL
}
unsafe extern "system" fn invoke(
    _: *mut Object,
    _: i32,
    _: *const Guid,
    _: u32,
    _: u16,
    _: *mut c_void,
    out: *mut Variant,
    _: *mut c_void,
    _: *mut u32,
) -> i32 {
    if !out.is_null() {
        unsafe {
            *out = Variant::empty();
        }
    }
    E_NOTIMPL
}
unsafe extern "system" fn parent(this: *mut Object, out: *mut *mut Object) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = std::ptr::null_mut();
        let o = &*this;
        if o.id.is_some() {
            if let Some(p) = Object::new(o.tree.clone(), o.window, None) {
                *out = p;
                return S_OK;
            }
        }
    }
    S_FALSE
}
unsafe extern "system" fn child_count(this: *mut Object, out: *mut i32) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        let o = &*this;
        *out = if o.id.is_none() {
            o.tree.children().len() as i32
        } else {
            0
        };
    }
    S_OK
}
unsafe extern "system" fn child(this: *mut Object, v: Variant, out: *mut *mut Object) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = std::ptr::null_mut();
        let o = &*this;
        let Some(n) = o.node(v) else {
            return E_INVALIDARG;
        };
        if n.id.is_none() {
            return S_FALSE;
        }
        if let Some(p) = Object::new(o.tree.clone(), o.window, n.id) {
            *out = p;
            return S_OK;
        }
    }
    E_FAIL
}
fn string(out: *mut *mut u16, text: &str) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    let text: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        *out = SysAllocStringLen(text.as_ptr(), text.len() as u32);
        if (*out).is_null() && !text.is_empty() {
            return E_FAIL;
        }
    }
    S_OK
}
unsafe extern "system" fn name(this: *mut Object, v: Variant, out: *mut *mut u16) -> i32 {
    let Some(n) = unsafe { &*this }.node(v) else {
        return string(out, "");
    };
    string(out, &n.name)
}
unsafe extern "system" fn value(this: *mut Object, v: Variant, out: *mut *mut u16) -> i32 {
    let Some(n) = unsafe { &*this }.node(v) else {
        return string(out, "");
    };
    string(out, &n.value)
}
unsafe extern "system" fn description(_: *mut Object, _: Variant, out: *mut *mut u16) -> i32 {
    string(out, "");
    S_FALSE
}
fn role_number(r: Role) -> i32 {
    match r {
        Role::Window => 9,
        Role::Label => 41,
        Role::Button => 43,
        Role::CheckBox => 44,
        Role::Entry | Role::TextArea => 42,
    }
}
unsafe extern "system" fn role(this: *mut Object, v: Variant, out: *mut Variant) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = Variant::empty();
    }
    let Some(n) = unsafe { &*this }.node(v) else {
        return E_FAIL;
    };
    unsafe {
        *out = Variant::int(role_number(n.role));
    }
    S_OK
}
unsafe extern "system" fn state(this: *mut Object, v: Variant, out: *mut Variant) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    let n = unsafe { &*this }.node(v);
    let flags = n.map_or(1 | 0x8000, |n| {
        i32::from(!n.enabled)
            | if n.focused { 4 } else { 0 }
            | if n.checked { 0x10 } else { 0 }
            | if n.focusable { 0x100000 } else { 0 }
    });
    unsafe {
        *out = Variant::int(flags);
    }
    S_OK
}
unsafe extern "system" fn help_topic(
    _: *mut Object,
    out: *mut *mut u16,
    _: Variant,
    topic: *mut i32,
) -> i32 {
    if !topic.is_null() {
        unsafe {
            *topic = 0;
        }
    }
    string(out, "");
    S_FALSE
}
unsafe extern "system" fn focus(this: *mut Object, out: *mut Variant) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = Variant::empty();
        let o = &*this;
        if o.id.is_none() {
            for (i, id) in o.tree.children().iter().enumerate() {
                if o.tree.node(Some(id)).is_some_and(|n| n.focused) {
                    *out = Variant::int(i as i32 + 1);
                    return S_OK;
                }
            }
            if o.tree.node(None).is_some_and(|n| n.focused) {
                *out = Variant::int(0);
                return S_OK;
            }
        }
    }
    S_FALSE
}
unsafe extern "system" fn selection(_: *mut Object, out: *mut Variant) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = Variant::empty();
    }
    S_FALSE
}
unsafe extern "system" fn action(this: *mut Object, v: Variant, out: *mut *mut u16) -> i32 {
    let Some(n) = unsafe { &*this }.node(v) else {
        return string(out, "");
    };
    let text = match n.role {
        Role::Button => "Press",
        Role::CheckBox => "Toggle",
        Role::Entry | Role::TextArea => "Focus",
        _ => "",
    };
    string(out, text)
}
unsafe extern "system" fn select(_: *mut Object, _: i32, _: Variant) -> i32 {
    E_NOTIMPL
}
unsafe extern "system" fn location(
    this: *mut Object,
    left: *mut i32,
    top: *mut i32,
    width: *mut i32,
    height: *mut i32,
    v: Variant,
) -> i32 {
    if [left, top, width, height].iter().any(|p| p.is_null()) {
        return E_POINTER;
    }
    unsafe {
        *left = 0;
        *top = 0;
        *width = 0;
        *height = 0;
    }
    let o = unsafe { &*this };
    let Some(n) = o.node(v) else {
        return E_FAIL;
    };
    let mut point = Point { x: n.x, y: n.y };
    if unsafe { ClientToScreen(o.window, &mut point) } == 0 {
        return E_FAIL;
    }
    unsafe {
        *left = point.x;
        *top = point.y;
        *width = n.width;
        *height = n.height;
    }
    S_OK
}
unsafe extern "system" fn navigate(
    this: *mut Object,
    direction: i32,
    start: Variant,
    out: *mut Variant,
) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = Variant::empty();
    }
    let o = unsafe { &*this };
    let children = o.tree.children();
    if o.id.is_none()
        && start.kind == 3
        && unsafe { start.data.number } == 0
        && !children.is_empty()
    {
        let index = match direction {
            7 => 1,
            8 => children.len() as i32,
            _ => return E_NOTIMPL,
        };
        unsafe {
            *out = Variant::int(index);
        }
        return S_OK;
    }
    S_FALSE
}
unsafe extern "system" fn hit(this: *mut Object, x: i32, y: i32, out: *mut Variant) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = Variant::empty();
    }
    let o = unsafe { &*this };
    let mut point = Point { x, y };
    if unsafe { ScreenToClient(o.window, &mut point) } == 0 {
        return E_FAIL;
    }
    for (i, id) in o.tree.children().iter().enumerate().rev() {
        if o.tree.node(Some(id)).is_some_and(|n| {
            point.x >= n.x && point.y >= n.y && point.x < n.x + n.width && point.y < n.y + n.height
        }) {
            unsafe {
                *out = Variant::int(i as i32 + 1);
            }
            return S_OK;
        }
    }
    if o.tree.node(None).is_some() {
        unsafe {
            *out = Variant::int(0);
        }
        return S_OK;
    }
    S_FALSE
}
unsafe extern "system" fn do_action(this: *mut Object, v: Variant) -> i32 {
    let o = unsafe { &*this };
    let Some(n) = o.node(v) else {
        return E_INVALIDARG;
    };
    let Some(id) = n.id else {
        return S_FALSE;
    };
    let Some(e) = o.tree.default_action(&id) else {
        return S_FALSE;
    };
    let packed = e.x as u16 as usize | ((e.y as u16 as usize) << 16);
    if unsafe { PostMessageW(o.window, 0x201, 1, packed as isize) } != 0
        && unsafe { PostMessageW(o.window, 0x202, 0, packed as isize) } != 0
    {
        S_OK
    } else {
        E_FAIL
    }
}
unsafe extern "system" fn put(_: *mut Object, _: Variant, _: *mut u16) -> i32 {
    E_NOTIMPL
}
static VTABLE: Vtable = Vtable {
    query,
    add,
    release,
    type_count,
    type_info,
    ids,
    invoke,
    parent,
    child_count,
    child,
    name,
    value,
    description,
    role,
    state,
    help: description,
    help_topic,
    shortcut: description,
    focus,
    selection,
    action,
    select,
    location,
    navigate,
    hit,
    do_action,
    put_name: put,
    put_value: put,
};
pub(super) fn get_object(window: Handle, wparam: usize, lparam: isize, tree: Tree) -> isize {
    if lparam as i32 != -4 {
        return 0;
    }
    let Some(object) = Object::new(tree, window, None) else {
        return 0;
    };
    let result = unsafe { LresultFromObject(&ACCESSIBLE, wparam, object.cast()) };
    unsafe {
        release(object);
    }
    result
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, mode: u32) -> i32;
    fn CoUninitialize();
}
pub(super) struct Apartment;
impl Apartment {
    pub fn new() -> std::io::Result<Self> {
        let result = unsafe { CoInitializeEx(std::ptr::null_mut(), 2) };
        if result < 0 {
            return Err(std::io::Error::other(
                "GuiAccessibilityUnavailable: COM apartment",
            ));
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[link(name = "oleacc")]
    unsafe extern "system" {
        fn AccessibleObjectFromWindow(
            window: Handle,
            id: u32,
            iid: *const Guid,
            out: *mut *mut c_void,
        ) -> i32;
    }
    #[link(name = "oleaut32")]
    unsafe extern "system" {
        fn SysFreeString(text: *mut u16);
        fn SysStringLen(text: *mut u16) -> u32;
    }
    unsafe fn read_name(object: *mut Object, child: i32) -> String {
        let mut text = std::ptr::null_mut();
        let hr = unsafe { ((*(*object).vtable).name)(object, Variant::int(child), &mut text) };
        assert_eq!(hr, 0);
        let len = unsafe { SysStringLen(text) } as usize;
        let value = if len == 0 {
            String::new()
        } else {
            String::from_utf16(unsafe { std::slice::from_raw_parts(text, len) }).unwrap()
        };
        unsafe {
            SysFreeString(text);
        }
        value
    }
    #[test]
    fn native_accessible_object_exposes_published_roles_states_and_closes_without_retaining_scene()
    {
        use super::super::{windows::Surface, Frame, Item};
        assert_eq!(
            std::mem::size_of::<Variant>(),
            if std::mem::size_of::<usize>() == 8 {
                24
            } else {
                16
            }
        );
        let mut surface = Surface::new().unwrap();
        surface.configure_accessibility(true);
        let mut frame = Frame {
            title: "REWIND accessibility native".into(),
            width: 320,
            height: 160,
            background: 0xffffff,
            items: vec![Item {
                id: "save".into(),
                kind: "button".into(),
                x: 10,
                y: 10,
                width: 100,
                height: 24,
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
        surface.present(&frame).unwrap();
        let mut pointer = std::ptr::null_mut();
        assert_eq!(
            unsafe {
                AccessibleObjectFromWindow(
                    surface.native_window_handle(),
                    (-4i32) as u32,
                    &ACCESSIBLE,
                    &mut pointer,
                )
            },
            0
        );
        assert!(!pointer.is_null());
        let object = pointer.cast::<Object>();
        let mut count = 0;
        assert_eq!(
            unsafe { ((*(*object).vtable).child_count)(object, &mut count) },
            0
        );
        assert_eq!(count, 1);
        assert_eq!(unsafe { read_name(object, 0) }, frame.title);
        assert_eq!(unsafe { read_name(object, 1) }, "保存");
        let mut role = Variant::empty();
        assert_eq!(
            unsafe { ((*(*object).vtable).role)(object, Variant::int(1), &mut role) },
            0
        );
        assert_eq!(unsafe { role.data.number }, 43);
        frame.items[0].enabled = false;
        frame.items[0].text = "Disabled".into();
        surface.present(&frame).unwrap();
        assert_eq!(unsafe { read_name(object, 1) }, "Disabled");
        let mut state = Variant::empty();
        assert_eq!(
            unsafe { ((*(*object).vtable).state)(object, Variant::int(1), &mut state) },
            0
        );
        assert_ne!(unsafe { state.data.number } & 1, 0);
        assert_eq!(
            unsafe { ((*(*object).vtable).do_action)(object, Variant::int(1)) },
            1
        );
        frame.items[0].enabled = true;
        surface.present(&frame).unwrap();
        assert_eq!(
            unsafe { ((*(*object).vtable).do_action)(object, Variant::int(1)) },
            0
        );
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut activated = false;
        while std::time::Instant::now() < until {
            if let Some(e) = surface.poll().unwrap() {
                if e.kind == "pointer" {
                    activated = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(activated);
        surface.close();
        assert_eq!(
            unsafe { ((*(*object).vtable).child_count)(object, &mut count) },
            0
        );
        assert_eq!(count, 0);
        assert_eq!(
            unsafe { ((*(*object).vtable).do_action)(object, Variant::int(1)) },
            E_INVALIDARG
        );
        unsafe {
            ((*(*object).vtable).release)(object);
        }
    }
}

/// Announce changes through the OS event service after the new frame is visible.
pub(super) fn changed(window: Handle, previous: Option<&super::Frame>, next: &super::Frame) {
    let old: std::collections::BTreeMap<_, _> = previous
        .into_iter()
        .flat_map(|f| f.items.iter())
        .map(|i| (&i.id, i))
        .collect();
    let old_ids: Vec<_> = previous
        .into_iter()
        .flat_map(|f| f.items.iter())
        .filter(|i| i.kind != "rect")
        .map(|i| &i.id)
        .collect();
    let new_ids: Vec<_> = next
        .items
        .iter()
        .filter(|i| i.kind != "rect")
        .map(|i| &i.id)
        .collect();
    unsafe {
        if old_ids != new_ids {
            NotifyWinEvent(0x8004, window, -4, 0);
        }
        if previous.is_none_or(|f| f.title != next.title) {
            NotifyWinEvent(0x800c, window, -4, 0);
        }
        for (index, i) in next.items.iter().filter(|i| i.kind != "rect").enumerate() {
            let child = index as i32 + 1;
            if let Some(before) = old.get(&i.id) {
                if before.text != i.text {
                    NotifyWinEvent(
                        if matches!(i.kind.as_str(), "textbox" | "textarea") {
                            0x800e
                        } else {
                            0x800c
                        },
                        window,
                        -4,
                        child,
                    );
                }
                if before.enabled != i.enabled
                    || before.checked != i.checked
                    || before.focused != i.focused
                {
                    NotifyWinEvent(0x800a, window, -4, child);
                }
                if before.x != i.x
                    || before.y != i.y
                    || before.width != i.width
                    || before.height != i.height
                {
                    NotifyWinEvent(0x800b, window, -4, child);
                }
            }
            if i.focused && old.get(&i.id).is_none_or(|before| !before.focused) {
                NotifyWinEvent(0x8005, window, -4, child);
            }
        }
    }
}
pub(super) fn announce_focus(window: Handle, tree: &Tree) {
    if let Some((index, _)) = tree
        .children()
        .iter()
        .enumerate()
        .find(|(_, id)| tree.node(Some(id)).is_some_and(|n| n.focused))
    {
        unsafe {
            NotifyWinEvent(0x8005, window, -4, index as i32 + 1);
        }
    }
}
