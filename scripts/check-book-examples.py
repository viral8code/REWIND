#!/usr/bin/env python3
"""Execute every complete book example; compare source and source-free output."""
import argparse, concurrent.futures, contextlib, hashlib, http.server, json, re, subprocess, tempfile, threading
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
BOOK=ROOT/"docs"/"book"
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rewind",type=Path,default=ROOT/"target/release/rewind")
parser.add_argument("--report",type=Path,default=BOOK/"validation.json")
args=parser.parse_args()
binary=args.rewind.resolve()
version=subprocess.check_output([str(binary),"--version"],text=True).strip()
if version!="rewind 2.0.0":raise SystemExit(f"Expected 2.0.0; got {version}")
sources={}
for chapter in sorted((BOOK/"chapters").glob("*.md")):
 for ident,code in re.findall(r"^~~~rewind (\S+)\n(.*?)^~~~$",chapter.read_text(),re.M|re.S):
  if ident in sources:raise ValueError(f"Duplicate {ident}")
  sources[ident]=code
for item in json.loads((BOOK/"worked-examples.json").read_text()):
 sources[item["id"]]=(BOOK/item["source"]).read_text()
contracts=json.loads((BOOK/"examples.json").read_text())
if set(sources)!=set(contracts):raise SystemExit("Example/contract mismatch")
http_observations=[]
class Handler(http.server.BaseHTTPRequestHandler):
 def do_GET(self):
  http_observations.append(self.path)
  body=b"book test";self.send_response(200);self.send_header("Content-Length",str(len(body)));self.end_headers();self.wfile.write(body)
 def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(("127.0.0.1",0),Handler)
thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
replays={"publish-revert","pending-revert","begin-reset","gui-minimal","gui-counter","db-sqlite","http-client","numeric-owner"}
def check(ident):
 cfg=contracts[ident];code=sources[ident]
 result={"id":ident,"source_sha256":hashlib.sha256(code.encode()).hexdigest(),"passed":False}
 try:
  with tempfile.TemporaryDirectory(prefix="rewind-book-") as temporary:
   directory=Path(temporary); source=directory/"main.rw";source.write_text(code)
   artifact=directory/"main.rwc";trace=directory/"trace.json"
   common=[]
   if cfg.get("effects"):common+=["--allow-effects",cfg["effects"]]
   if cfg.get("steps"):common+=["--steps",str(cfg["steps"])]
   if cfg.get("native_work"):common+=["--native-work",str(cfg["native_work"])]
   options=[]
   if cfg.get("fixture"):options+=["--gui-events",str(BOOK/cfg["fixture"])]
   application=list(cfg.get("args",[]))
   if cfg.get("http"):application=[f"http://127.0.0.1:{server.server_port}/book"]
   tail=["--",*application] if application else []
   def run(command):
    completed=subprocess.run([str(binary),*command],capture_output=True,text=True,timeout=90,cwd=directory)
    if completed.returncode!=0:
     raise AssertionError(f"exit {completed.returncode}: {completed.stderr[:4000]}")
    if completed.stderr:
     raise AssertionError(f"Unexpected stderr: {completed.stderr[:2000]}")
    return completed.stdout
   extra=["--record",str(trace),"--record-mode","compact"] if ident in replays else []
   output=run(["run",str(source),*common,*options,*extra,*tail])
   result["stdout"]=output
   if cfg["stdout"] is None:raise AssertionError("Expected stdout not yet reviewed")
   if output!=cfg["stdout"]:raise AssertionError(f"Expected {cfg['stdout']!r}, got {output!r}")
   for name,content in cfg.get("files",{}).items():
    if (directory/name).read_text()!=content:raise AssertionError(f"File mismatch: {name}")
   run(["compile",str(source),*common,"--output",str(artifact)])
   source.unlink()
   # New file observations must start from an independent state.
   for name in cfg.get("files",{}):
    (directory/name).unlink()
   artifact_output=run(["run",str(artifact),*common,*options,*extra,*tail])
   if artifact_output!=output:raise AssertionError("Source-free output differs")
   result["source_free"]=True
   if ident in replays:
    observed_before_replay=len(http_observations)
    replay=run(["replay",str(trace),"--root",str(directory),*common])
    if cfg.get("http"):
     if len(http_observations)!=observed_before_replay:raise AssertionError("Replay resent HTTP request")
     result["replay_did_not_send_http"]=True
    if replay!=output:raise AssertionError(f"Replay output differs: {replay!r}")
    result["compact_replay"]=True
   result["passed"]=True
 except Exception as error:result["error"]=str(error)
 return result
try:
 with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
  results=list(pool.map(check,sources))
finally:server.shutdown();server.server_close()
report={"compiler":version,"platform":"Linux x86_64","examples":results,"passed":sum(r["passed"] for r in results),"total":len(results)}
args.report.write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n")
for r in results:
 if not r["passed"]:print(r["id"],r.get("stdout",""),r.get("error"),sep="\n")
print(f"{report['passed']}/{report['total']} examples passed; {args.report}")
raise SystemExit(0 if report["passed"]==report["total"] else 1)
