#!/usr/bin/env python3
"""Source-free fixture GUI dispatch while bounded graph work remains pending."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
binary=Path(sys.argv[1]).resolve();root=Path(sys.argv[2]).resolve();root.mkdir()
source='''import std.gui as gui;import std.guiWindows as windows;import std.numericRange as range;import std.graphLargeAsync as jobs;import std.task as task;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
let view=take(gui.window("Graph work",320,160));take(gui.label(&mut view,"state","Calculating",10,10,280,36));take(windows.present("work",&view));publish;
let froms=take(range.integers(0,0,16384));let work=spawn jobs.bfs(16384,froms,froms,0);task.yieldNow();assert(!work.isDone());
match take(windows.pollAny()){Some(e)=>{assert_eq(e.window,"work");assert_eq(e.event.kind,"close");},None=>{panic("GUI input did not progress");}}
work.cancel();match await work{Err(TaskError::Cancelled)=>{Out.println("cancelled");},_=>{panic("partial result escaped");}}
take(windows.close("work"));publish;'''
def call(directory,*args):
 result=subprocess.run([str(binary),*args],cwd=directory,capture_output=True,timeout=90)
 if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
 return result
for mode in ['debug','compact']:
 directory=root/mode;directory.mkdir();entry=directory/'main.rw';entry.write_text(source,encoding='utf-8')
 call(directory,'compile','main.rw','--allow-effects','gui');entry.unlink();shutil.rmtree(directory/'.rewind',ignore_errors=True)
 events=directory/'events.json';events.write_text(json.dumps([{'window':'work','event':{'kind':'close','x':0,'y':0,'key':'','width':0,'height':0}}]),encoding='utf-8')
 result=call(directory,'run','main.rwc','--allow-effects','gui','--gui-window-events','events.json','--native-work','100000000','--record','trace.json','--record-mode',mode)
 assert result.stdout.replace(b'\r\n',b'\n')==b'cancelled\n',result.stdout
 events.unlink()
 replay=call(directory,'replay','trace.json','--allow-effects','gui');assert replay.stdout==result.stdout
print('Verified source-free graph task/fixture GUI dispatch and disconnected replay; not an OS latency measurement')
