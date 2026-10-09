# REWIND v1.9.57 — Cooperative shape kernels

`std.numericShapeAsync.reshapeLogical` and `sumToShape` provide cold tasks for
logical reshape copies and contraction of explicitly broadcast axes. Input
arrays share immutable native pages. The dimension List is owned by the task;
pass a newly constructed List or move an existing dimension List.

An already-contiguous reshape shares storage in O(rank). A noncontiguous reshape
copies at most 4096 raw logical cells per step and yields between steps. Raw NaN
payloads and signed zero are preserved; reshape does not introduce arithmetic
finite checks. Sum-to-shape preserves the synchronous per-cell Neumaier order,
including empty/scalar shapes and NonFinite/Overflow errors. Both accumulation
and final sum-plus-correction phases are bounded by 4096 units.

Contraction uses two paged accumulators, skips bitwise-unchanged writes and checks
the target-page footprint with a fixed-size stack table. Output buffers must be
canonical: a small prefix view over a large backing buffer cannot underprice
native copy admission. The same canonical output check hardens graph work;
normal graph snapshots and results keep their existing semantics.

No host worker, resource or new value/wire format is introduced. Work and memory
budgets remain fatal; cancellation/typed failure exposes no partial result.
Task/checkpoint roots retain the arrays and progress they reference. Existing
synchronous shape operations remain available. These standalone kernels do not
make autodiff.backward cooperative.

Before publication require independent cancellation-sensitive sums and indexed
references, synchronous wire agreement, raw reshape bit preservation, strided
and empty/scalar inputs, bounded progress, bad state and late errors, allocator
admission/cleanup, task handoff/checkpoint/cancel/GC, previous-language rejection,
source-free debug/compact replay and both extracted SDKs. This increment does
not assert v2 completion or a sustained UI latency guarantee.

## Optimized materialized-input comparison

`scripts/benchmark-shape-cooperation.py` uses the same fully materialized nonzero
FloatArray input for each implementation, then reverses it for logical copying
or contracts 256 rows. Compilation is excluded; input preparation, startup and
profiling are included. Linux, one warmup and three serial measured executions;
all cases at 65536, 262144 and 1048576 elements validated their endpoints and sum.

| 1048576 elements | v1.9.56 synchronous wall / CPU / peak RSS | v1.9.57 synchronous | v1.9.57 cooperative |
| --- | --- | --- | --- |
| reversed logical copy | 0.06935 s / 0.06902 s / 38916 KiB | 0.07051 s / 0.07016 s / 37912 KiB | 0.19876 s / 0.19825 s / 38808 KiB |
| 256-row contraction | 0.04971 s / 0.04935 s / 29216 KiB | 0.05001 s / 0.04979 s / 28100 KiB | 0.14848 s / 0.14813 s / 29208 KiB |

These cases show the cost of bounded steps and task handoff: the cooperative
path takes about three times the elapsed time at this size, with similar process
peak memory. Use the synchronous API for uninterrupted throughput and the
cooperative companion when cancellation and other ready tasks must progress.
This measurement does not establish sustained GUI latency or a general memory
reduction. Native allocator tests separately bound each step's temporary
allocations and verify error cleanup and retained snapshot independence.
