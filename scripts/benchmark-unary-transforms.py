#!/usr/bin/env python3
"""Compare source-free synchronous map and cooperative unary transform cost."""
import argparse,json,os,statistics,subprocess,tempfile,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('binary',type=Path);p.add_argument('--cooperative',action='store_true');p.add_argument('--output',type=Path);a=p.parse_args();binary=a.binary.resolve()
report={'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),'cooperative':a.cooperative,'workload':'native 262144-cell exp map, source-free, compile excluded; startup/profile included','samples':[]}
source='import std.numeric as n;'+('import std.numericTransformAsync as jobs;' if a.cooperative else '')+'fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}let shape=List<Int>();shape.add(262144);let x=take(n.zerosFloat(&shape));let y='+('take(take(await spawn jobs.mapFloat("exp",x)))' if a.cooperative else 'take(n.mapFloat("exp",&x))')+';assert_eq(take(n.sum(&y)),262144.0);Out.println(true);publish;'
with tempfile.TemporaryDirectory(prefix='rewind-unary-bench-') as d:
 root=Path(d);(root/'main.rw').write_text(source)
 result=subprocess.run([str(binary),'compile','main.rw'],cwd=root,capture_output=True);assert result.returncode==0,result.stderr.decode(errors='replace');(root/'main.rw').unlink()
 for index in range(4):
  with (root/'out').open('wb') as out,(root/'err').open('wb') as err:
   start=time.monotonic();proc=subprocess.Popen([str(binary),'profile','main.rwc','--history-memory','64MiB','--native-work','100000000','--steps','20000000'],cwd=root,stdout=out,stderr=err);_,status,usage=os.wait4(proc.pid,0);proc.returncode=os.waitstatus_to_exitcode(status);elapsed=time.monotonic()-start
  assert proc.returncode==0,(root/'err').read_text()[:3000];assert (root/'out').read_bytes()==b'true\n'
  if index:report['samples'].append({'wall_seconds':elapsed,'cpu_seconds':usage.ru_utime+usage.ru_stime,'peak_rss_kib':usage.ru_maxrss})
for key in ['wall_seconds','cpu_seconds','peak_rss_kib']:report['median_'+key]=statistics.median(s[key] for s in report['samples'])
encoded=json.dumps(report,indent=2)+'\n'
if a.output:a.output.write_text(encoded)
print(encoded,end='')
