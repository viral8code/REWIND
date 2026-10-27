# REWIND v1.9.62 — Cooperative forward differentiation

This increment groups forward tape construction and storage identity preparation.
It does not declare v2.0 complete. Publication requires both platform SDK gates.

`std.autodiffForwardAsync` consumes a Tape and returns `Result<Tuple<Tape,Node>,StdError>`.
Parameter/constant, add/sub/mul, the six existing unary operations, matrix multiplication,
reshape, transpose, broadcast, sum and mean preserve the synchronous operation order,
node identities, values and gradient behavior. Existing synchronous APIs keep their
pure effects and signatures. The pure owned bridge uses historical numeric primitives;
importing `std.autodiff` does not require the new task companion.

Take the returned Tape with `move result._0` and then read `result._1`. Moved Tape
bindings cannot be revived through assignment. Failures and cancellation return no
partial Tape. Retained checkpoints still own their referenced versions.

Finite validation visits at most 4096 logical cells per native step. The private
`TensorIdentityWork` visits at most 4096 storage cells/tree nodes per step, holds
shared numeric pages and a bounded traversal stack, and yields between steps.
Cold, warm and shared-subtree storage use the same traversal/handoff schedule so
cache warmth does not change record/replay budgets. Opaque state is serialized with
the referenced array and survives source-free debug/compact checkpoint restore.
VM-private snapshots preserve their native-computed prefix to populate the completed
root cache without an extra synchronous digest when storage is decoded cold. The
compiler's restore entry is restricted to reserved inaccessible fields, bound to
unchanged backing storage; no user byte/model decoder constructs this work type.
Ordinary Rust embedding restore is untrusted and cannot populate shared storage
digest caches with foreign prefix hashes. Tests cover both paths separately.

Pure transpose/broadcast and already-contiguous reshape reuse the parent storage
identity instead of rescanning its backing pages. Noncontiguous reshape copies logical
cells using the existing cooperative kernel and prepares its new storage identity.
The final append retains the old conservative full-storage native-work admission,
even when the actual digest is cached. Identity preparation therefore adds work
admission and scheduler overhead; it is not a general speed or memory improvement.
Initial input creation/materialization is still synchronous and needs separate
profiling before the final responsiveness criterion is met.

## Verification in progress

The actual development compiler passed all existing forward operation paths,
including noncontiguous reshape, against synchronous node/value/gradient results.
Invalid names, foreign nodes, unsupported operations, wrong shapes, empty means and
constant gradients are checked. Source-free debug/compact runs restore an unfinished
forward task, cancel without exposing partial Tape, and replay identically.
64 and 256 repeated cancellations trigger VM GC and keep final live numeric pages
below the test's 4 MiB bound under a 16 MiB history limit. This is not a process RSS
or long-running combined-service guarantee.

`examples/forward-gui` prepares nonzero materialized input and runs forward work
while receiving native pointer events. The Linux development compiler passed native
click cancellation in both trace modes and replay without a display. Optimized
measurements, complete compiler regression and Linux/Windows extracted-SDK
acceptance remain required. Actual Windows conversion-engine accessibility work
and sustained combined numerical/communication load remain in `v2-status.md`.

## Compiled-program loading

The compiled JSON artifact uses compact encoding. Its format, canonical payload
hash, type-IR validation and trusted bytecode recompilation remain unchanged.
After hash verification the loader takes ownership of the program JSON instead
of cloning its entire tree. Both compact and equivalent pretty-printed artifacts
execute after source/cache removal; existing forged/tampered artifact tests pass.
This reduces unnecessary storage/loading overhead while adding forward helpers.

## Optimized development measurement

`scripts/benchmark-forward-cooperation.py` uses identical materialized nonzero
sigmoid/square/mean graphs and independently checks the gradient sum. Backward is
synchronous in both cases. Linux, one warmup and three serial repetitions at
16384, 65536 and 262144 cells; compilation is excluded, source/cache are removed,
and input preparation, program loading, profiling and forward/backward are included.

| 262144 cells | median wall | median CPU | median peak RSS | final live numeric pages |
| --- | --- | --- | --- | --- |
| v1.9.61 synchronous | 0.11861 s | 0.11831 s | 46596 KiB | 14843568 bytes |
| v1.9.62 synchronous | 0.10407 s | 0.10367 s | 41868 KiB | 14843568 bytes |
| v1.9.62 cooperative forward | 0.23357 s | 0.23266 s | 48272 KiB | 14843568 bytes |

The cooperative whole-process route takes about 2.24 times the new synchronous
route here and uses more peak process memory. Final live numeric pages coincide;
scheduler/module/loading costs are not numeric-page ownership. A prototype before
compact artifact loading measured 0.16770 s / 48972 KiB for the synchronous route;
this motivated the loading cleanup. The observations do not isolate kernel
throughput or establish a general speed/space improvement. Use synchronous APIs
for uninterrupted computation and the task companion when handoff/cancel matters.
Both actual platform SDK gates remain required before publication.

The optimized native GUI fixture cancelled pending materialized forward work in
both trace modes and replayed without a display. Its two injection-to-cancellation
samples were 0.04408 s and 0.00550 s, including test window lookup. These are two
short samples, not latency percentiles or sustained responsiveness bounds.
