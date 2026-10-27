# REWIND v1.9.55 — Cooperative scalar reductions

`std.numericReduceAsync.sum`, `mean` and `variance` create cold tasks with
shared native-array inputs and constant scalar progress. A native step handles
at most 4096 logical values, with task handoff between steps. Arbitrary-rank,
strided and broadcast inputs are traversed without materialization.

Sum preserves the existing Neumaier accumulation order. Mean and variance
preserve Welford order and the count-ddof divisor, including existing empty,
negative-ddof, nonfinite and overflow errors. Empty sum remains zero. Mean
keeps the same finite mean-and-variance validation as synchronous numeric.mean.
The existing synchronous calls and numericAsync dot/norm2 remain available.

No host thread, external resource or new value/wire format is introduced.
Scalar progress and shared inputs are retained by normal task/checkpoint roots;
cancellation does not expose a partial scalar result. Fatal memory/execution
budgets keep their existing meaning. This companion requires selected language
1.9.55; standalone reductions do not make autodiff.backward cooperative.

Before publication, verify independent numerical references and synchronous
bitwise agreement, strided/broadcast/scalar/empty inputs, bounded cursor advance,
invalid progress, ddof and late overflow, handoff/checkpoint/cancellation,
previous selected-language rejection, source-free debug/compact replay and
both extracted platform SDKs. This increment does not assert v2.0 completion.

## Source-free scalar reduction measurement

Optimized Linux runs reduce 1048576 logical values broadcast from one native
0.5 cell. Compilation is excluded; startup and profile overhead are included.
One warm-up precedes three serial samples; medians are shown. This input is a
shared broadcast view, not a fully materialized million-cell model.

| Operation | Version / mode | Wall s | CPU s | Peak RSS KiB |
| --- | --- | ---: | ---: | ---: |
| sum | 1.9.54 synchronous | 0.02040 | 0.02015 | 20372 |
| sum | 1.9.55 synchronous | 0.02205 | 0.02184 | 19400 |
| sum | 1.9.55 cooperative | 0.03915 | 0.03889 | 20736 |
| mean | 1.9.54 synchronous | 0.02462 | 0.02443 | 20380 |
| mean | 1.9.55 synchronous | 0.02544 | 0.02526 | 19492 |
| mean | 1.9.55 cooperative | 0.04703 | 0.04680 | 20804 |
| variance | 1.9.54 synchronous | 0.02567 | 0.02547 | 20072 |
| variance | 1.9.55 synchronous | 0.02517 | 0.02495 | 19432 |
| variance | 1.9.55 cooperative | 0.04536 | 0.04497 | 20812 |

The cooperative calls pay task/step overhead. These measurements demonstrate
neither a synchronous throughput improvement nor GUI latency or large-model
memory capacity. Use scripts/benchmark-numeric-reductions.py with --operation
sum / mean / variance and optional --cooperative to reproduce this workload.
The cancellation acceptance separately repeats 512 and 2048 cancelled tasks under
an 8MiB history budget and verifies completed GC with retained numeric storage
below 4MiB; checkpointed roots retain their input/progress by design.
