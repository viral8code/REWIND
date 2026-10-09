#!/usr/bin/env python3
"""Actual platform accessibility input, VM restore, source-free and offline replay."""
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import threading
import time
# GI is a Linux fixture dependency, never part of the REWIND runtime.
if sys.platform!='win32':
    if os.environ.get('REWIND_GUI_PRIVATE_SESSION')!='1':
        # Use the same isolated cache/address setup as the SDK runner. Nesting
        # a bus while inheriting AT_SPI_BUS_ADDRESS points the registry daemon
        # at the outer bus instead of the bus that activated it.
        raise SystemExit(subprocess.call([sys.executable,str(Path(__file__).with_name('gui-session-fixture.py')),'--','/usr/bin/python3',str(Path(__file__).resolve()),*sys.argv[1:]]))
from native_accessibility_fixture import activate
binary=Path(sys.argv[1]).resolve();root=Path(sys.argv[2]).resolve()
source=Path(sys.argv[3]).resolve() if len(sys.argv)>3 else binary.parent.parent/'share/rewind/examples/gui-accessibility/main.rw'
root.mkdir()
for mode in ['debug','compact']:
    directory=root/mode;directory.mkdir();entry=directory/'main.rw';entry.write_text(source.read_text(encoding='utf-8'),encoding='utf-8')
    def call(*args,env=None):
        result=subprocess.run([str(binary),*map(str,args)],cwd=directory,env=env,capture_output=True,timeout=45)
        if result.returncode:raise RuntimeError(result.stderr.decode('utf-8',errors='replace'))
        return result
    call('compile',entry,'--allow-effects','gui');entry.unlink();shutil.rmtree(directory/'.rewind',ignore_errors=True)
    title='REWIND accessibility '+str(time.time_ns())
    process=subprocess.Popen([str(binary),'run','main.rwc','--allow-effects','gui','--record','trace.json','--record-mode',mode,'--',title],cwd=directory,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    output=queue.Queue();errors=[]
    def read():
        for line in process.stdout:output.put(line.decode('utf-8').rstrip('\r\n'))
        output.put(None)
    reader=threading.Thread(target=read,daemon=True);reader.start()
    error_reader=threading.Thread(target=lambda:errors.append(process.stderr.read()),daemon=True);error_reader.start()
    def expect(value):
        actual=output.get(timeout=15)
        if actual!=value:raise RuntimeError('Expected '+repr(value)+', received '+repr(actual))
    try:
        expect('ready');activate(title,'保存');expect('saved');expect('undo');activate(title,'保存');expect('saved')
        assert process.wait(timeout=15)==0
    finally:
        if process.poll() is None:process.kill();process.wait(timeout=5)
        reader.join(timeout=5);error_reader.join(timeout=5)
    assert not b''.join(errors),b''.join(errors).decode('utf-8',errors='replace')
    env=dict(os.environ);env.pop('DISPLAY',None);env.pop('DBUS_SESSION_BUS_ADDRESS',None)
    assert call('replay','trace.json','--allow-effects','gui',env=env).stdout.decode('utf-8').splitlines()==['ready','saved','undo','saved']
print('Verified source-free SDK: actual OS accessibility actions, VM undo and service-free replay')
