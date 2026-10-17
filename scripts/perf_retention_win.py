#!/usr/bin/env python3
"""PERF-03 / PERF-05 on Windows: does memory or a handle count keep growing as cycles repeat?

Each case is a fixed-shape cycle repeated N, 2N and 4N times under the same binary. A healthy case shows
peak working set / commit that does not scale with the cycle count (fixed root, flat live ledger) and a
handle count that returns to its baseline. The case only reports measurements; judge growth against the
allocator-retention noise printed next to it. Counters that could not be read are omitted, never zeroed.
"""
import argparse
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


SQLITE_CYCLE = r'''import std.db as db;
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(item)=>{return move item;},Err(_)=>{panic("database operation failed");}}
}
fn main()->Int {
var opening:Option<Task<Result<DbConnection,DbError>>>=None;
external {opening=Some(db.sqlite(":memory:",false,5000));}
match opening {
    None=>{panic("missing connection");},
    Some(task)=>{
        let connection=take(take(await task));
        var writing:Option<Task<Result<Int,DbError>>>=None;
        external {writing=Some(db.execute(&mut connection,"CREATE TABLE t (a INTEGER PRIMARY KEY, b TEXT)",freeze(List<DbValue>()),5000));}
        match writing {Some(task)=>{take(take(await task));},None=>{panic("missing write");}}
        var total=0;
        for i in 0..@CYCLES@ {
            let values=List<DbValue>();values.push(DbValue::Int(i % 16));values.push(DbValue::Text("row"));
            external {writing=Some(db.execute(&mut connection,"INSERT OR REPLACE INTO t VALUES (?1, ?2)",freeze(move values),5000));}
            match writing {Some(task)=>{take(take(await task));},None=>{panic("missing insert");}}
            let key=List<DbValue>();key.push(DbValue::Int(i % 16));
            var querying:Option<Task<Result<DbCursor,DbError>>>=None;
            external {querying=Some(db.query(&mut connection,"SELECT a,b FROM t WHERE a = ?1",freeze(move key),5000));}
            match querying {
                None=>{panic("missing query");},
                Some(task)=>{
                    let cursor=take(take(await task));
                    var reading:Option<Task<Result<DbBatch,DbError>>>=None;
                    external {reading=Some(db.next(&mut cursor,64,1048576,5000));}
                    match reading {None=>{panic("missing read");},Some(task)=>{
                        let batch=take(take(await task));
                        total+=batch.rows.len();
                    }}
                    var closing:Option<Task<Result<Unit,DbError>>>=None;
                    external {closing=Some(db.closeCursor(&mut cursor,5000));}
                    match closing {Some(task)=>{take(take(await task));},None=>{panic("missing close");}}
                }
            }
        }
        Out.println(total);
    }
}
var cleanup:Option<Task<Result<Unit,DbError>>>=None;
external {cleanup=Some(db.waitForCleanup(5000));}
match cleanup {None=>{panic("missing cleanup");},Some(task)=>{take(take(await task));}}
publish;
return 0;
}
'''


