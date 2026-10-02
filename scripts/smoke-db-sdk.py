#!/usr/bin/env python3
"""Exercise real SQLite, source-free execution and DB-free replay in an extracted SDK."""
from pathlib import Path
import shutil
import subprocess
import sys

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
root.mkdir()
sdk = binary.parent.parent
source = root / "main.rw"
source.write_text(
    (sdk / "share/rewind/examples/database/main.rw").read_text(encoding="utf-8")
    .replace('":memory:"', '"data.sqlite"'), encoding="utf-8"
)


def run(*args):
    result = subprocess.run([str(binary), *map(str, args)], cwd=root,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
    if result.returncode:
        raise RuntimeError(result.stderr.decode("utf-8", errors="replace"))
    return result.stdout.replace(b"\r\n", b"\n")


run("compile", source, "--allow-effects", "external,db,tasks")
source.unlink()
cache = root / ".rewind"
if cache.exists():
    shutil.rmtree(cache)
actual = run("run", root / "main.rwc", "--allow-effects", "external,db,tasks",
             "--record", root / "trace.json")
assert actual == b"1\nAlice\n42\n", actual
(root / "data.sqlite").unlink()
assert run("replay", root / "trace.json", "--root", root,
           "--allow-effects", "external,db,tasks") == actual
assert not (root / "data.sqlite").exists()
print("Verified extracted SQLite SDK: actual file DB, source-free run and DB-free replay")
