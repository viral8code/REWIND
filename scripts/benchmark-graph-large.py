#!/usr/bin/env python3
"""Serial source-free typed graph bulk construction and native BFS; bounded metrics only."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('binary',type=Path);p.add_argument('--sizes',type=int,nargs='+',default=[65536,200000,1000000]);p.add_argument('--repetitions',type=int,default=3);p.add_argument('--output',type=Path)
a=p.parse_args()
if not hasattr(os,'wait4') or a.repetitions<1 or any(n<1 or n>1048576 for n in a.sizes):p.error('requires Linux wait4, positive repetitions and graph sizes 1..1048576')
b=a.binary.resolve();report={'version':subprocess.check_output([str(b),'--version'],text=True).strip(),'workload':'source-free native input ranges, graph bulk construction, BFS and validation; compilation excluded; startup/profile included','cases':{}}
with tempfile.TemporaryDirectory(prefix='rewind-graph-large-benchmark-') as t:
 r=Path(t)
 for n in a.sizes:
  s=f'''import std.graphLarge as graph;import std.numericRange as range;import std.numericIndex as index;
fn take<T,E>(r:Result<T,E>)->T effects {{}} {{match move r{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("failed");}}}}}}
let froms=take(range.integers(0,1,{n-1}));let tos=take(range.integers(1,1,{n-1}));let weights=take(range.integers(0,0,{n-1}));
let chain=take(graph.fromEdges({n},&froms,tos,weights));let distance=take(graph.bfs(&chain,0));assert_eq(take(index.getInt(&distance,{n-1})),{n-1});Out.println({n-1});publish;
'''
  entry=r/'main.rw';entry.write_text(s,encoding='utf-8');c=subprocess.run([str(b),'compile','main.rw'],cwd=r,capture_output=True,timeout=90)
  if c.returncode:raise RuntimeError(c.stderr.decode(errors='replace')[:2000])
  entry.unlink();samples=[]
  for sample in range(a.repetitions+1):
   with (r/'out').open('wb') as out,(r/'err').open('wb') as err:
    start=time.monotonic();child=subprocess.Popen([str(b),'profile','main.rwc','--native-work','1000000000','--history-memory','256MiB'],cwd=r,stdout=out,stderr=err)
    _,status,usage=os.wait4(child.pid,0);child.returncode=os.waitstatus_to_exitcode(status);elapsed=time.monotonic()-start
   if child.returncode:raise RuntimeError((r/'err').read_text(errors='replace')[:2000])
   assert (r/'out').read_text().strip()==str(n-1)
   profile=json.loads(next(line for line in reversed((r/'err').read_text().splitlines()) if line.startswith('{')))
   if sample:samples.append({'wall_seconds':elapsed,'cpu_seconds':usage.ru_utime+usage.ru_stime,'peak_rss_kib':usage.ru_maxrss,'gc':profile.get('gc'),'numeric_pages':profile.get('numeric_pages'),'shared_payloads':profile.get('shared_payloads')})
  report['cases'][str(n)]={'samples':samples,'median_wall_seconds':statistics.median(s['wall_seconds'] for s in samples),'median_cpu_seconds':statistics.median(s['cpu_seconds'] for s in samples),'median_peak_rss_kib':statistics.median(s['peak_rss_kib'] for s in samples)}
encoded=json.dumps(report,indent=2)+'\n'
if a.output:a.output.write_text(encoded,encoding='utf-8')
print(encoded,end='')
