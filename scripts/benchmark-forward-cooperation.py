#!/usr/bin/env python3
"""Identical materialized nonlinear forward tape comparison."""
import argparse
import json
import math
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import time

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('binary',type=Path);p.add_argument('--cooperative',action='store_true')
p.add_argument('--sizes',type=int,nargs='+',default=[16384,65536,262144]);p.add_argument('--repetitions',type=int,default=3);p.add_argument('--output',type=Path)
a=p.parse_args();b=a.binary.resolve()
if not hasattr(os,'wait4') or a.repetitions<1 or any(n<1 or n>1048576 for n in a.sizes):p.error('Linux wait4, positive repetitions and sizes 1..1048576 required')
s=1/(1+math.exp(-0.5));expected=2*s*s*(1-s)
report={'version':subprocess.check_output([str(b),'--version'],text=True).strip(),'cooperative':a.cooperative,'workload':'materialized nonzero input, identical sigmoid/square/mean tape, gradient sum validation; backward synchronous in both routes; source/cache removed, compilation excluded; input preparation, forward pass and profile/startup included; one warmup then serial repetitions','cases':{}}
with tempfile.TemporaryDirectory(prefix='rewind-forward-bench-') as d:
 root=Path(d)
 for n in a.sizes:
  source='''import std.autodiff as ad;import std.numeric as n;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let one=List<Int>();one.add(1);let values=List<Float>();values.add(0.5);let scalar=take(n.fromFloat(&one,&values));
'''
  if a.cooperative:source='import std.autodiffForwardAsync as jobs;'+source
  source+=f'''let shape=List<Int>();shape.add({n});let repeated=take(n.broadcastFloat(&scalar,&shape));let x=take(n.materializeFloat(&repeated));'''
  if a.cooperative:
   source+="""let initial=take(take(await spawn jobs.parameter(ad.create(),"x",x)));let tape=move initial._0;let parameter=initial._1;
let transformed=take(take(await spawn jobs.unary(move tape,"sigmoid",parameter)));let tape2=move transformed._0;let activated=transformed._1;
let square=take(take(await spawn jobs.unary(move tape2,"square",activated)));let tape3=move square._0;let squared=square._1;
let reduced=take(take(await spawn jobs.mean(move tape3,squared)));let tape4=move reduced._0;let loss=reduced._1;
let result=take(ad.backward(&tape4,loss));"""
  else:
   source+="""var tape=ad.create();let parameter=take(ad.parameter(&mut tape,"x",x));let activated=take(ad.unary(&mut tape,"sigmoid",parameter));let squared=take(ad.unary(&mut tape,"square",activated));let loss=take(ad.mean(&mut tape,squared));let result=take(ad.backward(&tape,loss));"""
  source+=f'''match take(ad.gradient(&result,parameter)){{Some(g)=>{{assert(take(n.math("abs",take(n.sum(&g))-{expected:.17f}))<0.00000001);}},None=>{{panic("gradient");}}}}Out.println(true);publish;'''
  entry=root/'main.rw';entry.write_text(source)
  result=subprocess.run([str(b),'compile','main.rw'],cwd=root,capture_output=True,timeout=90)
  if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace')[:3000])
  entry.unlink();shutil.rmtree(root/'.rewind',ignore_errors=True);samples=[]
  for sample in range(a.repetitions+1):
   with (root/'out').open('wb') as out,(root/'err').open('wb') as err:
    start=time.monotonic();child=subprocess.Popen([str(b),'profile','main.rwc','--native-work','1000000000','--history-memory','256MiB','--steps','100000000','--task-steps','10000000'],cwd=root,stdout=out,stderr=err)
    _,status,usage=os.wait4(child.pid,0);child.returncode=os.waitstatus_to_exitcode(status);elapsed=time.monotonic()-start
   if child.returncode:raise RuntimeError((root/'err').read_text(errors='replace')[:3000])
   assert (root/'out').read_bytes()==b'true\n'
   if sample:
    profile=json.loads(next(line for line in reversed((root/'err').read_text().splitlines()) if line.startswith('{')))
    samples.append({'wall_seconds':elapsed,'cpu_seconds':usage.ru_utime+usage.ru_stime,'peak_rss_kib':usage.ru_maxrss,'live_numeric_bytes':profile['numeric_pages']['live_bytes']})
  report['cases'][str(n)]={'samples':samples,**{'median_'+key:statistics.median(s[key] for s in samples) for key in samples[0]}}
encoded=json.dumps(report,indent=2)+'\n'
if a.output:a.output.write_text(encoded)
print(encoded,end='')
