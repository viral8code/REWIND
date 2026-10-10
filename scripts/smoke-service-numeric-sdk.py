"""Combined numerical/GUI/HTTP/database progress, cleanup and disconnected replay."""
from pathlib import Path
from contextlib import closing
import http.client
import json
import os
import queue
import shutil
import socket
import sqlite3
import subprocess
import sys
import threading
import time

from native_gui_fixture import click

def resident_bytes(pid):
    """Observe the VM process, not the compiler/DB fixture or cumulative child maxima."""
    if sys.platform == 'win32':
        import ctypes
        from ctypes import wintypes
        class Counters(ctypes.Structure):
            _fields_ = [('cb',wintypes.DWORD),('faults',wintypes.DWORD)] + [
                (name,ctypes.c_size_t) for name in ['peak_working_set','working_set',
                'peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile']]
        kernel = ctypes.WinDLL('kernel32',use_last_error=True)
        kernel.OpenProcess.argtypes=[wintypes.DWORD,wintypes.BOOL,wintypes.DWORD]
        kernel.OpenProcess.restype=wintypes.HANDLE
        kernel.CloseHandle.argtypes=[wintypes.HANDLE]
        kernel.K32GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.POINTER(Counters),wintypes.DWORD]
        kernel.K32GetProcessMemoryInfo.restype=wintypes.BOOL
        handle=kernel.OpenProcess(0x400|0x10,False,pid)
        if not handle:raise OSError(ctypes.get_last_error(),'VM memory observation failed')
        try:
            counters=Counters();counters.cb=ctypes.sizeof(counters)
            if not kernel.K32GetProcessMemoryInfo(handle,ctypes.byref(counters),counters.cb):
                raise OSError(ctypes.get_last_error(),'VM working-set observation failed')
            return counters.working_set
        finally:kernel.CloseHandle(handle)
    status=Path('/proc')/str(pid)/'status'
    for line in status.read_text().splitlines():
        if line.startswith('VmRSS:'):return int(line.split()[1])*1024
    raise RuntimeError('VM resident memory observation unavailable')

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
source_path = Path(sys.argv[3]).resolve() if len(sys.argv)>3 else binary.parent.parent/'share/rewind/examples/service-numeric/main.rw'
if sys.platform!='win32' and not os.environ.get('DISPLAY'):
    raise SystemExit('Native GUI integration requires DISPLAY, such as Xvfb')
