#!/usr/bin/env python3
"""Compare source-free matrix throughput and foreground progress on Linux."""
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
parser.add_argument("--size", type=int, default=256)
parser.add_argument("--repetitions", type=int, default=3)
parser.add_argument("--output", type=Path)
args = parser.parse_args()
if not hasattr(os, "wait4") or not 1 <= args.size <= 1024 or args.repetitions < 1:
    parser.error("requires Linux wait4, size 1..1024, and positive repetitions")
binary = args.binary.resolve()
common = """import std.numeric as numeric;
import std.numericAsync as jobs;
import std.task as task;
fn take<T,E>(value:Result<T,E>)->T effects {} {
 match move value{Ok(item)=>{return move item;},Err(_)=>{panic("numeric failure");}}}
let shape=List<Int>();shape.add(SIZE);shape.add(SIZE);
let zeros=take(numeric.zerosFloat(&shape));let ones=take(numeric.mapFloat("exp",&zeros));
""".replace("SIZE", str(args.size))
variants = {
    "synchronous": "let product=take(numeric.matmul(&ones,&ones));",
    "cooperative": "let work=spawn jobs.matmul(ones,ones);let product=take(take(await work));",
    "cooperative_foreground": "let work=spawn jobs.matmul(ones,ones);var ticks=0;while !work.isDone(){ticks+=1;task.yieldNow();}let product=take(take(await work));Out.println(ticks);",
}
ending = f"let index=List<Int>();index.add(0);index.add(0);assert_eq(take(numeric.getFloat(&product,&index)),{args.size}.0);Out.println(1);publish;"
report = {"version": subprocess.check_output([str(binary), "--version"], text=True).strip(),
          "size": args.size, "workload": "source-free square FloatArray matrix of ones, including initialization and validation", "variants": {}}
with tempfile.TemporaryDirectory(prefix="rewind-cooperation-benchmark-") as temporary:
    root = Path(temporary)
    for name, body in variants.items():
        source = root / (name + ".rw")
        source.write_text(common + body + ending, encoding="utf-8")
        subprocess.run([str(binary), "compile", str(source)], cwd=root, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        source.unlink()
        samples = []
        for sample in range(args.repetitions + 1):
            with (root / "stdout").open("wb") as output, (root / "stderr").open("wb") as error:
                started = time.monotonic()
                process = subprocess.Popen([str(binary), "run", name + ".rwc", "--native-work", "10000000000", "--steps", "10000000", "--task-steps", "10000000"], cwd=root, stdout=output, stderr=error)
                _, status, usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)
                elapsed = time.monotonic() - started
            if process.returncode:
                raise RuntimeError((root / "stderr").read_text()[:2000])
            lines = (root / "stdout").read_text().splitlines()
            assert lines[-1] == "1", lines
            ticks = int(lines[0]) if name == "cooperative_foreground" else None
            if ticks is not None:
                assert ticks > 0
            if sample:
                samples.append({"elapsed_seconds": elapsed, "peak_rss_kib": usage.ru_maxrss, "foreground_handoffs": ticks})
        report["variants"][name] = {"samples": samples,
            "median_seconds": statistics.median(s["elapsed_seconds"] for s in samples),
            "median_peak_rss_kib": statistics.median(s["peak_rss_kib"] for s in samples)}
encoded = json.dumps(report, indent=2) + "\n"
if args.output:
    args.output.write_text(encoded, encoding="utf-8")
print(encoded, end="")
