#!/usr/bin/env python3
"""Serial source-free coordinate versus flat numeric access, including startup/profile."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary',type=Path)
parser.add_argument('--repetitions',type=int,default=5)
parser.add_argument('--elements',type=int,default=65536)
parser.add_argument('--output',type=Path)
args=parser.parse_args()
if not hasattr(os,'wait4') or args.repetitions<1 or not 1<=args.elements<=262144:
    parser.error('requires Linux wait4, positive repetitions and elements 1..262144')
binary=args.binary.resolve()
report={'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),
 'workload':'same compiler; source-free logical IntArray reads; compilation excluded; startup/profile included',
 'elements':args.elements,'variants':{}}
with tempfile.TemporaryDirectory(prefix='rewind-numeric-index-benchmark-') as temp:
    root=Path(temp)
    for module in ['numeric','numericIndex']:
        (root/(module+'.rw')).write_text((Path(__file__).resolve().parent.parent/'libraries/std'/(module+'.rw')).read_text(),encoding='utf-8')
    for variant in ['coordinates','flat']:
        header='import numeric as n;'
        if variant=='flat':header+='import numericIndex as flat;'
        source=header+'''fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let shape=List<Int>();shape.add(ELEMENTS);let zeros=take(n.zerosInt(&shape));
let last=List<Int>();last.add(ELEMENTS-1);let a=take(n.withInt(&zeros,&last,7));
var sum=0;
'''.replace('ELEMENTS',str(args.elements))
        if variant=='flat':source+=f'for i in 0..{args.elements}{{sum+=take(flat.getInt(&a,i));}}'
        else:source+=f'let index=List<Int>();index.add(0);for i in 0..{args.elements}{{index.set(0,i);sum+=take(n.getInt(&a,&index));}}'
        source+='Out.println(sum);publish;'
        entry=root/(variant+'.rw');entry.write_text(source,encoding='utf-8')
        result=subprocess.run([str(binary),'compile',entry.name],cwd=root,capture_output=True,timeout=90)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
        entry.unlink();samples=[]
        for sample in range(args.repetitions+1):
            with (root/'out').open('wb') as out,(root/'err').open('wb') as err:
                started=time.monotonic()
                process=subprocess.Popen([str(binary),'profile',variant+'.rwc','--steps','50000000','--native-work','100000000','--history-memory','64MiB'],cwd=root,stdout=out,stderr=err)
                _,status,usage=os.wait4(process.pid,0);process.returncode=os.waitstatus_to_exitcode(status)
                elapsed=time.monotonic()-started
            if process.returncode:raise RuntimeError((root/'err').read_text(errors='replace'))
            assert (root/'out').read_text().strip()=='7'
            p=json.loads(next(line for line in reversed((root/'err').read_text().splitlines()) if line.startswith('{')))
            if sample:samples.append({'wall_seconds':elapsed,'cpu_seconds':usage.ru_utime+usage.ru_stime,'peak_rss_kib':usage.ru_maxrss,'gc':p.get('gc'),'shared_payloads':p.get('shared_payloads')})
        report['variants'][variant]={'samples':samples,'median_wall_seconds':statistics.median(s['wall_seconds'] for s in samples),'median_cpu_seconds':statistics.median(s['cpu_seconds'] for s in samples),'median_peak_rss_kib':statistics.median(s['peak_rss_kib'] for s in samples)}
encoded=json.dumps(report,indent=2)+'\n'
if args.output:args.output.write_text(encoded,encoding='utf-8')
print(encoded,end='')
