#!/usr/bin/env python3
"""Serial source-free HTTP origin churn: wall time, CPU, peak RSS and peer counts."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import threading
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary', type=Path)
parser.add_argument('--origins', type=int, default=64)
parser.add_argument('--repetitions', type=int, default=3)
parser.add_argument('--output', type=Path)
args = parser.parse_args()
if not hasattr(os, 'wait4') or not 17 <= args.origins <= 256 or args.repetitions < 1:
    parser.error('requires Linux wait4, 17..256 origins, and positive repetitions')
binary = args.binary.resolve()

class Server(ThreadingHTTPServer):
    daemon_threads = True
    def __init__(self):
        super().__init__(('127.0.0.1', 0), Handler)
        self.accepted = 0
        self.requests = 0
    def get_request(self):
        peer, address = super().get_request()
        self.accepted += 1
        return peer, address

class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def do_GET(self):
        self.server.requests += 1
        self.send_response(200)
        self.send_header('Content-Length', '2')
        self.end_headers()
        self.wfile.write(b'ok')
    def log_message(self, *_):
        pass

servers = [Server() for _ in range(args.origins)]
workers = [threading.Thread(target=s.serve_forever, kwargs={'poll_interval': 0.01}, daemon=True) for s in servers]
for worker in workers:
    worker.start()
urls = [f'http://127.0.0.1:{s.server_port}/' for s in servers]
# Same-origin reuse first, then exceed the cache and reopen the evicted origin.
urls = [urls[0], *urls, urls[0]]
source = Path(__file__).resolve().parent.parent/'examples/http-pool/main.rw'
report = {'version': subprocess.check_output([str(binary), '--version'], text=True).strip(),
          'origins': args.origins, 'requests_per_run': len(urls),
          'workload': 'source-free verified 200 responses from independent HTTP/1 loopback origins; one reused and one evicted/reopened connection; compilation excluded',
          'samples': []}
try:
    with tempfile.TemporaryDirectory(prefix='rewind-http-client-benchmark-') as temp:
        root = Path(temp)
        entry = root/'main.rw'
        entry.write_text(source.read_text(encoding='utf-8'), encoding='utf-8')
        effects = 'external,network,tasks'
        result = subprocess.run([str(binary), 'compile', str(entry), '--allow-effects', effects],
                                cwd=root, capture_output=True, timeout=90)
        if result.returncode:
            raise RuntimeError(result.stderr.decode(errors='replace')[:2000])
        entry.unlink()
        shutil.rmtree(root/'.rewind', ignore_errors=True)
        for sample in range(args.repetitions + 1):
            before = (sum(s.accepted for s in servers), sum(s.requests for s in servers))
            with (root/'stdout').open('wb') as out, (root/'stderr').open('wb') as err:
                start = time.monotonic()
                process = subprocess.Popen([str(binary), 'run', 'main.rwc', '--allow-effects', effects,
                                            '--history-memory', '64MiB', '--', *urls], cwd=root, stdout=out, stderr=err)
                _, status, usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)
                elapsed = time.monotonic() - start
            if process.returncode:
                raise RuntimeError((root/'stderr').read_text(errors='replace')[:2000])
            assert (root/'stdout').read_bytes() == b'requests done\n'
            peers = sum(s.accepted for s in servers) - before[0]
            requests = sum(s.requests for s in servers) - before[1]
            assert peers == args.origins + 1, peers
            assert requests == len(urls), requests
            if sample:
                report['samples'].append({'elapsed_seconds': elapsed, 'cpu_user_seconds': usage.ru_utime,
                                          'cpu_system_seconds': usage.ru_stime, 'peak_rss_kib': usage.ru_maxrss,
                                          'physical_connections': peers, 'server_requests': requests})
finally:
    for server in servers:
        server.shutdown()
        server.server_close()
    for worker in workers:
        worker.join(timeout=2)
report['median_seconds'] = statistics.median(s['elapsed_seconds'] for s in report['samples'])
report['median_cpu_seconds'] = statistics.median(s['cpu_user_seconds'] + s['cpu_system_seconds'] for s in report['samples'])
report['median_peak_rss_kib'] = statistics.median(s['peak_rss_kib'] for s in report['samples'])
encoded = json.dumps(report, indent=2) + '\n'
if args.output:
    args.output.write_text(encoded, encoding='utf-8')
print(encoded, end='')
