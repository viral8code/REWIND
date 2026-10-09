#!/usr/bin/env python3
"""Identical materialized List-to-numeric-array conversion comparison."""
import argparse
import json
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
report={'version':subprocess.check_output([str(b),'--version'],text=True).strip(),'cooperative':a.cooperative,'workload':'identical native-prepared nonzero List, List-to-array conversion and value validation; source/cache removed; compilation excluded; one warmup then serial repetitions; process startup/input preparation/profile included','cases':{}}
with tempfile.TemporaryDirectory(prefix='rewind-input-bench-') as d:
 root=Path(d)
 for n in a.sizes:
  source='import std.numeric as n;\nfn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}\n'
  if a.cooperative:source='import std.numericInputAsync as jobs;'+source
  source+=f'let shape=List<Int>();shape.add({n});let zero=take(n.zerosFloat(&shape));let prepared=take(n.affine(&zero,0.0,0.5));let values=take(n.valuesFloat(&prepared));'
  source+=('let array=take(take(await spawn jobs.fromFloat(move shape,move values)));' if a.cooperative else 'let array=take(n.fromFloat(&shape,&values));')
  source+=f'assert_eq(take(n.sum(&array)),{n}.0*0.5);Out.println(true);publish;'
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
