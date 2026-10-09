//! AT-SPI2 on a private GLib context. No VM state or unpublished scene is exported.
use super::{
    accessibility::{Role, Tree},
    Event, Frame,
};
use libc::{c_char, c_int, c_void};
use std::{
    collections::{BTreeMap, VecDeque},
    ffi::{CStr, CString},
    io, ptr,
    sync::{Arc, Mutex, OnceLock},
};
type P = *mut c_void;
#[repr(C)]
struct Vtable {
    method: Option<
        unsafe extern "C" fn(
            P,
            *const c_char,
            *const c_char,
            *const c_char,
            *const c_char,
            P,
            P,
            P,
        ),
    >,
    property: Option<
        unsafe extern "C" fn(
            P,
            *const c_char,
            *const c_char,
            *const c_char,
            *const c_char,
            *mut P,
            P,
        ) -> P,
    >,
    set: Option<
        unsafe extern "C" fn(
            P,
            *const c_char,
            *const c_char,
            *const c_char,
            *const c_char,
            P,
            *mut P,
            P,
        ) -> c_int,
    >,
    padding: [P; 8],
}
unsafe impl Sync for Vtable {}
#[repr(C)]
#[derive(Clone, Copy)]
struct PollFd {
    fd: c_int,
    events: u16,
    revents: u16,
}
macro_rules! api {
    ($(fn $name:ident($($arg:ty),*) -> $ret:ty;)*) => {
        #[allow(non_snake_case)] struct Api { new: unsafe extern "C" fn(*const c_char,...)->P, $($name:unsafe extern "C" fn($($arg),*)->$ret,)* }
        impl Api {unsafe fn load()->Option<Self> {
            let h=unsafe{libc::dlopen(c"libgio-2.0.so.0".as_ptr(),libc::RTLD_NOW|libc::RTLD_LOCAL)};
            if h.is_null(){return None;}
            // Process-lifetime GLib/GIO caches and function pointers require a retained library.
            macro_rules! symbol {($n:expr,$t:ty)=>{{let p=unsafe{libc::dlsym(h,$n)};if p.is_null(){return None;}unsafe{std::mem::transmute::<P,$t>(p)}}};}
            Some(Self{new:symbol!(c"g_variant_new".as_ptr(),unsafe extern "C" fn(*const c_char,...)->P),$($name:symbol!(concat!(stringify!($name),"\0").as_ptr().cast(),unsafe extern "C" fn($($arg),*)->$ret),)*})
        }}
    }
}
api! {
fn g_bus_get_sync(c_int,P,*mut P)->P;
fn g_dbus_connection_call_sync(P,*const c_char,*const c_char,*const c_char,*const c_char,P,P,c_int,c_int,P,*mut P)->P;
fn g_dbus_connection_new_for_address_sync(*const c_char,c_int,P,P,*mut P)->P;
fn g_dbus_connection_get_unique_name(P)->*const c_char;
fn g_dbus_connection_register_object(P,*const c_char,P,*const Vtable,P,Option<unsafe extern "C" fn(P)>,*mut P)->u32;
fn g_dbus_connection_unregister_object(P,u32)->c_int;
fn g_dbus_connection_emit_signal(P,*const c_char,*const c_char,*const c_char,*const c_char,P,*mut P)->c_int;
fn g_dbus_connection_close_sync(P,P,*mut P)->c_int;
fn g_dbus_node_info_new_for_xml(*const c_char,*mut P)->P;
fn g_dbus_node_info_lookup_interface(P,*const c_char)->P;
fn g_dbus_node_info_unref(P)->();
fn g_dbus_method_invocation_return_value(P,P)->();
fn g_dbus_method_invocation_return_dbus_error(P,*const c_char,*const c_char)->();
fn g_variant_get_child_value(P,usize)->P;
fn g_variant_get_string(P,*mut usize)->*const c_char;
fn g_variant_get_int32(P)->i32;
fn g_variant_unref(P)->();
fn g_variant_new_array(P,*const P,usize)->P;
fn g_variant_new_tuple(*const P,usize)->P;
fn g_variant_type_new(*const c_char)->P;
fn g_variant_type_free(P)->();
fn g_object_unref(P)->();
fn g_error_free(P)->();
fn g_main_context_new()->P;
fn g_main_context_unref(P)->();
fn g_main_context_push_thread_default(P)->();
fn g_main_context_pop_thread_default(P)->();
fn g_main_context_iteration(P,c_int)->c_int;
fn g_main_context_acquire(P)->c_int;
fn g_main_context_release(P)->();
fn g_main_context_prepare(P,*mut c_int)->c_int;
fn g_main_context_query(P,c_int,*mut c_int,*mut PollFd,c_int)->c_int;
}
static API: OnceLock<Option<Api>> = OnceLock::new();
const ROOT: &str = "/org/rewind/accessible/root";
const WINDOW: &str = "/org/rewind/accessible/window";
const XML: &str = r#"<node>
<interface name="org.a11y.atspi.Accessible">
<property name="Name" type="s" access="read"/><property name="Description" type="s" access="read"/>
<property name="Parent" type="(so)" access="read"/><property name="ChildCount" type="i" access="read"/>
<method name="GetChildAtIndex"><arg type="i" direction="in"/><arg type="(so)" direction="out"/></method>
<method name="GetChildren"><arg type="a(so)" direction="out"/></method>
<method name="GetIndexInParent"><arg type="i" direction="out"/></method>
<method name="GetRole"><arg type="u" direction="out"/></method>
<method name="GetRoleName"><arg type="s" direction="out"/></method>
<method name="GetLocalizedRoleName"><arg type="s" direction="out"/></method>
<method name="GetState"><arg type="au" direction="out"/></method>
<method name="GetAttributes"><arg type="a{ss}" direction="out"/></method>
<method name="GetApplication"><arg type="(so)" direction="out"/></method>
<method name="GetInterfaces"><arg type="as" direction="out"/></method></interface>
<interface name="org.a11y.atspi.Application"><property name="ToolkitName" type="s" access="read"/>
<property name="Version" type="s" access="read"/><property name="Id" type="i" access="readwrite"/>
<method name="GetLocale"><arg type="u" direction="in"/><arg type="s" direction="out"/></method></interface>
<interface name="org.a11y.atspi.Action"><property name="NActions" type="i" access="read"/>
<method name="GetName"><arg type="i" direction="in"/><arg type="s" direction="out"/></method>
<method name="GetLocalizedName"><arg type="i" direction="in"/><arg type="s" direction="out"/></method>
<method name="GetDescription"><arg type="i" direction="in"/><arg type="s" direction="out"/></method>
<method name="GetKeyBinding"><arg type="i" direction="in"/><arg type="s" direction="out"/></method>
<method name="DoAction"><arg type="i" direction="in"/><arg type="b" direction="out"/></method></interface>
<interface name="org.a11y.atspi.Text">
<property name="CharacterCount" type="i" access="read"/><property name="CaretOffset" type="i" access="read"/>
<method name="GetText"><arg type="i" direction="in"/><arg type="i" direction="in"/><arg type="s" direction="out"/></method>
<method name="GetCharacterAtOffset"><arg type="i" direction="in"/><arg type="i" direction="out"/></method>
<method name="GetNSelections"><arg type="i" direction="out"/></method>
<method name="GetSelection"><arg type="i" direction="in"/><arg type="i" direction="out"/><arg type="i" direction="out"/></method>
</interface>
<interface name="org.a11y.atspi.Cache"><method name="GetItems"><arg type="a((so)(so)(so)iiassusau)" direction="out"/></method></interface>
</node>"#;
struct Callback {
    tree: Tree,
    id: Option<String>,
    application: bool,
    bus: String,
    queue: Arc<Mutex<VecDeque<Event>>>,
    application_id: Arc<Mutex<i32>>,
}
fn cs(s: &str) -> CString {
    CString::new(s).expect("validated native string")
}
fn path(id: &str) -> String {
    let mut p = String::from("/org/rewind/accessible/n");
    for b in id.bytes() {
        use std::fmt::Write;
        write!(p, "{b:02x}").unwrap();
    }
    p
}
impl Callback {
    fn children(&self) -> Vec<String> {
        if self.application {
            if self.tree.node(None).is_some() {
                vec![WINDOW.into()]
            } else {
                vec![]
            }
        } else if self.id.is_none() {
            self.tree.children().iter().map(|id| path(id)).collect()
        } else {
            vec![]
        }
    }
    fn parent(&self) -> String {
        if self.application {
            "/org/a11y/atspi/accessible/null".into()
        } else if self.id.is_none() {
            ROOT.into()
        } else {
            WINDOW.into()
        }
    }
    fn action(&self) -> bool {
        self.id
            .as_deref()
            .and_then(|id| self.tree.node(Some(id)))
            .is_some_and(|n| n.focusable)
    }
    fn reference(&self, p: &str) -> P {
        unsafe {
            (API.get().unwrap().as_ref().unwrap().new)(
                c"(so)".as_ptr(),
                cs(&self.bus).as_ptr(),
                cs(p).as_ptr(),
            )
        }
    }
}
unsafe extern "C" fn destroy(data: P) {
    drop(unsafe { Box::from_raw(data.cast::<Callback>()) });
}
fn string(s: &str) -> P {
    unsafe { (API.get().unwrap().as_ref().unwrap().new)(c"s".as_ptr(), cs(s).as_ptr()) }
}
fn int(n: i32) -> P {
    unsafe { (API.get().unwrap().as_ref().unwrap().new)(c"i".as_ptr(), n) }
}
fn array(kind: &CStr, values: &[P]) -> P {
    let a = API.get().unwrap().as_ref().unwrap();
    unsafe {
        let t = (a.g_variant_type_new)(kind.as_ptr());
        let v = (a.g_variant_new_array)(t, values.as_ptr(), values.len());
        (a.g_variant_type_free)(t);
        v
    }
}
unsafe extern "C" fn property(
    _: P,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    name: *const c_char,
    _: *mut P,
    data: P,
) -> P {
    let c = unsafe { &*data.cast::<Callback>() };
    let name = unsafe { CStr::from_ptr(name) }.to_bytes();
    let n = c.tree.node(c.id.as_deref());
    match name {
        b"Name" => string(if c.application {
            "REWIND"
        } else {
            n.as_ref().map_or("", |n| n.name.as_str())
        }),
        b"Description" => string(n.as_ref().map_or("", |n| n.value.as_str())),
        b"Parent" => {
            if c.application {
                unsafe {
                    (API.get().unwrap().as_ref().unwrap().new)(
                        c"(so)".as_ptr(),
                        c"org.a11y.atspi.Registry".as_ptr(),
                        c"/org/a11y/atspi/accessible/root".as_ptr(),
                    )
                }
            } else {
                c.reference(&c.parent())
            }
        }
        b"ChildCount" => int(c.children().len() as i32),
        b"ToolkitName" => string("REWIND"),
        b"Version" => string(env!("CARGO_PKG_VERSION")),
        b"Id" => int(*c.application_id.lock().unwrap()),
        b"CharacterCount" => int(n.as_ref().map_or(0, |n| n.value.chars().count()) as i32),
        b"CaretOffset" => int(n.as_ref().map_or(0, |n| n.cursor) as i32),
        b"NActions" => int(i32::from(c.action())),
        _ => ptr::null_mut(),
    }
}
unsafe extern "C" fn method(
    _: P,
    _: *const c_char,
    _: *const c_char,
    interface: *const c_char,
    name: *const c_char,
    args: P,
    invocation: P,
    data: P,
) {
    let a = API.get().unwrap().as_ref().unwrap();
    let c = unsafe { &*data.cast::<Callback>() };
    let name = unsafe { CStr::from_ptr(name) }.to_bytes();
    let interface = unsafe { CStr::from_ptr(interface) }.to_bytes();
    let index = || unsafe {
        let child = (a.g_variant_get_child_value)(args, 0);
        let n = (a.g_variant_get_int32)(child);
        (a.g_variant_unref)(child);
        n
    };
    let n = c.tree.node(c.id.as_deref());
    let (role, role_name) = if c.application {
        (75, "application")
    } else {
        match n.as_ref().map(|n| n.role) {
            Some(Role::Window) => (69, "window"),
            Some(Role::Label) => (29, "label"),
            Some(Role::Button) => (43, "push button"),
            Some(Role::CheckBox) => (7, "check box"),
            Some(Role::Entry) => (79, "entry"),
            Some(Role::TextArea) => (61, "text"),
            None => (0, "invalid"),
        }
    };
    let v = if interface == b"org.a11y.atspi.Cache" && name == b"GetItems" {
        // No eager mirror: the standard client reads current published properties on demand.
        array(c"((so)(so)(so)iiassusau)", &[])
    } else if interface == b"org.a11y.atspi.Text" {
        let text = n.as_ref().map_or("", |n| n.value.as_str());
        match name {
            b"GetText" => {
                let start = index();
                let child = unsafe { (a.g_variant_get_child_value)(args, 1) };
                let end = unsafe { (a.g_variant_get_int32)(child) };
                unsafe {
                    (a.g_variant_unref)(child);
                }
                let len = text.chars().count();
                let end = if end == -1 { len } else { end.max(0) as usize };
                if start < 0 || end < start as usize || end > len {
                    string("")
                } else {
                    string(
                        &text
                            .chars()
                            .skip(start as usize)
                            .take(end - start as usize)
                            .collect::<String>(),
                    )
                }
            }
            b"GetCharacterAtOffset" => int(if index() < 0 {
                0
            } else {
                text.chars().nth(index() as usize).map_or(0, |c| c as i32)
            }),
            b"GetNSelections" => int(i32::from(n.as_ref().is_some_and(|n| n.cursor != n.anchor))),
            b"GetSelection" => {
                let (start, end) = if index() == 0 {
                    n.as_ref()
                        .map_or((0, 0), |n| (n.cursor.min(n.anchor), n.cursor.max(n.anchor)))
                } else {
                    (0, 0)
                };
                unsafe { (a.new)(c"(ii)".as_ptr(), start as i32, end as i32) }
            }
            _ => ptr::null_mut(),
        }
    } else if interface == b"org.a11y.atspi.Action" {
        match name {
            b"DoAction" => {
                let event = if index() == 0 {
                    c.id.as_deref().and_then(|id| c.tree.default_action(id))
                } else {
                    None
                };
                let mut q = c.queue.lock().unwrap();
                let accepted = event.is_some() && q.len() < 64;
                if accepted {
                    q.push_back(event.unwrap());
                }
                unsafe { (a.new)(c"b".as_ptr(), c_int::from(accepted)) }
            }
            b"GetName" | b"GetLocalizedName" => string(if index() == 0 && c.action() {
                "activate"
            } else {
                ""
            }),
            b"GetDescription" | b"GetKeyBinding" => string(""),
            _ => ptr::null_mut(),
        }
    } else {
        match name {
            b"GetRole" => unsafe { (a.new)(c"u".as_ptr(), role as u32) },
            b"GetRoleName" | b"GetLocalizedRoleName" => string(role_name),
            b"GetChildren" => array(
                c"(so)",
                &c.children()
                    .iter()
                    .map(|p| c.reference(p))
                    .collect::<Vec<_>>(),
            ),
            b"GetChildAtIndex" => {
                let i = index();
                let children = c.children();
                c.reference(if i >= 0 {
                    children
                        .get(i as usize)
                        .map_or("/org/a11y/atspi/accessible/null", String::as_str)
                } else {
                    "/org/a11y/atspi/accessible/null"
                })
            }
            b"GetIndexInParent" => int(if c.application {
                -1
            } else if c.id.is_none() {
                0
            } else {
                c.tree
                    .children()
                    .iter()
                    .position(|id| Some(id) == c.id.as_ref())
                    .map_or(-1, |i| i as i32)
            }),
            b"GetApplication" => c.reference(ROOT),
            b"GetAttributes" => array(c"{ss}", &[]),
            b"GetInterfaces" => {
                let mut names = vec!["org.a11y.atspi.Accessible"];
                if c.application {
                    names.push("org.a11y.atspi.Application");
                } else if c.action() {
                    names.push("org.a11y.atspi.Action");
                }
                if n.as_ref()
                    .is_some_and(|n| matches!(n.role, Role::Entry | Role::TextArea))
                {
                    names.push("org.a11y.atspi.Text");
                }
                array(c"s", &names.into_iter().map(string).collect::<Vec<_>>())
            }
            b"GetState" => {
                let mut bits = 0u64;
                let mut set = |b: u32| bits |= 1u64 << b;
                if let Some(n) = n {
                    set(23);
                    set(30);
                    if n.enabled {
                        set(8);
                        set(24);
                    }
                    if n.checked {
                        set(4);
                    }
                    if n.focusable {
                        set(11);
                    }
                    if n.focused {
                        set(12);
                    }
                    if matches!(n.role, Role::Entry | Role::TextArea) {
                        set(7);
                    }
                    if n.role == Role::TextArea {
                        set(17);
                    } else if n.role == Role::Entry {
                        set(26);
                    }
                } else {
                    set(6);
                }
                array(
                    c"u",
                    &[unsafe { (a.new)(c"u".as_ptr(), bits as u32) }, unsafe {
                        (a.new)(c"u".as_ptr(), (bits >> 32) as u32)
                    }],
                )
            }
            b"GetLocale" => string(""),
            _ => ptr::null_mut(),
        }
    };
    unsafe {
        if v.is_null() {
            (a.g_dbus_method_invocation_return_dbus_error)(
                invocation,
                c"org.freedesktop.DBus.Error.UnknownMethod".as_ptr(),
                c"Unsupported accessibility method".as_ptr(),
            );
        } else {
            (a.g_dbus_method_invocation_return_value)(
                invocation,
                if interface == b"org.a11y.atspi.Text" && name == b"GetSelection" {
                    v
                } else {
                    (a.g_variant_new_tuple)(&v, 1)
                },
            );
        }
    }
}
unsafe extern "C" fn set_property(
    _: P,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    name: *const c_char,
    value: P,
    _: *mut P,
    data: P,
) -> c_int {
    let c = unsafe { &*data.cast::<Callback>() };
    if c.application && unsafe { CStr::from_ptr(name) }.to_bytes() == b"Id" {
        *c.application_id.lock().unwrap() =
            unsafe { (API.get().unwrap().as_ref().unwrap().g_variant_get_int32)(value) };
        1
    } else {
        0
    }
}
static VTABLE: Vtable = Vtable {
    method: Some(method),
    property: Some(property),
    set: Some(set_property),
    padding: [ptr::null_mut(); 8],
};
pub(super) struct Bridge {
    api: &'static Api,
    connection: P,
    context: P,
    info: P,
    tree: Tree,
    bus: String,
    objects: BTreeMap<String, Vec<u32>>,
    queue: Arc<Mutex<VecDeque<Event>>>,
    application_id: Arc<Mutex<i32>>,
}
impl Bridge {
    /// Absence of an accessibility session is normal for headless and minimal desktops.
    pub fn start(tree: Tree) -> io::Result<Option<Self>> {
        let Some(a) = API.get_or_init(|| unsafe { Api::load() }).as_ref() else {
            return Ok(None);
        };
        unsafe {
            let mut error = ptr::null_mut();
            let session = (a.g_bus_get_sync)(2, ptr::null_mut(), &mut error);
            if session.is_null() {
                if !error.is_null() {
                    (a.g_error_free)(error);
                }
                return Ok(None);
            }
            let reply = (a.g_dbus_connection_call_sync)(
                session,
                c"org.a11y.Bus".as_ptr(),
                c"/org/a11y/bus".as_ptr(),
                c"org.a11y.Bus".as_ptr(),
                c"GetAddress".as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                1000,
                ptr::null_mut(),
                &mut error,
            );
            (a.g_object_unref)(session);
            if reply.is_null() {
                if !error.is_null() {
                    (a.g_error_free)(error);
                }
                return Ok(None);
            }
            let child = (a.g_variant_get_child_value)(reply, 0);
            let address =
                CStr::from_ptr((a.g_variant_get_string)(child, ptr::null_mut())).to_owned();
            (a.g_variant_unref)(child);
            (a.g_variant_unref)(reply);
            let context = (a.g_main_context_new)();
            (a.g_main_context_push_thread_default)(context);
            let connection = (a.g_dbus_connection_new_for_address_sync)(
                address.as_ptr(),
                9,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut error,
            );
            (a.g_main_context_pop_thread_default)(context);
            if connection.is_null() {
                (a.g_main_context_unref)(context);
                if !error.is_null() {
                    (a.g_error_free)(error);
                }
                return Ok(None);
            }
            let info = (a.g_dbus_node_info_new_for_xml)(cs(XML).as_ptr(), &mut error);
            if info.is_null() {
                (a.g_object_unref)(connection);
                (a.g_main_context_unref)(context);
                if !error.is_null() {
                    (a.g_error_free)(error);
                }
                return Err(io::Error::other("GuiAccessibilityUnavailable"));
            }
            let bus = CStr::from_ptr((a.g_dbus_connection_get_unique_name)(connection))
                .to_string_lossy()
                .into_owned();
            let mut bridge = Self {
                api: a,
                connection,
                context,
                info,
                tree,
                bus,
                objects: BTreeMap::new(),
                queue: Arc::default(),
                application_id: Arc::default(),
            };
            bridge.sync()?;
            let parameters = (a.g_variant_new_tuple)(&bridge.reference(ROOT), 1);
            let reply = (a.g_dbus_connection_call_sync)(
                connection,
                c"org.a11y.atspi.Registry".as_ptr(),
                c"/org/a11y/atspi/accessible/root".as_ptr(),
                c"org.a11y.atspi.Socket".as_ptr(),
                c"Embed".as_ptr(),
                parameters,
                ptr::null_mut(),
                0,
                1000,
                ptr::null_mut(),
                &mut error,
            );
            if !reply.is_null() {
                (a.g_variant_unref)(reply);
            } else if !error.is_null() {
                (a.g_error_free)(error);
                return Ok(None);
            }
            Ok(Some(bridge))
        }
    }
    fn reference(&self, p: &str) -> P {
        unsafe { (self.api.new)(c"(so)".as_ptr(), cs(&self.bus).as_ptr(), cs(p).as_ptr()) }
    }
    fn register(&mut self, p: String, id: Option<String>, application: bool) -> io::Result<()> {
        let names = if p == "/org/a11y/atspi/cache" {
            vec!["org.a11y.atspi.Cache"]
        } else if application {
            vec!["org.a11y.atspi.Accessible", "org.a11y.atspi.Application"]
        } else if id.is_some() {
            vec![
                "org.a11y.atspi.Accessible",
                "org.a11y.atspi.Action",
                "org.a11y.atspi.Text",
            ]
        } else {
            vec!["org.a11y.atspi.Accessible"]
        };
        let mut registrations = vec![];
        unsafe {
            (self.api.g_main_context_push_thread_default)(self.context);
        }
        for name in names {
            let callback = Box::new(Callback {
                tree: self.tree.clone(),
                id: id.clone(),
                application,
                bus: self.bus.clone(),
                queue: self.queue.clone(),
                application_id: self.application_id.clone(),
            });
            let data = Box::into_raw(callback).cast();
            let mut error = ptr::null_mut();
            let n = unsafe {
                (self.api.g_dbus_connection_register_object)(
                    self.connection,
                    cs(&p).as_ptr(),
                    (self.api.g_dbus_node_info_lookup_interface)(self.info, cs(name).as_ptr()),
                    &VTABLE,
                    data,
                    Some(destroy),
                    &mut error,
                )
            };
            if n == 0 {
                unsafe {
                    destroy(data);
                    if !error.is_null() {
                        (self.api.g_error_free)(error);
                    }
                    for n in registrations {
                        (self.api.g_dbus_connection_unregister_object)(self.connection, n);
                    }
                    (self.api.g_main_context_pop_thread_default)(self.context);
                }
                return Err(io::Error::other("GuiAccessibilityUnavailable"));
            }
            registrations.push(n);
        }
        unsafe {
            (self.api.g_main_context_pop_thread_default)(self.context);
        }
        self.objects.insert(p, registrations);
        Ok(())
    }
    #[cfg(test)]
    pub fn path_count(&self) -> usize {
        self.objects.len()
    }
    #[cfg(test)]
    pub fn bus_name(&self) -> &str {
        &self.bus
    }
    pub fn needs_rebind(&self) -> bool {
        let new = self
            .tree
            .children()
            .iter()
            .filter(|id| !self.objects.contains_key(&path(id)))
            .count();
        self.objects.len().saturating_add(new) > 4099
    }
    pub fn sync(&mut self) -> io::Result<()> {
        let mut desired = BTreeMap::from([
            (ROOT.into(), (None, true)),
            (WINDOW.into(), (None, false)),
            ("/org/a11y/atspi/cache".into(), (None, false)),
        ]);
        for id in self.tree.children() {
            desired.insert(path(&id), (Some(id), false));
        }
        // Keep vanished paths defunct until the connection is closed. GDBus
        // may already have queued a property callback against their vtable.
        for (p, (id, application)) in desired {
            if !self.objects.contains_key(&p) {
                self.register(p, id, application)?;
            }
        }
        Ok(())
    }
    fn emit(&self, path: &str, signal: &CStr, detail: &str, number: i32, value: P) {
        unsafe {
            let empty = array(c"{sv}", &[]);
            let args = (self.api.new)(
                c"(siiv@a{sv})".as_ptr(),
                cs(detail).as_ptr(),
                number,
                0i32,
                value,
                empty,
            );
            let mut error = ptr::null_mut();
            (self.api.g_dbus_connection_emit_signal)(
                self.connection,
                ptr::null(),
                cs(path).as_ptr(),
                c"org.a11y.atspi.Event.Object".as_ptr(),
                signal.as_ptr(),
                args,
                &mut error,
            );
            if !error.is_null() {
                (self.api.g_error_free)(error);
            }
        }
    }
    pub fn changed(&self, previous: Option<&Frame>, next: &Frame) {
        let old: BTreeMap<_, _> = previous
            .into_iter()
            .flat_map(|f| f.items.iter())
            .map(|i| (&i.id, i))
            .collect();
        let new: BTreeMap<_, _> = next.items.iter().map(|i| (&i.id, i)).collect();
        for (index, i) in previous
            .into_iter()
            .flat_map(|f| f.items.iter())
            .filter(|i| i.kind != "rect")
            .enumerate()
        {
            if !new.contains_key(&i.id) {
                self.emit(
                    WINDOW,
                    c"ChildrenChanged",
                    "remove",
                    index as i32,
                    self.reference(&path(&i.id)),
                );
            }
        }
        for (index, i) in next.items.iter().filter(|i| i.kind != "rect").enumerate() {
            let p = path(&i.id);
            if let Some(before) = old.get(&i.id) {
                if before.text != i.text {
                    self.emit(
                        &p,
                        c"PropertyChange",
                        if matches!(i.kind.as_str(), "textbox" | "textarea") {
                            "accessible-description"
                        } else {
                            "accessible-name"
                        },
                        0,
                        string(&i.text),
                    );
                }
                if before.enabled != i.enabled {
                    self.emit(&p, c"StateChanged", "enabled", i32::from(i.enabled), int(0));
                    self.emit(
                        &p,
                        c"StateChanged",
                        "sensitive",
                        i32::from(i.enabled),
                        int(0),
                    );
                }
                if before.checked != i.checked {
                    self.emit(&p, c"StateChanged", "checked", i32::from(i.checked), int(0));
                }
                if before.focused != i.focused {
                    self.emit(
                        &p,
                        c"StateChanged",
                        "focused",
                        i32::from(self.tree.node(Some(&i.id)).is_some_and(|n| n.focused)),
                        int(0),
                    );
                }
            } else {
                self.emit(
                    WINDOW,
                    c"ChildrenChanged",
                    "add",
                    index as i32,
                    self.reference(&p),
                );
            }
        }
        if previous.is_none_or(|f| f.title != next.title) {
            self.emit(
                WINDOW,
                c"PropertyChange",
                "accessible-name",
                0,
                string(&next.title),
            );
        }
    }
    pub fn pump(&mut self) -> Option<Event> {
        unsafe {
            for _ in 0..64 {
                if (self.api.g_main_context_iteration)(self.context, 0) == 0 {
                    break;
                }
            }
        }
        self.queue.lock().unwrap().pop_front()
    }
    /// Include GLib's wakeup and transport descriptors in the blocking X11 wait.
    pub fn wait(&self, xfd: c_int) -> io::Result<()> {
        unsafe {
            if (self.api.g_main_context_acquire)(self.context) == 0 {
                return Err(io::Error::other("GuiAccessibilityThread"));
            }
            let mut priority = 0;
            let mut timeout = -1;
            (self.api.g_main_context_prepare)(self.context, &mut priority);
            let count = (self.api.g_main_context_query)(
                self.context,
                priority,
                &mut timeout,
                ptr::null_mut(),
                0,
            );
            if !(0..=4096).contains(&count) {
                (self.api.g_main_context_release)(self.context);
                return Err(io::Error::other("GuiAccessibilityQueue"));
            }
            let mut fds = vec![
                PollFd {
                    fd: 0,
                    events: 0,
                    revents: 0
                };
                count as usize
            ];
            let actual = (self.api.g_main_context_query)(
                self.context,
                priority,
                &mut timeout,
                fds.as_mut_ptr(),
                count,
            );
            (self.api.g_main_context_release)(self.context);
            if actual > count {
                return Ok(());
            }
            fds.truncate(actual.max(0) as usize);
            fds.push(PollFd {
                fd: xfd,
                events: libc::POLLIN as u16,
                revents: 0,
            });
            if libc::poll(fds.as_mut_ptr().cast(), fds.len() as libc::nfds_t, timeout) < 0
                && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        // Surface owns the published-tree lifetime, including connection rebinding.
        self.queue.lock().unwrap().clear();
        unsafe {
            // Stop incoming requests before retiring interface vtables. GLib can
            // already have queued Properties.Set callbacks for Application.Id.
            let mut error = ptr::null_mut();
            (self.api.g_dbus_connection_close_sync)(self.connection, ptr::null_mut(), &mut error);
            if !error.is_null() {
                (self.api.g_error_free)(error);
            }
            for _ in 0..64 {
                if (self.api.g_main_context_iteration)(self.context, 0) == 0 {
                    break;
                }
            }
            for registrations in self.objects.values() {
                for n in registrations {
                    (self.api.g_dbus_connection_unregister_object)(self.connection, *n);
                }
            }
            // Do not dispatch property callbacks after unregistering their vtable.
            (self.api.g_object_unref)(self.connection);
            (self.api.g_dbus_node_info_unref)(self.info);
            (self.api.g_main_context_unref)(self.context);
        }
    }
}
