#!/usr/bin/env python3
"""Verify the shipped shared container admission task without source or compilation cache."""
import json
from pathlib import Path
import shutil
import subprocess
import sys

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
root.mkdir()
source = (binary.parent.parent / 'share/rewind/examples/container-admission/main.rw').read_text(encoding='utf-8')

def call(directory, *args):
    result = subprocess.run([str(binary), *map(str, args)], cwd=directory,
                            capture_output=True, timeout=120)
    if result.returncode:
        raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
    return result

for mode in ['debug', 'compact']:
    directory = root / mode
    directory.mkdir()
    entry = directory / 'main.rw'
    entry.write_text(source.replace('0..4096','0..8') if mode=='debug' else source, encoding='utf-8')
    call(directory, 'compile', entry)
    entry.unlink()
    shutil.rmtree(directory / '.rewind', ignore_errors=True)
    result = call(directory, 'run', 'main.rwc', '--steps', '20000000',
                  '--task-steps', '2000000', '--native-work', '100000000', '--history-memory', '8MiB',
                  '--record', 'trace.json', '--record-mode', mode)
    assert result.stdout.replace(b'\r\n', b'\n') == b'31\n0\n', result.stdout
    trace = json.loads((directory / 'trace.json').read_text(encoding='utf-8'))
    replay = call(directory, 'replay', 'trace.json')
    assert replay.stdout == result.stdout
print('Verified extracted shared container admission SDK: small budget, checkpoint and source-free replay')
