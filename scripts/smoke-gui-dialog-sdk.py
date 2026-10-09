#!/usr/bin/env python3
"""Native file selection, recorded rollback and display-free source-free replay."""
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import threading
import time
from native_gui_fixture import dialog_response
# GTK consumes real device key events. Isolate focus and those events from other fixtures.
if sys.platform!='win32' and os.environ.get('REWIND_DIALOG_FIXTURE_ISOLATED')!='1':
    import select
    import tempfile
    executable=os.environ.get('REWIND_GUI_TEST_XVFB') or shutil.which('Xvfb')
    if not executable:raise RuntimeError('Native dialog fixture requires Xvfb')
    read_fd,write_fd=os.pipe();server=None
    try:
        command=[executable,'-displayfd',str(write_fd),'-screen','0','1024x768x24','-nolisten','tcp']
        fonts=os.environ.get('REWIND_GUI_TEST_FONTS')
        if fonts:command+=['-fp',fonts]
        with tempfile.TemporaryFile() as errors:
            server=subprocess.Popen(command,pass_fds=(write_fd,),stdout=subprocess.DEVNULL,stderr=errors)
            os.close(write_fd);write_fd=-1
            if not select.select([read_fd],[],[],10)[0]:raise RuntimeError('Private display did not start')
            number=os.read(read_fd,64).decode('ascii').strip()
            if not number.isdigit():
                errors.seek(0);raise RuntimeError('Private display failed: '+errors.read(4096).decode('utf-8',errors='replace'))
            isolated=dict(os.environ,REWIND_DIALOG_FIXTURE_ISOLATED='1',DISPLAY=':'+number)
            result=subprocess.call([sys.executable,*sys.argv],env=isolated)
        raise SystemExit(result)
    finally:
        os.close(read_fd)
        if write_fd>=0:os.close(write_fd)
        if server is not None:
            server.terminate()
            try:server.wait(timeout=5)
            except subprocess.TimeoutExpired:server.kill();server.wait(timeout=5)

binary=Path(sys.argv[1]).resolve();root=Path(sys.argv[2]).resolve()
source=Path(sys.argv[3]).resolve() if len(sys.argv)>3 else binary.parent.parent/'share/rewind/examples/gui-dialog/main.rw'
root.mkdir();env=dict(os.environ,GSETTINGS_BACKEND='memory')
for mode in ['debug','compact']:
 for action,accepted in [('open',True),('save',True),('open',False)]:
  directory=root/(mode+'-'+action+'-'+str(accepted));directory.mkdir()
  entry=directory/'main.rw';entry.write_text(source.read_text(encoding='utf-8'),encoding='utf-8')
  path=directory/('destination-界.txt' if action=='save' else 'original-界.txt')
  if action=='open':path.write_text('unchanged',encoding='utf-8')
  def call(*args,call_env=env):
   result=subprocess.run([str(binary),*map(str,args)],cwd=directory,env=call_env,capture_output=True,timeout=45)
   if result.returncode:raise RuntimeError(result.stderr.decode('utf-8',errors='replace'))
   return result
  call('compile',entry,'--allow-effects','gui,external');entry.unlink();shutil.rmtree(directory/'.rewind',ignore_errors=True)
  title='REWIND dialog '+str(time.time_ns());trace=directory/'trace.json'
  p=subprocess.Popen([str(binary),'run','main.rwc','--allow-effects','gui,external','--record',str(trace),'--record-mode',mode,'--',title,str(path),action],cwd=directory,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  delivered=queue.Queue();output=[];errors=[]
  def reader():
   for line in p.stdout:output.append(line);delivered.put(line.decode('utf-8').rstrip('\r\n'))
   delivered.put(None)
  def stderr():errors.append(p.stderr.read())
  a=threading.Thread(target=reader,daemon=True);b=threading.Thread(target=stderr,daemon=True);a.start();b.start()
  selected='selected' if accepted else 'cancelled';expected=['ready','progress',selected,'cached']
  try:
   actual=delivered.get(timeout=15);assert actual=='ready',(actual,errors)
   actual=delivered.get(timeout=15);assert actual=='progress',(actual,errors)
   # Loading an initial native directory is asynchronous; do not target an unready view.
   time.sleep(1)
   dialog_response(title,accepted,path=path)
   for value in expected[2:]:
    actual=delivered.get(timeout=20);assert actual==value,(actual,value,errors)
   assert p.wait(timeout=15)==0
  finally:
   if p.poll() is None:p.kill();p.wait(timeout=5)
   a.join(timeout=5);b.join(timeout=5)
  if b''.join(errors):raise RuntimeError(b''.join(errors).decode('utf-8',errors='replace'))
  if action=='open':assert path.read_text(encoding='utf-8')=='unchanged'
  else:assert not path.exists(),'Choosing a save destination must not create it'
  offline=dict(env);offline.pop('DISPLAY',None)
  replay=call('replay',str(trace),'--allow-effects','gui,external',call_env=offline)
  assert replay.stdout.decode('utf-8').splitlines()==expected
print('Verified native file dialog SDK: open/save/cancel, rollback, unchanged files and disconnected source-free replay')
