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

json = root / "json-stream.rw"
json.write_text((sdk / "share/rewind/examples/json-stream/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", json)
json.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "json-stream.rwc", "--record", root / "json-stream-trace.json")
assert actual.decode("utf-8") == '1267650600228229401496703205376\n2\n3\n日本\n3\n0\n', actual
assert run("replay", root / "json-stream-trace.json", "--root", root) == actual
print("Verified extracted incremental JSON SDK: Unicode / numeric tokens, checkpoint and source-free replay")

lexical = root / "lexical.rw"
helper = root / "helper.rw"
helper.write_text('pub fn source()->Int effects {} {return 7;} pub fn choose(value:Option<Int>)->Int effects {} {let before=source();match value {None=>{return before;},Some(source)=>{return source+before;}}}', encoding="utf-8")
lexical.write_text('import helper as h;fn classify<T>(value:Result<Option<T>,String>)->Int effects {} {match move value {Err(_)=>{return -1;},Ok(None)=>{return 0;},Ok(Some(_))=>{return 1;}}}let value:Result<Option<Int>,String>=Ok(Some(42));Out.println(classify(move value));Out.println(h.choose(Some(3)));publish;', encoding="utf-8")
run("compile", lexical)
lexical.unlink()
helper.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "lexical.rwc", "--record", root / "lexical-trace.json")
assert actual == b"1\n10\n", actual
assert run("replay", root / "lexical-trace.json", "--root", root) == actual
print("Verified nested generic match and lexical module bindings without sources")

pages = root / "pages.rw"
pages.write_text('File.writeText("virtual.txt","' + "a" * 8192 + 'tail");using handle=File.openSnapshot("virtual.txt");handle.seek(4094);commit range;File.writeText("virtual.txt","changed");assert_eq(handle.read(5),"aaaaa");revert range;assert_eq(handle.read(5),"aaaaa");Out.println(1);publish;', encoding="utf-8")
run("compile", pages, "--allow-effects", "fileRead,fileWrite")
pages.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "pages.rwc", "--allow-effects", "fileRead,fileWrite", "--record", root / "pages-trace.json")
assert actual == b"1\n", actual
assert run("replay", root / "pages-trace.json", "--root", root, "--allow-effects", "fileRead,fileWrite") == actual
print("Verified file page reads, snapshot isolation and source-free replay")

scheduler = root / "scheduler.rw"
scheduler.write_text('async fn work()->Int effects {} {return 7;}for i in 0..160 {let task=work();assert_eq(await task,Ok(7));}Out.println(1);publish;', encoding="utf-8")
run("compile", scheduler)
scheduler.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "scheduler.rwc", "--record", root / "scheduler-trace.json")
assert actual == b"1\n", actual
assert run("replay", root / "scheduler-trace.json", "--root", root) == actual
print("Verified scheduler collection with source-free replay")

budget = root / "budget.rw"
budget.write_text('Out.println(7);publish;', encoding="utf-8")
run("compile", budget)
budget.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "budget.rwc", "--history-memory", "1MiB",
             "--history-storage", "2MiB", "--spill-threshold", "0",
             "--record", root / "budget-trace.json")
assert actual == b"7\n", actual
assert run("replay", root / "budget-trace.json", "--root", root) == actual
print("Verified CLI history budgets with source-free replay")

flow = root / "flow.rw"
flow.write_text((sdk / "share/rewind/examples/flow/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", flow)
flow.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "flow.rwc", "--record", root / "flow-trace.json", "--native-work", "100000000")
assert actual == b"5\n2\n", actual
assert run("replay", root / "flow-trace.json", "--root", root) == actual
print("Verified maximum flow, matching and checkpoint source-free replay")

compact = root / "compact.rw"
compact.write_text('var sum=0;for i in 0..10000 {sum+=i;}commit saved;sum=0;revert saved;drop saved;Out.println(sum);publish;', encoding="utf-8")
run("compile", compact)
compact.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "compact.rwc", "--record", root / "compact-trace.json", "--record-mode", "compact")
assert actual == b"49995000\n", actual
assert (root / "compact-trace.json").stat().st_size < 16384
assert run("replay", root / "compact-trace.json", "--root", root) == actual
print("Verified compact recording, checkpoint and long source-free replay")

