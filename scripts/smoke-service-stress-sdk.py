#!/usr/bin/env python3
"""Bounded repeated native GUI/HTTP/DB transactions, shutdown and recorded replay."""
from pathlib import Path
import subprocess
import sys
binary=Path(sys.argv[1]).resolve()
root=Path(sys.argv[2]).resolve()
source=Path(sys.argv[3]).resolve() if len(sys.argv)>3 else binary.parent.parent/'share/rewind/examples/service-data/main.rw'
raise SystemExit(subprocess.call([sys.executable,str(Path(__file__).with_name('smoke-service-data-sdk.py')),str(binary),str(root),str(source),'8']))
