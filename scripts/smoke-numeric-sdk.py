#!/usr/bin/env python3
"""Verify typed numeric storage and source-free replay in the actual extracted SDK."""
from pathlib import Path
import shutil
import subprocess
import sys

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
root.mkdir()
sdk = binary.parent.parent
source = root / "main.rw"
source.write_text((sdk / "share/rewind/examples/numeric/main.rw").read_text(encoding="utf-8"), encoding="utf-8")


def run(*args):
    result = subprocess.run([str(binary), *map(str, args)], cwd=root,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
    if result.returncode:
        raise RuntimeError(result.stderr.decode("utf-8", errors="replace"))
    return result.stdout.replace(b"\r\n", b"\n")


run("compile", source)
source.unlink()
cache = root / ".rewind"
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "main.rwc", "--record", root / "trace.json")
assert actual == b"1\n2\n99\n0\n", actual
assert run("replay", root / "trace.json", "--root", root) == actual
print("Verified extracted numeric SDK: pivoted LU, COW update, revert and source-free replay")

analysis = root / "analysis.rw"
analysis.write_text((sdk / "share/rewind/examples/numeric-analysis/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", analysis)
analysis.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "analysis.rwc", "--record", root / "analysis-trace.json")
assert actual == b"2\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\n4\ntrue\ntrue\n", actual
assert run("replay", root / "analysis-trace.json", "--root", root) == actual
print("Verified extracted numerical-analysis SDK: QR, least squares, eigenvalues, statistics and random replay")

exact = root / "exact.rw"
exact.write_text((sdk / "share/rewind/examples/bigint/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", exact)
exact.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "exact.rwc", "--record", root / "exact-trace.json")
assert actual == b'1267650600228229401496703205378\n1267650600228229401496703205376\n"1267650600228229401496703205376"\n1\n', actual
assert run("replay", root / "exact-trace.json", "--root", root) == actual
print("Verified extracted exact-integer SDK: BigInt, Map, checkpoint, JSON and source-free replay")

decimal = root / "decimal.rw"
decimal.write_text((sdk / "share/rewind/examples/decimal/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", decimal)
decimal.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "decimal.rwc", "--record", root / "decimal-trace.json")
assert actual == b'0.3\n0.1\n2\ntrue\n1\n"0.1"\n', actual
assert run("replay", root / "decimal-trace.json", "--root", root) == actual
print("Verified extracted Decimal SDK: exact arithmetic, rounding, canonical Map keys and source-free replay")
