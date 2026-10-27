# REWIND v1.9.45 — Shared container admission

Language 1.9.45 admits persistent list/map/heap native owners once per Runtime.
Distinct checkpoint roots share unchanged nodes, entries, keys and values;
changed paths and independently allocated owners are charged separately. Existing
flat serialization formats and the public Value representation remain unchanged.

Node and entry registrations reserve native size and bookkeeping. Leaf pointer
capacity, inline type names, short owned text capacity and unused inline vector
capacity are admitted. Raw Arc value/key owners use weak runtime-local indices,
which do not retain payloads. Map-owned text/byte key capacity is charged by key
owner. Dropping the last node/entry releases its registration; dead value/key index
entries are pruned during GC, profile observation or memory pressure. Registrations
memoize unchanged subtrees, including scalar-only pages. New native admission
contributes to the existing collection trigger rather than repeatedly counting
logical contents of an aliased persistent container.

Existing shared Text/Bytes and numeric-page owners remain separate charges; their
logical payload totals are excluded from the replaced native root contribution
so payloads are not subtracted twice. Profile shared_payloads now also includes
unique persistent native owners and their reserved metadata. This remains a
conservative admission estimate, not an allocator or RSS measurement. Earlier
selected languages retain their per-root container admission; native representation
and owned-capacity reserves reflect this compiler's layout.

## Acceptance before publication

Independently count native owners on changed small list pages; verify lazy,
separate-runtime, weak value/key indices and inline capacity. Check cached changed
paths on 65,536-element scalar lists and 8,192-entry maps. Retain 128 large-list
checkpoints within 32 MiB and 64 large-map checkpoints within 16 MiB, restore old
values, then drop all roots and verify zero live shared admission. Verify 32 roots
of a 4,096-element list within 8 MiB, old-language conservative rejection and
source-free debug/compact replay. Run full tests and extracted signed Linux and
Windows SDK acceptance. Record/compiler compatibility remains strict.

This covers persistent list/map/heap owners. Plain global/frame map storage,
other native internal sharing, sensitive-pattern provenance and external receipt
storage retain their separate accounting policies. No redaction pattern is erased
and no host effect is reversed. Long-run CPU/RSS/GC and large input capacity still
require their own acceptance; this increment does not complete the v2 plan.

## Optimized serial measurement

On one Linux worker, original 1.9.44 and 1.9.45 release compilers each compile
and run their own source-free workload. Compilation is excluded; process startup
and profile output are included. After one warm-up, three measured runs give:

| List / changed checkpoints | 1.9.44 wall / CPU seconds | 1.9.45 wall / CPU seconds | 1.9.44 / 1.9.45 median peak RSS KiB |
| --- | --- | --- | --- |
| 4,096 / 32 | 0.07179 / 0.07160 | 0.08701 / 0.08666 | 15,964 / 15,756 |
| 8,192 / 128 | 0.15458 / 0.15428 | 0.17934 / 0.17907 | 22,392 / 22,464 |

Both versions print the same restored values. New owner admission triggers 5 / 11
collections; collection work is 12,094 / 49,493 units, with median accumulated
collection time about 0.59 / 1.39 ms. The previous version reports no collections
for these workloads. The new accounting adds about 16–21% wall time in this short
measurement; it does not demonstrate an RSS reduction or general speedup. A
trusted-pointer hash experiment did not improve these workloads and was removed.
Use `scripts/benchmark-container-admission.py` with the original compiler for each
version to reproduce the comparison. Large scalar-container GC traversal and
sustained workloads remain separate optimization and acceptance work.

## Cancellation and collection timing

Dropping/cancelling a task removes its roots; unreachable VM heap values are
reclaimed at a later safe-point collection. More accurate sharing changes when
the existing 4 MiB cumulative-allocation trigger fires. In a repeated 65×65 QR
cancellation check, 128 / 512 / 1,024 tasks completed with 2 / 10 / 20 collections
and reclaimed 99 / 499 / 999 heap values. Final numeric admission was 1,733,312 /
780,224 / 1,495,040 bytes and task quota rows were 30 / 14 / 26. These checks bound
the uncollected tail under 4 MiB and 64 rows, rather than assuming an incidental
sub-1-MiB tail after exactly 128 tasks. They do not establish sustained RSS bounds.