def cases(cycles):
    c = {}
    big = 'var base="x";for k in 0..19 {base=base+base;}'  # 512 KiB, built at run time (source size limit is 1 MiB)
    c["temp_text_512KiB"] = (
        f"fn make(base:String)->Int effects {{}} {{let copy=base+\"a\";return copy.byteLen();}}\nfn main()->Int {{\n{big}\nvar total=0;for i in 0..{cycles} {{total+=make(base);}}"
        f"Out.println(total);publish;return 0;}}\n", f"{cycles * 524289}\n", [])
    c["temp_map_100"] = (
        f"fn make()->Int effects {{}} {{let m=Map<Int,Int>();for k in 0..100 {{m.set(k,k);}}return m.len();}}\nfn main()->Int {{\nvar total=0;for i in 0..{cycles} {{total+=make();}}Out.println(total);publish;return 0;}}\n",
        f"{cycles * 100}\n", [])
    c["temp_map_4"] = (
        f"fn make()->Int effects {{}} {{let m=Map<Int,Int>();for k in 0..4 {{m.set(k,k);}}return m.len();}}\nfn main()->Int {{\nvar total=0;for i in 0..{cycles} {{total+=make();}}Out.println(total);publish;return 0;}}\n",
        f"{cycles * 4}\n", [])
    c["temp_list_1000"] = (
        f"fn make()->Int effects {{}} {{let l=List<Int>();for k in 0..1000 {{l.add(k);}}return l.len();}}\nfn main()->Int {{\nvar total=0;for i in 0..{cycles} {{total+=make();}}Out.println(total);publish;return 0;}}\n",
        f"{cycles * 1000}\n", [])
    c["temp_numeric_1M"] = (
        COMMON + "fn main()->Int {\nlet shape=List<Int>();shape.add(1000000);var total=0.0;\n"
        f"for i in 0..{max(cycles // 20, 5)} {{let z=take(n.zerosFloat(&shape));let a=take(n.affine(&z,1.0,0.5));total+=take(n.sum(&a));}}\n"
        "Out.println(total>0.0);publish;return 0;}\n", "true\n", [])
    c["checkpoint_cycle_fixed"] = (
        f"fn main()->Int {{\nlet items=List<Int>();for i in 0..4096 {{items.add(i);}}commit base;\n"
        f"for i in 0..{cycles} {{items.set(i % 4096,i);commit cycle;items.set(0,0);revert cycle;drop cycle;}}\n"
        "Out.println(items.len());publish;drop base;return 0;}\n", "4096\n", [])
    c["task_complete"] = (
        f"async fn work(x:Int)->Int effects {{}} {{return x+1;}}\nfn main()->Int {{\nvar total=0;\n"
        f"for i in 0..{cycles} {{match await work(i) {{Ok(v)=>{{total+=v;}},Err(_)=>{{panic(\"task\");}}}}}}\nOut.println(total);publish;return 0;}}\n",
        f"{cycles * (cycles + 1) // 2}\n", [])
    c["task_cancel_numeric"] = (
        COMMON + "import std.numericAsync as jobs;import std.task as task;\nfn main()->Int {\nlet shape=List<Int>();shape.add(65536);\n"
        "let z=take(n.zerosFloat(&shape));let ones=take(n.mapFloat(\"exp\",&z));var cancelled=0;\n"
        f"for i in 0..{cycles} {{let work=spawn jobs.dot(ones,ones);task.yieldNow();work.cancel();match await work {{Err(TaskError::Cancelled)=>{{cancelled+=1;}},_=>{{}}}}}}\n"
        "Out.println(cancelled>0);publish;return 0;}\n", "true\n", [])
    c["sqlite_cycle"] = (SQLITE_CYCLE.replace("@CYCLES@", str(cycles)), f"{cycles}\n", [], "external,db,tasks")
    c["record_compact_loop"] = (
        f"fn main()->Int {{\nvar s=0;for i in 0..{cycles * 8} {{s+=i;}}Out.println(s);publish;return 0;}}\n",
        f"{sum(range(cycles * 8))}\n", ["--record", "trace.json", "--record-mode", "compact"], "tasks")
    return c


def profile_counters(stderr_text):
    for line in reversed(stderr_text.splitlines()):
        if line.startswith("{"):
            try:
                d = json.loads(line)
            except ValueError:
                return {}
            rt = d.get("runtime", {})
            out = {"instructions": d.get("task_instruction_total"),
                   "numeric_live_bytes": (d.get("numeric_pages") or {}).get("live_bytes"),
                   "shared_live_bytes": (d.get("shared_payloads") or {}).get("live_bytes"),
                   "heap_objects": rt.get("heap_objects"),
                   "retained_heap_logical_bytes": rt.get("retained_heap_logical_bytes"),
                   "checkpoints": len(rt.get("checkpoints", [])),
                   "tasks": len((d.get("scheduler") or {}).get("tasks", {})),
                   "gc": d.get("gc")}
            return {k: v for k, v in out.items() if v is not None}
    return {}


