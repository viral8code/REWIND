#!/usr/bin/env python3
"""Compare complete native vector reductions, including validation, on Linux."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binaries", nargs="+", type=Path)
parser.add_argument("--repetitions", type=int, default=3)
parser.add_argument("--output", type=Path)
args = parser.parse_args()
if not hasattr(os, "wait4") or args.repetitions < 1:
    parser.error("requires Linux wait4 and positive repetitions")
source = '''import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(item)=>{return move item;},Err(_)=>{panic("numeric operation failed");}}
}
let shape=List<Int>();shape.push(1048576);
let vector=take(numeric.zerosFloat(&shape));
var total=0.0;
for i in 0..40 {total+=take(numeric.sum(&vector));total+=take(numeric.dot(&vector,&vector));}
assert_eq(total,0.0);Out.println(1);publish;
'''
report = {"workload": "40 sum and dot reductions of a 1048576-element contiguous FloatArray", "runs": []}
for binary in args.binaries:
    binary = binary.resolve()
    samples = []
    with tempfile.TemporaryDirectory(prefix="rewind-numeric-benchmark-") as temporary:
        root = Path(temporary)
        (root / "main.rw").write_text(source, encoding="utf-8")
        version = subprocess.check_output([str(binary), "--version"], text=True).strip()
        for _ in range(args.repetitions):
            with (root / "stdout").open("wb") as output, (root / "stderr").open("wb") as error:
                started = time.monotonic()
                process = subprocess.Popen([str(binary), "run", "main.rw", "--native-work", "10000000000", "--steps", "10000000"], cwd=root, stdout=output, stderr=error)
                _, status, usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)
                elapsed = time.monotonic() - started
            if process.returncode or (root / "stdout").read_bytes() != b"1\n":
                raise RuntimeError((root / "stderr").read_text()[:2000])
            samples.append({"elapsed_seconds": elapsed, "peak_rss_kib": usage.ru_maxrss})
        report["runs"].append({"version": version, "samples": samples,
                              "median_seconds": statistics.median(s["elapsed_seconds"] for s in samples),
                              "median_peak_rss_kib": statistics.median(s["peak_rss_kib"] for s in samples)})
encoded = json.dumps(report, indent=2) + "\n"
if args.output:
    args.output.write_text(encoded, encoding="utf-8")
print(encoded, end="")
