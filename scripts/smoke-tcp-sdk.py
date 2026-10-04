#!/usr/bin/env python3
"""Actual source-free TCP and disconnected replay in the extracted SDK."""
from pathlib import Path
import json
import shutil
import socket
import subprocess
import sys
import threading
binary=Path(sys.argv[1]).resolve()
root=Path(sys.argv[2]).resolve()
root.mkdir()
sdk=binary.parent.parent

def call(directory,*args):
    value=subprocess.run([str(binary),*map(str,args)],cwd=directory,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=90)
    if value.returncode:
        raise RuntimeError(value.stderr.decode('utf-8',errors='replace'))
    return value

for mode in ['debug','compact','live']:
    directory=root/mode
    directory.mkdir()
    listener=socket.socket()
    listener.bind(('127.0.0.1',0))
    listener.listen()
    listener.settimeout(30)
    received=[]
    errors=[]
    def server():
        try:
            peer,_=listener.accept()
            with peer:
                peer.settimeout(30)
                body=bytearray()
                while True:
                    chunk=peer.recv(4096)
                    if not chunk:
                        break
                    body.extend(chunk)
                received.append(bytes(body))
                peer.sendall(b'ok')
        except Exception as error:
            errors.append(error)
    worker=threading.Thread(target=server,daemon=True)
    worker.start()
    source=(sdk/'share/rewind/examples/tcp/main.rw').read_text(encoding='utf-8').replace('PORT',str(listener.getsockname()[1]))
    effects='external,network,tasks'
    if mode=='live':
        source=source.replace('external {','external live {').replace('effects {external,network,tasks}','effects {external,network,tasks,live}')
        effects+=',live'
    entry=directory/'main.rw'
    entry.write_text(source,encoding='utf-8')
    try:
        call(directory,'compile',entry,'--allow-effects',effects)
        entry.unlink()
        shutil.rmtree(directory/'.rewind',ignore_errors=True)
        if mode=='live':
            result=call(directory,'profile','main.rwc','--allow-effects',effects)
            profile=next(json.loads(line) for line in reversed(result.stderr.decode().splitlines()) if line.startswith('{'))
            live=profile['runtime']['external_live']
            for key in ['retained_operations','recorded_operations','recorded_polls','native_resources']:
                assert live[key]==0,(key,live)
        else:
            result=call(directory,'run','main.rwc','--allow-effects',effects,'--record','trace.json','--record-mode',mode)
        assert result.stdout.replace(b'\r\n',b'\n')==b'tcp done\n',result.stdout
        worker.join(timeout=30)
        assert not worker.is_alive(),'TCP fixture did not complete'
        assert not errors,errors
        assert received==[b'ping'],received
    finally:
        listener.close()
    if mode!='live':
        replay=call(directory,'replay','trace.json','--allow-effects',effects)
        assert replay.stdout==result.stdout
print('Verified source-free TCP SDK: byte count, half-close, EOF, checkpoint receipts, live cleanup and disconnected replay')
