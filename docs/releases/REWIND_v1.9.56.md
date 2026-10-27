# REWIND v1.9.56 — Cooperative bulk graph construction and BFS

`std.graphLargeAsync.bfs(vertices, froms, tos, source)` creates a cold task
that owns shared immutable snapshots of the rank-one IntArray edge endpoints.
It returns the shortest directed distance from `source`, with -1 for each
unreachable vertex. Self-loops and parallel edges are allowed. Vertex and edge
counts retain the existing maximum of 1,048,576; work and memory budgets apply.

Every native step performs at most 4096 units across endpoint validation,
adjacency construction, queue processing, edge traversal and conversion of
unreachable distances. Phase transitions and isolated vertices also consume
units. The task yields between steps. All endpoints are validated before
adjacency construction begins; invalid endpoints produce NumericIndex without
exposing a partial result. NumericType/Shape/Size/Domain retain their typed
meaning, while fatal execution/memory limits retain their existing semantics.

The queue, adjacency and output use native integer pages with copy-on-write
snapshots and bounded temporary admission, including scattered writes.
Task progress is an opaque immutable GraphBfsWork value composed of existing
native arrays and integer cursors. Normal task and checkpoint roots retain it;
cancellation allows unreachable work to be collected. No host thread or external
resource is created. Existing synchronous graphLarge APIs remain available.

Before publication, validate against an independent queue implementation,
parallel/self-loop/disconnected graphs, reversed endpoint views, long chains,
checkpoint/cancellation, complete validation before adjacency writes, malformed
progress and old selected-language rejection. Require source-free debug/compact
record/replay and both extracted SDKs. Measure optimized bulk graph construction
and exploration before making performance claims. This increment does not
assert GUI latency guarantees or v2.0 completion; autodiff backward and other
synchronous kernels remain separate work.

The fixture GUI acceptance polls a named-window close event while graph work
is pending, cancels that work and replays with the event fixture removed in
both trace modes. It tests scheduler/GUI dispatch integration, not physical OS
input latency. The extracted SDK also runs the existing native clipboard tests;
actual UI latency under sustained materialised workloads remains separate.

Native pointer acceptance also injects an OS window event during a pending
million-edge graph task, waits for cancellation and replays with DISPLAY
removed. The script reports injection-to-cancellation wall time as a local
measurement, with no universal latency claim. Linux CI runs the native test
under Xvfb; Windows runs it on the hosted desktop. Both extracted SDKs run it.

## Optimized source-free graph measurement

Linux runs construct native input ranges, build a directed chain and validate
its last BFS distance. Compilation and the compile cache are excluded; startup
and profile overhead are included. One warm-up precedes three serial samples.
The synchronous mode retains its existing weighted Graph representation; the
cooperative mode consumes endpoint arrays directly. These are measurements of
those public APIs, not identical private representations.

| Vertices | Version / mode | Wall s | CPU s | Peak RSS KiB |
| ---: | --- | ---: | ---: | ---: |
| 65536 | 1.9.55 synchronous | 0.08502 | 0.08451 | 47132 |
| 65536 | 1.9.56 synchronous | 0.09172 | 0.09150 | 46424 |
| 65536 | 1.9.56 cooperative | 0.04600 | 0.04584 | 17672 |
| 200000 | 1.9.55 synchronous | 0.14307 | 0.14257 | 52552 |
| 200000 | 1.9.56 synchronous | 0.13788 | 0.13746 | 51944 |
| 200000 | 1.9.56 cooperative | 0.14412 | 0.14389 | 25324 |
| 1000000 | 1.9.55 synchronous | 0.42698 | 0.42667 | 101368 |
| 1000000 | 1.9.56 synchronous | 0.45166 | 0.45100 | 100792 |
| 1000000 | 1.9.56 cooperative | 0.77309 | 0.77277 | 70252 |

At a million vertices the cooperative run trades additional CPU time for lower
peak RSS and bounded task handoff. This is not a general throughput improvement
or a leak-fix claim. Reproduce with scripts/benchmark-graph-async.py and optional
--cooperative. Retained numeric storage and completed GC are included in its JSON.

The separate optimized native-window test measured injection-to-cancellation
at 0.02843 s (debug trace) and 0.00829 s (compact trace) on the development host.
The workload uses virtual-zero repeated edges; window lookup/injection, task
progress, observation delivery and cancellation are included. These two samples
are not a percentile distribution or a sustained materialised-workload latency
guarantee. Both modes replayed with DISPLAY removed.
