#!/usr/bin/env python3
"""Isolate Linux GUI service sockets and explicitly select the private AT-SPI bus."""
import ast
import os
from pathlib import Path
import subprocess
import shutil
import sys
import tempfile

args=sys.argv[1:]
if args and args[0]=='--':args=args[1:]
if not args:raise SystemExit('usage: gui-session-fixture.py -- COMMAND [ARG ...]')
if sys.platform=='win32':raise SystemExit('This fixture is for Linux native GUI acceptance')
if os.environ.get('REWIND_GUI_PRIVATE_SESSION')!='1':
    with tempfile.TemporaryDirectory(prefix='rewind-gui-services-') as temporary:
        root=Path(temporary)
        for name in ['runtime','cache']:(root/name).mkdir(mode=0o700)
        env=dict(os.environ,REWIND_GUI_PRIVATE_SESSION='1',XDG_RUNTIME_DIR=str(root/'runtime'),XDG_CACHE_HOME=str(root/'cache'),GSETTINGS_BACKEND='memory')
        env.pop('AT_SPI_BUS_ADDRESS',None)
        result=subprocess.call(['dbus-run-session','--',sys.executable,str(Path(__file__).resolve()),'--',*args],env=env)
    raise SystemExit(result)
gdbus=shutil.which('gdbus') or str(Path(os.environ.get('REWIND_TEST_IME_ROOT','/'))/'usr/bin/gdbus')
address=subprocess.run([gdbus,'call','--session','--dest','org.a11y.Bus','--object-path','/org/a11y/bus','--method','org.a11y.Bus.GetAddress'],capture_output=True,text=True,timeout=10,check=True)
reply=ast.literal_eval(address.stdout)
if not isinstance(reply,tuple) or len(reply)!=1 or not isinstance(reply[0],str) or len(reply[0])>4096:raise RuntimeError('Unexpected private accessibility service address')
env=dict(os.environ,AT_SPI_BUS_ADDRESS=reply[0])
raise SystemExit(subprocess.call(args,env=env))
