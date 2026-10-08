#!/usr/bin/env python3
"""Check committed standard-library version metadata before expensive CI builds."""
import json
from pathlib import Path
import re
import sys

root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
def field(path, name):
    values = re.findall(r'^'+name+r'\s*=\s*"([^"\n]+)"\s*$', path.read_text(encoding='utf-8'), re.M)
    if len(values) != 1:
        raise SystemExit(f'{path}: expected one {name} field')
    return values[0]

version = field(root/'Cargo.toml', 'version')
language = field(root/'libraries/std/rewind.toml', 'language')
lock = json.loads((root/'libraries/std/rewind.lock').read_text(encoding='utf-8'))
if language != version or lock.get('compiler') != version or lock.get('language') != language:
    raise SystemExit('Standard-library version metadata is stale. Run this compiler\'s rewind update --root libraries/std, and commit rewind.toml and rewind.lock with the version change.')
print(f'Standard-library lock matches compiler {version}')
