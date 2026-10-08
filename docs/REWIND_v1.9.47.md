# REWIND v1.9.47 — Heap lookup and cached entry edges

Heap ID reads borrow their eight-byte big-endian key instead of allocating a
fresh Vec/MapKey for every lookup. Existing IDs, unsigned order, sparse storage,
flat heap wire format and public Value/Map representation remain unchanged.
Writes retain their owned keys and existing COW/admission. Generic Map lookup
keeps its existing primitive-key comparison contract.

Language 1.9.47 also uses the immutable entry edge summary introduced in 1.9.46.
GC marks a referenced heap ID live before examining its current entry; entries
without HeapRef / CellRef need no further value traversal. A live heap-owned
scalar list, ordered map or numeric payload remains retained. Entries with
captured cells, nested values or cycles follow the existing bounded mark stack.
Summaries are structural, recomputed on writes, and refer to the current heap;
checkpoint restoration recovers the checkpoint's entries and their summaries.
Earlier selected languages keep their previous GC work contract. Lookup buffer
elimination applies to this compiler independently of the selected language.

## Acceptance before publication

Compare unsigned IDs through u64::MAX, missing IDs, independent ordered-map wire,
COW updates and snapshots. Verify large inline scalar data remains live under
small work, hidden captured cells, changing edges and restored heaps, failure
atomicity, cycles and independent graph reachability in both cached traversal
modes. Verify allocation counts for many heap reads; compare original optimized
release compilers serially for wall/CPU/RSS/GC metrics. Run full tests,
source-free debug/compact replay, standard-library/API contracts, actual
GUI/HTTP/DB workflows and extracted signed Linux/Windows SDK acceptance.

Remaining large-input storage/capacity, secret-pattern provenance, cooperative
gradient operations, additional GUI facilities and sustained v2 acceptance retain
their separate work. This increment does not assert v2 completion.

## Heap lookup allocation check

A separate single-threaded Rust probe counts allocator calls only during 100,000
existing-ID reads from a 1,024-entry HeapStore. The previous implementation makes
100,000 temporary allocations; this implementation makes zero. Construction and
report formatting are outside the counted section. This demonstrates the removed
lookup buffer, not a general CPU or RSS improvement. Semantic tests independently
cover unsigned IDs, misses, COW and wire compatibility.

## Serial optimized comparison

Original optimized v1.9.46 and v1.9.47 compilers run the same source-free
checkpoint workload sequentially without a concurrent local build. Compilation
is excluded; process startup and profiling are included. One warm-up precedes
five samples; wall/CPU/RSS columns are medians.

| List elements / updates | 46 wall s | 47 wall s | 46 CPU s | 47 CPU s | 46 RSS KiB | 47 RSS KiB | 46 GC work | 47 GC work |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 4096 / 32 | 0.08727 | 0.08541 | 0.08707 | 0.08514 | 16552 | 15884 | 81 | 63 |
| 8192 / 128 | 0.19224 | 0.18419 | 0.19177 | 0.18395 | 23576 | 22684 | 176 | 137 |
| 65536 / 32 | 1.61729 | 1.56444 | 1.61693 | 1.56417 | 32412 | 31860 | 1741 | 1357 |

The first three-sample comparison had mixed wall-time results, including slower
small and large cases. The repeat above shows modest reductions in this workload;
these short runs do not establish general throughput, sustained memory use or
GUI latency. GC work and the independent allocation probe provide separate
evidence of the specific traversal and lookup changes.
