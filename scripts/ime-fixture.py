#!/usr/bin/env python3
"""Run real IBus/Anthy acceptance on a private X11 display and session bus."""
import ast
import ctypes.util
import os
from pathlib import Path
import select
import shutil
import subprocess
import sys
import tempfile
import time


def executable(prefix, name, candidates):
    for path in candidates:
        candidate=prefix/path
        if candidate.is_file():return str(candidate)
    found=shutil.which(name)
    if found:return found
    raise RuntimeError('IME fixture needs '+name)


def main():
    if sys.platform=='win32':raise SystemExit('This real IBus fixture runs on Linux')
    args=sys.argv[1:]
    if args and args[0]=='--':args=args[1:]
    if not args:raise SystemExit('usage: ime-fixture.py -- COMMAND [ARG ...]')
    if os.environ.get('REWIND_IME_PRIVATE_BUS')!='1':
        with tempfile.TemporaryDirectory(prefix='rewind-ime-bus-') as temporary:
            root=Path(temporary)
            for name in ['runtime','cache']:(root/name).mkdir(mode=0o700)
            environment=dict(os.environ,REWIND_IME_PRIVATE_BUS='1',XDG_RUNTIME_DIR=str(root/'runtime'),XDG_CACHE_HOME=str(root/'cache'))
            # Services activated by this bus must discover their own AT-SPI bus,
            # rather than the outer SDK session or its X11 root property. The
            # child below starts and selects its dedicated Xvfb display.
            for name in ['AT_SPI_BUS_ADDRESS','REWIND_GUI_PRIVATE_SESSION','DISPLAY']:environment.pop(name,None)
            return subprocess.call(['dbus-run-session','--',sys.executable,str(Path(__file__).resolve()),'--',*args],env=environment)
    prefix=Path(os.environ.get('REWIND_TEST_IME_ROOT','/'))
    xserver=os.environ.get('REWIND_GUI_TEST_XVFB') or shutil.which('Xvfb')
    if not xserver:raise RuntimeError('IME fixture needs Xvfb')
    if not ctypes.util.find_library('Xtst'):raise RuntimeError('IME fixture needs libXtst')
    daemon=executable(prefix,'ibus-daemon',['usr/bin/ibus-daemon'])
    xim=executable(prefix,'ibus-x11',['usr/libexec/ibus-x11','usr/lib/ibus/ibus-x11'])
    compiler=executable(prefix,'glib-compile-schemas',['usr/bin/glib-compile-schemas'])
    children=[]
    with tempfile.TemporaryDirectory(prefix='rewind-real-ime-') as temporary:
        root=Path(temporary)
        try:
            environment=dict(os.environ,REWIND_TEST_IME_SERVICE='1',XMODIFIERS='@im=ibus',GSETTINGS_BACKEND='memory')
            for name in ['runtime','config','cache','components']:
                (root/name).mkdir(mode=0o700)
            environment.update(XDG_RUNTIME_DIR=str(root/'runtime'),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),IBUS_COMPONENT_PATH=str(root/'components'),IBUS_ADDRESS='unix:path='+str(root/'ibus.socket'))
            schemas=root/'schemas';schemas.mkdir()
            for name in ['org.freedesktop.ibus.gschema.xml','org.freedesktop.ibus.engine.anthy.gschema.xml']:
                text=(prefix/'usr/share/glib-2.0/schemas'/name).read_text()
                key='enable-by-default' if 'engine.anthy' not in name else 'input-mode'
                begin=text.index('<key name="'+key+'"');end=text.index('</key>',begin)
                default='true' if key=='enable-by-default' else '0'
                region=text[begin:end];a=region.index('<default>')+len('<default>');b=region.index('</default>',a)
                text=text[:begin]+region[:a]+default+region[b:]+text[end:]
                (schemas/name).write_text(text)
            subprocess.run([compiler,str(schemas)],check=True)
            environment['GSETTINGS_SCHEMA_DIR']=str(schemas)
            # Installed engine source is copied only into this disposable test fixture.
            # Its learning/configuration files are kept away from user directories.
            engine=root/'engine';shutil.copytree(prefix/'usr/share/ibus-anthy/engine',engine)
            config=engine/'_config.py';config.write_text(config.read_text()+f'\nPKGDATADIR={str(prefix/"usr/share/ibus-anthy")!r}\n')
            code=engine/'engine.py';text=code.read_text();lines=text.splitlines(keepends=True)
            for node in ast.walk(ast.parse(text)):
                if isinstance(node,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='ANTHY_CONFIG_PATH' for t in node.targets):
                    lines[node.lineno-1:node.end_lineno]=['ANTHY_CONFIG_PATH = '+repr(str(root/'anthy'))+'\n'];break
            else:raise RuntimeError('Unsupported Anthy configuration-path layout')
            code.write_text(''.join(lines))
            if prefix!=Path('/'):
                environment['GI_TYPELIB_PATH']=':'.join(str(prefix/p) for p in ['usr/lib/girepository-1.0','usr/lib/x86_64-linux-gnu/girepository-1.0'])
                environment['LD_LIBRARY_PATH']=str(prefix/'usr/lib/x86_64-linux-gnu')
                entry=engine/'main.py';text=entry.read_text();needle='\nIBus.init()\n';injection=f"\nimport ctypes\n_native_anthy=ctypes.CDLL('libanthy.so.1')\n_native_anthy.anthy_conf_override.argtypes=[ctypes.c_char_p,ctypes.c_char_p]\n_native_anthy.anthy_conf_override(b'CONFFILE',{str(prefix/'etc/anthy/anthy-conf').encode()!r})\n"
                if needle not in text:raise RuntimeError('Unsupported Anthy startup layout')
                entry.write_text(text.replace(needle,injection+needle,1))
            read_fd,write_fd=os.pipe()
            try:
                command=[xserver,'-displayfd',str(write_fd),'-screen','0','1024x768x24','-nolisten','tcp']
                if os.environ.get('REWIND_GUI_TEST_FONTS'):command+=['-fp',os.environ['REWIND_GUI_TEST_FONTS']]
                with (root/'display.log').open('w') as log:
                    children.append(subprocess.Popen(command,pass_fds=(write_fd,),stdout=log,stderr=subprocess.STDOUT))
                os.close(write_fd);write_fd=-1
                if not select.select([read_fd],[],[],10)[0]:raise RuntimeError('IME fixture display did not start')
                display=os.read(read_fd,64).decode().strip()
                if not display.isdigit():raise RuntimeError('IME fixture display failed')
                environment['DISPLAY']=':'+display
            finally:
                os.close(read_fd)
                if write_fd>=0:os.close(write_fd)
            for name,command in [('ibus',[daemon,'--single','--address',environment['IBUS_ADDRESS'],'--cache','none']),('xim',[xim]),('anthy',['/usr/bin/python3',str(engine/'main.py')])]:
                with (root/(name+'.log')).open('w') as log:
                    children.append(subprocess.Popen(command,env=environment,stdout=log,stderr=subprocess.STDOUT))
                # Bounded startup preparation, excluded from input latency samples.
                time.sleep(.6)
                if children[-1].poll() is not None:raise RuntimeError(name+' startup failed: '+(root/(name+'.log')).read_text()[-4096:])
            selector="import gi;gi.require_version('IBus','1.0');from gi.repository import IBus;IBus.init();b=IBus.Bus();b.set_global_engine('anthy');e=b.get_global_engine();assert e and e.get_name()=='anthy'"
            subprocess.run(['/usr/bin/python3','-c',selector],env=environment,check=True,timeout=10)
            result=subprocess.call(args,env=environment)
            if result:
                for name in ['ibus','xim','anthy']:
                    detail=(root/(name+'.log')).read_text()[-4096:]
                    if detail:print(name+': '+detail,file=sys.stderr)
            return result
        finally:
            for child in reversed(children):
                if child.poll() is None:child.terminate()
            for child in children:
                try:child.wait(timeout=5)
                except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)


if __name__=='__main__':raise SystemExit(main())
