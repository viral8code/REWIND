#!/usr/bin/env python3
"""Compare peak RSS and elapsed time for large temporary heap payloads on Linux."""
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
source = ('fn make()->Int effects {} {let xs=List<String>();xs.push("' + 'x' * 524288 +
          '");return xs.len();}\nvar count=0;for i in 0..200 {count+=make();}Out.println(count);publish;')
report = {"workload": "200 temporary List<String> values with 512 KiB payload", "runs": []}
for binary in args.binaries:
    binary = binary.resolve()
    samples = []
    with tempfile.TemporaryDirectory(prefix="rewind-gc-benchmark-") as temporary:
        root = Path(temporary)
        (root / "main.rw").write_text(source, encoding="utf-8")
        version = subprocess.check_output([str(binary), "--version"], text=True).strip()
        for _ in range(args.repetitions):
            with (root / "stdout").open("wb") as output, (root / "stderr").open("wb") as error:
                started = time.monotonic()
                process = subprocess.Popen([str(binary), "run", "main.rw", "--native-work", "1000000000", "--steps", "10000000"], cwd=root, stdout=output, stderr=error)
                _, status, usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)
                elapsed = time.monotonic() - started
            if process.returncode or (root / "stdout").read_bytes() != b"200\n":
                raise RuntimeError((root / "stderr").read_text()[:2000])
            samples.append({"elapsed_seconds": elapsed, "peak_rss_kib": usage.ru_maxrss})
        report["runs"].append({"version": version, "samples": samples,
                              "median_seconds": statistics.median(s["elapsed_seconds"] for s in samples),
                              "median_peak_rss_kib": statistics.median(s["peak_rss_kib"] for s in samples)})
encoded = json.dumps(report, indent=2) + "\n"
if args.output:
    args.output.write_text(encoded, encoding="utf-8")
print(encoded, end="")
