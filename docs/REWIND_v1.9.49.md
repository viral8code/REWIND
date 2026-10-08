# REWIND v1.9.49 — Native graph adjacency and BFS

`std.graphLarge` is an explicit typed-array companion to `std.graph`; the
existing graph API and old selected-language imports remain unchanged. It
retains adjacency in native Int64 pages. `fromEdges` accepts equal-length rank-one
IntArray from/to/weight views and constructs heads/links in a native loop without
per-edge VM callbacks or point-result objects. Destination/weight views retain their storage
version; froms is borrowed during construction. `create`/`add` remain available for incremental construction.

Native `bfs` returns an IntArray with -1 for unreachable vertices. Other existing
algorithms are available over the new adjacency through the library. Vertex/edge
capacity is at most 1,048,576; configured work/memory budgets still apply. Forest
construction currently retains the disjoint-set limit of 65,536 vertices and
ancestor construction retains its 4,096-vertex bound. These are separate limits.

Raw adjacency kernels validate types, ranks, endpoints and edge partition before
traversal, rejecting cyclic/duplicated/unlinked edge chains. Array and checkpoint
COW, view order and wire format follow the existing numeric storage contract.
Independent algorithm/corruption tests, source-free replay, old API compatibility,
actual large-input performance and both-OS extracted SDK acceptance are required
before publication. This increment does not assert v2 completion.

`std.numericRange.integers(start,step,length)` builds checked arithmetic
progressions directly in typed pages, providing bulk graph inputs without an
intermediate VM List. Length is bounded by the existing numeric 16,777,216-element
limit and VM budgets; empty ranges allow any start/step. Int64 overflow is
validated before storage creation. All-zero progressions use existing virtual-zero storage instead of materializing pages. Existing std.numericIndex source is unchanged.
Native adjacency/BFS kernels are synchronous; this version does not promise
mid-kernel task handoff or cancellation.

## Measured bulk construction and BFS

On the Linux development host, optimized source-free execution constructs native
input ranges, builds a chain graph, runs BFS and validates its final distance.
Compilation is excluded; process startup and profiling are included. One warm-up
and three serial samples were run per size. These are single-workload medians,
not general throughput or sustained-memory guarantees.

| Vertices | Wall seconds | CPU seconds | Peak RSS KiB | Retained numeric bytes | Completed GC cycles |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 65,536 | 0.0996 | 0.0994 | 46,412 | 3,098,752 | 0 |
| 200,000 | 0.2079 | 0.2071 | 51,748 | 9,454,472 | 1 |
| 1,000,000 | 0.4852 | 0.4846 | 100,672 | 47,207,232 | 4 |

Reproduce with `scripts/benchmark-graph-large.py`; the benchmark reports bounded
metrics without printing heap contents. Retained arrays are live roots in this
workload, so completed collections do not imply reclamation of these pages.
Other graph algorithms still execute through library code; these measurements
apply to native bulk adjacency and BFS only.
