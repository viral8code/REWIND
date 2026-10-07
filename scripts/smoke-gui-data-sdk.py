#!/usr/bin/env python3
"""Real multi-window GUI, retry/cancel, HTTP JSON, SQLite/PostgreSQL and offline replay."""
from contextlib import closing
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import queue
import shutil
import sqlite3
import subprocess
import sys
import threading
import time
from native_gui_fixture import click

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
source = Path(sys.argv[3]).resolve() if len(sys.argv) > 3 else binary.parent.parent/'share/rewind/examples/gui-data/main.rw'
if sys.platform != 'win32' and not os.environ.get('DISPLAY'):
    raise SystemExit('Native GUI integration requires DISPLAY, such as Xvfb')
dsn = os.environ.get('REWIND_TEST_PG_DSN')
if not dsn:
    raise SystemExit('A real TLS PostgreSQL fixture is required')
root.mkdir()
effects = 'gui,external,network,db,tasks,env,fileRead,output'
profile_sizes = []
for backend in ['sqlite', 'postgres']:
    for mode in ['source', 'debug', 'compact']:
        directory = root / (backend+'-'+mode)
        directory.mkdir()
        entry = directory/'main.rw'
        entry.write_text(source.read_text(encoding='utf-8'), encoding='utf-8')
        if backend == 'postgres':
            shutil.copyfile(os.environ['REWIND_TEST_PG_CA'], directory/'ca.der')
        env = os.environ.copy()
        env['REWIND_PG_DSN'] = dsn
        def call(*args, environment=env):
            result = subprocess.run([str(binary), *map(str,args)], cwd=directory,
                                    env=environment, capture_output=True, timeout=60)
            if result.returncode:
                raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
            return result
        call('compile', entry, '--allow-effects', effects)
        if mode != 'source':
            entry.unlink()
        shutil.rmtree(directory/'.rewind', ignore_errors=True)
        arrived = threading.Event()
        release = threading.Event()
        received = []
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                received.append(self.path)
                attempt = len(received)
                if attempt == 1:
                    status, body = 503, b'unavailable'
                elif attempt == 2:
                    status, body = 200, json.dumps({'text':'日本語 — saved'},ensure_ascii=False).encode('utf-8')
                else:
                    arrived.set()
                    release.wait(30)
                    status, body = 200, b'{}'
                try:
                    self.send_response(status)
                    self.send_header('Content-Type','application/json; charset=utf-8')
                    self.send_header('Content-Length',str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
                    pass
            def log_message(self, *_):
                pass
        server = ThreadingHTTPServer(('127.0.0.1',0), Handler)
        server.daemon_threads = True
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        title = 'REWIND data '+backend+' '+mode+' '+str(time.time_ns())
        trace = directory/'trace.json'
        args = [str(binary),'profile',str(entry if mode == 'source' else directory/'main.rwc'),
                '--allow-effects',effects,'--secret-env','REWIND_PG_DSN',
                '--record',str(trace),'--record-mode','debug' if mode == 'source' else mode,
                '--',backend,'http://127.0.0.1:'+str(server.server_port)+'/data',title]
        process = subprocess.Popen(args,cwd=directory,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        if sys.platform == 'linux' and os.environ.get('REWIND_TEST_STDERR_PIPE_BYTES'):
            import fcntl
            fcntl.fcntl(process.stderr.fileno(),fcntl.F_SETPIPE_SZ,int(os.environ['REWIND_TEST_STDERR_PIPE_BYTES']))
        lines = []
        errors = []
        events = queue.Queue()
        def consume():
            for line in process.stdout:
                lines.append(line)
                events.put(line.strip())
            events.put(None)
        reader = threading.Thread(target=consume,daemon=True)
        reader.start()
        def consume_error():
            for chunk in iter(lambda: process.stderr.read1(4096), b''):
                errors.append(chunk)
        error_reader = threading.Thread(target=consume_error,daemon=True)
        error_reader.start()
        def expect(value):
            try:
                actual = events.get(timeout=45)
            except queue.Empty:
                raise RuntimeError('GUI workflow did not progress to '+value.decode())
            if actual != value:
                if process.poll() is not None:
                    error_reader.join(timeout=5)
                detail = b''.join(errors).decode('utf-8',errors='replace')
                raise RuntimeError('Unexpected GUI workflow output '+repr(actual)+' '+detail)
        try:
            expect(b'retry')
            click(title,20,75)
            expect(b'saved')
            click(title,330,75)
            expect(b'undone')
            click(title,20,75)
            assert arrived.wait(15),'Reload did not start'
            click(title,190,75)
            expect(b'cancelled')
            process.wait(timeout=30)
            reader.join(timeout=5)
            error_reader.join(timeout=5)
            assert not reader.is_alive() and not error_reader.is_alive(),'GUI output readers did not finish'
            error = b''.join(errors)
            profile_sizes.append(len(error))
            assert process.returncode == 0,error.decode('utf-8',errors='replace')
            output = b''.join(lines)
            assert output.replace(b'\r\n',b'\n') == b'retry\nsaved\nundone\ncancelled\n',output
            profile = next(json.loads(line) for line in reversed(error.decode().splitlines()) if line.startswith('{'))
            assert profile['runtime']['native_resources'] == 0,profile['runtime']
            assert profile['runtime']['gui_windows']['published'] == [],profile['runtime']
            assert received == ['/data']*3,received
            if backend == 'sqlite':
                with closing(sqlite3.connect(directory/'state.sqlite')) as db:
                    assert db.execute('SELECT entry,text FROM rewind_gui_data').fetchall() == [(title,'日本語 — saved')]
            else:
                fixture=json.loads((Path(os.environ['REWIND_PG_FIXTURE'])/'fixture.json').read_text(encoding='utf-8'))
                psql=Path(fixture['bin'])/('psql.exe' if os.name=='nt' else 'psql')
                query="SELECT json_build_array(entry,text)::text FROM rewind_gui_data WHERE entry=:'entry';\n"
                checked=subprocess.run([str(psql),'-X','-A','-t','-v','ON_ERROR_STOP=1','-v','entry='+title,'-d',dsn],
                    input=query.encode(),env=dict(env,PGCLIENTENCODING='UTF8'),capture_output=True,timeout=15)
                assert checked.returncode==0,'Independent PostgreSQL read failed'
                assert json.loads(checked.stdout.decode('utf-8').strip())==[title,'日本語 — saved']
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            reader.join(timeout=5)
            error_reader.join(timeout=5)
            release.set()
            server.shutdown()
            worker.join(timeout=5)
            server.server_close()
        assert not worker.is_alive()
        if mode != 'source' and entry.exists():
            entry.unlink()
        shutil.rmtree(directory/'.rewind',ignore_errors=True)
        if backend == 'sqlite':
            (directory/'state.sqlite').unlink()
        else:
            (directory/'ca.der').unlink()
        assert dsn not in trace.read_text(encoding='utf-8')
        replay_env = dict(env)
        replay_env.pop('DISPLAY',None)
        replay_env['REWIND_PG_DSN']='host=127.0.0.1 port=1 user=replay dbname=postgres sslmode=require'
        replay = call('replay',trace,'--allow-effects',effects,'--secret-env','REWIND_PG_DSN',environment=replay_env)
        assert replay.stdout == output
        assert received == ['/data']*3
        assert not (directory/'state.sqlite').exists()
        assert not (directory/'ca.der').exists()
        if entry.exists():
            entry.unlink()
print('Verified native GUI data workflow: both DBs, HTTP error/retry/JSON/cancel, Undo and disconnected source-free replay (max stderr '+str(max(profile_sizes))+' bytes)')
