#!/usr/bin/env python3
"""Serial source-free persistent scalar list checkpoint histories: wall, CPU and peak RSS."""
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
parser.add_argument('--include-large',action='store_true',help='also measure 65,536 scalar values and 32 changed checkpoint roots')
args=parser.parse_args()
if not hasattr(os,'wait4') or args.repetitions<1:parser.error('requires Linux wait4 and positive repetitions')
binary=args.binary.resolve()
report={'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),
        'workload':'source-free changed list checkpoint roots; compilation excluded; startup included',
        'variants':{}}
variants={'small_history':(4096,32),'larger_history':(8192,128)}
if args.include_large:variants['large_scalar_history']=(65536,32)
with tempfile.TemporaryDirectory(prefix='rewind-container-benchmark-') as temp:
    root=Path(temp)
    for name,(length,iterations) in variants.items():
        source=f'let items=List<Int>();for i in 0..{length}{{items.add(i);}}commit original;'
        for i in range(iterations):source+=f'items.set(0,{i});commit snapshot{i};'
        source+='Out.println(items.get(0));publish;revert original;Out.println(items.get(0));publish;'
        for i in range(iterations):source+=f'drop snapshot{i};'
        source+='drop original;';expected=f'{iterations-1}\n0'
        entry=root/(name+'.rw');entry.write_text(source,encoding='utf-8')
        result=subprocess.run([str(binary),'compile',str(entry)],cwd=root,capture_output=True,timeout=90)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace')[:2000])
        entry.unlink()
        samples=[]
        for sample in range(args.repetitions+1):
            with (root/'out').open('wb') as out,(root/'err').open('wb') as err:
                started=time.monotonic()
                process=subprocess.Popen([str(binary),'profile',name+'.rwc','--steps','20000000','--native-work','10000000000','--history-memory','512MiB'],cwd=root,stdout=out,stderr=err)
                _,status,usage=os.wait4(process.pid,0);process.returncode=os.waitstatus_to_exitcode(status)
                elapsed=time.monotonic()-started
            if process.returncode:raise RuntimeError((root/'err').read_text(errors='replace')[:2000])
            assert (root/'out').read_text().strip()==expected
            profile=json.loads(next(line for line in reversed((root/'err').read_text().splitlines()) if line.startswith('{')))
            if sample:samples.append({'shared_payloads':profile.get('shared_payloads'),'gc':profile.get('gc'),'elapsed_seconds':elapsed,'cpu_user_seconds':usage.ru_utime,'cpu_system_seconds':usage.ru_stime,'peak_rss_kib':usage.ru_maxrss})
        report['variants'][name]={'list_elements':length,'iterations':iterations,'samples':samples,
            'median_seconds':statistics.median(s['elapsed_seconds'] for s in samples),
            'median_cpu_seconds':statistics.median(s['cpu_user_seconds']+s['cpu_system_seconds'] for s in samples),
            'median_peak_rss_kib':statistics.median(s['peak_rss_kib'] for s in samples)}
encoded=json.dumps(report,indent=2)+'\n'
if args.output:args.output.write_text(encoded,encoding='utf-8')
print(encoded,end='')
