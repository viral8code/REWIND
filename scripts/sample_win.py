#!/usr/bin/env python3
"""Poor-man's sampling profiler for a Windows child process (no admin rights needed).

Usage: sample_win.py [--top N] [--interval-ms M] -- EXE ARGS...
Suspends each thread of the child every few ms, reads RIP and resolves it to function and source line through
dbghelp using the PDB next to the executable (build with CARGO_PROFILE_RELEASE_DEBUG=line-tables-only for lines).
Flat leaf profile only: inlined frames are reported at their source line, callers are not unwound.
"""
import argparse
import collections
import ctypes
from ctypes import wintypes
import os
import subprocess
import sys
import time

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
dbg = ctypes.WinDLL("dbghelp", use_last_error=True)
TH32CS_SNAPTHREAD = 0x4
THREAD_ALL = 0x1FFFFF
CONTEXT_CONTROL = 0x100001


class THREADENTRY32(ctypes.Structure):
    _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD), ("th32ThreadID", wintypes.DWORD),
                ("th32OwnerProcessID", wintypes.DWORD), ("tpBasePri", wintypes.LONG), ("tpDeltaPri", wintypes.LONG),
                ("dwFlags", wintypes.DWORD)]


class SYMBOL_INFO(ctypes.Structure):
    _fields_ = [("SizeOfStruct", wintypes.ULONG), ("TypeIndex", wintypes.ULONG), ("Reserved", ctypes.c_uint64 * 2),
                ("Index", wintypes.ULONG), ("Size", wintypes.ULONG), ("ModBase", ctypes.c_uint64),
                ("Flags", wintypes.ULONG), ("Value", ctypes.c_uint64), ("Address", ctypes.c_uint64),
                ("Register", wintypes.ULONG), ("Scope", wintypes.ULONG), ("Tag", wintypes.ULONG),
                ("NameLen", wintypes.ULONG), ("MaxNameLen", wintypes.ULONG), ("Name", ctypes.c_char * 512)]


class IMAGEHLP_LINE64(ctypes.Structure):
    _fields_ = [("SizeOfStruct", wintypes.DWORD), ("Key", ctypes.c_void_p), ("LineNumber", wintypes.DWORD),
                ("FileName", ctypes.c_char_p), ("Address", ctypes.c_uint64)]


k32.OpenThread.restype = wintypes.HANDLE
k32.OpenThread.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
k32.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
k32.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
k32.Thread32First.argtypes = [wintypes.HANDLE, ctypes.POINTER(THREADENTRY32)]
k32.Thread32Next.argtypes = [wintypes.HANDLE, ctypes.POINTER(THREADENTRY32)]
k32.SuspendThread.argtypes = [wintypes.HANDLE]
k32.ResumeThread.argtypes = [wintypes.HANDLE]
k32.GetThreadContext.argtypes = [wintypes.HANDLE, ctypes.c_void_p]
k32.CloseHandle.argtypes = [wintypes.HANDLE]
k32.OpenProcess.restype = wintypes.HANDLE
k32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
dbg.SymInitialize.argtypes = [wintypes.HANDLE, ctypes.c_char_p, wintypes.BOOL]
dbg.SymFromAddr.argtypes = [wintypes.HANDLE, ctypes.c_uint64, ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(SYMBOL_INFO)]
dbg.SymGetLineFromAddr64.argtypes = [wintypes.HANDLE, ctypes.c_uint64, ctypes.POINTER(wintypes.DWORD), ctypes.POINTER(IMAGEHLP_LINE64)]
dbg.SymLoadModuleEx.restype = ctypes.c_uint64
dbg.SymLoadModuleEx.argtypes = [wintypes.HANDLE, wintypes.HANDLE, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint64,
                                wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD]
dbg.SymSetOptions.argtypes = [wintypes.DWORD]


