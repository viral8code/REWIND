# REWIND v1.9.48 — Flat numeric view indexing

`std.numericIndex` adds lengthFloat/Int, getFloat/Int and withFloat/Int for
existing typed arrays. A single zero-based Int addresses the view in logical
row-major order, including transpose, negative-stride slice and broadcast.
Reads allocate no coordinate List; updates retain page COW and previous views.
Broadcast writes return NumericReadOnly; invalid indices return NumericIndex.
No raw backing-storage index is exposed. The existing std.numeric API remains
unchanged and older selected languages retain its import compatibility.

Full semantic, budget, source-free replay and both-OS SDK acceptance is required
before publication. Large graph storage and sustained v2 acceptance remain
separate work; this release does not assert that v2 is complete.

## Optimized access measurement

The same optimized v1.9.48 compiler reads 65,536 IntArray elements, using either
a reused coordinate List updated at each read or the flat API. Initialization,
startup and profiling are included; compilation is excluded. One warm-up precedes
five serial samples.

| Access | Median wall s | Median CPU s | Median peak RSS KiB |
| --- | ---: | ---: | ---: |
| coordinates | 4.07043 | 4.06626 | 19404 |
| flat | 3.16306 | 3.16179 | 19468 |

The flat path reduces wall/CPU time in this workload; peak RSS is essentially
unchanged. It removes coordinate List updates and conversion, not every VM
allocation. This short measurement is not evidence of sustained GUI latency
or general application throughput.

## Focused semantic acceptance

Independent native tests cover multi-page transpose/negative-stride order,
scalar/empty/broadcast arrays, invalid writes without mutation, Int64 extremes
and exact IEEE Float64 bits across COW and wire round trips. CLI tests cover
old language 1.9.47 imports, primitive gating, both source-free trace modes and
checkpoint restoration. A 262,144-element virtual-zero array undergoes 4,096
point updates under 8 MiB with completed GC while its original view remains
unchanged. A native-work rejection leaves publication output empty.
Both packagers run the shipped numeric-index example from the extracted SDK.
