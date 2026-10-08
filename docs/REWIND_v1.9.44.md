# REWIND v1.9.44 — Shared Bytes admission

Language 1.9.44 extends runtime-scoped shared payload admission to immutable
VM Bytes owners. Aliases of one Arc buffer are charged once per Runtime using
capacity plus 256 bytes of bookkeeping reserve. Independently allocated equal
buffers remain separate. Empty buffers still reserve metadata. This is an
admission estimate, not an allocator or RSS measurement.

The existing Bytes representation and serialized byte-array format remain
unchanged. A runtime-local weak owner index does not keep buffers alive. Dead
entries and sparse index capacity are pruned at heap collection, profile
observation, or before memory admission would otherwise fail. Pruning scans the
registered owner index at those points, not at every opcode. New admitted
allocations contribute to the existing collection trigger. Safe Rust Arc mutation
dissociates weak owners or copies shared data, so changed capacity is readmitted.

Persistent list, map and heap nodes cache logical byte payload totals and owner
presence separately, including zero-length Bytes. Registration visits changed
paths and pages without rescanning unchanged subtrees. Checkpoint/task owners
keep live buffers admitted. The shared_payloads profile includes Bytes alongside
long Text. Selected earlier languages retain logical Bytes payload admission.
Native container size reserves reflect the new representation in this compiler.

## Acceptance before publication

Verify alias sharing, distinct equal buffers, capacity rather than length,
copy-on-write, empty metadata, separate Runtime ledgers, weak lifetime release,
pruning before pressure denial, checkpoint retention, and cached list/map paths.
Verify one MiB referenced 32 times under an eight-MiB budget, older language
rejection, and source-free debug/compact replay. Run the full test suite and
extracted Linux/Windows SDK acceptance. Record/compiler compatibility remains
strict; replay records with their original compiler.

Map key-owned byte vectors, unique container metadata accounting, sensitive
pattern provenance and external receipt storage remain separate work. Strong
host-side buffer references conservatively keep their admitted charge. This
increment does not erase redaction patterns or reverse host effects, and does
not establish completion of the v2 plan.

## Reproducible optimized comparison

Run `python3 scripts/benchmark-bytes-admission.py /path/to/rewind --output result.json`
separately with each original compiler. The Linux source-free profile workloads
use a 64-KiB buffer, one warm-up and three measured runs. Compilation is excluded;
process startup and profile serialization are included. These are short, bounded
observations, not GUI latency, Windows, allocator-efficiency or long-run leak proofs.

| Compiler | Workload | Median wall seconds | Median CPU seconds | Median peak RSS KiB | Completed GC |
| --- | --- | ---: | ---: | ---: | ---: |
| 1.9.43 | aliases | 0.38526 | 0.38467 | 74928 | 0 |
| 1.9.43 | ephemeral | 0.04489 | 0.04467 | 95792 | 0 |
| 1.9.43 | longer_ephemeral | 0.16332 | 0.16300 | 95792 | 0 |
| 1.9.44 | aliases | 0.37694 | 0.37651 | 74900 | 0 |
| 1.9.44 | ephemeral | 0.04526 | 0.04503 | 95764 | 16 |
| 1.9.44 | longer_ephemeral | 0.16829 | 0.16799 | 95764 | 64 |

Aliases perform 8,192 reads from 32 references to one buffer. Ephemeral workloads
encode/drop 1,024 or 4,096 buffers. Final live shared admission in the alias case
is 131,584 bytes (one Text and one Bytes owner); temporary cases finish at 65,792
bytes (the retained Text owner). The old compiler's shared_payloads counter covers
Text only and is not a comparable Bytes-retention counter. New cumulative admission
is 67,436,800 / 269,549,824 bytes for the temporary workloads; reference counting
releases payloads, while collection removes dead weak index entries. These runs
show similar time and peak RSS; no general speedup or RSS reduction is claimed.
The SDK tests use a 256-byte buffer for verbose debug recording and one MiB for
compact recording, preserving the existing independent record-size limits.
