#!/usr/bin/env python3
"""Build, extract and smoke-test the native Windows SDK before release upload."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import struct
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

def pe_imports(path):
    """Read PE import names without requiring developer tools on the user machine."""
    data = path.read_bytes()
    def u32(offset):
        return struct.unpack_from('<I', data, offset)[0]
    pe = u32(0x3c)
    if data[pe:pe+4] != b'PE\0\0':
        raise RuntimeError('Not a PE executable')
    sections = struct.unpack_from('<H', data, pe+6)[0]
    optional_size = struct.unpack_from('<H', data, pe+20)[0]
    optional = pe+24
    if struct.unpack_from('<H', data, optional)[0] != 0x20b:
        raise RuntimeError('Expected PE32+ x64 executable')
    section_table = optional+optional_size
    def offset(rva):
        for i in range(sections):
            section = section_table+i*40
            virtual_size, virtual_address, raw_size, raw_pointer = struct.unpack_from('<IIII',data,section+8)
            if virtual_address <= rva < virtual_address+max(virtual_size,raw_size):
                return raw_pointer+rva-virtual_address
        raise RuntimeError('Invalid PE RVA')
    descriptor = offset(u32(optional+120))  # Import directory, second data-directory entry.
    imports = []
    for i in range(256):
        fields = struct.unpack_from('<IIIII',data,descriptor+i*20)
        if not any(fields):
            break
        name = offset(fields[3])
        end = data.index(b'\0',name,name+256)
        imports.append(data[name:end].decode('ascii').lower())
    else:
        raise RuntimeError('Too many PE imports')
    if any(name.startswith(('vcruntime','msvcp','msvcr','api-ms-win-crt')) or name == 'ucrtbase.dll' for name in imports):
        raise RuntimeError('Windows SDK must statically link the C runtime: '+repr(imports))
    return sorted(set(imports))

runtime_dependencies = pe_imports(binary)

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
    shutil.copyfile(sdk/'share/rewind/examples/conditional-revert/main.rw',source)
    assert run([exe,'run',source]) == b'Odd\n'
    run([compiler,source])
    source.unlink()
    shutil.rmtree(app/'.rewind')
    assert run([exe,app/'main.rwc']) == b'Odd\n'
    gui = work/'gui'
    gui.mkdir()
    shutil.copyfile(sdk/'share/rewind/examples/gui/main.rw',gui/'main.rw')
    shutil.copyfile(sdk/'share/rewind/examples/gui/events.json',gui/'events.json')
    assert run([exe,'run',gui/'main.rw','--allow-effects','gui','--gui-events',gui/'events.json','--record',gui/'trace.json']) == b'1\n'
    assert run([exe,'replay',gui/'trace.json','--root',gui,'--allow-effects','gui']) == b'1\n'
    run([compiler,gui/'main.rw','--allow-effects','gui'])
    (gui/'main.rw').unlink()
    shutil.rmtree(gui/'.rewind')
    assert run([exe,gui/'main.rwc','--allow-effects','gui','--gui-events',gui/'events.json']) == b'1\n'
    notes = work/'notes'
    shutil.copytree(sdk/'share/rewind/examples/notes',notes)
    permission='gui,fileRead,fileWrite'
    run([exe,'run',notes/'main.rw','--allow-effects',permission,'--gui-events',notes/'events.json','--record',notes/'trace.json'])
    assert (notes/'notes.txt').read_text(encoding='utf-8') == '日本語\nnotes'
    run([exe,'replay',notes/'trace.json','--root',notes,'--allow-effects',permission])
    run([compiler,notes/'main.rw','--allow-effects',permission])
    (notes/'main.rw').unlink()
    (notes/'notes.txt').unlink()
    shutil.rmtree(notes/'.rewind')
    run([exe,notes/'main.rwc','--allow-effects',permission,'--gui-events',notes/'events.json'])
    assert (notes/'notes.txt').read_text(encoding='utf-8') == '日本語\nnotes'
    external = work/'external'
    shutil.copytree(sdk/'share/rewind/examples/external',external)
    observed=run([exe,'run',external/'main.rw','--allow-effects','external,clock','--record',external/'trace.json'])
    lines=observed.splitlines()
    assert len(lines)==2 and lines[0]==lines[1] and int(lines[0])>0
    assert run([exe,'replay',external/'trace.json','--root',external,'--allow-effects','external,clock'])==observed
    run([compiler,external/'main.rw','--allow-effects','external,clock'])
    (external/'main.rw').unlink()
    shutil.rmtree(external/'.rewind')
    lines=run([exe,external/'main.rwc','--allow-effects','external,clock']).splitlines()
    run([sys.executable,root/'scripts/smoke-http-sdk.py',exe,work/'http'])
    run([sys.executable,root/'scripts/smoke-http-stream-sdk.py',exe,work/'http-stream'])
    run([sys.executable,root/'scripts/smoke-db-sdk.py',exe,work/'database'])
    run([sys.executable,root/'scripts/smoke-numeric-sdk.py',exe,work/'numeric'])
    run([sys.executable,root/'scripts/smoke-tcp-tls-sdk.py',exe,work/'tcp-tls'])
    run([sys.executable,root/'scripts/smoke-fft-sdk.py',exe,work/'fft-async'])
    run([sys.executable,root/'scripts/smoke-sparse-sdk.py',exe,work/'sparse-async'])
    run([sys.executable,root/'scripts/smoke-vector-sdk.py',exe,work/'vector-async'])
    run([sys.executable,root/'scripts/smoke-http-pool-sdk.py',exe,work/'http-pool'])
    run([sys.executable,root/'scripts/smoke-text-sdk.py',exe,work/'text-storage'])
    run([sys.executable,root/'scripts/smoke-solve-sdk.py',exe,work/'solve-async'])
    run([sys.executable,root/'scripts/smoke-qr-sdk.py',exe,work/'qr-async'])
    run([sys.executable,root/'scripts/smoke-http-server-auth-sdk.py',exe,work/'http-server-auth'])
    run([sys.executable,root/'scripts/smoke-http-server-tls-sdk.py',exe,work/'http-server-tls'])
    run([sys.executable,root/'scripts/smoke-http-server-sdk.py',exe,work/'http-server'])
    run([sys.executable,root/'scripts/smoke-tcp-sdk.py',exe,work/'tcp'])
    run([sys.executable,root/'scripts/smoke-live-sdk.py',exe,work/'live'])
    run([sys.executable,root/'scripts/smoke-gui-async-sdk.py',exe,work/'gui-async'])
    run([sys.executable,root/'scripts/smoke-gui-data-sdk.py',exe,work/'gui-data'])
    run([sys.executable,root/'scripts/smoke-gui-async-sdk.py',exe,work/'gui-live','--live'])
    run([sys.executable,root/'scripts/smoke-postgres-sdk.py',exe,work/'postgres'])
    assert len(lines)==2 and lines[0]==lines[1] and int(lines[0])>0
    assert b'rewind compile' in run([exe,'compile','--help'])
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
    run([sdk/'bin/rewindc.exe',shortest/'main.rw'])
    (shortest/'main.rw').unlink()
    shutil.rmtree(shortest/'vendor')
    assert run([exe,'run-artifact',work/'app.json','--root',shortest,'--task-steps','2000000','--allow-effects','input,output'], b'4 3\n0 1 4\n0 2 1\n2 1 1\n') == expected
    compiled_trace = shortest/'compiled-trace.json'
    assert run([exe,'run',shortest/'main.rwc','--task-steps','2000000','--record',compiled_trace,'--record-mode','compact'], b'4 3\n0 1 4\n0 2 1\n2 1 1\n') == expected
    assert run([exe,'replay',compiled_trace,'--root',shortest]) == expected

    (output/f'{name}-sdk.pub').write_text(public+'\n', encoding='utf-8')
    shutil.copyfile(root/'docs/getting-started.md',output/'GETTING_STARTED.windows.md')
    (output/'BUILD_INFO.windows.json').write_text(json.dumps(dict(version=version,commit=commit,target='x86_64-pc-windows-msvc',minimum_windows='Windows 10 x64',crt_linkage='static',runtime_dependencies=runtime_dependencies,sdk_public_key=public,signing_key_scope='this release only',rust_toolchain='1.98.1',build_command='cargo build --release --locked'),indent=2)+'\n', encoding='utf-8')
    checks = ''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n' for path in sorted(output.iterdir()) if path.is_file())
    (output/'SHA256SUMS.windows').write_text(checks, encoding='utf-8')
print(f'Verified Windows release assets: {output}')
