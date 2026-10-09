"""Operate the actual platform accessibility interface of a named native window."""
import ctypes as C
import sys
import time


def activate(title, caption, timeout=10):
    if sys.platform == 'win32':
        return _windows_activate(title, caption, timeout)
    from gi.repository import Gio, GLib
    session=Gio.bus_get_sync(Gio.BusType.SESSION,None)
    address=session.call_sync('org.a11y.Bus','/org/a11y/bus','org.a11y.Bus','GetAddress',None,None,0,5000,None).unpack()[0]
    connection=Gio.DBusConnection.new_for_address_sync(address,Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
    def call(bus,path,interface,method,args=None):
        return connection.call_sync(bus,path,interface,method,args,None,0,5000,None).unpack()
    def name(bus,path):
        return call(bus,path,'org.freedesktop.DBus.Properties','Get',GLib.Variant('(ss)',('org.a11y.atspi.Accessible','Name')))[0]
    deadline=time.monotonic()+timeout
    try:
        while time.monotonic()<deadline:
            apps=call('org.a11y.atspi.Registry','/org/a11y/atspi/accessible/root','org.a11y.atspi.Accessible','GetChildren')[0]
            for bus,path in apps:
                windows=call(bus,path,'org.a11y.atspi.Accessible','GetChildren')[0]
                for window_bus,window_path in windows:
                    if name(window_bus,window_path)!=title:continue
                    controls=call(window_bus,window_path,'org.a11y.atspi.Accessible','GetChildren')[0]
                    for control_bus,control_path in controls:
                        if name(control_bus,control_path)==caption:
                            role=call(control_bus,control_path,'org.a11y.atspi.Accessible','GetRole')[0]
                            assert role==43,role
                            accepted=call(control_bus,control_path,'org.a11y.atspi.Action','DoAction',GLib.Variant('(i)',(0,)))[0]
                            if not accepted:raise RuntimeError('Accessible action was rejected')
                            return
            time.sleep(.005)
        raise RuntimeError('Accessible window or control did not appear')
    finally:
        connection.close_sync(None)


def _windows_activate(title, caption, timeout):
    class Guid(C.Structure):
        _fields_=[('a',C.c_uint32),('b',C.c_uint16),('c',C.c_uint16),('d',C.c_ubyte*8)]
    class Data(C.Union):
        _fields_=[('number',C.c_int32),('pointer',C.c_void_p),('pair',C.c_size_t*2)]
    class Variant(C.Structure):
        _fields_=[('kind',C.c_uint16),('a',C.c_uint16),('b',C.c_uint16),('c',C.c_uint16),('data',Data)]
    assert C.sizeof(Variant)==(24 if C.sizeof(C.c_void_p)==8 else 16)
    user=C.WinDLL('user32',use_last_error=True)
    user.FindWindowW.argtypes=[C.c_wchar_p,C.c_wchar_p];user.FindWindowW.restype=C.c_void_p
    ole=C.WinDLL('ole32');ole.CoInitializeEx.argtypes=[C.c_void_p,C.c_uint32];ole.CoInitializeEx.restype=C.c_int32
    result=ole.CoInitializeEx(None,2)
    if result<0:raise RuntimeError('Accessibility client apartment initialization failed')
    acc=C.WinDLL('oleacc');acc.AccessibleObjectFromWindow.argtypes=[C.c_void_p,C.c_uint32,C.POINTER(Guid),C.POINTER(C.c_void_p)];acc.AccessibleObjectFromWindow.restype=C.c_int32
    strings=C.WinDLL('oleaut32');strings.SysStringLen.argtypes=[C.c_void_p];strings.SysStringLen.restype=C.c_uint32;strings.SysFreeString.argtypes=[C.c_void_p]
    iid=Guid(0x618736e0,0x3c3d,0x11cf,(C.c_ubyte*8)(0x81,0x0c,0,0xaa,0,0x38,0x9b,0x71))
    pointer=C.c_void_p()
    def method(index,*args):
        vtable=C.cast(pointer,C.POINTER(C.POINTER(C.c_void_p))).contents
        return C.WINFUNCTYPE(C.c_int32,C.c_void_p,*args)(vtable[index])
    def variant(index):
        value=Variant();value.kind=3;value.data.number=index;return value
    deadline=time.monotonic()+timeout
    try:
        while time.monotonic()<deadline:
            window=user.FindWindowW(None,title)
            if window and acc.AccessibleObjectFromWindow(window,0xfffffffc,C.byref(iid),C.byref(pointer))==0 and pointer.value:break
            time.sleep(.005)
        else:raise RuntimeError('Accessible window did not appear')
        count=C.c_int32();assert method(8,C.POINTER(C.c_int32))(pointer,C.byref(count))==0
        for index in range(1,count.value+1):
            text=C.c_void_p();assert method(10,Variant,C.POINTER(C.c_void_p))(pointer,variant(index),C.byref(text))==0
            try:name=C.wstring_at(text,strings.SysStringLen(text)) if text.value else ''
            finally:strings.SysFreeString(text)
            if name!=caption:continue
            role=Variant();assert method(13,Variant,C.POINTER(Variant))(pointer,variant(index),C.byref(role))==0
            assert role.kind==3 and role.data.number==43
            assert method(25,Variant)(pointer,variant(index))==0
            return
        raise RuntimeError('Accessible control did not appear')
    finally:
        if pointer.value:method(2)(pointer)
        ole.CoUninitialize()
