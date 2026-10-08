#!/usr/bin/env python3
"""Verify numeric page outputs and restore after removing all source files."""
import json
from pathlib import Path
import shutil
import subprocess
import sys

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
root.mkdir()
source = (Path(sys.argv[3]).resolve() if len(sys.argv)>3 else binary.parent.parent / 'share/rewind/examples/numeric-pages/main.rw').read_text(encoding='utf-8')

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
                  '--native-work', '100000000', '--record', 'trace.json',
                  '--record-mode', mode)
    assert result.stdout.replace(b'\r\n', b'\n') == b'pages done\nrestore done\n', result.stdout
    assert isinstance(json.loads((directory / 'trace.json').read_text(encoding='utf-8')), dict)
    assert call(directory, 'replay', 'trace.json').stdout == result.stdout
print('Verified extracted numeric pages: views, map/zip, COW, restore and source-free replay')
