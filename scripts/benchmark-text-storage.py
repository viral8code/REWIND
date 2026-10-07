#!/usr/bin/env python3
"""Serial source-free immutable text copies: wall, CPU and peak RSS."""
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
        'workload':'source-free string argument/return and local copies plus byte-length checks; compilation excluded',
        'variants':{}}
variants={'short':('brief label',16384),'large':('日本語 / text\n'*4096,2048),'mixed':('日本語 / text\n'*2048,4096)}
with tempfile.TemporaryDirectory(prefix='rewind-text-benchmark-') as temp:
    root=Path(temp)
    for name,(text,iterations) in variants.items():
        literal=json.dumps(text,ensure_ascii=False);length=len(text.encode('utf-8'))
        if name=='mixed':
            setup=f'let rows=List<String>();rows.add("brief label");rows.add({literal});'
            expression='rows.get(i%2)';expected=iterations//2*(len('brief label')+length)
        else:setup=f'let original={literal};';expression='original';expected=iterations*length
        source='fn identity(value:String)->String effects {}{return value;}'+setup+f'var total=0;for i in 0..{iterations}{{let copied=identity({expression});total+=copied.byteLen();}}assert_eq(total,{expected});Out.println(total);publish;'
        entry=root/(name+'.rw');entry.write_text(source,encoding='utf-8')
        result=subprocess.run([str(binary),'compile',str(entry)],cwd=root,capture_output=True,timeout=90)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace')[:2000])
        entry.unlink()
        samples=[]
        for sample in range(args.repetitions+1):
            with (root/'out').open('wb') as out,(root/'err').open('wb') as err:
                started=time.monotonic()
                process=subprocess.Popen([str(binary),'run',name+'.rwc','--steps','10000000','--native-work','10000000000'],cwd=root,stdout=out,stderr=err)
                _,status,usage=os.wait4(process.pid,0);process.returncode=os.waitstatus_to_exitcode(status)
                elapsed=time.monotonic()-started
            if process.returncode:raise RuntimeError((root/'err').read_text(errors='replace')[:2000])
            assert (root/'out').read_text().strip()==str(expected)
            if sample:samples.append({'elapsed_seconds':elapsed,'cpu_user_seconds':usage.ru_utime,'cpu_system_seconds':usage.ru_stime,'peak_rss_kib':usage.ru_maxrss})
        report['variants'][name]={'text_bytes':length,'iterations':iterations,'samples':samples,
            'median_seconds':statistics.median(s['elapsed_seconds'] for s in samples),
            'median_cpu_seconds':statistics.median(s['cpu_user_seconds']+s['cpu_system_seconds'] for s in samples),
            'median_peak_rss_kib':statistics.median(s['peak_rss_kib'] for s in samples)}
encoded=json.dumps(report,indent=2)+'\n'
if args.output:args.output.write_text(encoded,encoding='utf-8')
print(encoded,end='')
