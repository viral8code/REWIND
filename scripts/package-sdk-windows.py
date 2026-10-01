#!/usr/bin/env python3
"""Build, extract and smoke-test the native Windows SDK before release upload."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

root = Path(__file__).resolve().parent.parent
os.chdir(root)
if sys.platform != 'win32':
    raise SystemExit('Run this script on Windows x64 using the native release build')
version = json.loads((root/'libraries/std/rewind.lock').read_text())['compiler']
binary = root/'target/release/rewind.exe'
output = Path(sys.argv[1]).resolve()
output.mkdir()  # Never overwrite a previous distribution.
commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
if subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=no'], text=True):
    raise SystemExit('Tracked files must be clean')

def run(args, input=None):
    result = subprocess.run([str(a) for a in args], input=input, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
    return result.stdout

with tempfile.TemporaryDirectory(prefix='rewind-sdk-') as tmp:
    work = Path(tmp)
    seed = work/'signing-seed'
    public = run([binary, 'keygen', seed]).decode().strip()
    name = f'rewind-{version}-windows-x86_64'
    sdk = work/name
    run([binary, 'sdk-build', '--output', sdk, '--key', seed])
    run([binary, 'sdk-verify', '--sdk', sdk, '--public-key', public])
    for baseline in (root/'libraries/api').glob('*.api.json'):
        run([binary, 'api-diff', baseline, sdk/'share/rewind/doc/std'/baseline.name, '--deny-breaking'])
    archive = output/f'{name}.zip'
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as z:
        for path in sorted(sdk.rglob('*')):
            if path.is_file():
                z.write(path, path.relative_to(work).as_posix())
    extracted = work/'extracted'
    with zipfile.ZipFile(archive) as z:
        z.extractall(extracted)
    sdk = extracted/name
    exe = sdk/'bin/rewind.exe'
    compiler = sdk/'bin/rewindc.exe'
    run([exe, 'sdk-verify', '--sdk', sdk, '--public-key', public])
    app = work/'space and 日本語'
    app.mkdir()
    source = app/'main.rw'
    shutil.copyfile(sdk/'share/rewind/examples/checkpoint/main.rw', source)
    assert run([exe, 'run', source]) == b'99\n10\n'
    run([compiler, source])
    source.unlink()
    shutil.rmtree(app/'.rewind')
    assert run([exe, app/'main.rwc']) == b'99\n10\n'
    # Existing-file replacement and checkpoint restore are exercised on the host OS.
    source.write_text('File.writeText("state.txt","old");publish;commit old;File.writeText("state.txt","new");publish;revert old;File.writeText("state.txt","last");publish;', encoding='utf-8')
    run([exe, 'run', source, '--allow-effects', 'fileRead,fileWrite'])
    assert (app/'state.txt').read_text() == 'last'
    shortest = work/'shortest'
    shutil.copytree(sdk/'share/rewind/examples/shortest', shortest)
    run([exe,'sdk-install','--root',shortest,'--sdk',sdk,'--public-key',public])
    trace = work/'trace.json'
    expected = b'0\n2\n1\nunreachable\n'
    assert run([exe,'run','--root',shortest,'--task-steps','2000000','--record',trace], b'4 3\r\n0 1 4\r\n0 2 1\r\n2 1 1\r\n') == expected
    assert run([exe,'replay',trace,'--root',shortest]) == expected
    run([exe,'build','--root',shortest,'--output',work/'app.json'])
    (shortest/'main.rw').unlink()
    shutil.rmtree(shortest/'vendor')
    assert run([exe,'run-artifact',work/'app.json','--root',shortest,'--task-steps','2000000','--allow-effects','input,output'], b'4 3\n0 1 4\n0 2 1\n2 1 1\n') == expected
    (output/f'{name}-sdk.pub').write_text(public+'\n', encoding='utf-8')
    shutil.copyfile(root/'docs/getting-started.md',output/'GETTING_STARTED.windows.md')
    (output/'BUILD_INFO.windows.json').write_text(json.dumps(dict(version=version,commit=commit,target='x86_64-pc-windows-msvc',minimum_windows='Windows 10 x64',sdk_public_key=public,signing_key_scope='this release only',rust_toolchain='1.98.1',build_command='cargo build --release --locked'),indent=2)+'\n', encoding='utf-8')
    checks = ''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n' for path in sorted(output.iterdir()) if path.is_file())
    (output/'SHA256SUMS.windows').write_text(checks, encoding='utf-8')
print(f'Verified Windows release assets: {output}')
