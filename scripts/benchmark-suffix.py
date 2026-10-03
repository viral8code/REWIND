#!/usr/bin/env python3
"""Measure native suffix/LCP construction on repeated bytes in a source-free SDK."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument("binary", type=Path)
parser.add_argument("--size", type=int, default=1048576)
parser.add_argument("--repetitions", type=int, default=3)
parser.add_argument("--output", type=Path)
args = parser.parse_args()
binary = args.binary.resolve()
assert 1 <= args.size <= 1048576
source = f'''
import std.suffix as suffix;import std.numeric as numeric;
fn take<T,E>(r:Result<T,E>)->T effects {{}} {{match move r{{Ok(v)=>{{return move v;}},Err(_)=>{{panic("unexpected error");}}}}}}
let input=take(File.readBytes("input.dat"));let index=take(suffix.build(input));
assert_eq(suffix.length(&index),{args.size});let sa=suffix.order(&index);let lcp=suffix.lcp(&index);
let indices=List<Int>();indices.add(0);assert_eq(numeric.getInt(&sa,&indices),Ok({args.size-1}));
indices.set(0,{args.size-1});assert_eq(numeric.getInt(&lcp,&indices),Ok({args.size-1}));
Out.println(1);publish;
'''
samples = []
with tempfile.TemporaryDirectory(prefix="rewind-suffix-") as temporary:
    root = Path(temporary)
    (root / "main.rw").write_text(source, encoding="utf-8")
    (root / "input.dat").write_bytes(b"a" * args.size)
    subprocess.run([str(binary), "compile", "main.rw", "--allow-effects", "fileRead"], cwd=root, check=True, capture_output=True)
    (root / "main.rw").unlink()
    for iteration in range(args.repetitions + 1):
        with (root / "stdout").open("wb") as output, (root / "stderr").open("wb") as error:
            started = time.monotonic()
            process = subprocess.Popen([str(binary), "run", "main.rwc", "--allow-effects", "fileRead", "--native-work", "1000000000"], cwd=root, stdout=output, stderr=error)
            _, status, usage = os.wait4(process.pid, 0)
            process.returncode = os.waitstatus_to_exitcode(status)
            elapsed = time.monotonic() - started
        if process.returncode or (root / "stdout").read_bytes() != b"1\n":
            raise RuntimeError((root / "stderr").read_text()[:2000])
        if iteration:
            samples.append({"elapsed_seconds": elapsed, "peak_rss_kib": usage.ru_maxrss})
report = {"version": subprocess.check_output([str(binary), "--version"], text=True).strip(),
          "workload": "source-free repeated-byte suffix array and LCP, including file read and validation",
          "bytes": args.size, "warmups": 1, "samples": samples,
          "median_seconds": statistics.median(s["elapsed_seconds"] for s in samples),
          "median_peak_rss_kib": statistics.median(s["peak_rss_kib"] for s in samples)}
encoded = json.dumps(report, indent=2) + "\n"
if args.output:
    args.output.write_text(encoded, encoding="utf-8")
print(encoded, end="")
