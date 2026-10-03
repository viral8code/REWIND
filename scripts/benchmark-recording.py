#!/usr/bin/env python3
"""Compare debug and compact recording of the same source-free execution on Linux."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", type=Path)
parser.add_argument("--repetitions", type=int, default=3)
parser.add_argument("--output", type=Path)
args = parser.parse_args()
if not hasattr(os, "wait4") or args.repetitions < 1:
    parser.error("requires Linux wait4 and positive repetitions")
binary = args.binary.resolve()
report = {"version": subprocess.check_output([str(binary), "--version"], text=True).strip(),
          "workload": "1000 integer loop iterations from a source-free artifact", "runs": []}
with tempfile.TemporaryDirectory(prefix="rewind-record-benchmark-") as temporary:
    root = Path(temporary)
    (root / "main.rw").write_text("var sum=0;for i in 0..1000 {sum+=i;}Out.println(sum);publish;", encoding="utf-8")
    subprocess.run([str(binary), "compile", "main.rw"], cwd=root, check=True, capture_output=True)
    (root / "main.rw").unlink()
    for mode in ["debug", "compact"]:
        samples = []
        for _ in range(args.repetitions):
            with (root / "stdout").open("wb") as output, (root / "stderr").open("wb") as error:
                started = time.monotonic()
                process = subprocess.Popen([str(binary), "run", "main.rwc", "--record", "trace.json", "--record-mode", mode], cwd=root, stdout=output, stderr=error)
                _, status, usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)
                elapsed = time.monotonic() - started
            if process.returncode or (root / "stdout").read_bytes() != b"499500\n":
                raise RuntimeError((root / "stderr").read_text()[:2000])
            replay = subprocess.run([str(binary), "replay", "trace.json"], cwd=root, capture_output=True, check=True)
            if replay.stdout != b"499500\n":
                raise RuntimeError("replay output mismatch")
            samples.append({"elapsed_seconds": elapsed, "peak_rss_kib": usage.ru_maxrss,
                            "trace_bytes": (root / "trace.json").stat().st_size})
        report["runs"].append({"mode": mode, "samples": samples,
                              "median_seconds": statistics.median(s["elapsed_seconds"] for s in samples),
                              "median_peak_rss_kib": statistics.median(s["peak_rss_kib"] for s in samples),
                              "median_trace_bytes": statistics.median(s["trace_bytes"] for s in samples)})
encoded = json.dumps(report, indent=2) + "\n"
if args.output:
    args.output.write_text(encoded, encoding="utf-8")
print(encoded, end="")
