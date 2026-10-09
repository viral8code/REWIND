#!/usr/bin/env python3
"""Verify shipped cooperative bulk graph construction and BFS after removing source and compile cache."""
import json
from pathlib import Path
import shutil
import subprocess
import sys

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
root.mkdir()
source = (Path(sys.argv[3]).resolve() if len(sys.argv)>3 else binary.parent.parent / 'share/rewind/examples/graph-async/main.rw').read_text(encoding='utf-8')

def call(directory, *args):
    result = subprocess.run([str(binary), *map(str, args)], cwd=directory,
                            capture_output=True, timeout=90)
    if result.returncode:
        raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
    return result

for mode in ['debug', 'compact']:
    directory = root / mode
    directory.mkdir()
    entry = directory / 'main.rw'
    entry.write_text(source, encoding='utf-8')
    call(directory, 'compile', entry)
    entry.unlink()
    shutil.rmtree(directory / '.rewind', ignore_errors=True)
    result = call(directory, 'run', 'main.rwc', '--steps', '20000000',
                  '--task-steps', '2000000', '--native-work', '1000000000',
                  '--record', 'trace.json', '--record-mode', mode)
    assert result.stdout.replace(b'\r\n', b'\n') == b'true\ntrue\ngraph done\n', result.stdout
    trace = json.loads((directory / 'trace.json').read_text(encoding='utf-8'))
    assert len(trace['schedule_choices']) > 10
    replay = call(directory, 'replay', 'trace.json')
    assert replay.stdout == result.stdout
print('Verified extracted cooperative graph SDK: views, checkpoint, cancellation and source-free replay')
