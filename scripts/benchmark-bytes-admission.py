#!/usr/bin/env python3
"""Serial source-free shared Bytes aliases and ephemeral byte buffers: wall, CPU and peak RSS."""
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
parser.add_argument('--repetitions',type=int,default=3)
parser.add_argument('--output',type=Path)
args=parser.parse_args()
if not hasattr(os,'wait4') or args.repetitions<1:parser.error('requires Linux wait4 and positive repetitions')
binary=args.binary.resolve()
report={'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),
        'workload':'source-free byte aliases or repeated encode/drop; compilation excluded; startup included',
        'variants':{}}
variants={'aliases':8192,'ephemeral':1024,'longer_ephemeral':4096}
with tempfile.TemporaryDirectory(prefix='rewind-bytes-benchmark-') as temp:
    root=Path(temp)
    for name,iterations in variants.items():
        text='x'*65536;length=len(text);literal=json.dumps(text)
        take='fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("encoding");}}}'
        setup=f'let original={literal};'
        if name=='aliases':
            setup+='let data=take(stdEncode(original));let rows=List<Bytes>();for i in 0..32{rows.add(data);}'
            expression='rows.get(i%32)'
        else:expression='take(stdEncode(original))'
        expected=iterations*length
        source=take+setup+f'var total=0;for i in 0..{iterations}{{let copied={expression};total+=stdBytesLength(copied);}}assert_eq(total,{expected});Out.println(total);publish;'
        entry=root/(name+'.rw');entry.write_text(source,encoding='utf-8')
        result=subprocess.run([str(binary),'compile',str(entry)],cwd=root,capture_output=True,timeout=90)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace')[:2000])
        entry.unlink()
        samples=[]
        for sample in range(args.repetitions+1):
            with (root/'out').open('wb') as out,(root/'err').open('wb') as err:
                started=time.monotonic()
                process=subprocess.Popen([str(binary),'profile',name+'.rwc','--steps','10000000','--native-work','10000000000','--history-memory','128MiB'],cwd=root,stdout=out,stderr=err)
                _,status,usage=os.wait4(process.pid,0);process.returncode=os.waitstatus_to_exitcode(status)
                elapsed=time.monotonic()-started
            if process.returncode:raise RuntimeError((root/'err').read_text(errors='replace')[:2000])
            assert (root/'out').read_text().strip()==str(expected)
            profile=json.loads(next(line for line in reversed((root/'err').read_text().splitlines()) if line.startswith('{')))
            if sample:samples.append({'shared_payloads':profile.get('shared_payloads'),'gc':profile.get('gc'),'elapsed_seconds':elapsed,'cpu_user_seconds':usage.ru_utime,'cpu_system_seconds':usage.ru_stime,'peak_rss_kib':usage.ru_maxrss})
        report['variants'][name]={'buffer_bytes':length,'iterations':iterations,'samples':samples,
            'median_seconds':statistics.median(s['elapsed_seconds'] for s in samples),
            'median_cpu_seconds':statistics.median(s['cpu_user_seconds']+s['cpu_system_seconds'] for s in samples),
            'median_peak_rss_kib':statistics.median(s['peak_rss_kib'] for s in samples)}
encoded=json.dumps(report,indent=2)+'\n'
if args.output:args.output.write_text(encoded,encoding='utf-8')
print(encoded,end='')
