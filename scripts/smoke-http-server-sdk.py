import pathlib,subprocess,tempfile,socket,threading,time,http.client,json,shutil,sys
binary=str(pathlib.Path(sys.argv[1]).resolve())
work=pathlib.Path(sys.argv[2]).resolve();work.mkdir()
source=(pathlib.Path(binary).parent.parent/'share/rewind/examples/http-server/main.rw').read_text(encoding='utf-8')
for mode in ('debug','compact','live'):
 live=mode=='live'
 with tempfile.TemporaryDirectory(prefix='rewind-v1925-',dir=work) as tmp:
  root=pathlib.Path(tmp)
  with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
  text=source.replace('PORT',str(port));text=text.replace('external {','external live {') if live else text
  effects='external,network,tasks'+(',live' if live else '')
  (root/'main.rw').write_text(text)
  def call(args):
   p=subprocess.run([binary,*args],cwd=root,capture_output=True,timeout=20)
   if p.returncode:raise RuntimeError(p.stderr.decode())
   return p
  call(['compile','main.rw','--allow-effects',effects])
  (root/'main.rw').unlink();shutil.rmtree(root/'.rewind',ignore_errors=True)
  errors=[]
  def peer():
   try:
    end=time.monotonic()+8
    while True:
     connection=http.client.HTTPConnection('127.0.0.1',port,timeout=5)
     try:connection.connect();break
     except ConnectionRefusedError:
      connection.close()
      if time.monotonic()>end:raise
      time.sleep(.01)
    connection.request('POST','/echo?q=one',b'ping',{'Connection':'close'})
    response=connection.getresponse();assert response.status==200;assert response.read()==b'ok';connection.close()
    shutdown=http.client.HTTPConnection('127.0.0.1',port,timeout=5);shutdown.request('GET','/shutdown',headers={'Connection':'close'})
    try:shutdown.getresponse();raise AssertionError('shutdown should close the connection')
    except (http.client.RemoteDisconnected,ConnectionResetError):pass
    finally:shutdown.close()
   except Exception as e:errors.append(e)
  worker=threading.Thread(target=peer);worker.start()
  args=['run','main.rwc','--allow-effects',effects]
  if live:args[0]='profile'
  else:args+=['--record','trace.json','--record-mode',mode]
  result=call(args);worker.join(8);assert not worker.is_alive();assert not errors,errors;assert result.stdout.replace(b'\r\n',b'\n')==b'http server done\n',result.stdout
  if not live:
   replay=call(['replay','trace.json','--allow-effects',effects]);assert replay.stdout==result.stdout
  else:
   profile=next(json.loads(line) for line in reversed(result.stderr.decode().splitlines()) if line.startswith('{'));
   for key in ['native_resources','retained_operations','recorded_operations','recorded_polls']:assert profile['runtime']['external_live'][key]==0,(key,profile)
  print('source-free HTTP server '+mode+' passed')
