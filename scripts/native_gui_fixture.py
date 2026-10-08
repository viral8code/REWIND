"""Send native pointer input to a test surface on Win32 or X11."""
import ctypes as C
import ctypes.util
import sys
import time

def ctrl_key(title, key, timeout=10):
    if key not in ('C', 'V', 'X'):
        raise ValueError('Unsupported clipboard test key')
    click(title, timeout=timeout, _key=key)

def click(title, px=20, py=20, timeout=10, _key=None):
    deadline = time.monotonic() + timeout
    if sys.platform == "win32":
        api = C.WinDLL("user32", use_last_error=True)
        api.FindWindowW.argtypes = [C.c_wchar_p, C.c_wchar_p]
        api.FindWindowW.restype = C.c_void_p
        api.PostMessageW.argtypes = [C.c_void_p, C.c_uint, C.c_size_t, C.c_ssize_t]
        api.PostMessageW.restype = C.c_int
        while time.monotonic() < deadline:
            window = api.FindWindowW(None, title)
            if window:
                if _key:
                    api.SetForegroundWindow.argtypes = [C.c_void_p]
                    api.keybd_event.argtypes = [C.c_ubyte, C.c_ubyte, C.c_uint, C.c_size_t]
                    api.SetForegroundWindow(window)
                    api.keybd_event(0x11, 0, 0, 0)
                    api.keybd_event(ord(_key), 0, 0, 0)
                    time.sleep(.05)
                    api.keybd_event(ord(_key), 0, 2, 0)
                    api.keybd_event(0x11, 0, 2, 0)
                    return
                assert api.PostMessageW(window, 0x201, 1, px | (py << 16))
                assert api.PostMessageW(window, 0x202, 0, px | (py << 16))
                return
            time.sleep(.01)
    else:
        x = C.CDLL(ctypes.util.find_library("X11"))
        x.XOpenDisplay.argtypes = [C.c_char_p]; x.XOpenDisplay.restype = C.c_void_p
        x.XDefaultRootWindow.argtypes = [C.c_void_p]; x.XDefaultRootWindow.restype = C.c_ulong
        x.XQueryTree.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_ulong), C.POINTER(C.POINTER(C.c_ulong)), C.POINTER(C.c_uint)]
        x.XFetchName.argtypes = [C.c_void_p, C.c_ulong, C.POINTER(C.c_void_p)]
        x.XFree.argtypes = [C.c_void_p]
        x.XCloseDisplay.argtypes = [C.c_void_p]
        x.XFlush.argtypes = [C.c_void_p]
        x.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]
        x.XKeysymToKeycode.restype = C.c_ubyte
        class Button(C.Structure):
            _fields_ = [("type",C.c_int),("serial",C.c_ulong),("send_event",C.c_int),("display",C.c_void_p),("window",C.c_ulong),("root",C.c_ulong),("subwindow",C.c_ulong),("time",C.c_ulong),("x",C.c_int),("y",C.c_int),("x_root",C.c_int),("y_root",C.c_int),("state",C.c_uint),("button",C.c_uint),("same_screen",C.c_int)]
        class Event(C.Union):
            _fields_ = [("button",Button),("pad",C.c_long*24)]
        x.XSendEvent.argtypes = [C.c_void_p,C.c_ulong,C.c_int,C.c_long,C.POINTER(Event)]
        display = x.XOpenDisplay(None)
        if not display: raise RuntimeError("XOpenDisplay failed")
        root_window = x.XDefaultRootWindow(display)
        def find(window, depth=0):
            name = C.c_void_p()
            if x.XFetchName(display,window,C.byref(name)) and name.value:
                try:
                    if C.string_at(name).decode("utf-8",errors="replace") == title: return window
                finally: x.XFree(name)
            if depth >= 4: return None
            a=C.c_ulong();b=C.c_ulong();children=C.POINTER(C.c_ulong)();count=C.c_uint()
            if x.XQueryTree(display,window,C.byref(a),C.byref(b),C.byref(children),C.byref(count)):
                try:
                    for index in range(count.value):
                        result=find(children[index],depth+1)
                        if result: return result
                finally:
                    if children: x.XFree(children)
            return None
        try:
            while time.monotonic() < deadline:
                window=find(root_window)
                if window:
                    event=Event();event.button=Button(4,0,1,display,window,root_window,0,0,px,py,px,py,0,1,1)
                    if _key:
                        event.button.type = 2
                        event.button.state = 4
                        event.button.button = x.XKeysymToKeycode(display, ord(_key.lower()))
                    assert x.XSendEvent(display,window,0,1 if _key else 1<<2,C.byref(event))
                    x.XFlush(display)
                    return
                time.sleep(.01)
        finally: x.XCloseDisplay(display)
    raise RuntimeError("Native window did not appear")

