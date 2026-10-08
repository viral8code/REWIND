#!/usr/bin/env python3
"""Check execution-wide redaction accounting in the extracted SDK."""
import json,shutil,subprocess,sys
from pathlib import Path
binary=Path(sys.argv[1]).resolve();root=Path(sys.argv[2]).resolve();root.mkdir();source=binary.parent.parent/'share/rewind/examples/secret-accounting/main.rw'
for mode in ['debug','compact']:
 directory=root/mode;directory.mkdir();entry=directory/'main.rw';entry.write_text(source.read_text(encoding='utf-8'),encoding='utf-8')
 def call(*args):
  result=subprocess.run([str(binary),*map(str,args)],cwd=directory,capture_output=True,timeout=60)
  if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace')[:2000])
  return result
 call('compile','main.rw');entry.unlink();shutil.rmtree(directory/'.rewind',ignore_errors=True)
 run=call('profile','main.rwc','--history-memory','256KiB','--native-work','10000000','--record','trace.json','--record-mode',mode)
 assert run.stdout.replace(b'\r\n',b'\n')==b'<redacted: Secret>\n<redacted: Secret>\n'
 profile=[json.loads(line) for line in run.stderr.decode().splitlines() if line.startswith('{')][-1]
 metrics=profile['sensitive_registry'];assert metrics['text_patterns']==1 and 17<=metrics['text_bytes']<256
 assert 'example-sensitive' not in (directory/'trace.json').read_text(encoding='utf-8')
 assert call('replay','trace.json').stdout==run.stdout
print('Verified sensitive registry: duplicate admission, begin/revert retention, masked profile and source-free replay')
