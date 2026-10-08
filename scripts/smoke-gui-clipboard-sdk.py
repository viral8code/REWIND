#!/usr/bin/env python3
"""Native clipboard input, VM undo and disconnected source-free replay."""
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import threading
import time
from native_gui_fixture import ctrl_key

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
source = (Path(sys.argv[3]).resolve() if len(sys.argv) > 3 else
          binary.parent.parent / 'share/rewind/examples/gui-clipboard/main.rw')
root.mkdir()
expected = ['ready', '界🙂clipboard', 'undo', '界🙂clipboard']
for mode in ['debug', 'compact']:
    directory = root / mode
    directory.mkdir()
    entry = directory / 'main.rw'
    entry.write_text(source.read_text(encoding='utf-8'), encoding='utf-8')
    def call(*args, env=None):
        result = subprocess.run([str(binary), *map(str, args)], cwd=directory,
                                env=env, capture_output=True, timeout=45)
        if result.returncode:
            raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
        return result
    call('compile', entry, '--allow-effects', 'gui')
    entry.unlink()
    shutil.rmtree(directory / '.rewind', ignore_errors=True)
    title = 'REWIND clipboard ' + mode + ' ' + str(time.time_ns())
    process = subprocess.Popen([str(binary), 'run', 'main.rwc', '--allow-effects', 'gui',
                                '--record', 'trace.json', '--record-mode', mode, '--', title],
                               cwd=directory, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    delivered = queue.Queue()
    output = []
    errors = []
    def read_output():
        for line in process.stdout:
            output.append(line)
            delivered.put(line.decode('utf-8').rstrip('\r\n'))
        delivered.put(None)
    def read_errors():
        errors.append(process.stderr.read())
    reader = threading.Thread(target=read_output, daemon=True)
    error_reader = threading.Thread(target=read_errors, daemon=True)
    reader.start(); error_reader.start()
    def expect(value):
        actual = delivered.get(timeout=15)
        if actual != value:
            raise RuntimeError('Expected ' + repr(value) + ', received ' + repr(actual))
    try:
        expect('ready')
        ctrl_key(title + ' copy', 'C')
        ctrl_key(title + ' paste', 'V')
        expect(expected[1]); expect('undo')
        ctrl_key(title + ' paste', 'V')
        expect(expected[3])
        assert process.wait(timeout=15) == 0
    finally:
        if process.poll() is None:
            process.kill(); process.wait(timeout=5)
        reader.join(timeout=5); error_reader.join(timeout=5)
    assert not errors or not b''.join(errors), b''.join(errors).decode('utf-8', errors='replace')
    env = os.environ.copy()
    env.pop('DISPLAY', None)
    replay = call('replay', 'trace.json', '--allow-effects', 'gui', env=env)
    assert replay.stdout.decode('utf-8').splitlines() == expected
print('Verified native clipboard SDK: Unicode, VM undo, source-free and disconnected replay')
