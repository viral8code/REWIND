#!/usr/bin/env python3
"""Measure repeated source-free nonlinear minibatch training, reporting bounded metrics."""
import argparse,json,os,statistics,subprocess,tempfile,time,sys
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('binary',type=Path);p.add_argument('--iterations',type=int,nargs='+',default=[250,1000,5000]);p.add_argument('--repetitions',type=int,default=3);p.add_argument('--source',type=Path,default=Path(__file__).resolve().parent.parent/'examples/nonlinear-training/main.rw');p.add_argument('--output',type=Path);a=p.parse_args()
if not hasattr(os,'wait4') or any(not 250<=n<=10000 for n in a.iterations) or a.repetitions<1:p.error('requires Linux, iterations 250..10000 and positive repetitions')
binary=a.binary.resolve();report={'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),'workload':'unequal sigmoid minibatches, named reverse gradients, merge, clipping, Adam and model persistence; native arrays; one retained initial checkpoint then dropped; compilation excluded, startup and profile included; no kernel latency claim','cases':{}}
with tempfile.TemporaryDirectory(prefix='rewind-nonlinear-bench-') as directory:
 root=Path(directory);source=root/'main.rw';source.write_text(a.source.read_text(encoding='utf-8'),encoding='utf-8')
 result=subprocess.run([str(binary),'compile','main.rw','--allow-effects','fileRead,fileWrite'],cwd=root,capture_output=True)
 if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace')[:3000])
 source.unlink()
 for count in a.iterations:
  print(f"Measuring {count} updates",file=sys.stderr,flush=True)
  samples=[]
  for index in range(a.repetitions+1):
   with (root/'out').open('wb') as out,(root/'err').open('wb') as err:
    started=time.monotonic();proc=subprocess.Popen([str(binary),'profile','main.rwc','--allow-effects','fileRead,fileWrite','--native-work','10000000000','--steps','1000000000','--history-memory','128MiB','--',str(count)],cwd=root,stdout=out,stderr=err)
    _,status,usage=os.wait4(proc.pid,0);proc.returncode=os.waitstatus_to_exitcode(status);elapsed=time.monotonic()-started
   if proc.returncode:raise RuntimeError((root/'err').read_text()[:3000])
   assert (root/'out').read_bytes()==b'true\n'
   profile=[json.loads(line) for line in (root/'err').read_text().splitlines() if line.startswith('{')][-1]
   if index:samples.append({'wall_seconds':elapsed,'cpu_seconds':usage.ru_utime+usage.ru_stime,'peak_rss_kib':usage.ru_maxrss,'gc':profile['gc'],'numeric_pages':profile['numeric_pages'],'shared_payloads':profile['shared_payloads']})
  report['cases'][str(count)]={'samples':samples,'median_wall_seconds':statistics.median(s['wall_seconds'] for s in samples),'median_cpu_seconds':statistics.median(s['cpu_seconds'] for s in samples),'median_peak_rss_kib':statistics.median(s['peak_rss_kib'] for s in samples)}
encoded=json.dumps(report,indent=2)+'\n'
if a.output:a.output.write_text(encoded,encoding='utf-8')
print(encoded,end='')
