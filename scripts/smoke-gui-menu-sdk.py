#!/usr/bin/env python3
"""Native menu input, VM undo and disconnected source-free replay."""
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import threading
import time
from native_gui_fixture import command_key

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
source = (Path(sys.argv[3]).resolve() if len(sys.argv) > 3 else
          binary.parent.parent / 'share/rewind/examples/gui-menu/main.rw')
root.mkdir()
expected = ['ready','toggle','undo','toggle','open']
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
    title = 'REWIND menus ' + mode + ' ' + str(time.time_ns())
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
            if actual is None:
                error_reader.join(timeout=5)
            detail = b''.join(errors).decode('utf-8', errors='replace')
            raise RuntimeError('Expected ' + repr(value) + ', received ' + repr(actual) + ': ' + detail)
    try:
        expect('ready')
        command_key(title,'Ctrl+T')
        expect('toggle');expect('undo')
        command_key(title,'Ctrl+T');expect('toggle')
        command_key(title,'Ctrl+O');expect('open')
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
print('Verified native menu SDK: shortcuts, focus, VM undo, source-free and disconnected replay')
