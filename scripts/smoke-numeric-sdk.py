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

datetime = root / "datetime.rw"
datetime.write_text((sdk / "share/rewind/examples/datetime/main.rw").read_text(encoding="utf-8") + '\nlet encodedDuration=take(datetime.durationToJson(&distance));let decodedDuration=take(datetime.durationFromJson(move encodedDuration));Out.println(decodedDuration==distance);publish;\n', encoding="utf-8")
run("compile", datetime)
datetime.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "datetime.rwc", "--record", root / "datetime-trace.json")
assert actual == b"3600\n2024-11-03T06:30:00.123456789Z\n2024-11-03T05:30:00.123456789Z\n32400\nsame instant\ntrue\n", actual
assert run("replay", root / "datetime-trace.json", "--root", root) == actual
print("Verified extracted datetime SDK: exact nanoseconds, DST policy, canonical Map keys and source-free replay")

unicode = root / "unicode.rw"
unicode.write_text((sdk / "share/rewind/examples/unicode/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", unicode)
unicode.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "unicode.rwc", "--record", root / "unicode-trace.json")
assert actual.decode("utf-8") == "3\n🇯🇵\nfalse\ntrue\nA1ffi\nSTRASSE\n3\n", actual
assert run("replay", root / "unicode-trace.json", "--root", root) == actual
print("Verified extracted Unicode SDK: grapheme clusters, normalization, case mapping and source-free replay")

regex = root / "regex.rw"
regex.write_text((sdk / "share/rewind/examples/regex/main.rw").read_text(encoding="utf-8") + '\nlet encodedPattern=take(regex.toJson(&pattern));let decodedPattern=take(regex.fromJson(encodedPattern));Out.println(decodedPattern==pattern);publish;\n', encoding="utf-8")
run("compile", regex)
regex.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "regex.rwc", "--record", root / "regex-trace.json")
assert actual == b"Alice-42\nAlice\nBob-7\nfalse\ntrue\n3\ntrue\ntrue\n", actual
assert run("replay", root / "regex-trace.json", "--root", root) == actual
print("Verified extracted regex SDK: captures, checkpoint, Map keys, JSON and source-free replay")

csv = root / "csv-stream.rw"
csv.write_text((sdk / "share/rewind/examples/csv-stream/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", csv)
csv.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "csv-stream.rwc", "--record", root / "csv-stream-trace.json")
assert actual.decode("utf-8") == 'name\nnote\n日本語\na"b\n日\nname\n', actual
assert run("replay", root / "csv-stream-trace.json", "--root", root) == actual
print("Verified extracted incremental CSV SDK: byte chunk boundaries, quoted UTF-8, checkpoint and source-free replay")
