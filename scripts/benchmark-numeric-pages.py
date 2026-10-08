#!/usr/bin/env python3
"""Measure source-free native map/zip outputs; Linux wait4 RSS, not VM fees."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary',type=Path)
parser.add_argument('--operation',choices=['map','float-zip','int-zip'],default='map')
parser.add_argument('--output',type=Path)
args=parser.parse_args()
binary=args.binary.resolve()
integer=args.operation=='int-zip'
source='import std.numeric as n;fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}let shape=List<Int>();shape.add(1048576);let x=take(n.'+('zerosInt' if integer else 'zerosFloat')+'(&shape));let y=take(n.'+({'map':'mapFloat("exp",&x)','float-zip':'zipFloat("add",&x,&x)','int-zip':'zipInt("add",&x,&x)'}[args.operation])+');let first=List<Int>();first.add(0);let last=List<Int>();last.add(1048575);assert_eq(take(n.'+('getInt' if integer else 'getFloat')+'(&y,&first)),'+('0' if integer else ('1.0' if args.operation=='map' else '0.0'))+');assert_eq(take(n.'+('getInt' if integer else 'getFloat')+'(&y,&last)),'+('0' if integer else ('1.0' if args.operation=='map' else '0.0'))+');Out.println(true);publish;'
report={'version':subprocess.check_output([str(binary),'--version'],text=True).strip(),'operation':args.operation,'elements':1048576,'workload':'source-free native zero input and map/zip output; compile excluded; startup/profile included; first and last values checked','samples':[]}
with tempfile.TemporaryDirectory(prefix='rewind-pages-bench-') as directory:
 root=Path(directory);(root/'main.rw').write_text(source)
 compiled=subprocess.run([str(binary),'compile','main.rw'],cwd=root,capture_output=True)
 assert compiled.returncode==0,compiled.stderr.decode(errors='replace')
 (root/'main.rw').unlink()
 for index in range(6):
  with (root/'stdout').open('wb') as out,(root/'stderr').open('wb') as err:
   start=time.monotonic()
   process=subprocess.Popen([str(binary),'profile','main.rwc','--history-memory','128MiB','--native-work','100000000','--steps','20000000'],cwd=root,stdout=out,stderr=err)
   _,status,usage=os.wait4(process.pid,0)
   process.returncode=os.waitstatus_to_exitcode(status)
   elapsed=time.monotonic()-start
  assert process.returncode==0,(root/'stderr').read_text()[:3000]
  assert (root/'stdout').read_bytes()==b'true\n'
  if index:report['samples'].append({'wall_seconds':elapsed,'cpu_seconds':usage.ru_utime+usage.ru_stime,'peak_rss_kib':usage.ru_maxrss})
for key in ['wall_seconds','cpu_seconds','peak_rss_kib']:
 report['median_'+key]=statistics.median(sample[key] for sample in report['samples'])
encoded=json.dumps(report,indent=2)+'\n'
if args.output:args.output.write_text(encoded)
print(encoded,end='')
