#!/usr/bin/env python3
"""Exercise an extracted SDK against a local server, then replay it offline."""
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
import shutil
import subprocess
import sys
import threading

binary = Path(sys.argv[1]).resolve()
root = Path(sys.argv[2]).resolve()
root.mkdir()
received = []


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        received.append(self.path)
        self.send_response(200)
        self.send_header("Content-Length", "3")
        self.end_headers()
        self.wfile.write(b"a\xff\0")

    def log_message(self, *args):
        pass


server = HTTPServer(("127.0.0.1", 0), Handler)
server.timeout = 5
url = f"http://127.0.0.1:{server.server_port}/binary"
source = root / "main.rw"
source.write_text(f'''import std.http as http;
var pending:Option<Task<Result<HttpResponse,HttpError>>>=None;
external {{pending=Some(http.get("{url}",2000,128));}}
match pending {{None=>{{panic("missing task");}},Some(task)=>{{
 match await task {{Ok(Ok(r))=>{{Out.println(r.status);Out.println(stdBytesLength(r.body));}},_=>{{panic("HTTP failed");}}}}
}}}}publish;
''', encoding="utf-8")


def run(*args):
    return subprocess.run([str(binary), *map(str, args)], cwd=root, check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30).stdout


run("compile", source, "--allow-effects", "external,network,tasks")
source.unlink()
shutil.rmtree(root / ".rewind", ignore_errors=True)
worker = threading.Thread(target=server.handle_request, daemon=True)
worker.start()
try:
    live = run("run", root / "main.rwc", "--allow-effects", "external,network,tasks",
               "--record", root / "trace.json")
    assert live == b"200\n3\n", live
    worker.join(timeout=6)
    assert not worker.is_alive() and received == ["/binary"], received
finally:
    server.server_close()
replay = run("replay", root / "trace.json", "--root", root,
             "--allow-effects", "external,network,tasks")
assert replay == live, replay
assert received == ["/binary"], received
print("SDK HTTP: source-free execution and offline replay verified")
