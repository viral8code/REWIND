import base64,os,ssl,pathlib,subprocess,tempfile,socket,threading,time,http.client,json,shutil,sys
binary=str(pathlib.Path(sys.argv[1]).resolve())
work=pathlib.Path(sys.argv[2]).resolve();work.mkdir()
fixture=pathlib.Path(__file__).resolve().parent.parent/'tests/fixtures/tls'
private_key=(fixture/'localhost-key.pem').read_text(encoding='utf-8')
private_token='fixture-only-client-token-0123456789'
context=ssl.create_default_context(cafile=str(fixture/'localhost-ca.pem'))
environment=dict(os.environ,REWIND_HTTP_SERVER_KEY=private_key,REWIND_HTTP_SERVER_TOKEN=private_token)
source_path=pathlib.Path(sys.argv[3]) if len(sys.argv)>3 else pathlib.Path(binary).parent.parent/'share/rewind/examples/http-server-auth/main.rw'
source=source_path.read_text(encoding='utf-8')
def https_peer(port):
 # Listener is IPv4; keep localhost as the verified TLS hostname/SNI while
 # avoiding Windows IPv6 refusal delays on every authentication probe.
 connection=http.client.HTTPSConnection('localhost',port,timeout=5,context=context)
 connection._create_connection=lambda address,timeout,source_address=None: socket.create_connection(('127.0.0.1',address[1]),timeout,source_address)
 return connection
def assert_key_private(path):
 text=path.read_text(encoding='utf-8')
 for secret_form in (private_key,json.dumps(private_key)[1:-1],private_key.splitlines()[1],base64.b64encode(private_key.encode()).decode(),private_token,base64.b64encode(private_token.encode()).decode(),base64.b64encode(('Bearer '+private_token).encode()).decode()):
  assert secret_form not in text,'private credential leaked into '+path.name
for mode in ('debug','compact','live'):
 live=mode=='live'
 with tempfile.TemporaryDirectory(prefix='rewind-v1929-',dir=work) as tmp:
  root=pathlib.Path(tmp)
  with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
  text=source.replace('PORT',str(port));text=text.replace('external {','external live {') if live else text
  effects='external,network,tasks,env,fileRead'+(',live' if live else '')
  (root/'main.rw').write_text(text)
  shutil.copyfile(fixture/'localhost-cert.pem',root/'server.pem')
  def call(args,replay=False):
   selected_environment=dict(environment)
   if replay:selected_environment["REWIND_HTTP_SERVER_TOKEN"]="different-replay-only-token-9876543210"
   p=subprocess.run([binary,*args],cwd=root,env=selected_environment,capture_output=True,timeout=20)
   if p.returncode:raise RuntimeError(p.stderr.decode())
   return p
  call(['compile','main.rw','--allow-effects',effects])
  assert_key_private(root/'main.rwc')
  (root/'main.rw').unlink();shutil.rmtree(root/'.rewind',ignore_errors=True)
  errors=[]
  def peer():
   try:
    end=time.monotonic()+8
    while True:
     connection=https_peer(port)
     try:connection.connect();break
     except ConnectionRefusedError:
      connection.close()
      if time.monotonic()>end:raise
      time.sleep(.01)
    connection.request('POST','/echo?q=one',b'ignored',{'Connection':'close'})
    denied=connection.getresponse();assert denied.status==401;assert denied.read()==b'';connection.close()
    wrong=https_peer(port)
    wrong.request('POST','/echo?q=one',b'ignored',{'Authorization':'Bearer wrong-client-token-012345','Connection':'close'})
    denied=wrong.getresponse();assert denied.status==401;assert denied.read()==b'';wrong.close()
    connection=https_peer(port)
    connection.request('POST','/echo?q=one',b'ping',{'Authorization':'Bearer '+private_token,'Connection':'close'})
    response=connection.getresponse();assert response.status==200;assert response.read()==b'ok';connection.close()
    shutdown=https_peer(port);shutdown.request('GET','/shutdown',headers={'Authorization':'Bearer '+private_token,'Connection':'close'})
    try:shutdown.getresponse();raise AssertionError('shutdown should close the connection')
    except (http.client.RemoteDisconnected,ConnectionResetError):pass
    finally:shutdown.close()
   except Exception as e:errors.append(e)
  worker=threading.Thread(target=peer);worker.start()
  args=['run','main.rwc','--allow-effects',effects,'--secret-env','REWIND_HTTP_SERVER_KEY','--secret-env','REWIND_HTTP_SERVER_TOKEN']
  if live:args[0]='profile'
  else:args+=['--record','trace.json','--record-mode',mode]
  try:result=call(args)
  except Exception as error:
   worker.join(8)
   raise RuntimeError(str(error)+'; peer errors: '+repr(errors)) from error
  worker.join(8);assert not worker.is_alive();assert not errors,errors;assert result.stdout.replace(b'\r\n',b'\n')==b'authenticated https server done\n',result.stdout
  if not live:
   assert_key_private(root/'trace.json')
   def no_authorization(value):
    if isinstance(value,dict):
     assert value.get('name')!='authorization','authorization header reached recorded input'
     for child in value.values():no_authorization(child)
    elif isinstance(value,list):
     for child in value:no_authorization(child)
   no_authorization(json.loads((root/'trace.json').read_text(encoding='utf-8')))
   (root/'server.pem').unlink()
   replay=call(['replay','trace.json','--allow-effects',effects,'--secret-env','REWIND_HTTP_SERVER_KEY','--secret-env','REWIND_HTTP_SERVER_TOKEN'],replay=True);assert replay.stdout==result.stdout
  else:
   profile=next(json.loads(line) for line in reversed(result.stderr.decode().splitlines()) if line.startswith('{'));
   for key in ['native_resources','retained_operations','recorded_operations','recorded_polls']:assert profile['runtime']['external_live'][key]==0,(key,profile)
  print('source-free authenticated HTTPS server '+mode+' passed')
