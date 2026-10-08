#!/usr/bin/env python3
"""Verify bounded scalar-storage GC work using the extracted SDK's shipped history demo."""
import json
from pathlib import Path
import shutil
import subprocess
import sys

binary=Path(sys.argv[1]).resolve()
root=Path(sys.argv[2]).resolve()
root.mkdir()
source=(binary.parent.parent/'share/rewind/examples/container-admission/main.rw').read_text(encoding='utf-8')
(root/'main.rw').write_text(source,encoding='utf-8')
def call(*args):
    out=subprocess.run([str(binary),*map(str,args)],cwd=root,capture_output=True,timeout=120)
    if out.returncode:raise RuntimeError(out.stderr.decode('utf-8',errors='replace'))
    return out
call('compile','main.rw')
(root/'main.rw').unlink()
shutil.rmtree(root/'.rewind',ignore_errors=True)
out=call('profile','main.rwc','--steps','20000000','--native-work','100000000','--history-memory','8MiB')
assert out.stdout.replace(b'\r\n',b'\n')==b'31\n0\n'
profile=json.loads(next(l for l in reversed(out.stderr.decode().splitlines()) if l.startswith('{')))
assert profile['gc']['completed']>0,profile['gc']
assert profile['gc']['work']<1024,profile['gc']
print('Verified extracted SDK scalar histories: restored values, collections and bounded cached traversal')