def slope(series, key_index=1):
    """KiB per second over the second half of the polled series (None when too short)."""
    half = series[len(series) // 2:]
    if len(half) < 4 or half[-1][0] - half[0][0] < 0.5:
        return None
    return round((half[-1][key_index] - half[0][key_index]) / (half[-1][0] - half[0][0]), 1)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("binary", type=Path)
    p.add_argument("--cycles", type=int, default=2500, help="base N; each case runs N, 2N and 4N")
    p.add_argument("--cases", nargs="*")
    p.add_argument("--repeat", type=int, default=1)
    p.add_argument("--output", type=Path)
    a = p.parse_args()
    binary = a.binary.resolve()
    base_flags = ["--history-memory", "1GiB", "--native-work", "100000000000", "--steps", "1000000000", "--task-steps", "1000000000"]
    report = {"binary": str(a.binary), "version": subprocess.check_output([str(binary), "--version"], text=True).strip(),
              "env": {"os": platform.platform(), "cpu_count": os.cpu_count()}, "base_cycles": a.cycles, "cases": {}}
    names = a.cases or list(cases(a.cycles))
    with tempfile.TemporaryDirectory(prefix="rewind-retention-") as d:
        for name in names:
            rows = []
            for mult in (1, 2, 4):
                n = a.cycles * mult
                source, expected, extra, *rest = cases(n)[name]
                effects = rest[0] if rest else "tasks"
                r = Path(d) / f"{name}-{mult}"
                r.mkdir()
                (r / "main.rw").write_text(source, encoding="utf-8")
                done = subprocess.run([str(binary), "compile", "main.rw", "--allow-effects", effects], cwd=r, capture_output=True)
                if done.returncode:
                    raise RuntimeError(f"{name}: " + done.stderr.decode(errors="replace")[:1500])
                (r / "main.rw").unlink()
                shutil.rmtree(r / ".rewind", ignore_errors=True)
                flags = base_flags + ["--allow-effects", effects] + extra
                best = None
                for _ in range(a.repeat):
                    with (r / "out").open("wb") as o, (r / "err").open("wb") as e:
                        m = perf_win.run([str(binary), "run", "main.rwc"] + flags, cwd=r, stdout=o, stderr=e, poll_seconds=0.05)
                    out = (r / "out").read_bytes().replace(b"\r\n", b"\n").decode(errors="replace")
                    if m["returncode"] or out != expected:
                        raise RuntimeError(f"{name} x{n}: rc={m['returncode']} out={out[:120]!r} err={(r/'err').read_text(errors='replace')[:1200]}")
                    if best is None or m["wall_seconds"] < best["wall_seconds"]:
                        best = m
                series = best.pop("series")
                with (r / "out").open("wb") as o, (r / "err").open("wb") as e:
                    subprocess.run([str(binary), "profile", "main.rwc"] + flags, cwd=r, stdout=o, stderr=e)
                row = {"cycles": n, "wall_seconds": round(best["wall_seconds"], 3), "peak_ws_kib": best["peak_ws_kib"],
                       "peak_commit_kib": best["peak_commit_kib"], "samples": len(series),
                       "ws_slope_kib_per_s_second_half": slope(series, 1), "commit_slope_kib_per_s_second_half": slope(series, 2),
                       "handles_max": max((s[3] for s in series), default=None),
                       "handles_first": series[0][3] if series else None, "handles_last": series[-1][3] if series else None,
                       "ledger": profile_counters((r / "err").read_text(errors="replace"))}
                rows.append(row)
                print(name, {k: row[k] for k in ("cycles", "wall_seconds", "peak_ws_kib", "peak_commit_kib", "ws_slope_kib_per_s_second_half", "handles_max")}, flush=True)
            report["cases"][name] = rows
    text = json.dumps(report, indent=2) + "\n"
    if a.output:
        a.output.write_text(text, encoding="utf-8")


main()