dsn=os.environ.get('REWIND_TEST_PG_DSN')
if not dsn:raise SystemExit('A real TLS PostgreSQL fixture is required')
fixture=json.loads((Path(os.environ['REWIND_PG_FIXTURE'])/'fixture.json').read_text(encoding='utf-8'))
env=os.environ.copy();env['REWIND_PG_DSN']=dsn
effects='gui,external,network,db,tasks,env,fileRead,output'
cycles = int(sys.argv[4]) if len(sys.argv)>4 else 1
if not 1 <= cycles <= 16:raise SystemExit('Service cycles must be 1..16')
stress = cycles > 1
scenario=sys.argv[6] if len(sys.argv)>6 else 'normal'
if scenario not in ['normal','disconnect','early-stop']:raise SystemExit('Scenario must be normal, disconnect or early-stop')
modes = ['compact'] if stress or scenario!='normal' else ['source', 'debug', 'compact']
root.mkdir()
reports = []
backends = sys.argv[5].split(',') if len(sys.argv)>5 else ['sqlite', 'postgres']
if not backends or any(b not in ['sqlite','postgres'] for b in backends) or len(set(backends))!=len(backends):raise SystemExit('Backend selection must be sqlite, postgres or sqlite,postgres')
for backend in backends:
    for mode in modes:
        directory = root/(backend+'-'+mode)
        directory.mkdir()
        source = source_path.read_text(encoding='utf-8')
        assert '// FIXTURE_DELAY' in source,'Missing service fixture insertion point'
        if backend == 'postgres':
            shutil.copyfile(env['REWIND_TEST_PG_CA'], directory/'ca.der')
        delay_sql = "SELECT 1 AS waited FROM pg_sleep(2)" if backend == 'postgres' else "WITH RECURSIVE counter(x) AS (SELECT 0 UNION ALL SELECT x+1 FROM counter WHERE x<5000000) SELECT sum(x) FROM counter"
        source = source.replace('// FIXTURE_DELAY', '''var delay:Option<Task<Result<DbCursor,DbError>>>=None;
   external fresh{delay=Some(db.query(&mut connection,"DELAY_SQL",freeze(List<DbValue>()),15000));}
   match delay{None=>{panic("missing delay");},Some(t)=>{let delayed=database(take(await t));var readingDelay:Option<Task<Result<DbBatch,DbError>>>=None;external fresh{readingDelay=Some(db.next(&mut delayed,1,1048576,15000));}match readingDelay{None=>{panic("missing delay batch");},Some(t)=>{database(take(await t));}}database(take(await closeRows(&mut delayed)));}}'''.replace('DELAY_SQL',delay_sql))
        if stress:
            assert '// FIXTURE_CALCULATION' in source
            source=source.replace('for pass in 0..1 {','while true {').replace('dimensions.add(128)','dimensions.add(16384)')
            source = source.replace('for round in 0..2{',f'for round in 0..{cycles*2}{{\n  let cycle=round/2;let roundEntry=entry+"-"+cycle.format();')
            source = source.replace('round==0','round%2==0').replace('DbValue::Text(entry)','DbValue::Text(roundEntry)')
            source = source.replace(')),round);',')),round%2);')
        entry = directory/'main.rw'
        entry.write_text(source,encoding='utf-8')
        def call(*args, environment=env):
            r = subprocess.run([str(binary), *map(str,args)], cwd=directory,
                               env=environment, capture_output=True, timeout=600 if stress else 120)
            if r.returncode:
                raise RuntimeError(r.stderr.decode(errors='replace'))
            return r
        call('compile', entry, '--allow-effects', effects)
        if mode != 'source': entry.unlink()
        shutil.rmtree(directory/'.rewind', ignore_errors=True)
        with socket.socket() as reserve:
            reserve.bind(('127.0.0.1',0))
            port = reserve.getsockname()[1]
        title = f'REWIND service {backend} {mode} {time.time_ns()}'
        trace = directory/'trace.json'
        live_started=time.monotonic()
        process = subprocess.Popen([str(binary), 'profile',
            str(entry if mode == 'source' else directory/'main.rwc'),
            '--allow-effects', effects, '--secret-env', 'REWIND_PG_DSN',
            '--history-memory', '384MiB', '--task-steps', '100000000', '--steps', '200000000', '--native-work', str(5000000000*cycles), '--record', str(trace), '--record-mode', 'debug' if mode=='source' else mode,
            '--', backend,str(port),title,title], cwd=directory, env=env,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        events = queue.Queue(); lines=[]; errors=[]
        def stdout():
            for line in process.stdout:
                lines.append(line); events.put(line.strip())
            events.put(None)
        def stderr():
            for part in iter(lambda: process.stderr.read1(4096), b''): errors.append(part)
        readers=[threading.Thread(target=stdout), threading.Thread(target=stderr)]
        for reader in readers: reader.start()
        numerical_progress = []
        def expect(text):
            event = events.get(timeout=40)
            while event == b"calculated":
                numerical_progress.append(time.monotonic())
                event = events.get(timeout=40)
            if event != text.encode():
                raise AssertionError((text,event,[line for line in b''.join(errors).decode(errors='replace').splitlines() if not line.startswith('{')][-8:]))
        latencies = []
        memory_samples = []
        lock = None
        try:
            expect('ready')
            if stress:memory_samples.append(resident_bytes(process.pid))
            results=queue.Queue();disconnect_ready=threading.Event()
            def request(path):
                try:
                    connection=http.client.HTTPConnection('127.0.0.1',port,timeout=20)
                    connection.request('POST',path,b'saved text')
                    if scenario=='disconnect' and path=='/rollback':
                        assert disconnect_ready.wait(10),'Request was not admitted before disconnection'
                        connection.close();results.put('disconnected');return
                    response=connection.getresponse()
                    results.put((response.status,response.read()))
                    connection.close()
                except Exception as e: results.put(e)
            for cycle in range(cycles):
                for path,reply in [('/rollback','rolled back'),('/save','saved')]:
                    client=threading.Thread(target=request,args=(path,));client.start()
                    expect('transaction')
                    disconnect_ready.set()
                    if scenario=='early-stop':
                        click(title,300,80);expect('stopped early');client.join(timeout=25)
                        assert not client.is_alive()
                        outcome=results.get(timeout=1)
                        # Releasing the admitted request closes its reply channel,
                        # producing an empty 503. Closing the listener may win the
                        # race and close the transport first instead. Neither may
                        # deliver an application success response or time out.
                        assert outcome==(503,b'') or isinstance(outcome,(http.client.RemoteDisconnected,ConnectionResetError,ConnectionAbortedError)), {
                            'expected':'empty cancellation 503 or closed connection',
                            'type':type(outcome).__name__,
                            'errno':getattr(outcome,'errno',None),
                            'winerror':getattr(outcome,'winerror',None),
                            'response':outcome if isinstance(outcome,tuple) else None,
                        }
                        break
                    if stress or path=='/rollback':
                        started=time.monotonic()
                        click(title,20,80)
                        expect('responsive')
                        latencies.append(time.monotonic()-started)
                        if stress:memory_samples.append(resident_bytes(process.pid))
                    expect(reply);client.join(timeout=25)
                    assert not client.is_alive()
                    assert results.get(timeout=1)==('disconnected' if scenario=='disconnect' and path=='/rollback' else (200,reply.encode()))
            if scenario!='early-stop':
                connection=http.client.HTTPConnection('127.0.0.1',port,timeout=10)
                connection.request('POST','/shutdown',b'')
                try: connection.getresponse().read()
                except (http.client.RemoteDisconnected,ConnectionResetError): pass
                connection.close()
                expect('stopped')
            process.wait(timeout=25)
            live_seconds=time.monotonic()-live_started
            for reader in readers: reader.join(timeout=5)
            assert process.returncode==0,b''.join(errors).decode(errors='replace')
            output=b''.join(lines)
            expected = b'ready\n' + (b'transaction\nresponsive\nrolled back\ntransaction\n' + (b'responsive\n' if stress else b'') + b'saved\n')*cycles + b'stopped\n'
            if scenario=='early-stop':expected=b'ready\ntransaction\nstopped early\n'
            assert output.replace(b'\r\n',b'\n').replace(b'calculated\n',b'')==expected,output
            if scenario!='early-stop':assert numerical_progress, 'No complete forward/backward calculation overlapped the service workload'
            profile=next(json.loads(line) for line in reversed(b''.join(errors).decode().splitlines()) if line.startswith('{'))
            assert profile['runtime']['native_resources']==0,profile['runtime']
            # Short runs can finish below the automatic collection threshold.
            # Repeated cancellation/long workloads separately require completed GC.
            assert profile['numeric_pages']['live_bytes']<=(4*1024*1024 if stress else 65536),profile['numeric_pages']
            assert len(profile['scheduler']['tasks'])<64,profile['scheduler']
            if stress:assert profile['gc']['completed']>0,profile['gc']
            assert profile['runtime']['checkpoints']==['begin'],profile['runtime']
            assert profile['runtime']['http_retained_clients']==0,profile['runtime']
            assert profile['runtime']['gui_windows']['published']==[],profile['runtime']
            if backend=='sqlite':
                with closing(sqlite3.connect(directory/'state.sqlite')) as database:
                    assert sorted(database.execute('SELECT entry,text FROM rewind_service_data').fetchall())==([] if scenario=='early-stop' else sorted([(title+'-'+str(i) if stress else title,'saved text') for i in range(cycles)]))
                (directory/'state.sqlite').unlink()
            else:
                query="SELECT json_build_array(entry,text)::text FROM rewind_service_data WHERE entry=:'entry' OR left(entry,length(:'entry')+1)=:'entry'||'-' ORDER BY entry;\n"
                checked=subprocess.run([str(Path(fixture['bin'])/('psql.exe' if os.name=='nt' else 'psql')),'-X','-A','-t','-v','ON_ERROR_STOP=1','-v','entry='+title,'-d',dsn],input=query.encode(),env=dict(env,PGCLIENTENCODING='UTF8'),capture_output=True,timeout=15)
                assert checked.returncode==0,'Independent PostgreSQL read failed'
                assert [json.loads(line) for line in checked.stdout.decode().splitlines()]==([] if scenario=='early-stop' else sorted([[title+'-'+str(i) if stress else title,'saved text'] for i in range(cycles)]))
                (directory/'ca.der').unlink()
            if mode!='source' and entry.exists():entry.unlink()
            shutil.rmtree(directory/'.rewind',ignore_errors=True)
            assert dsn not in trace.read_text(encoding='utf-8')
            replay_env=dict(env);replay_env.pop('DISPLAY',None)
            replay_env['REWIND_PG_DSN']='host=127.0.0.1 port=1 user=replay dbname=postgres sslmode=require'
            replay_started=time.monotonic()
            replay=call('replay',trace,'--allow-effects',effects,'--secret-env','REWIND_PG_DSN',environment=replay_env)
            replay_seconds=time.monotonic()-replay_started
            assert replay.stdout==output
            assert not (directory/'state.sqlite').exists()
            if entry.exists():entry.unlink()
            reports.append({'backend':backend,'mode':mode,'scenario':scenario,'native_resources':0,'live_numeric_bytes':profile['numeric_pages']['live_bytes'],'numerical_completed':len(numerical_progress),'cycles':cycles,'requests':1 if scenario=='early-stop' else cycles*2,'injection_to_dispatch_seconds':latencies,'trace_bytes':trace.stat().st_size,'live_seconds':live_seconds,'replay_seconds':replay_seconds,'vm_resident_bytes':memory_samples,'history_memory_limit_bytes':384*1024*1024,'replay':'disconnected/source-free' if mode!='source' else 'disconnected/source'})
            (root/'validation.json').write_text(json.dumps(reports,indent=2))
            print(backend,mode,'PASS',cycles,'cycles; click samples',latencies,flush=True)
        finally:
            (directory/"stdout.log").write_bytes(b"".join(lines))
            (directory/"stderr.log").write_bytes(b"".join(errors))
            (directory/"progress.json").write_text(json.dumps({"numerical_completed":len(numerical_progress),"click_seconds":latencies,"vm_resident_bytes":memory_samples},indent=2))
            if lock:lock.rollback();lock.close()
            if process.poll() is None:process.kill();process.wait()
            for reader in readers:reader.join(timeout=5)

print('Verified GUI/server integration:', ','.join(backends),scenario,'rollback/commit, responsiveness, cleanup and disconnected source-free replay')
