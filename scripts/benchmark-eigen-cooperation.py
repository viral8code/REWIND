#!/usr/bin/env python3
"""Serial source-free synchronous/cooperative symmetric eigen execution measurements (Linux)."""
import argparse,json,os,statistics,subprocess,tempfile,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('binary',type=Path);p.add_argument('--size',type=int,default=65);p.add_argument('--repetitions',type=int,default=3);p.add_argument('--output',type=Path);a=p.parse_args()
if not hasattr(os,'wait4') or a.repetitions<1 or not 65<=a.size<=257:p.error('requires Linux wait4, positive repetitions and size 65..257')
binary=a.binary.resolve();report={'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),'size':a.size,'workload':'source-free execution including shared array setup; compilation excluded','variants':{}}
setup='''import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
async fn marker()->Int effects {tasks} {var sum=0;for i in 0..20{sum+=i;task.yieldNow();}return sum;}
let size=SIZE;let shape=List<Int>();shape.add(size);shape.add(size);
let zero=take(n.zerosFloat(&shape));let ones=take(n.mapFloat("exp",&zero));var builder=take(n.scale(&ones,0.01));var factor=2.0;
for col in 0..size{let index=List<Int>();index.add(col);index.add(col);builder=take(n.withFloat(&builder,&index,factor));factor+=0.1;}
let matrix=builder;
'''.replace('SIZE',str(a.size))
bodies={'synchronous':'let answer=take(n.eigenSymmetric(&matrix,1e-13,80));','cooperative':'let answer=take(take(await spawn jobs.eigenSymmetric(matrix,1e-13,80)));','foreground':'let work=spawn jobs.eigenSymmetric(matrix,1e-13,80);let foreground=spawn marker();assert_eq(take(await foreground),190);assert(!work.isDone());let answer=take(take(await work));'}
with tempfile.TemporaryDirectory(prefix='rewind-eigen-benchmark-') as temp:
 root=Path(temp)
 for variant,body in bodies.items():
  entry=root/(variant+'.rw');entry.write_text(setup+body+'assert(answer.sweeps>0);Out.println(size);publish;',encoding='utf-8')
  subprocess.run([str(binary),'compile',entry.name],cwd=root,check=True,capture_output=True);entry.unlink()
  import shutil
  shutil.rmtree(root/'.rewind',ignore_errors=True)
  samples=[]
  for run in range(a.repetitions+1):
   with tempfile.TemporaryFile() as out,tempfile.TemporaryFile() as err:
    start=time.perf_counter();child=subprocess.Popen([str(binary),'run',variant+'.rwc','--native-work','10000000000','--steps','20000000','--task-steps','2000000'],cwd=root,stdout=out,stderr=err)
    _,status,usage=os.wait4(child.pid,0);child.returncode=os.waitstatus_to_exitcode(status);elapsed=time.perf_counter()-start
    out.seek(0);err.seek(0);stdout=out.read();stderr=err.read()
    if child.returncode or stdout!=f'{a.size}\n'.encode():raise RuntimeError(f'{variant}: {child.returncode}: {stderr.decode(errors="replace")} {stdout!r}')
    if run:samples.append({'elapsed_seconds':elapsed,'cpu_seconds':usage.ru_utime+usage.ru_stime,'peak_rss_kib':usage.ru_maxrss})
  report['variants'][variant]={'samples':samples,**{'median_'+key:statistics.median(sample[key] for sample in samples) for key in samples[0]}}
text=json.dumps(report,indent=2)
if a.output:a.output.write_text(text+'\n')
print(text)