def threads(pid):
    snap = k32.CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
    e = THREADENTRY32()
    e.dwSize = ctypes.sizeof(e)
    out = []
    ok = k32.Thread32First(snap, ctypes.byref(e))
    while ok:
        if e.th32OwnerProcessID == pid:
            out.append(e.th32ThreadID)
        ok = k32.Thread32Next(snap, ctypes.byref(e))
    k32.CloseHandle(snap)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--top", type=int, default=30)
    ap.add_argument("--interval-ms", type=float, default=2.0)
    ap.add_argument("--main-only", action="store_true", help="sample only the first (main) thread; many more samples per second")
    ap.add_argument("--lines", action="store_true", help="group by source line instead of function")
    ap.add_argument("cmd", nargs=argparse.REMAINDER)
    a = ap.parse_args()
    cmd = a.cmd[1:] if a.cmd and a.cmd[0] == "--" else a.cmd
    p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    hproc = k32.OpenProcess(0x1F0FFF, False, p.pid)
    dbg.SymSetOptions(0x2 | 0x10 | 0x1000)  # UNDNAME | LOAD_LINES | ... (no deferred)
    time.sleep(0.05)
    dbg.SymInitialize(hproc, ctypes.c_char_p(os.path.dirname(os.path.abspath(cmd[0])).encode()), True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    mods = (ctypes.c_void_p * 8)()
    need = wintypes.DWORD(0)
    psapi.EnumProcessModules.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
    psapi.EnumProcessModules(hproc, ctypes.byref(mods), ctypes.sizeof(mods), ctypes.byref(need))
    base = mods[0] or 0
    buf = ctypes.create_string_buffer(1232 + 16)
    addr = (ctypes.addressof(buf) + 15) & ~15
    counts = collections.Counter()
    raw = collections.Counter()
    total = 0
    sym = SYMBOL_INFO()
    sym.SizeOfStruct = 88
    sym.MaxNameLen = 512
    line = IMAGEHLP_LINE64()
    line.SizeOfStruct = ctypes.sizeof(line)
    cache = {}
    tids = []
    iteration = 0
    t0 = time.time()
    while p.poll() is None:
        if not tids or iteration % 200 == 0:
            tids = threads(p.pid) or tids
        iteration += 1
        if a.main_only and tids:
            tids = [min(tids)]
        for tid in tids:
            h = k32.OpenThread(THREAD_ALL, False, tid)
            if not h:
                continue
            if k32.SuspendThread(h) != 0xFFFFFFFF:
                ctypes.memset(addr, 0, 1232)
                ctypes.c_uint32.from_address(addr + 48).value = CONTEXT_CONTROL
                if k32.GetThreadContext(h, addr):
                    rip = ctypes.c_uint64.from_address(addr + 248).value
                    k32.ResumeThread(h)
                    raw[rip] += 1
                    total += 1
                else:
                    k32.ResumeThread(h)
            k32.CloseHandle(h)
        time.sleep(a.interval_ms / 1000)
    k32.GetCurrentProcess.restype = wintypes.HANDLE
    hself = k32.GetCurrentProcess()
    dbg.SymCleanup.argtypes = [wintypes.HANDLE]
    dbg.SymCleanup(hproc)
    dbg.SymInitialize(hself, ctypes.c_char_p(os.path.dirname(os.path.abspath(cmd[0])).encode()), False)
    loaded = dbg.SymLoadModuleEx(hself, None, os.path.abspath(cmd[0]).encode(), None, base, os.path.getsize(cmd[0]), None, 0)
    if not loaded:
        print('SymLoadModuleEx failed', ctypes.get_last_error(), file=sys.stderr)
    hproc = hself
    for rip, n in raw.items():
        d = ctypes.c_uint64(0)
        name = "?"
        if dbg.SymFromAddr(hproc, rip, ctypes.byref(d), ctypes.byref(sym)):
            name = sym.Name.decode(errors="replace")[:90]
        ln = ""
        dd = wintypes.DWORD(0)
        if dbg.SymGetLineFromAddr64(hproc, rip, ctypes.byref(dd), ctypes.byref(line)) and line.FileName:
            ln = line.FileName.decode(errors="replace").replace("\\", "/").split("REWIND/")[-1].split("/library/")[-1] + ":" + str(line.LineNumber)
        if name.startswith(("NtWait", "ZwWait", "NtDelay")):
            total -= n
            continue
        counts[ln if a.lines else name] += n
    print(f"samples={total} wall={time.time()-t0:.2f}s base={base:#x}")
    for k, v in counts.most_common(a.top):
        print(f"{100*v/total:5.1f}%  {v:6d}  {k}")


main()
