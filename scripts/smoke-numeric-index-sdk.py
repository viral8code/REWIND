#!/usr/bin/env python3
"""Verify logical numeric view indexing and checkpoint storage from the extracted SDK."""
import shutil
import subprocess
import sys
from pathlib import Path
binary=Path(sys.argv[1]).resolve()
root=Path(sys.argv[2]).resolve()
source=binary.parent.parent/'share/rewind/examples/numeric-index/main.rw'
root.mkdir()
for mode in ['debug','compact']:
    directory=root/mode;directory.mkdir()
    entry=directory/'main.rw';entry.write_text(source.read_text(encoding='utf-8'),encoding='utf-8')
    def call(*args):
        output=subprocess.run([str(binary),*map(str,args)],cwd=directory,capture_output=True,timeout=60)
        if output.returncode:raise RuntimeError(output.stderr.decode(errors='replace'))
        return output.stdout
    call('compile',entry);entry.unlink();shutil.rmtree(directory/'.rewind',ignore_errors=True)
    output=call('run','main.rwc','--native-work','10000000','--record','trace.json','--record-mode',mode)
    assert output.replace(b'\r\n',b'\n')==b'99\n2\n',output
    assert call('replay','trace.json')==output
print('Verified extracted flat numeric views: bounds, COW, checkpoint restore and source-free replay')
