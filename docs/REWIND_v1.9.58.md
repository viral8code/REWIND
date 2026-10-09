# REWIND v1.9.58 — Cooperative reverse-mode differentiation

`std.autodiffAsync.backward(move tape, loss)` is a cold task that consumes its
operation tape and returns the existing `autodiff.Gradients`. Node keys keep the
same branch/storage identity checks. The synchronous `backward(&tape, loss)`
retains its signature, effects and arithmetic order.

Tape metadata is initialized in batches of at most 64 entries. Reverse traversal
hands control to other ready tasks between nodes, including unreachable leaves.
Vector additions/products/scaling, matrix products, activation derivatives,
logical reshape copies and broadcast contractions use the existing bounded
cooperative kernels. Transpose and broadcast remain shared views. Input/tape
versions are not copied into a host worker. Forward tape construction remains
synchronous; this increment does not claim that every forward operation is
preemptible.

The pure `autodiff` module supplies an owned `BackwardWork` bridge so importing
existing synchronous APIs does not acquire task effects. The bridge validates
node/loss/state/index/shape and preserves constant/unreachable `None` gradients.
The companion accumulates complete incoming tensors in the original reverse
order, stores them only after successful computation and returns no partial
Gradients on typed failure or task cancellation. Work/memory budgets remain
fatal. Checkpoint roots retain their referenced task/tape/numeric pages; cancelling
an unretained task releases its temporary work through ordinary VM GC.

The shipped `autodiff-async` example compares gradients against the synchronous
result, restores an unfinished backward task and cancels a second task. Acceptance
includes independent finite differences, all derivative paths and branch reuse,
constant/foreign/empty/non-scalar cases, metadata and kernel cancellation, repeated
cancel/GC, source-free debug/compact replay and both extracted SDKs. Real
materialized-workload measurements and full platform validation are required
before publication. This increment does not declare v2 complete or establish a
sustained GUI latency bound.

## Focused library validation before compiler packaging

The source modules have passed independent finite differences for relu (positive
and negative inputs), sigmoid, tanh, exp, log and square; shared-parent add/sub/mul,
matmul, transpose/reshape/broadcast/mean, constants and invalid node/loss cases
agree with the existing synchronous gradients. The unfinished-task example
restores and cancels correctly. These initial checks use the previous compiler
with explicit source modules; the actual new compiler and extracted SDK must
also pass before release.

64 and 256 repeated kernel cancellations passed a 16 MiB history limit. The
256-iteration run completed 28 collections, reclaimed 4518 VM objects and ended
with 1562192 bytes of retained numeric pages. The cumulative native allocation
counter was 79974592 bytes; it is not simultaneously live memory. The larger
intentional test uses a 3000000000 native-work limit because tape key computation
is charged conservatively. A 4096-node tape passed bounded metadata preparation
and traversal with an explicit 2000000 per-task instruction limit. The expected
gradient is checked by value, since its broadcast view intentionally differs
from a newly materialized one-element array.

## Optimized materialized-workload comparison

`scripts/benchmark-autodiff-cooperation.py` uses identical fully materialized
nonzero inputs and sigmoid/square/mean tapes. It independently checks the sum of
the gradients. Input preparation, forward construction, process startup and
profiling are included; compilation is excluded and program source/cache are
removed. Linux, one warmup and three serial measured repetitions; every case at
16384, 65536 and 262144 elements passed.

| 262144 cells | median wall | median CPU | median peak RSS |
| --- | --- | --- | --- |
| v1.9.57 synchronous | 0.10032 s | 0.09972 s | 42920 KiB |
| v1.9.58 synchronous | 0.11118 s | 0.11079 s | 45704 KiB |
| v1.9.58 cooperative | 0.19851 s | 0.19822 s | 55428 KiB |

The cooperative route incurs task/module/step overhead, taking roughly 1.8 times
the new synchronous path in this case. The synchronous whole-process measurement
also increases in this increment; the measurement includes startup and does not
isolate kernel throughput. No general speed or process-memory improvement is
claimed. Existing synchronous APIs remain appropriate for uninterrupted work.

The optimized native GUI example uses 262144 materialized cells. Native pointer
input cancelled pending backward work in both trace modes; disconnected replay
passed. Its two injection-to-cancellation samples were 0.09549 s and 0.00879 s.
They exclude forward setup, include the test's target-window lookup and are not
latency percentiles or a sustained GUI responsiveness guarantee. Full platform
and extracted-SDK acceptance remain required before publication.