grapheme = root / "grapheme.rw"
grapheme.write_text((sdk / "share/rewind/examples/gui-grapheme/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", grapheme)
grapheme.unlink()
if cache.exists():
    shutil.rmtree(cache)
for mode in ["debug", "compact"]:
    actual = run("run", root / "grapheme.rwc", "--record", root / "grapheme-trace.json", "--record-mode", mode)
    assert actual == "éb\n".encode("utf-8"), actual
    assert run("replay", root / "grapheme-trace.json", "--root", root) == actual
print("Verified extracted GUI grapheme model: cluster editing, scalar offsets, revert, source-free replay")

algorithms = root / "algorithms.rw"
algorithms.write_text((sdk / "share/rewind/examples/advanced-algorithms/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", algorithms)
algorithms.unlink()
if cache.exists():
    shutil.rmtree(cache)
for mode in ["debug", "compact"]:
    actual = run("run", root / "algorithms.rwc", "--record", root / "algorithms-trace.json", "--record-mode", mode)
    assert actual == b"28\n2\n1\n1\n", actual
    assert run("replay", root / "algorithms-trace.json", "--root", root) == actual
print("Verified extracted algorithms: lazy ranges, trie, native suffix, geometry, revert and source-free replay")

windows = root / "windows.rw"
windows.write_text((sdk / "share/rewind/examples/gui-windows/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", windows, "--allow-effects", "gui")
windows.unlink()
if cache.exists():
    shutil.rmtree(cache)
for mode in ["debug", "compact"]:
    fixture = root / "windows-events.json"
    fixture.write_bytes((sdk / "share/rewind/examples/gui-windows/events.json").read_bytes())
    actual = run("run", root / "windows.rwc", "--allow-effects", "gui", "--gui-window-events", fixture,
                 "--record", root / "windows-trace.json", "--record-mode", mode)
    assert actual == b"1\n0\n", actual
    fixture.unlink()
    assert run("replay", root / "windows-trace.json", "--root", root, "--allow-effects", "gui") == actual
print("Verified extracted named GUI windows: routed input, undo, source-free debug and compact replay without fixture")

cooperative = root / "cooperative.rw"
cooperative.write_text((sdk / "share/rewind/examples/numeric-async/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", cooperative)
cooperative.unlink()
if cache.exists():
    shutil.rmtree(cache)
for mode in ["debug", "compact"]:
    actual = run("run", root / "cooperative.rwc", "--native-work", "100000000", "--record", root / "cooperative-trace.json", "--record-mode", mode)
    assert actual == b"9000\n32\n32\n", actual
    assert run("replay", root / "cooperative-trace.json", "--root", root) == actual
print("Verified extracted cooperative numeric kernels: foreground task, bitwise matrix result, checkpoint, source-free replay and restored work budget")

shared = root / "shared-memory.rw"
shared.write_text((sdk / "share/rewind/examples/numeric-memory/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", shared)
shared.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "shared-memory.rwc", "--history-memory", "64MiB", "--native-work", "100000000", "--steps", "10000000", "--task-steps", "10000000", "--record", root / "shared-memory-trace.json", "--record-mode", "compact")
assert actual == b"1000000\n64\n0\n", actual
assert run("replay", root / "shared-memory-trace.json", "--root", root) == actual
print("Verified extracted shared numeric memory: million-element task, 64 COW checkpoints, source-free compact replay and restored history budget")

controls = root / "gui-controls.rw"
controls.write_text((sdk / "share/rewind/examples/gui-controls/main.rw").read_text(encoding="utf-8"), encoding="utf-8")
run("compile", controls, "--allow-effects", "gui")
controls.unlink()
if cache.exists():
    shutil.rmtree(cache)
fixture = root / "controls-events.json"
fixture.write_bytes((sdk / "share/rewind/examples/gui-controls/events.json").read_bytes())
actual = run("run", root / "gui-controls.rwc", "--allow-effects", "gui", "--gui-window-events", fixture,
             "--record", root / "controls-trace.json", "--record-mode", "compact")
assert actual == '{"active":false,"age":13,"name":"日本語"}\n1\n日本\n'.encode("utf-8"), actual
fixture.unlink()
assert run("replay", root / "controls-trace.json", "--root", root, "--allow-effects", "gui") == actual
print("Verified extracted form validation and viewport table: editing, typed submission, row selection, undo, source-free compact replay without fixture")
