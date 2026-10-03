#!/usr/bin/env python3
"""Measure source-free GUI model rendering on Linux; no native surface or input fixture."""
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
parser.add_argument("--iterations", type=int, nargs="+", default=[50, 500])
parser.add_argument("--repetitions", type=int, default=3)
parser.add_argument("--output", type=Path)
args = parser.parse_args()
if not hasattr(os, "wait4") or any(not 1 <= n <= 10000 for n in args.iterations) or args.repetitions < 1:
    parser.error("requires Linux wait4, iterations 1..10000 and positive repetitions")
source = r'''import std.gui as gui;import std.guiForm as form;import std.guiTable as table;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("GUI benchmark failed");}}}
let schema=List<form.Field>();schema.add(form.Field("value","Value",form.Kind::Text,true,128,"initial"));schema.add(form.Field("count","Count",form.Kind::Integer(0,1000),true,12,"13"));schema.add(form.Field("active","Active",form.Kind::Check,false,5,"false"));let model=take(form.create(&schema,3));
let headers=List<String>();headers.add("Row");headers.add("Value");let cells=List<String>();for i in 0..1000{cells.add(i.format());cells.add("row "+i.format());}let grid=take(table.create(freeze(headers),freeze(cells),4));
var inputView=take(form.render(&model,"Form",640,300,72,"f"));var gridView=take(table.render(&grid,"Rows",640,240,30,"t"));
let arguments=Args.all();let iterations=take(stdParseInt(arguments.get(0),10));commit retained;
for i in 0..iterations {take(form.setValue(&mut model,"value",i.format()));take(table.select(&mut grid,i%1000));inputView=take(form.render(&model,"Form",640,300,72,"f"));gridView=take(table.render(&grid,"Rows",640,240,30,"t"));assert(take(gui.scene(&inputView)).byteLen()<65536);assert(take(gui.scene(&gridView)).byteLen()<65536);}
Out.println(table.rows(&grid));publish;drop retained;
'''
binaries = {"candidate": args.binary.resolve()}
if args.baseline:
    binaries["baseline"] = args.baseline.resolve()
report = {"rows": 1000, "visible_rows": 4, "form_fields": 3, "repetitions": args.repetitions,
          "workload": "source-free model updates, View construction and JSON scene serialization; initialization included, compilation excluded; one retained checkpoint; no native window", "binaries": {}}
with tempfile.TemporaryDirectory(prefix="rewind-gui-model-") as temporary:
    for label, binary in binaries.items():
        root = Path(temporary) / label
        root.mkdir()
        path = root / "main.rw"
        path.write_text(source, encoding="utf-8")
        compiled = subprocess.run([str(binary), "compile", str(path)], cwd=root, capture_output=True)
        if compiled.returncode:
            raise RuntimeError(compiled.stderr.decode(errors="replace")[:2000])
        path.unlink()
        result = {"version": subprocess.check_output([str(binary), "--version"], text=True).strip(), "iterations": {}}
        report["binaries"][label] = result
        for count in args.iterations:
            samples = []
            for sample in range(args.repetitions + 1):
                with (root / "stdout").open("wb") as output, (root / "stderr").open("wb") as error:
                    started = time.monotonic()
                    process = subprocess.Popen([str(binary), "run", "main.rwc", "--history-memory", "64MiB", "--native-work", "1000000000", "--steps", "1000000000", "--", str(count)], cwd=root, stdout=output, stderr=error)
                    _, status, usage = os.wait4(process.pid, 0)
                    process.returncode = os.waitstatus_to_exitcode(status)
                    elapsed = time.monotonic() - started
                if process.returncode:
                    raise RuntimeError((root / "stderr").read_text()[:2000])
                assert (root / "stdout").read_bytes() == b"1000\n"
                if sample:
                    samples.append({"elapsed_seconds": elapsed, "peak_rss_kib": usage.ru_maxrss})
            result["iterations"][str(count)] = {"samples": samples,
                "median_seconds": statistics.median(s["elapsed_seconds"] for s in samples),
                "median_peak_rss_kib": statistics.median(s["peak_rss_kib"] for s in samples)}
encoded = json.dumps(report, indent=2) + "\n"
if args.output:
    args.output.write_text(encoded, encoding="utf-8")
print(encoded, end="")
