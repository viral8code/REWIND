#!/usr/bin/env python3
"""Actual TLS trust, plaintext byte counts and replay from the extracted SDK."""
from pathlib import Path
import json
import shutil
import socket
import ssl
import subprocess
import sys
import threading
binary=Path(sys.argv[1]).resolve()
root=Path(sys.argv[2]).resolve()
root.mkdir()
sdk=binary.parent.parent
fixture=Path(__file__).resolve().parent.parent/'tests/fixtures/tls'
ca=(fixture/'localhost-ca.pem').read_text(encoding='utf-8')
context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(fixture/'localhost-cert.pem',fixture/'localhost-key.pem')

def call(directory,*args):
    value=subprocess.run([str(binary),*map(str,args)],cwd=directory,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=90)
    if value.returncode:
        raise RuntimeError(value.stderr.decode('utf-8',errors='replace'))
    return value

for mode in ['debug','compact','live','untrusted','wrong-name','bad-ca']:
    directory=root/mode
    directory.mkdir()
    address='127.0.0.2' if mode=='wrong-name' else '127.0.0.1'
    listener=socket.socket()
    listener.bind((address,0))
    listener.listen()
    listener.settimeout(30)
    received=[]
    errors=[]
    rejected=mode in ['untrusted','wrong-name','bad-ca']
    def server():
        try:
            peer,_=listener.accept()
            peer.settimeout(30)
            try:
                with context.wrap_socket(peer,server_side=True) as secure:
                    body=bytearray()
                    while len(body)<4:
                        chunk=secure.recv(4-len(body))
                        if not chunk:
                            break
                        body.extend(chunk)
                    received.append(bytes(body))
                    secure.sendall(b'ok')
                    try:
                        plain=secure.unwrap()
                        plain.close()
                    except (ssl.SSLError,ConnectionError):
                        # REWIND's explicit close drops both halves after receiving EOF.
                        pass
            except ssl.SSLError:
                peer.close()
                if not rejected:
                    raise
        except Exception as error:
            errors.append(error)
    worker=None
    if mode!='bad-ca':
        worker=threading.Thread(target=server,daemon=True)
        worker.start()
    host='127.0.0.2' if mode=='wrong-name' else 'localhost'
    port=listener.getsockname()[1]
    trust='not a certificate' if mode=='bad-ca' else ('' if mode=='untrusted' else ca)
    request=f'tcp.configuredTls({json.dumps(host)},{port},take(bytes.encode({json.dumps(trust)})),5000)'
    source=(sdk/'share/rewind/examples/tcp/main.rw').read_text(encoding='utf-8').replace('tcp.connect("127.0.0.1",PORT,5000)',request).replace('take(take(await shutdown(&mut socket)));','')
    effects='external,network,tasks'
    if rejected:
        source='''import std.tcp as tcp;import std.bytes as bytes;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value {Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
var opening:Option<Task<Result<TcpSocket,TcpError>>>=None;
external {opening=Some(REQUEST);}
match opening {None=>{panic("missing");},Some(task)=>{match take(await task) {Err(e)=>{Out.println(e.code);},Ok(socket)=>{panic("unverified TLS was accepted");}}}}
publish;'''.replace('REQUEST',request)
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
            state=profile['runtime']['external_live']
            for key in ['retained_operations','recorded_operations','recorded_polls','native_resources']:
                assert state[key]==0,(key,state)
        else:
            result=call(directory,'run','main.rwc','--allow-effects',effects,'--record','trace.json','--record-mode',mode if mode in ['debug','compact'] else 'compact')
        expected=b'TcpTlsCa\n' if mode=='bad-ca' else (b'TcpTls\n' if rejected else b'tcp done\n')
        assert result.stdout.replace(b'\r\n',b'\n')==expected,result.stdout
        if worker:
            worker.join(timeout=30)
            assert not worker.is_alive(),'TLS fixture did not complete'
        else:
            listener.setblocking(False)
            try:
                unexpected,_=listener.accept()
                unexpected.close()
                raise AssertionError('invalid CA contacted the peer')
            except BlockingIOError:
                pass
        assert not errors,errors
        assert received==([] if rejected else [b'ping']),received
    finally:
        listener.close()
    if mode!='live':
        replay=call(directory,'replay','trace.json','--allow-effects',effects)
        assert replay.stdout==result.stdout
print('Verified source-free TCP TLS SDK: CA and hostname verification, byte count, EOF, live cleanup and disconnected replay')
