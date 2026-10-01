#!/usr/bin/env python3
"""Advance compiler, bundled std and lock together; historical examples stay fixed."""
import json, re, sys
from pathlib import Path
version=sys.argv[1]
if not re.fullmatch(r'(?:0\.9\.[5-9]|1\.[012]\.0)',version):
 raise SystemExit('expected 0.9.5..0.9.9 or 1.0.0/1.1.0/1.2.0')
root=Path(__file__).resolve().parent.parent
p=root/'Cargo.toml';p.write_text(re.sub(r'^version = "[^"]+"',f'version = "{version}"',p.read_text(),count=1,flags=re.M))
p=root/'libraries/std/rewind.toml';p.write_text(re.sub(r'language = "[^"]+"',f'language = "{version}"',p.read_text()))
p=root/'libraries/std/rewind.lock';d=json.loads(p.read_text());d.update(compiler=version,language=version);p.write_text(json.dumps(d,indent=2,sort_keys=True)+'\n')
