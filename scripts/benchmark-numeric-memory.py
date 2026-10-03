#!/usr/bin/env python3
"""Measure source-free numeric sharing and update churn on Linux."""
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
parser.add_argument("--baseline", type=Path)
parser.add_argument("--iterations", type=int, default=100000)
parser.add_argument("--repetitions", type=int, default=3)
parser.add_argument("--output", type=Path)
args = parser.parse_args()
if not hasattr(os, "wait4") or not 1 <= args.iterations <= 1000000 or args.repetitions < 1:
    parser.error("requires Linux wait4, iterations 1..1000000, and positive repetitions")

common = """import std.numeric as numeric;
fn take<T,E>(r:Result<T,E>)->T effects {} {
 match move r{Ok(v)=>{return move v;},Err(_)=>{panic("numeric failure");}}
}
let shape=List<Int>();shape.add(1000000);
"""
array = "var buffer=take(numeric.zerosFloat(&shape));let coordinate=List<Int>();coordinate.add(0);\n"
churn = f"for i in 0..{args.iterations} {{buffer=take(numeric.withFloat(&buffer,&coordinate,1.0));}}\n"
checkpoints = "".join(
    f"commit c{i};buffer=take(numeric.withFloat(&buffer,&coordinate,{i+1}.0));\n"
    for i in range(64)
)
checkpoints += "Out.println(take(numeric.getFloat(&buffer,&coordinate)));publish;revert c0;Out.println(take(numeric.getFloat(&buffer,&coordinate)));publish;\n"
checkpoints += "".join(f"drop c{i};\n" for i in range(64))
sources = {
    "churn": (common + array + churn + "Out.println(take(numeric.getFloat(&buffer,&coordinate)));publish;", b"1\n"),
    "churn_with_checkpoint": (common + array + "commit retained;\n" + churn + "Out.println(take(numeric.getFloat(&buffer,&coordinate)));publish;revert retained;Out.println(take(numeric.getFloat(&buffer,&coordinate)));publish;drop retained;", b"1\n0\n"),
    "checkpoint_versions": (common + array + checkpoints, b"64\n0\n"),
    "large_task": (common + "import std.numericAsync as jobs;import std.task as task;\nlet zeros=take(numeric.zerosFloat(&shape));let ones=take(numeric.mapFloat(\"exp\",&zeros));let work=spawn jobs.dot(ones,ones);task.yieldNow();assert(!work.isDone());Out.println(take(take(await work)));publish;", b"1000000\n"),
}
binaries = {"candidate": args.binary.resolve()}
if args.baseline:
    binaries["baseline"] = args.baseline.resolve()
report = {"iterations": args.iterations, "elements": 1000000,
          "workload": "source-free FloatArray, including initialization, validation and optional checkpoints; compilation excluded", "binaries": {}}
with tempfile.TemporaryDirectory(prefix="rewind-numeric-memory-") as temporary:
    for label, binary in binaries.items():
        root = Path(temporary) / label
        root.mkdir()
        result = {"version": subprocess.check_output([str(binary), "--version"], text=True).strip(), "workloads": {}}
        report["binaries"][label] = result
        for name, (source, expected) in sources.items():
            path = root / (name + ".rw")
            path.write_text(source, encoding="utf-8")
            compiled = subprocess.run([str(binary), "compile", str(path)], cwd=root, capture_output=True)
            if compiled.returncode:
                raise RuntimeError(compiled.stderr.decode(errors="replace")[:2000])
            path.unlink()
            samples = []
            failure = None
            for sample in range(args.repetitions + 1):
                with (root / "stdout").open("wb") as output, (root / "stderr").open("wb") as error:
                    started = time.monotonic()
                    process = subprocess.Popen([str(binary), "run", name + ".rwc", "--history-memory", "64MiB", "--native-work", "1000000000", "--steps", "100000000", "--task-steps", "100000000"], cwd=root, stdout=output, stderr=error)
                    _, status, usage = os.wait4(process.pid, 0)
                    process.returncode = os.waitstatus_to_exitcode(status)
                    elapsed = time.monotonic() - started
                if process.returncode:
                    failure = {"exit": process.returncode, "diagnostic": (root / "stderr").read_text()[:2000]}
                    if label == "candidate":
                        raise RuntimeError(failure)
                    break
                actual = (root / "stdout").read_bytes()
                assert actual == expected, (name, actual)
                if sample:
                    samples.append({"elapsed_seconds": elapsed, "peak_rss_kib": usage.ru_maxrss})
            result["workloads"][name] = {"samples": samples, "failure": failure}
            if samples:
                result["workloads"][name].update(
                    median_seconds=statistics.median(s["elapsed_seconds"] for s in samples),
                    median_peak_rss_kib=statistics.median(s["peak_rss_kib"] for s in samples))
encoded = json.dumps(report, indent=2) + "\n"
if args.output:
    args.output.write_text(encoded, encoding="utf-8")
print(encoded, end="")
