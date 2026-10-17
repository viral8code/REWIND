#!/usr/bin/env python3
"""PERF-02 / NUM-01 / PERF-03 / PERF-05 local cases on Windows (see docs/v2-next-performance.md).

Each case is compiled once (compilation excluded), then run `--warmup` + `--repetitions` times alternating
between binaries. Output: JSON with environment, per-sample wall/cpu/peak working set/peak commit, and
VM-side counters from `rewind profile` where available. Unmeasured counters are omitted, never zeroed.
"""
import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).parent))
import perf_win

COMMON = """import std.numeric as n;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("numeric failure");}}}
"""


def shape1(count):
    return f"let shape=List<Int>();shape.add({count});\n"


def wrap(src):
    """Split leading `import`/`fn` helper lines from the statements and wrap statements in fn main (language 3)."""
    head, body, leading = [], [], True
    for ln in src.split("\n"):
        if leading and (ln.startswith("import ") or ln.startswith("fn ") or ln == ""):
            head.append(ln)
        else:
            leading = False
            body.append(ln)
    return "\n".join(head) + "\nfn main()->Int {\n" + "\n".join(body) + "\nreturn 0;\n}\n"


def cases(size):
    c = {}
    # --- PERF-02 MEM-01 / PERF-03: fixed-root churn of large temporary heap payloads
    c["gc_temp_list_string"] = ("fn make()->Int effects {} {let xs=List<String>();xs.push(\"" + "x" * 524288 +
                                "\");return xs.len();}\nvar count=0;for i in 0..200 {count+=make();}Out.println(count);publish;", b"200\n")
    # --- NUM-01 baseline pieces
    c["num_list_to_array"] = (COMMON + shape1(size) +
                              "let zero=take(n.zerosFloat(&shape));let prepared=take(n.affine(&zero,0.0,0.5));let values=take(n.valuesFloat(&prepared));"
                              "let array=take(n.fromFloat(&shape,&values));"
                              f"assert_eq(take(n.sum(&array)),{size}.0*0.5);Out.println(true);publish;", b"true\n")
    c["num_pipeline_chain"] = (COMMON + shape1(size) +
                               "let z=take(n.zerosFloat(&shape));let a=take(n.affine(&z,1.0,0.5));let b=take(n.scale(&a,2.0));"
                               "let c=take(n.activation(\"relu\",&b));let d=take(n.mapFloat(\"exp\",&c));let e=take(n.zipFloat(\"add\",&d,&d));"
                               "let s=take(n.sum(&e));Out.println(s>0.0);publish;", b"true\n")
    c["num_pipeline_views"] = (COMMON + shape1(size) +
                               "let z=take(n.zerosFloat(&shape));let a=take(n.affine(&z,1.0,0.5));"
                               f"let rs=List<Int>();rs.add({size//2});rs.add(2);let m=take(n.reshapeFloat(&a,&rs));let axes=List<Int>();axes.add(1);axes.add(0);let t=take(n.transposeFloat(&m,&axes));"
                               "let s0=take(n.sum(&t));let mat=take(n.materializeFloat(&t));let s=take(n.sum(&mat));"
                               "Out.println(s>0.0 && s0>0.0);publish;", b"true\n")
    c["num_view_sum_only"] = (COMMON + shape1(size) +
                              "let z=take(n.zerosFloat(&shape));let a=take(n.affine(&z,1.0,0.5));"
                              f"let rs=List<Int>();rs.add({size//2});rs.add(2);let m=take(n.reshapeFloat(&a,&rs));"
                              "let axes=List<Int>();axes.add(1);axes.add(0);let t=take(n.transposeFloat(&m,&axes));"
                              "var s=0.0;for i in 0..20 {s+=take(n.sum(&t));}Out.println(s>0.0);publish;", b"true\n")
    c["num_materialize_sum"] = (COMMON + shape1(size) +
                                "let z=take(n.zerosFloat(&shape));let a=take(n.affine(&z,1.0,0.5));"
                                f"let rs=List<Int>();rs.add({size//2});rs.add(2);let m=take(n.reshapeFloat(&a,&rs));"
                                "let axes=List<Int>();axes.add(1);axes.add(0);let t=take(n.transposeFloat(&m,&axes));"
                                "let mat=take(n.materializeFloat(&t));var s=0.0;for i in 0..20 {s+=take(n.sum(&mat));}Out.println(s>0.0);publish;", b"true\n")
    c["num_withfloat_churn"] = (COMMON + "let shape=List<Int>();shape.add(1000000);var buffer=take(n.zerosFloat(&shape));let coord=List<Int>();coord.add(0);\n"
                                "for i in 0..100000 {buffer=take(n.withFloat(&buffer,&coord,1.0));}Out.println(take(n.getFloat(&buffer,&coord)));publish;", b"1\n")
    c["num_withfloat_churn_checkpoint"] = (COMMON + "let shape=List<Int>();shape.add(1000000);var buffer=take(n.zerosFloat(&shape));let coord=List<Int>();coord.add(0);\ncommit retained;\n"
                                           "for i in 0..100000 {buffer=take(n.withFloat(&buffer,&coord,1.0));}Out.println(take(n.getFloat(&buffer,&coord)));publish;revert retained;"
                                           "Out.println(take(n.getFloat(&buffer,&coord)));publish;drop retained;", b"1\n0\n")
    return c


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()[:16]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("binaries", nargs="+", type=Path)
    p.add_argument("--cases", nargs="*")
    p.add_argument("--size", type=int, default=1048576)
    p.add_argument("--warmup", type=int, default=3)
    p.add_argument("--repetitions", type=int, default=30)
    p.add_argument("--output", type=Path)
    a = p.parse_args()
    all_cases = cases(a.size)
    names = a.cases or list(all_cases)
    report = {"env": {"os": platform.platform(), "cpu": platform.processor(), "python": sys.version.split()[0],
                      "cpu_count": os.cpu_count()},
              "size": a.size, "warmup": a.warmup, "repetitions": a.repetitions,
              "memory_definitions": "peak_ws_kib=PeakWorkingSetSize; peak_commit_kib=PeakPagefileUsage (private commit)",
              "binaries": {str(b): {"sha256_16": sha(b), "version": subprocess.check_output([str(b), "--version"], text=True).strip()} for b in a.binaries},
              "cases": {}}
    flags = ["--history-memory", "1GiB", "--native-work", "100000000000", "--steps", "1000000000", "--task-steps", "1000000000"]
    with tempfile.TemporaryDirectory(prefix="rewind-perf-") as d:
        root = Path(d)
        for name in names:
            source, expected = all_cases[name]
            source = wrap(source)
            res = {}
            for bi, b in enumerate(a.binaries):
                r = root / f"{name}-{bi}"
                r.mkdir()
                (r / "main.rw").write_text(source, encoding="utf-8")
                done = subprocess.run([str(b.resolve()), "compile", "main.rw"], cwd=r, capture_output=True)
                if done.returncode:
                    raise RuntimeError(name + ": " + done.stderr.decode(errors="replace")[:2000])
                (r / "main.rw").unlink()
                shutil.rmtree(r / ".rewind", ignore_errors=True)
            samples = {str(b): [] for b in a.binaries}
            for i in range(a.warmup + a.repetitions):
                for bi, b in enumerate(a.binaries):
                    r = root / f"{name}-{bi}"
                    with (r / "out").open("wb") as o, (r / "err").open("wb") as e:
                        m = perf_win.run([str(b.resolve()), "run", "main.rwc"] + flags, cwd=r, stdout=o, stderr=e)
                    if m["returncode"] or (r / "out").read_bytes().replace(b"\r\n", b"\n") != expected:
                        raise RuntimeError(f"{name}: rc={m['returncode']} out={(r/'out').read_bytes()[:200]!r} err={(r/'err').read_text(errors='replace')[:1500]}")
                    if i >= a.warmup:
                        samples[str(b)].append(m)
            res = {}
            for b, ss in samples.items():
                res[b] = {k: perf_win.summarize(ss, k) for k in ("wall_seconds", "cpu_seconds", "peak_ws_kib", "peak_commit_kib")}
            report["cases"][name] = res
            print(name, {b: (round(v["wall_seconds"]["median"], 3), v["peak_ws_kib"]["median"], v["peak_commit_kib"]["median"]) for b, v in res.items()}, flush=True)
    text = json.dumps(report, indent=2) + "\n"
    if a.output:
        a.output.write_text(text, encoding="utf-8")


main()
