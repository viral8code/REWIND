#!/usr/bin/env python3
"""Committed IME input, VM undo and display-free source-free SDK replay."""
import ctypes as C
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import threading
import time
from native_gui_fixture import click

if sys.platform!='win32' and os.environ.get('REWIND_TEST_IME_SERVICE')!='1':
    raise SystemExit(subprocess.call([sys.executable,str(Path(__file__).with_name('ime-fixture.py')),'--',sys.executable,*sys.argv]))
binary=Path(sys.argv[1]).resolve();root=Path(sys.argv[2]).resolve()
source=Path(sys.argv[3]).resolve() if len(sys.argv)>3 else binary.parent.parent/'share/rewind/examples/gui-ime/main.rw'
root.mkdir()
for mode in ['debug','compact']:
    directory=root/mode;directory.mkdir();entry=directory/'main.rw';entry.write_text(source.read_text(encoding='utf-8'),encoding='utf-8')
    def call(*args,env=None):
        result=subprocess.run([str(binary),*map(str,args)],cwd=directory,env=env,capture_output=True,timeout=45)
        if result.returncode:raise RuntimeError(result.stderr.decode('utf-8',errors='replace'))
        return result
    call('compile',entry,'--allow-effects','gui');entry.unlink();shutil.rmtree(directory/'.rewind',ignore_errors=True)
    title='REWIND IME '+str(time.time_ns());trace=directory/'trace.json'
    p=subprocess.Popen([str(binary),'run','main.rwc','--allow-effects','gui','--record',str(trace),'--record-mode',mode,'--',title],cwd=directory,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    delivered=queue.Queue();output=[];errors=[]
    def stdout():
        for line in p.stdout:output.append(line);delivered.put(line.decode('utf-8').rstrip('\r\n'))
        delivered.put(None)
    def stderr():errors.append(p.stderr.read())
    readers=[threading.Thread(target=stdout,daemon=True),threading.Thread(target=stderr,daemon=True)]
    for thread in readers:thread.start()
    def expect(value):
        actual=delivered.get(timeout=15)
        if actual!=value:raise RuntimeError(f'Expected {value!r}, received {actual!r}: '+b''.join(errors).decode('utf-8',errors='replace'))
    def confirm(roman,text):
        if sys.platform=='win32':
            # Native committed Unicode messages exercise the SDK/VM boundary.
            # They do not emulate or prove a Japanese conversion engine.
            api=C.WinDLL('user32',use_last_error=True);api.FindWindowW.argtypes=[C.c_wchar_p,C.c_wchar_p];api.FindWindowW.restype=C.c_void_p
            api.PostMessageW.argtypes=[C.c_void_p,C.c_uint,C.c_size_t,C.c_ssize_t];api.PostMessageW.restype=C.c_int
            window=api.FindWindowW(None,title)
            if not window:raise RuntimeError('Native IME SDK window missing')
            units=text.encode('utf-16-le')
            for i in range(0,len(units),2):
                if not api.PostMessageW(window,0x102,int.from_bytes(units[i:i+2],'little'),0):raise C.WinError(C.get_last_error())
        else:
            for key in roman:
                click(title,_key=key,_modifiers=(),_focus=True,_physical=True);time.sleep(.1)
            # The native service has preedit, while no committed VM output is emitted.
            time.sleep(.1)
            assert delivered.empty(),'Preedit escaped before native confirmation'
            click(title,_key='Enter',_modifiers=(),_focus=True,_physical=True)
    try:
        expect('ready');confirm('ka','か');expect('か');expect('undo');confirm('ki','き');expect('き')
        assert p.wait(timeout=15)==0
    finally:
        if p.poll() is None:p.kill();p.wait(timeout=5)
        for thread in readers:thread.join(timeout=5)
    assert not b''.join(errors),b''.join(errors).decode('utf-8',errors='replace')
    env=dict(os.environ);env.pop('DISPLAY',None);env.pop('XMODIFIERS',None);env.pop('IBUS_ADDRESS',None)
    replay=call('replay',trace,'--allow-effects','gui',env=env);assert replay.stdout==b''.join(output)
print('Verified native committed Unicode/VM undo and source-free disconnected replay'+('; real Linux IBus/Anthy preedit' if sys.platform!='win32' else '; Windows conversion-engine acceptance is separate'))
