# REWIND v1.9.46 — Cached persistent heap edges

Language 1.9.46 caches structural heap-reference presence when persistent list
slots and map entries are written. Immutable node summaries propagate through
changed paths. GC skips subtrees with no HeapRef / CellRef and visits shared
persistent nodes once per collection. Nested inline values, captured cells,
ordered-map keys, enum fields and Option / Result are included. Cached presence
means an edge exists structurally, not that its target exists or remains live;
every collection resolves edges against the current heap.

Current globals, stack, call-frame locals and supplied external roots retain their
existing roles. Mark/sweep preserves cycles reachable from those roots and removes
unreachable cycles. Checkpoint heaps remain independent immutable roots; collecting
the current heap does not discard the checkpoint's heap or resurrect host effects.
An unsuccessful bounded collection does not remove heap entries.

Node inspections, examined leaf slots, pending values and the final heap sweep
remain work-bounded. The visited-node index is local to one collection; it cannot
retain VM values or suppress traversal in a subsequent collection. Traversal
workspace grows with charged work. New leaf-slot flags and node/entry metadata
are reflected in native owner admission. Flat Value and persistent-container wire
formats remain unchanged; deserialization reconstructs structural summaries.
Immutable list slots also memoize their last admitted Runtime identity, so a copied
leaf can reuse already registered value owners without another weak-index lookup.
The slot retains its Arc; pruning cannot remove a live owner. New slots clear the
memo, and distinct monotonic ledger identities force separate Runtime admission.
The bounded per-slot metadata is included in native leaf-capacity admission.
Earlier selected languages retain the prior full traversal and its work budget
contract. Existing source-free compiler/record compatibility remains strict.

## Acceptance before publication

Check scalar lists of 65,536 elements and maps of 8,192 entries under small GC work;
sparse edges and repeated aliases; COW removal of edges and checkpoint restoration;
reachable captures/cycles, dead cycles, current heap mutation between collections;
stack/frame/external roots, flat serialization, failure atomicity and independently
computed reachable graph IDs. Verify profile work for changed scalar histories,
old-language traversal and source-free debug/compact replay. Run full tests,
standard-library/API contracts, actual GUI/HTTP/DB integration and extracted signed
Linux/Windows SDK acceptance. Compare original release compilers serially for wall,
CPU, peak RSS, collection counts and work before claiming performance gains.

This removes repeated scalar traversal in persistent storage. General inline-value
traversal, current-heap sweep and collection scheduling still consume bounded work.
It does not complete sensitive-pattern provenance, large-input capacity,
cooperative gradient operations, remaining GUI facilities or sustained v2 acceptance.

## Optimized serial measurement

Original 1.9.45 and 1.9.46 release compilers each compile and run their own
source-free histories on one Linux worker. Compilation is excluded; startup and
profile output are included. One warm-up precedes three measured runs.

| Elements / checkpoints | 1.9.45 wall / CPU seconds | 1.9.46 wall / CPU seconds | 1.9.45 / 1.9.46 median peak RSS KiB | GC work 1.9.45 / 1.9.46 |
| --- | --- | --- | --- | --- |
| 4,096 / 32 | 0.08984 / 0.08947 | 0.09551 / 0.09511 | 16,384 / 15,664 | 12,094 / 81 |
| 8,192 / 128 | 0.21431 / 0.21397 | 0.17228 / 0.17205 | 23,084 / 22,664 | 49,493 / 176 |
| 65,536 / 32 | 1.62509 / 1.62396 | 1.64363 / 1.64315 | 30,020 / 31,676 | 3,756,900 / 1,741 |

Collection counts are 5 / 11 / 110 before and 6 / 13 / 128 after; additional
slot metadata also contributes to the existing allocation trigger. For the
65,536-element case, median accumulated collection time falls from about
125.3 ms to 50.1 ms. The smaller work budget is verified independently of
execution speed. Overall wall time improves in the 8,192-element case, is slower
in the smallest case, and is similar in the largest case. Peak RSS rises in the
largest case with additional slot metadata. These short runs do not demonstrate
a general speedup, an RSS reduction or sustained bounds. Reproduce with
`scripts/benchmark-container-admission.py --include-large` and each original
compiler. Output and restored values are identical.
