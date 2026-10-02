#!/usr/bin/env python3
"""Extracted SDK, real PostgreSQL and source-free/offline replay contract."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
root.mkdir()
dsn = os.environ.get("REWIND_TEST_PG_DSN")
if not dsn:
    raise RuntimeError("A real PostgreSQL fixture is required for the release SDK check")
sdk = binary.parent.parent
source = root / "main.rw"
shutil.copyfile(sdk / "share/rewind/examples/postgres/main.rw", source)
shutil.copyfile(os.environ["REWIND_TEST_PG_CA"], root / "ca.der")
effects = "external,db,tasks,env,fileRead,output"


def run(*args, connection=dsn):
    env = os.environ.copy()
    env["REWIND_PG_DSN"] = connection
    result = subprocess.run([str(binary), *map(str, args)], cwd=root, env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
    if result.returncode:
        raise RuntimeError(result.stderr.decode("utf-8", errors="replace"))
    return result.stdout.replace(b"\r\n", b"\n")


run("compile", source, "--allow-effects", effects)
source.unlink()
cache = root / ".rewind"
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "main.rwc", "--allow-effects", effects,
             "--secret-env", "REWIND_PG_DSN", "--record", root / "trace.json")
assert actual == b"true\n42\nAlice\n", actual
trace = (root / "trace.json").read_text(encoding="utf-8")
assert dsn not in trace and "rewind-fixture-only-password" not in trace
(root / "ca.der").unlink()
assert run("replay", root / "trace.json", "--root", root, "--allow-effects", effects,
           "--secret-env", "REWIND_PG_DSN",
           connection="host=127.0.0.1 port=1 user=replay dbname=postgres sslmode=require") == actual
assert not (root / "ca.der").exists()
print("Verified extracted PostgreSQL SDK: SCRAM, verified TLS, source-free run and disconnected replay")

# Verify the typed Decimal facade on the same real server, then disconnect replay.
decimal = root / "decimal.rw"
shutil.copyfile(sdk / "share/rewind/examples/decimal-postgres/main.rw", decimal)
shutil.copyfile(os.environ["REWIND_TEST_PG_CA"], root / "ca.der")
run("compile", decimal, "--allow-effects", effects)
decimal.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "decimal.rwc", "--allow-effects", effects, "--secret-env", "REWIND_PG_DSN", "--record", root / "decimal-trace.json")
assert actual == b"0.3\n", actual
trace = (root / "decimal-trace.json").read_text(encoding="utf-8")
assert dsn not in trace and "rewind-fixture-only-password" not in trace
(root / "ca.der").unlink()
assert run("replay", root / "decimal-trace.json", "--root", root, "--allow-effects", effects, "--secret-env", "REWIND_PG_DSN", connection="host=127.0.0.1 port=1 user=replay dbname=postgres sslmode=require") == actual
assert not (root / "ca.der").exists()
print("Verified extracted Decimal PostgreSQL SDK: NUMERIC wire arithmetic is exact; replay stays disconnected")

datetime = root / "datetime.rw"
shutil.copyfile(sdk / "share/rewind/examples/datetime-postgres/main.rw", datetime)
shutil.copyfile(os.environ["REWIND_TEST_PG_CA"], root / "ca.der")
run("compile", datetime, "--allow-effects", effects)
datetime.unlink()
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "datetime.rwc", "--allow-effects", effects, "--secret-env", "REWIND_PG_DSN", "--record", root / "datetime-trace.json")
assert actual == b"2024-02-29T03:34:56.123456Z\n", actual
trace = (root / "datetime-trace.json").read_text(encoding="utf-8")
assert dsn not in trace and "rewind-fixture-only-password" not in trace
(root / "ca.der").unlink()
assert run("replay", root / "datetime-trace.json", "--root", root, "--allow-effects", effects, "--secret-env", "REWIND_PG_DSN", connection="host=127.0.0.1 port=1 user=replay dbname=postgres sslmode=require") == actual
assert not (root / "ca.der").exists()
print("Verified extracted datetime PostgreSQL SDK: exact TIMESTAMPTZ microseconds; replay stays disconnected")
