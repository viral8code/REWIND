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
