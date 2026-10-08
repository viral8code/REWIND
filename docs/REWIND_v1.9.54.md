# REWIND v1.9.54 — Direct numeric output pages

Synchronous native float maps, elementwise float zips and integer zips now build
result pages directly. They previously retained a contiguous result buffer and
then copied it into persistent pages. The new fallible page builder holds at
most one page of value scratch while retaining the completed output pages.
Page references and the balanced storage tree still require metadata space.

This changes temporary allocation behavior, not array values, shape/dtype checks,
callback order, first-error behavior, storage identity, wire format or COW rules.
The existing memory admission estimates remain conservative and unchanged.
Results retained by commits remain owned by those commits. Partial output pages
are released when a callback fails; this is a peak-memory improvement, not a fix
for a previously leaking error path. Ordinary infallible array construction keeps
its existing path. Synchronous maps remain synchronous; this does not establish
GUI latency or make autodiff backward cooperative.

Acceptance covers independent strided/broadcast references, integer extremes,
empty and scalar shapes, signed zero, late errors, COW and wire roundtrips.
A thread-local allocator test measures requested allocation sizes separately
from VM budget fees and OS RSS. It rejects a full-size temporary allocation for
1048576-element map and zip results and checks that late-error allocations are
fully released. Source-free execution and replay, prior selected language
contracts, and both extracted platform SDKs must pass before publication.

This increment does not assert v2.0 completion.

## Allocation and source-free measurements

Independent thread-local System allocator instrumentation measured the largest
requested block during each 1048576-element native map / float zip / integer
zip: 8388608 bytes in1.9.53 and32768 bytes in1.9.54. The latter block stores
page references, not all values. Output pages and tree metadata still occupy
memory. Both versions release all captured allocations after a late error;
these counters measure requested layouts, not allocator padding or OS RSS.

Optimized Linux measurements use native zero inputs, check first and last output
values, and exclude compilation. Startup and profile overhead are included.
One warm-up precedes five serial samples; the table gives medians.

| Operation | Version | Wall s | CPU s | Peak RSS KiB |
| --- | --- | ---: | ---: | ---: |
| map | 1.9.53 | 0.04375 | 0.04332 | 35524 |
| map | 1.9.54 | 0.04343 | 0.04315 | 27716 |
| float-zip | 1.9.53 | 0.03848 | 0.03816 | 35528 |
| float-zip | 1.9.54 | 0.03840 | 0.03807 | 27812 |
| int-zip | 1.9.53 | 0.03903 | 0.03875 | 35464 |
| int-zip | 1.9.54 | 0.03822 | 0.03800 | 27788 |

This workload demonstrates a lower peak RSS with similar execution time; it does
not establish a general speed improvement or a large-model memory ceiling.
Use scripts/benchmark-numeric-pages.py with --operation map / float-zip /
int-zip to reproduce this particular source-free workload on Linux.
