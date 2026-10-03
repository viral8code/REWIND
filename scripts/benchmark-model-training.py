#!/usr/bin/env python3
"""Measure native-array tape / SGD allocation, retained checkpoint and GC on Linux."""
import argparse,json,os,statistics,subprocess,tempfile,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('binary',type=Path);p.add_argument('--sizes',type=int,nargs='+',default=[4096,65536]);p.add_argument('--iterations',type=int,nargs='+',default=[10,100]);p.add_argument('--repetitions',type=int,default=3);p.add_argument('--output',type=Path);a=p.parse_args()
if not hasattr(os,'wait4') or any(n<1 or n>1048576  for n in a.sizes) or any(not 1<=n<=1000 for n in a.iterations) or a.repetitions<1:p.error('requires Linux, sizes 1..2^20, iterations 1..1000, positive repetitions')
binary=a.binary.resolve()
source=r'''import std.numeric as numeric;import std.autodiff as ad;import std.optimize as optimize;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("model benchmark");}}}
let arguments=Args.all();let count=take(stdParseInt(arguments.get(0),10));let iterations=take(stdParseInt(arguments.get(1),10));let shape=List<Int>();shape.add(count);let zero=take(numeric.zerosFloat(&shape));let initial=take(numeric.affine(&zero,0.0,1.0));var weights=Map<String,FloatArray>();weights.set("x",initial);
commit inputs;
for i in 0..iterations {var tape=ad.create();var array=initial;match weights.get("x"){Some(v)=>{array=v;},None=>{panic("parameter");}}let x=take(ad.parameter(&mut tape,"x",array));let square=take(ad.unary(&mut tape,"square",x));let loss=take(ad.mean(&mut tape,square));let derivatives=take(ad.backward(&tape,loss));let gradients=Map<String,FloatArray>();match take(ad.gradient(&derivatives,x)){Some(g)=>{gradients.set("x",g);},None=>{panic("gradient");}}weights=take(optimize.sgd(&weights,&gradients,0.1));}
match weights.get("x"){Some(v)=>{let mean=take(numeric.mean(&v));assert(mean>0.0 && mean<1.0);},None=>{panic("result");}}
Out.println(iterations);publish;drop inputs;
'''
report={'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),'workload':'source-free fresh tape, mean-square reverse-mode and SGD each iteration, one retained initial checkpoint; native constant input, initialization and validation and GC included, compilation excluded; whole program, no kernel latency claim','workloads':{}}
with tempfile.TemporaryDirectory(prefix='rewind-model-bench-') as directory:
 root=Path(directory);path=root/'main.rw';path.write_text(source,encoding='utf-8')
 c=subprocess.run([str(binary),'compile',str(path)],cwd=root,capture_output=True)
 if c.returncode:raise RuntimeError(c.stderr.decode(errors='replace'))
 path.unlink()
 for n in a.sizes:
  for loops in a.iterations:
   samples=[]
   for index in range(a.repetitions+1):
    with (root/'out').open('wb') as out,(root/'err').open('wb') as err:
     started=time.monotonic();proc=subprocess.Popen([str(binary),'profile','main.rwc','--history-memory','256MiB','--native-work','10000000000000','--steps','1000000000','--',str(n),str(loops)],cwd=root,stdout=out,stderr=err)
     _,status,usage=os.wait4(proc.pid,0);proc.returncode=os.waitstatus_to_exitcode(status);elapsed=time.monotonic()-started
    if proc.returncode:raise RuntimeError((root/'err').read_text()[:3000])
    assert (root/'out').read_bytes()==f'{loops}\n'.encode()
    profiles=[json.loads(line) for line in (root/'err').read_text().splitlines() if line.startswith('{')];assert profiles
    if index:samples.append({'elapsed_seconds':elapsed,'peak_rss_kib':usage.ru_maxrss,'storage_work':profiles[-1]['storage_work'],'gc':profiles[-1]['gc'],'numeric_pages':profiles[-1]['numeric_pages'],'runtime':profiles[-1]['runtime']})
   report['workloads'][f'{n}x{loops}']={'samples':samples,'median_seconds':statistics.median(x['elapsed_seconds'] for x in samples),'median_peak_rss_kib':statistics.median(x['peak_rss_kib'] for x in samples)}
encoded=json.dumps(report,indent=2)+'\n'
if a.output:a.output.write_text(encoded,encoding='utf-8')
print(encoded,end='')
