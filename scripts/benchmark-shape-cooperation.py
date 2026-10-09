#!/usr/bin/env python3
"""Compare identical materialized inputs for synchronous and cooperative shape kernels."""
import argparse
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import time

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('binary', type=Path)
p.add_argument('--cooperative', action='store_true')
p.add_argument('--sizes', type=int, nargs='+', default=[65536, 262144, 1048576])
p.add_argument('--repetitions', type=int, default=3)
p.add_argument('--output', type=Path)
a = p.parse_args()
if not hasattr(os, 'wait4') or a.repetitions < 1 or any(n < 256 or n > 1048576 or n % 256 for n in a.sizes):
    p.error('requires Linux wait4, positive repetitions and sizes divisible by 256 within 256..1048576')
b = a.binary.resolve()
report = {'version': subprocess.check_output([str(b), '--version'], text=True).strip(),
          'cooperative': a.cooperative,
          'workload': 'identical materialized nonzero FloatArray input; reversed logical copy or 256-row contraction; input preparation and startup/profile included, compilation excluded; one warmup then serial repetitions',
          'cases': {}}
with tempfile.TemporaryDirectory(prefix='rewind-shape-bench-') as d:
    root = Path(d)
    for n in a.sizes:
        for operation in ['copy', 'contraction']:
            prefix = '''import std.numeric as n;import std.numericIndex as index;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
fn dims(length:Int)->List<Int> effects {} {let d=List<Int>();d.add(length);return move d;}
let one=dims(1);let values=List<Float>();values.add(0.5);let scalar=take(n.fromFloat(&one,&values));
'''
            if a.cooperative:
                prefix = 'import std.numericShapeAsync as jobs;'+prefix
            prefix += f'let shape=dims({n});let broadcast=take(n.broadcastFloat(&scalar,&shape));let materialized=take(n.materializeFloat(&broadcast));'
            if operation == 'copy':
                prefix += f'let x=take(n.sliceFloat(&materialized,0,{n-1},{n},-1));let target=dims({n});'
                call = 'take(take(await spawn jobs.reshapeLogical(x,move target)))' if a.cooperative else 'take(stdNumericReshapeLogical(&x,&target))'
                expected, last = '0.5', n-1
            else:
                prefix += f'let matrix=List<Int>();matrix.add(256);matrix.add({n//256});let x=take(n.reshapeFloat(&materialized,&matrix));let target=List<Int>();target.add(1);target.add({n//256});'
                call = 'take(take(await spawn jobs.sumToShape(x,move target)))' if a.cooperative else 'take(n.sumToShape(&x,&target))'
                expected, last = '128.0', n//256-1
            source = prefix+f'let answer={call};assert_eq(take(index.getFloat(&answer,0)),{expected});assert_eq(take(index.getFloat(&answer,{last})),{expected});assert_eq(take(n.sum(&answer)),{n*0.5});Out.println(true);publish;'
            entry = root/'main.rw'
            entry.write_text(source)
            compiled = subprocess.run([str(b), 'compile', 'main.rw'], cwd=root, capture_output=True, timeout=90)
            if compiled.returncode:
                raise RuntimeError(compiled.stderr.decode(errors='replace')[:3000])
            entry.unlink()
            shutil.rmtree(root/'.rewind', ignore_errors=True)
            samples = []
            for sample in range(a.repetitions+1):
                with (root/'out').open('wb') as out, (root/'err').open('wb') as err:
                    start = time.monotonic()
                    child = subprocess.Popen([str(b), 'profile', 'main.rwc', '--native-work', '1000000000', '--history-memory', '256MiB', '--steps', '100000000', '--task-steps', '100000000'], cwd=root, stdout=out, stderr=err)
                    _, status, usage = os.wait4(child.pid, 0)
                    child.returncode = os.waitstatus_to_exitcode(status)
                    elapsed = time.monotonic()-start
                if child.returncode:
                    raise RuntimeError((root/'err').read_text(errors='replace')[:3000])
                assert (root/'out').read_bytes() == b'true\n'
                if sample:
                    samples.append({'wall_seconds': elapsed, 'cpu_seconds': usage.ru_utime+usage.ru_stime, 'peak_rss_kib': usage.ru_maxrss})
            report['cases'][f'{operation}-{n}'] = {'samples': samples, **{'median_'+key: statistics.median(s[key] for s in samples) for key in samples[0]}}
encoded = json.dumps(report, indent=2)+'\n'
if a.output:
    a.output.write_text(encoded)
print(encoded, end='')
