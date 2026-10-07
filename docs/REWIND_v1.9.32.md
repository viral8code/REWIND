# REWIND v1.9.32 — GUI, HTTP and database workflow

The SDK adds `gui-data`, a two-window application using asynchronous GUI input,
HTTP JSON and either SQLite or verified-TLS PostgreSQL. It handles HTTP error,
explicit Retry, loading, a parameterized transaction, VM View Undo, Reload and
request cancellation. Closing either surface ends the application. New
operations after Undo use `external fresh`; committed DB data and physical
requests remain outside VM rollback. Write failures are reported without an
automatic retry that could duplicate an uncertain transaction.

The checkpoint belongs to the live scope containing the interaction loop.
Saving it only inside a completed branch would not provide a valid continuation
for a later Undo. The example restores the initial view, then reads committed
data through a fresh query and drops the checkpoint. New runs use a new entry
key; the sample is not a migration or concurrent multi-user editing framework.

`rewind profile` exposes `runtime.native_resources` for recorded operations as
well as live mode, including zero. This is the current native resource count,
not historical operation receipts, credential configurations, HTTP client idle
pools or all allocator bytes. It lets the application integration checks verify that scope-owned DB
connections and active host operations have been released after cleanup. The
existing live-mode field and all previous profile meanings remain available.

Both OS SDK checks run actual pointer input, an HTTP server returning 503 then
UTF-8 JSON, and a subsequent held response for cancellation. SQLite and
PostgreSQL rows are independently read after REWIND closes its connection.
Source execution is tested separately from compiled debug/compact execution.
Compiled traces replay after removing source/cache and stopping HTTP, with no
GUI display and a disconnected PostgreSQL configuration. Public CA observations
are restored without reading their deleted fixture file; private DSNs are
re-injected and excluded from the trace. VM Undo neither removes nor inserts a
second saved row.

Native GUI pointer injection is shared with the previous async GUI fixture;
existing recorded/live cancellation checks remain in the packages. No public
language syntax, DB wire format or library signature changes are introduced.

## Vector measurements

`scripts/benchmark-vector-cooperation.py` measured release binaries serially on
the same Linux executor without simultaneous compilation/tests. Each variant
had one warm-up followed by three runs. A source-free program creates a
1,048,577-element broadcast array and computes scale, addition and norm;
compilation is excluded, while artifact loading, setup and result checks are
included. These are end-to-end measurements, not isolated native-kernel times.

| Version / variant | Median seconds | Median peak RSS KiB |
| --- | ---: | ---: |
| 1.9.30 synchronous | 0.0721 | 46,620 |
| 1.9.31 synchronous | 0.0703 | 46,732 |
| 1.9.31 cooperative | 0.1101 | 38,956 |
| 1.9.31 cooperative with foreground loop | 0.1303 | 39,684 |

The foreground loop progressed 771 times. The cooperative API trades additional
dispatch/COW work for task handoff; the synchronous API preserves its measured
throughput here. RSS includes process/runtime overhead and is not the VM's
retained-page ledger. Shorter 262,145-element samples varied enough to require
the larger workload before comparing versions. These measurements do not
establish GUI input latency, solver/FFT throughput or every workload's memory
behavior. The script also records CPU user/system time for subsequent runs.

GUI clipboard/dialog/menu, IME/accessibility, cooperative LU/QR/eigen/autodiff,
general snapshot/secret/native-cache accounting, long-run and numerical
measurements and the remaining v2 acceptance conditions continue.
