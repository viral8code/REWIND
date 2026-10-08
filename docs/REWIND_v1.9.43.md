# REWIND v1.9.43 — Shared text admission

Language 1.9.43 adds runtime-scoped accounting of shared long text storage.
Text already used reference-counted immutable buffers; this change removes
repeated logical payload charges when distinct VM roots share that buffer.
Each live owner is admitted once per runtime using its capacity plus 256 bytes
of metadata reserve. Equal but independently allocated text remains distinct.
Short owned text keeps its existing representation and conservative per-value
admission. This is admission accounting, not an allocator or RSS measurement.

Persistent list/map/heap nodes cache shared payload totals and memoize ledger
registration. Updating a container visits its new paths and pages, without
rescanning unchanged subtrees. Node and entry admission reserves include their
native sizes and bookkeeping. These representation reserves apply in the new
compiler; earlier selected language versions retain logical text payload charges.

Owner registrations hold only weak ledger references. Editing performs copy on
write and readmits the new capacity. Converting an owner to an ordinary Rust
String releases its old registration. The last owning reference releases the
charge; a checkpoint or task that still owns a buffer keeps it charged. New
shared allocations contribute to the existing collection trigger. Profile output
adds `shared_payloads` with live admission bytes, cumulative admitted allocations
and registration visits, separately from `numeric_pages`.

## Acceptance before publication

Check shared clones, independent equal texts, edits and capacity growth, last-owner
release, owned conversion, independent runtime ledgers, cached list/map paths,
checkpoint restoration and source-free debug/compact replay. Check that a one-MiB
buffer referenced 32 times fits an 8-MiB budget, while older selected language
admission still rejects it. Check that retaining an old checkpoint plus a changed
copy cannot evade memory admission. Run all tests and extracted signed SDK
acceptance on Linux and Windows. Artifact/record compiler version checks remain
strict; replay records with their original compiler.

This increment covers long VM Text owners. Shared Bytes, container metadata
sharing, sensitive pattern lifetime and external receipt storage remain separate
accounting work. It does not forget redaction patterns or roll back host effects,
and it does not establish completion of the v2 plan.
