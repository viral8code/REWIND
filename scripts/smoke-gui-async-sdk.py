#!/usr/bin/env python3
"""Actual native GUI input during a pending HTTP request, SQLite undo boundary, offline replay."""
import ctypes as C
from contextlib import closing
import ctypes.util
from http.server import BaseHTTPRequestHandler, HTTPServer
import os
import json
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import threading
import time

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
live_mode = "--live" in sys.argv[3:]
source_args = [value for value in sys.argv[3:] if value != "--live"]
source = Path(source_args[0]).resolve() if source_args else binary.parent.parent / "share/rewind/examples/gui-async/main.rw"
if sys.platform != "win32" and not os.environ.get("DISPLAY"):
    raise SystemExit("Native GUI integration requires DISPLAY; run under Xvfb on Linux")
root.mkdir()
program = root / "main.rw"
program_text = source.read_text(encoding="utf-8")
if live_mode:
    program_text = program_text.replace("external fresh {", "external live {").replace("external {", "external live {").replace("windows.nextEventAnyAsync()", "liveInput()")
    program_text = program_text.replace("let arguments=Args.all();", """fn liveInput()->Task<Result<windows.WindowEvent,windows.GuiError>> effects {gui,tasks,external,live} {
 var waiting:Option<Task<Result<windows.WindowEvent,windows.GuiError>>>=None;
 external live {waiting=Some(windows.nextEventAnyLiveAsync());}
 match waiting {Some(task)=>{return move task;},None=>{panic("missing live wait");}}
}
let arguments=Args.all();""")
program.write_text(program_text, encoding="utf-8")
effects = "gui,external,network,db,tasks" + (",live" if live_mode else "")
subprocess.run([str(binary), "compile", str(program), "--allow-effects", effects], cwd=root, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
program.unlink()
shutil.rmtree(root / ".rewind", ignore_errors=True)


def click(title, timeout=10):
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
                assert api.PostMessageW(window, 0x201, 1, 20 | (20 << 16))
                assert api.PostMessageW(window, 0x202, 0, 20 | (20 << 16))
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
                    event=Event();event.button=Button(4,0,1,display,window,root_window,0,0,20,20,20,20,0,1,1)
                    assert x.XSendEvent(display,window,0,1<<2,C.byref(event))
                    x.XFlush(display)
                    return
                time.sleep(.01)
        finally: x.XCloseDisplay(display)
    raise RuntimeError("Native window did not appear")


for mode in (["live"] if live_mode else ["debug", "compact"]):
    arrived=threading.Event();release=threading.Event();received=[]
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            received.append(self.path);arrived.set()
            release.wait(15)
            try:
                self.send_response(200);self.send_header("Content-Length","2");self.end_headers();self.wfile.write(b"ok")
            except (BrokenPipeError,ConnectionResetError,OSError): pass
        def log_message(self,*args): pass
    server=HTTPServer(("127.0.0.1",0),Handler)
    worker=threading.Thread(target=server.handle_request,daemon=True);worker.start()
    title=f"REWIND async SDK {root.name} {mode}"
    url=f"http://127.0.0.1:{server.server_port}/pending"
    trace=root / f"{mode}.json"
    recording=[] if live_mode else ["--record",str(trace),"--record-mode",mode]
    process=subprocess.Popen([str(binary),"profile" if live_mode else "run",str(root/"main.rwc"),"--allow-effects",effects,*recording,"--",url,title],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        assert arrived.wait(10), "HTTP driver did not submit while GUI was published"
        assert process.poll() is None, "Application ended before native input"
        click(title)
        output,error=process.communicate(timeout=15)
        assert process.returncode==0,error.decode("utf-8",errors="replace")
        assert output==b"cancelled\n",output
        assert received==["/pending"],received
        with closing(sqlite3.connect(root/"state.sqlite")) as db:
            assert db.execute("SELECT text FROM notes").fetchall()==[("日本語",)]
    finally:
        if process.poll() is None: process.kill();process.communicate()
        release.set();worker.join(timeout=5);server.server_close()
    assert not worker.is_alive()
    (root/"state.sqlite").unlink()
    if live_mode:
        profile=next(json.loads(line) for line in reversed(error.decode().splitlines()) if line.startswith('{'))
        state=profile['runtime']['external_live']
        assert state['retained_operations']==state['recorded_operations']==state['recorded_polls']==0,state
        assert state['native_resources']==0,state
        assert profile['runtime'].get('gui_windows',{}).get('observed',0)==0,profile
        continue
    # The server and database are gone; replay virtualizes GUI. Linux also lacks DISPLAY.
    replay_env=os.environ.copy();replay_env.pop("DISPLAY",None)
    replay=subprocess.run([str(binary),"replay",str(trace),"--root",str(root),"--allow-effects",effects],cwd=root,env=replay_env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20)
    assert replay.returncode==0,replay.stderr.decode("utf-8",errors="replace")
    assert replay.stdout==output
    assert not (root/"state.sqlite").exists()
    assert received==["/pending"]
print("Verified native live GUI / HTTP / SQLite: source-free real click cancellation, VM undo boundary and released observations" if live_mode else "Verified native GUI while HTTP waits: real input cancellation, parameterized SQLite write surviving revert, cleanup, source-free and disconnected debug/compact replay")
