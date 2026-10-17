#!/usr/bin/env python3
"""Windows measurement helper: wall time, CPU time and peak working set/pagefile of a child process.

Linux benchmarks in this directory use os.wait4; this module provides the same figures on Windows via
GetProcessMemoryInfo/GetProcessTimes on a handle opened before the child exits.
Definitions: peak_ws_kib = PeakWorkingSetSize (resident), peak_commit_kib = PeakPagefileUsage (private commit).
"""
import ctypes
from ctypes import wintypes
import statistics
import subprocess
import time

PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010
_k32 = ctypes.WinDLL("kernel32", use_last_error=True)
_psapi = ctypes.WinDLL("psapi", use_last_error=True)


class _Counters(ctypes.Structure):
    _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t), ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t), ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t), ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t), ("PeakPagefileUsage", ctypes.c_size_t)]


_k32.OpenProcess.restype = wintypes.HANDLE
_k32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
_k32.CloseHandle.argtypes = [wintypes.HANDLE]
_k32.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
_psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(_Counters), wintypes.DWORD]


def _ft(f):
    return ((f.dwHighDateTime << 32) | f.dwLowDateTime) / 1e7


def run(cmd, cwd=None, stdout=None, stderr=None, timeout=600):
    """Run cmd; return dict(returncode, wall_seconds, cpu_seconds, peak_ws_kib, peak_commit_kib)."""
    start = time.perf_counter()
    p = subprocess.Popen(cmd, cwd=cwd, stdout=stdout, stderr=stderr)
    h = _k32.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, p.pid)
    if not h:
        h = _k32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, p.pid)
    try:
        p.wait(timeout=timeout)
        wall = time.perf_counter() - start
        c = _Counters()
        c.cb = ctypes.sizeof(c)
        ok = _psapi.GetProcessMemoryInfo(h, ctypes.byref(c), c.cb)
        cr, ex, kr, us = (wintypes.FILETIME() for _ in range(4))
        _k32.GetProcessTimes(h, ctypes.byref(cr), ctypes.byref(ex), ctypes.byref(kr), ctypes.byref(us))
        return {"returncode": p.returncode, "wall_seconds": wall, "cpu_seconds": _ft(kr) + _ft(us),
                "peak_ws_kib": c.PeakWorkingSetSize // 1024 if ok else None,
                "peak_commit_kib": c.PeakPagefileUsage // 1024 if ok else None}
    except subprocess.TimeoutExpired:
        p.kill()
        raise
    finally:
        _k32.CloseHandle(h)


def summarize(samples, key):
    v = sorted(s[key] for s in samples)
    return {"n": len(v), "min": v[0], "median": statistics.median(v), "max": v[-1],
            "p95": v[min(len(v) - 1, int(round(0.95 * (len(v) - 1))))]}
