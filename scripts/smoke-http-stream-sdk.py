#!/usr/bin/env python3
"""Verify streaming transport in an extracted SDK with no source files or server on replay."""
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
        received.append(("GET", self.path.encode("ascii")))
        self.send_response(200)
        body = {"/json": '{"name":"日\\u672c","items":[1,2,3]}'.encode("utf-8"), "/csv": b"1,2\r\n3,4\n"}.get(self.path, b"a\xff\0")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        assert self.headers.get("Transfer-Encoding") == "chunked"
        body = bytearray()
        while True:
            count = int(self.rfile.readline().strip(), 16)
            if count == 0:
                assert self.rfile.readline() == b"\r\n"
                break
            assert len(body) + count <= 1024
            body.extend(self.rfile.read(count))
            assert self.rfile.read(2) == b"\r\n"
        received.append(("POST", bytes(body)))
        self.send_response(201)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, *args):
        pass


server = HTTPServer(("127.0.0.1", 0), Handler)
url = f"http://127.0.0.1:{server.server_port}/"
source = root / "main.rw"
source.write_text('''import std.http as http;
import std.jsonStream as json;
import std.csvStream as csv;
fn take<T,E>(value:Result<T,E>)->T effects {} {
 match move value {Ok(item)=>{return move item;},Err(_)=>{panic("HTTP failed");}}
}
let empty=take(stdEncode(""));let body=take(stdEncode("payload"));
var downloading:Option<Task<Result<HttpDownload,HttpError>>>=None;
external {downloading=Some(http.download(http.Request("GET","URL",empty,5000,1024,freeze(List<HttpHeader>()),empty,"")));}
match downloading {Some(task)=>{
 let connection=take(take(await task));Out.println(connection.status);
 var total=0;var ended=false;
 while !ended {
  var reading:Option<Task<Result<Option<Bytes>,HttpError>>>=None;
  external {reading=Some(http.read(&mut connection,2));}
  match reading {Some(t)=>{match take(take(await t)){Some(bytes)=>{total+=stdBytesLength(bytes);},None=>{ended=true;}}},None=>{panic("missing read");}}
 }
 Out.println(total);
},None=>{panic("missing download");}}

fn readJson(url:String)->Int effects {external,network,tasks,output} {
 let reader=take(json.reader());let empty=take(stdEncode(""));var opening:Option<Task<Result<HttpDownload,HttpError>>>=None;
 external {opening=Some(http.download(http.Request("GET",url,empty,5000,1024,freeze(List<HttpHeader>()),empty,"")));}
 var count=0;
 match opening{None=>{panic("missing JSON download");},Some(task)=>{
 let connection=take(take(await task));var ended=false;
 while !ended{var reading:Option<Task<Result<Option<Bytes>,HttpError>>>=None;external{reading=Some(http.read(&mut connection,2));}
 match reading{None=>{panic("missing JSON read");},Some(task)=>{match take(take(await task)){None=>{take(json.finish(&mut reader));ended=true;},Some(bytes)=>{take(json.feed(&mut reader,bytes,false));}}}}
 var draining=true;while draining{match take(json.next(&mut reader)){None=>{draining=false;},Some(event)=>{match event{JsonStreamEvent::Number(_,_)=>{count+=1;},JsonStreamEvent::Text(text,_)=>{Out.println(text);},_=>{}}}}}
 }
 }}return count;
}
fn readCsv(url:String)->Int effects {external,network,tasks} {
 let reader=take(csv.reader());let empty=take(stdEncode(""));var opening:Option<Task<Result<HttpDownload,HttpError>>>=None;
 external {opening=Some(http.download(http.Request("GET",url,empty,5000,1024,freeze(List<HttpHeader>()),empty,"")));}
 var total=0;
 match opening{None=>{panic("missing CSV download");},Some(task)=>{
 let connection=take(take(await task));var ended=false;
 while !ended{var reading:Option<Task<Result<Option<Bytes>,HttpError>>>=None;external{reading=Some(http.read(&mut connection,3));}
 match reading{None=>{panic("missing CSV read");},Some(task)=>{match take(take(await task)){None=>{take(csv.finish(&mut reader));ended=true;},Some(bytes)=>{take(csv.feed(&mut reader,bytes,false));}}}}
 var draining=true;while draining{match take(csv.next(&mut reader)){None=>{draining=false;},Some(row)=>{total+=take(stdParseInt(row.get(0),10));}}}
 }
 }}return total;
}
Out.println(readJson("URLjson"));Out.println(readCsv("URLcsv"));
var uploading:Option<Task<Result<HttpUpload,HttpError>>>=None;
external {uploading=Some(http.upload(http.Request("POST","URL",empty,5000,1024,freeze(List<HttpHeader>()),empty,""),1024));}
match uploading {Some(task)=>{
 let connection=take(take(await task));
 var writing:Option<Task<Result<Unit,HttpError>>>=None;
 external {writing=Some(http.write(&mut connection,body));}
 match writing {Some(t)=>{take(take(await t));},None=>{panic("missing write");}}
 var finishing:Option<Task<Result<HttpResponse,HttpError>>>=None;
 external {finishing=Some(http.finish(&mut connection));}
 match finishing {Some(t)=>{let response=take(take(await t));Out.println(response.status);},None=>{panic("missing finish");}}
},None=>{panic("missing upload");}}
publish;
'''.replace("URL", url), encoding="utf-8")


def run(*args):
    completed = subprocess.run([str(binary), *map(str, args)], cwd=root,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
    if completed.returncode:
        raise RuntimeError(completed.stderr.decode("utf-8", errors="replace"))
    return completed.stdout


run("compile", source, "--allow-effects", "external,network,tasks")
source.unlink()
shutil.rmtree(root / ".rewind", ignore_errors=True)
worker = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.05}, daemon=True)
worker.start()
try:
    live = run("run", root / "main.rwc", "--allow-effects", "external,network,tasks",
               "--record", root / "trace.json")
    assert live == "200\n3\n日本\n3\n4\n201\n".encode("utf-8"), live
    assert received == [("GET", b"/"), ("GET", b"/json"), ("GET", b"/csv"), ("POST", b"payload")], received
finally:
    server.shutdown()
    server.server_close()
    worker.join(timeout=6)
replay = run("replay", root / "trace.json", "--root", root,
             "--allow-effects", "external,network,tasks")
assert replay == live, replay
assert len(received) == 4, received
print("SDK streaming HTTP: source-free download/upload, incremental JSON/CSV and offline replay verified")
