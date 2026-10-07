#!/usr/bin/env python3
"""Serial source-free LU throughput, CPU, RSS and foreground handoff measurement."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('binary', type=Path)
p.add_argument('--size', type=int, default=129)
p.add_argument('--repetitions', type=int, default=3)
p.add_argument('--synchronous-only', action='store_true')
p.add_argument('--output', type=Path)
a = p.parse_args()
if not hasattr(os, 'wait4') or not 1 <= a.size <= 512 or a.repetitions < 1:
    p.error('requires Linux wait4, size 1..512, and positive repetitions')
binary = a.binary.resolve()
common = r'''import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let ms=List<Int>();ms.add(SIZE);ms.add(SIZE);let zero=take(n.zerosFloat(&ms));let ones=take(n.mapFloat("exp",&zero));
var builder=take(n.scale(&ones,0.01));
for i in 0..SIZE{let index=List<Int>();index.add(i);index.add((i+1)%SIZE);builder=take(n.withFloat(&builder,&index,100.0));}
let matrix=builder;let vs=List<Int>();vs.add(SIZE);let vz=take(n.zerosFloat(&vs));let vo=take(n.mapFloat("exp",&vz));
let rhs=take(n.scale(&vo,RIGHT));
'''.replace('SIZE', str(a.size)).replace('RIGHT', repr(100.0 + (a.size-1)*0.01))
variants = {
    'synchronous': 'let solution=take(n.solve(&matrix,&rhs,1e-13));',
    'cooperative': 'let solution=take(take(await spawn jobs.solve(matrix,rhs,1e-13)));',
    'cooperative_foreground': 'var ticks=0;let work=spawn jobs.solve(matrix,rhs,1e-13);while !work.isDone(){ticks+=1;task.yieldNow();}let solution=take(take(await work));Out.println(ticks);'
}
if a.synchronous_only:
    variants = {'synchronous': variants['synchronous']}
ending = f'''for i in 0..{a.size}{{let index=List<Int>();index.add(i);let value=take(n.getFloat(&solution,&index));assert(value>0.999999999 && value<1.000000001);}}Out.println(1);publish;'''
report = {'version': subprocess.check_output([str(binary), '--version'], text=True).strip(),
          'size': a.size, 'workload': 'source-free partial-pivot LU of a cyclically permuted diagonally dominant system, including setup and every solution component check',
          'variants': {}}
with tempfile.TemporaryDirectory(prefix='rewind-solve-benchmark-') as temporary:
    root = Path(temporary)
    for name, body in variants.items():
        source = root / (name + '.rw')
        source.write_text(common + body + ending, encoding='utf-8')
        result = subprocess.run([str(binary), 'compile', source], cwd=root,
                                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        if result.returncode:
            raise RuntimeError(result.stderr.decode('utf-8', errors='replace')[:2000])
        source.unlink()
        samples = []
        for sample in range(a.repetitions + 1):
            with (root / 'stdout').open('wb') as out, (root / 'stderr').open('wb') as err:
                start = time.monotonic()
                process = subprocess.Popen([str(binary), 'run', name + '.rwc', '--native-work', '10000000000',
                                            '--steps', '10000000', '--task-steps', '10000000'], cwd=root, stdout=out, stderr=err)
                _, status, usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)
                elapsed = time.monotonic() - start
            if process.returncode:
                raise RuntimeError((root / 'stderr').read_text()[:2000])
            lines = (root / 'stdout').read_text().splitlines()
            assert lines[-1] == '1', lines
            ticks = int(lines[0]) if name == 'cooperative_foreground' else None
            if ticks is not None:
                assert ticks > 0
            if sample:
                samples.append({'elapsed_seconds': elapsed, 'cpu_user_seconds': usage.ru_utime,
                                'cpu_system_seconds': usage.ru_stime, 'peak_rss_kib': usage.ru_maxrss,
                                'foreground_handoffs': ticks})
        report['variants'][name] = {'samples': samples,
            'median_seconds': statistics.median(s['elapsed_seconds'] for s in samples),
            'median_cpu_seconds': statistics.median(s['cpu_user_seconds'] + s['cpu_system_seconds'] for s in samples),
            'median_peak_rss_kib': statistics.median(s['peak_rss_kib'] for s in samples)}
encoded = json.dumps(report, indent=2) + '\n'
if a.output:
    a.output.write_text(encoded, encoding='utf-8')
print(encoded, end='')
