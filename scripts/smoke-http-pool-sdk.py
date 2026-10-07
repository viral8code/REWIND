#!/usr/bin/env python3
"""Check bounded client ownership and physical HTTP/1 keepalive in a shipped SDK."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import shutil
import subprocess
import sys
import threading

binary=Path(sys.argv[1]).resolve()
root=Path(sys.argv[2]).resolve()
source=Path(sys.argv[3]).resolve() if len(sys.argv)>3 else binary.parent.parent/'share/rewind/examples/http-pool/main.rw'
root.mkdir()
effects='external,network,tasks'
for mode in ['debug','compact']:
    directory=root/mode;directory.mkdir();entry=directory/'main.rw'
    entry.write_text(source.read_text(encoding='utf-8'),encoding='utf-8')
    def call(*args):
        result=subprocess.run([str(binary),*map(str,args)],cwd=directory,capture_output=True,timeout=90)
        if result.returncode:raise RuntimeError(result.stderr.decode('utf-8',errors='replace'))
        return result
    call('compile',entry,'--allow-effects',effects);entry.unlink();shutil.rmtree(directory/'.rewind',ignore_errors=True)
    lock=threading.Lock();stats={'accepted':0,'closed':0,'requests':0}
    class Server(ThreadingHTTPServer):
        daemon_threads=True
        def get_request(self):
            connection=super().get_request()
            with lock:stats['accepted']+=1
            return connection
    class Handler(BaseHTTPRequestHandler):
        protocol_version='HTTP/1.1'
        def do_GET(self):
            with lock:stats['requests']+=1
            self.send_response(200);self.send_header('Content-Length','2');self.end_headers();self.wfile.write(b'ok')
        def finish(self):
            try:super().finish()
            finally:
                with lock:stats['closed']+=1
        def log_message(self,*_):pass
    servers=[];workers=[]
    try:
        for _ in range(17):
            server=Server(('127.0.0.1',0),Handler);servers.append(server)
            worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start();workers.append(worker)
        urls=['http://127.0.0.1:'+str(server.server_port)+'/' for server in servers]
        arguments=[urls[0],urls[0],*urls[1:],urls[0]]
        run=call('profile','main.rwc','--allow-effects',effects,'--record','trace.json','--record-mode',mode,'--',*arguments)
        assert run.stdout.replace(b'\r\n',b'\n')==b'requests done\n',run.stdout
        profile=next(json.loads(line) for line in reversed(run.stderr.decode().splitlines()) if line.startswith('{'))
        assert profile['runtime']['http_cached_clients']==16,profile['runtime']
        assert profile['runtime']['http_retained_clients']==16,profile['runtime']
        assert profile['runtime']['http_client_reservation_bytes']>=16*1024*1024
        assert stats['requests']==19 and stats['accepted']==18,stats
    finally:
        for server in servers:server.shutdown();server.server_close()
        for worker in workers:worker.join(timeout=5)
    assert all(not worker.is_alive() for worker in workers)
    before=dict(stats)
    replay=call('replay','trace.json','--allow-effects',effects)
    assert replay.stdout==run.stdout
    assert stats==before
print('Verified extracted bounded HTTP clients: keepalive, eviction, profile and disconnected source-free replay')

# A second cold origin cannot be admitted under 2 MiB; check no physical send.
directory=root/'budget';directory.mkdir();entry=directory/'main.rw'
entry.write_text('''import std.http as http;
let urls=Args.all();
for i in 0..2 {
 var pending:Option<Task<Result<HttpResponse,HttpError>>>=None;
 external fresh {pending=Some(http.get(urls.get(i),5000,128));}
 match pending {None=>{panic("missing");},Some(work)=>{
  match await work {
   Ok(Ok(response))=>{assert_eq(i,0);assert_eq(response.status,200);},
   Ok(Err(error))=>{assert_eq(i,1);assert_eq(error.code,"HttpMemoryLimit");assert_eq(error.phase,"NotSent");},
   _=>{panic("task failed");}
  }
 }}
}
Out.println("not sent");publish;
''',encoding='utf-8')
received=[]
class BudgetHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        received.append(self.server.server_port)
        self.send_response(200);self.send_header('Content-Length','2');self.end_headers();self.wfile.write(b'ok')
    def log_message(self,*_):pass
servers=[ThreadingHTTPServer(('127.0.0.1',0),BudgetHandler) for _ in range(2)]
workers=[threading.Thread(target=server.serve_forever,daemon=True) for server in servers]
for worker in workers:worker.start()
try:
    run=subprocess.run([str(binary),'profile',str(entry),'--allow-effects',effects,'--history-memory','2MiB','--',
                        *['http://127.0.0.1:'+str(server.server_port)+'/' for server in servers]],
                       cwd=directory,capture_output=True,timeout=60)
    assert run.returncode==0,run.stderr.decode('utf-8',errors='replace')
    assert run.stdout.replace(b'\r\n',b'\n')==b'not sent\n',run.stdout
    assert received==[servers[0].server_port],received
finally:
    for server in servers:server.shutdown();server.server_close()
    for worker in workers:worker.join(timeout=5)
print('Verified HTTP client memory denial before physical submission')
