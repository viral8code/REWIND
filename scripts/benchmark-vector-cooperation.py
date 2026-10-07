#!/usr/bin/env python3
"""Measure source-free vector throughput, RSS and foreground handoffs on Linux."""
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
p.add_argument('--size', type=int, default=262145)
p.add_argument('--repetitions', type=int, default=3)
p.add_argument('--synchronous-only', action='store_true')
p.add_argument('--output', type=Path)
a = p.parse_args()
if not hasattr(os, 'wait4') or not 1 <= a.size <= 16777216 or a.repetitions < 1:
    p.error('requires Linux wait4, size 1..16777216, and positive repetitions')
binary = a.binary.resolve()
common = r'''import std.numeric as n;import std.numericAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let small=List<Int>();small.add(1);let v=List<Float>();v.add(2.0);let scalar=take(n.fromFloat(&small,&v));
let shape=List<Int>();shape.add(SIZE);let input=take(n.broadcastFloat(&scalar,&shape));
let index=List<Int>();index.add(SIZE-1);
'''.replace('SIZE', str(a.size))
variants = {
    'synchronous': 'let scaled=take(n.scale(&input,3.0));let zipped=take(n.zipFloat("add",&scaled,&input));let norm=take(n.norm2(&input));',
    'cooperative': 'let scaled=take(take(await spawn jobs.scale(input,3.0)));let zipped=take(take(await spawn jobs.zipFloat("add",scaled,input)));let norm=take(take(await spawn jobs.norm2(input)));',
    'cooperative_foreground': '''var ticks=0;
let scaling=spawn jobs.scale(input,3.0);while !scaling.isDone(){ticks+=1;task.yieldNow();}let scaled=take(take(await scaling));
let zipping=spawn jobs.zipFloat("add",scaled,input);while !zipping.isDone(){ticks+=1;task.yieldNow();}let zipped=take(take(await zipping));
let norming=spawn jobs.norm2(input);while !norming.isDone(){ticks+=1;task.yieldNow();}let norm=take(take(await norming));Out.println(ticks);'''
}
if a.synchronous_only:
    variants = {'synchronous': variants['synchronous']}
ending = f'assert_eq(take(n.getFloat(&zipped,&index)),8.0);let expected=2.0*take(n.math("sqrt",take({a.size}.toFloatChecked())));assert_eq(norm,expected);Out.println(1);publish;'
report = {'version': subprocess.check_output([str(binary), '--version'], text=True).strip(),
          'size': a.size, 'workload': 'source-free scale, add and norm of broadcast FloatArray, including setup and final-cell validation', 'variants': {}}
with tempfile.TemporaryDirectory(prefix='rewind-vector-benchmark-') as temporary:
    root = Path(temporary)
    for name, body in variants.items():
        source = root / (name + '.rw')
        source.write_text(common + body + ending, encoding='utf-8')
        subprocess.run([str(binary), 'compile', source], cwd=root, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        source.unlink()
        samples = []
        for sample in range(a.repetitions + 1):
            with (root / 'stdout').open('wb') as out, (root / 'stderr').open('wb') as err:
                start = time.monotonic()
                process = subprocess.Popen([str(binary), 'run', name + '.rwc', '--native-work', '1000000000',
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
