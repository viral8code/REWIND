"""Send native pointer input to a test surface on Win32 or X11."""
import ctypes as C
import ctypes.util
import sys
import time

def ctrl_key(title, key, timeout=10):
    if key not in ('C', 'V', 'X'):
        raise ValueError('Unsupported clipboard test key')
    click(title, timeout=timeout, _key=key)

def command_key(title, key, timeout=10):
    parts=key.split('+');base=parts[-1];modifiers=tuple(parts[:-1])
    if any(m not in ('Ctrl','Alt','Shift') for m in modifiers) or len(set(modifiers))!=len(modifiers):raise ValueError('Invalid command modifiers')
    if not (len(base)==1 and 'A'<=base<='Z') and not (base.startswith('F') and base[1:].isdigit() and 1<=int(base[1:])<=24):raise ValueError('Invalid command key')
    click(title,timeout=timeout,_key=base,_modifiers=modifiers)

def dialog_response(title, accept, timeout=10):
    if sys.platform=="win32":
        api=C.WinDLL("user32",use_last_error=True)
        api.FindWindowW.argtypes=[C.c_wchar_p,C.c_wchar_p];api.FindWindowW.restype=C.c_void_p
        api.PostMessageW.argtypes=[C.c_void_p,C.c_uint,C.c_size_t,C.c_ssize_t];api.PostMessageW.restype=C.c_int
        api.GetDlgItem.argtypes=[C.c_void_p,C.c_int];api.GetDlgItem.restype=C.c_void_p
        api.IsWindowEnabled.argtypes=[C.c_void_p];api.IsWindowEnabled.restype=C.c_int
        api.IsWindowVisible.argtypes=[C.c_void_p];api.IsWindowVisible.restype=C.c_int
        deadline=time.monotonic()+timeout
        ready_since=None
        while time.monotonic()<deadline:
            window=api.FindWindowW(None,title)
            button=api.GetDlgItem(window,1 if accept else 2) if window else None
            if button and api.IsWindowEnabled(button) and api.IsWindowVisible(window):
                if ready_since is None:ready_since=time.monotonic()
                if time.monotonic()-ready_since>=.25:
                    if not api.PostMessageW(button,0xf5,0,0):raise C.WinError(C.get_last_error())
                    return
            else:ready_since=None
            time.sleep(.01)
        raise RuntimeError("Native dialog did not appear")
    click(title,timeout=timeout,_key="Enter" if accept else "Escape",_modifiers=(),_focus=True,_physical=True)

def click(title, px=20, py=20, timeout=10, _key=None, _modifiers=('Ctrl',),_focus=False,_physical=False):
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
                    # Foreground activation can be denied on a hosted desktop.
                    # Join the target input queue and deliver a bounded native
                    # key message with its Ctrl state, without global key events.
                    kernel = C.WinDLL('kernel32', use_last_error=True)
                    kernel.GetCurrentThreadId.restype = C.c_uint
                    api.GetWindowThreadProcessId.argtypes = [C.c_void_p, C.POINTER(C.c_uint)]
                    api.GetWindowThreadProcessId.restype = C.c_uint
                    api.AttachThreadInput.argtypes = [C.c_uint, C.c_uint, C.c_int]
                    api.AttachThreadInput.restype = C.c_int
                    api.GetKeyboardState.argtypes = [C.POINTER(C.c_ubyte)]
                    api.SetKeyboardState.argtypes = [C.POINTER(C.c_ubyte)]
                    api.SendMessageTimeoutW.argtypes = [C.c_void_p, C.c_uint, C.c_size_t,
                                                       C.c_ssize_t, C.c_uint, C.c_uint,
                                                       C.POINTER(C.c_size_t)]
                    api.SendMessageTimeoutW.restype = C.c_ssize_t
                    api.PeekMessageW.argtypes = [C.c_void_p, C.c_void_p, C.c_uint, C.c_uint, C.c_uint]
                    message = C.create_string_buffer(64)
                    api.PeekMessageW(message, None, 0, 0, 0)
                    sender = kernel.GetCurrentThreadId()
                    target = api.GetWindowThreadProcessId(window, None)
                    attached = sender != target
                    if attached and not api.AttachThreadInput(sender, target, 1):
                        raise C.WinError(C.get_last_error())
                    saved = (C.c_ubyte * 256)()
                    try:
                        if not api.GetKeyboardState(saved):
                            raise C.WinError(C.get_last_error())
                        pressed = (C.c_ubyte * 256)(*saved)
                        for code,name in [(0x11,'Ctrl'),(0x12,'Alt'),(0x10,'Shift')]:pressed[code]=0x80 if name in _modifiers else 0
                        if not api.SetKeyboardState(pressed):
                            raise C.WinError(C.get_last_error())
                        result = C.c_size_t()
                        virtual={"Enter":0x0d,"Escape":0x1b}.get(_key)
                        if virtual is None:virtual=ord(_key) if len(_key)==1 else 0x70+int(_key[1:])-1
                        if not api.SendMessageTimeoutW(window, 0x104 if 'Alt' in _modifiers else 0x100, virtual, 0, 3, 5000, C.byref(result)):
                            raise C.WinError(C.get_last_error())
                    finally:
                        api.SetKeyboardState(saved)
                        if attached:
                            api.AttachThreadInput(sender, target, 0)
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
        x.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong]
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
                    if _focus:x.XSetInputFocus(display,window,2,0)
                    event=Event();event.button=Button(4,0,1,display,window,root_window,0,0,px,py,px,py,0,1,1)
                    if _key:
                        event.button.type = 2
                        event.button.state = sum(mask for name,mask in [('Ctrl',4),('Alt',8),('Shift',1)] if name in _modifiers)
                        symbol={"Enter":0xff0d,"Escape":0xff1b}.get(_key)
                        if symbol is None:symbol=ord(_key.lower()) if len(_key)==1 else 0xffbe+int(_key[1:])-1
                        event.button.button = x.XKeysymToKeycode(display, symbol)
                    if _physical:
                        xt=C.CDLL(ctypes.util.find_library("Xtst"))
                        xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong];xt.XTestFakeKeyEvent.restype=C.c_int
                        assert xt.XTestFakeKeyEvent(display,event.button.button,1,0)
                        assert xt.XTestFakeKeyEvent(display,event.button.button,0,0)
                    else:
                        assert x.XSendEvent(display,window,0,1 if _key else 1<<2,C.byref(event))
                    x.XFlush(display)
                    return
                time.sleep(.01)
        finally: x.XCloseDisplay(display)
    raise RuntimeError("Native window did not appear")

