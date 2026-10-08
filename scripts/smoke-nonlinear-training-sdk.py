#!/usr/bin/env python3
"""Exercise unequal-batch nonlinear training with the extracted SDK."""
import shutil,subprocess,sys
from pathlib import Path
binary=Path(sys.argv[1]).resolve();root=Path(sys.argv[2]).resolve();root.mkdir()
sources=binary.parent.parent/'share/rewind/examples/nonlinear-training'
for mode in ['debug','compact']:
 directory=root/mode;directory.mkdir();entry=directory/'main.rw';entry.write_text((sources/('trace-small.rw' if mode=='debug' else 'main.rw')).read_text(encoding='utf-8'),encoding='utf-8')
 def call(*args):
  output=subprocess.run([str(binary),*map(str,args)],cwd=directory,capture_output=True,timeout=120)
  if output.returncode:raise RuntimeError(output.stderr.decode(errors='replace'))
  return output.stdout
 call('compile','main.rw','--allow-effects','fileRead,fileWrite');entry.unlink();shutil.rmtree(directory/'.rewind',ignore_errors=True)
 output=call('run','main.rwc','--allow-effects','fileRead,fileWrite','--steps','100000000','--native-work','1000000000','--history-memory','128MiB','--record','trace.json','--record-mode',mode)
 assert output.replace(b'\r\n',b'\n')==b'true\n',output
 if mode=='compact':(directory/'nonlinear.rwm').unlink()
 assert call('replay','trace.json','--allow-effects','fileRead,fileWrite')==output
print('Verified nonlinear unequal-batch training, clipping, model persistence and source-free disconnected replay')
