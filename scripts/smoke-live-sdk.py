#!/usr/bin/env python3
"""Exercise live result lifetimes and explicit new sends in the extracted SDK."""
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import shutil
import subprocess
import sys
import threading

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
root.mkdir()
sdk = binary.parent.parent


def call(*args, success=True):
    output = subprocess.run([str(binary), *map(str, args)], cwd=root,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
    if success and output.returncode:
        raise RuntimeError(output.stderr.decode('utf-8', errors='replace'))
    return output


source = root / 'sqlite.rw'
source.write_text((sdk / 'share/rewind/examples/live-external/main.rw').read_text(encoding='utf-8'), encoding='utf-8')
effects = 'external,db,tasks,live'
call('compile', source, '--allow-effects', effects)
source.unlink()
shutil.rmtree(root / '.rewind', ignore_errors=True)
result = call('profile', root / 'sqlite.rwc', '--allow-effects', effects, '--native-work', '5000000')
assert result.stdout.replace(b'\r\n', b'\n') == b'live sqlite\n'
profile = next(json.loads(line) for line in reversed(result.stderr.decode().splitlines()) if line.startswith('{'))
assert profile['runtime']['external_live']['retained_operations'] == 0, profile
assert profile['runtime']['external_live']['recorded_polls'] == 0, profile
assert profile['runtime']['external_live']['native_resources'] == 0, profile
print('Verified source-free live SQLite SDK: checkpoint receipts, repeated inserts and resource cleanup')


class Handler(BaseHTTPRequestHandler):
    count = 0
    def do_GET(self):
        type(self).count += 1
        self.send_response(200)
        self.send_header('Content-Length', '2')
        self.end_headers()
        self.wfile.write(b'ok')
    def log_message(self, *args):
        pass


server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
server.daemon_threads = True
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    url = f'http://127.0.0.1:{server.server_port}/'
    source = root / 'http.rw'
    source.write_text('''import std.http as http;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value {Ok(v)=>{return move v;},Err(_)=>{panic("request failed");}}}
var pending:Option<Task<Result<HttpResponse,HttpError>>>=None;
external live {pending=Some(http.get("URL",5000,1024));}
commit waiting;
match pending {None=>{panic("missing");},Some(task)=>{assert_eq(take(take(await task)).status,200);}}
revert waiting;
match pending {None=>{panic("missing");},Some(task)=>{assert_eq(take(take(await task)).status,200);}}
drop waiting;
for i in 0..2 {
 var again:Option<Task<Result<HttpResponse,HttpError>>>=None;
 external live {again=Some(http.get("URL",5000,1024));}
 match again {None=>{panic("missing");},Some(task)=>{assert_eq(take(take(await task)).status,200);}}
}
Out.println("live http");publish;
'''.replace('URL', url), encoding='utf-8')
    effects = 'external,network,tasks,live'
    call('compile', source, '--allow-effects', effects)
    source.unlink()
    shutil.rmtree(root / '.rewind', ignore_errors=True)
    for mode in ['debug', 'compact']:
        trace = root / f'{mode}.json'
        denied = call('run', root / 'http.rwc', '--allow-effects', effects,
                      '--record', trace, '--record-mode', mode, success=False)
        assert denied.returncode and b'ExternalLiveRecordingUnsupported' in denied.stderr
        assert not trace.exists() and not denied.stdout
        assert Handler.count == 0
    result = call('profile', root / 'http.rwc', '--allow-effects', effects, '--native-work', '5000000')
    assert result.stdout.replace(b'\r\n', b'\n') == b'live http\n'
    assert Handler.count == 3, Handler.count
    profile = next(json.loads(line) for line in reversed(result.stderr.decode().splitlines()) if line.startswith('{'))
    live = profile['runtime']['external_live']
    assert live['retained_operations'] == live['recorded_operations'] == live['recorded_polls'] == 0, live
    print('Verified source-free live HTTP SDK: three physical sends, restored receipt, early record denial and released results')
finally:
    server.shutdown()
    server.server_close()
    thread.join(timeout=5)
