#!/usr/bin/env python3
"""Serial Linux RSS and native storage measurements for cancelled least-squares tasks."""
import argparse,json,os,subprocess,tempfile,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('binary',type=Path);p.add_argument('--output',type=Path);args=p.parse_args()
if not hasattr(os,'wait4'):p.error('requires Linux wait4')
binary=args.binary.resolve()
source='''import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {}{match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(65);shape.add(65);let a=take(n.zerosFloat(&shape));let rhsShape=List<Int>();rhsShape.add(65);let b=take(n.zerosFloat(&rhsShape));var done=0;
for i in 0..COUNT{let work=spawn jobs.leastSquares(a,b,0.0);task.yieldNow();assert(!work.isDone());work.cancel();
match await work{Err(TaskError::Cancelled)=>{done+=1;},_=>{panic("cancel");}}}
Out.println(done);publish;
'''
results=[]
for count in [0,128,512,2048]:
 with tempfile.TemporaryDirectory(prefix='rewind-ls-cancel-measure-') as tmp:
  root=Path(tmp);(root/'main.rw').write_text(source.replace('COUNT',str(count)))
  subprocess.run([str(binary),'compile','main.rw'],cwd=root,check=True,capture_output=True)
  (root/'main.rw').unlink()
  import shutil
  shutil.rmtree(root/'.rewind',ignore_errors=True)
  with tempfile.TemporaryFile() as out,tempfile.TemporaryFile() as err:
   started=time.perf_counter();child=subprocess.Popen([str(binary),'profile','main.rwc','--native-work','20000000000','--steps','20000000','--history-memory','8MiB'],cwd=root,stdout=out,stderr=err)
   _,status,u=os.wait4(child.pid,0);child.returncode=os.waitstatus_to_exitcode(status);elapsed=time.perf_counter()-started
   out.seek(0);err.seek(0);stdout=out.read();stderr=err.read().decode()
   if child.returncode or stdout.strip()!=str(count).encode():raise RuntimeError(stderr)
   profile=json.loads(next(line for line in reversed(stderr.splitlines()) if line.startswith('{')))
   live=profile['numeric_pages']['live_bytes'];gc=profile['gc']['completed'];tasks=len(profile['task_instructions'])
   if count:assert gc>0 and tasks<64 and live<results[0]['numeric_live_bytes']+4*1024*1024,(count,profile)
   results.append(dict(count=count,seconds=elapsed,cpu_seconds=u.ru_utime+u.ru_stime,peak_rss_kib=u.ru_maxrss,numeric_live_bytes=live,gc_completed=gc,task_entries=tasks))
report={'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),'workload':'serial optimized source-free 65x65 zero matrix, one yield then cancellation; startup included, compilation excluded','results':results}
if args.output:args.output.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2),flush=True)
