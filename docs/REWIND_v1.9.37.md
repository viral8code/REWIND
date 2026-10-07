# REWIND v1.9.37 — Cooperative column-pivoted QR

`numericAsync.qr(matrix, tolerance)` executes column-pivoted economy Householder
QR as a pure VM Task. Input copy, scaling, original/pivot norms, column swaps,
reflector construction, compensated dot products, matrix/Q updates, output
scaling and permutation initialization execute in at most 4096 scalar work
units per native step. The source wrapper yields between steps. No external
worker, host observation or partial result is exposed.

The task returns `Result<QrArrayResult,StdError>`, with FloatArray `q` / `r`,
IntArray `permutation` and Int `rank`. This preserves native page ownership at
completion; it does not materialize a per-element List. Synchronous `numeric.qr`
continues to return QrResult with its original frozen List permutation.
`A[:, permutation] = Q R` and economy Q/R dimensions match synchronous QR.
Finite nonnegative relative rank tolerance, numeric errors and arithmetic order
remain the same. Rank deficiency is represented by rank, not a QR failure.

Private QrWork retains input and COW native work arrays. Its native fields cannot
be forged or read by REWIND source; the name is reserved starting at 1.9.37.
Checkpoints retain their reachable work versions. Cancellation/failure publishes
no arrays and unreachable private pages are released. Input initialization uses
shared logarithmic zero storage and bounded page-copy admission. Later steps
retain conservative COW admission; this is not an exact peak allocator bound.
Each private step applies scalar writes in the original arithmetic order, then
refreshes each touched page/tree digest once before returning. A bounded set of
node identities tracks dirty paths without dereferencing raw pointers. Dirty
arrays never escape a step, and failed steps discard their edits. Scratch
admission includes the identity set. Native work and memory budget failures
remain fatal task/runtime budget failures.

Validation covers synchronous bit agreement for rectangular / transposed /
rank-deficient / scaled / empty matrices, immutable mid-work versions, late
nonfinite input and invalid metadata, bounded initial copy, small-budget task
cancellation, source-free debug/compact replay and extracted signed SDK use.
Independent reconstruction and orthogonality remain covered by the synchronous
QR tests; execution and memory measurements are required before publication.

Cooperative eigen/least squares/autodiff, general snapshot/secret accounting,
remaining GUI features and integrated v2 acceptance remain in progress.

## Serial execution measurements

Linux optimized builds, one warm-up and three measured source-free runs per
variant; medians below include common matrix setup and runtime startup, and
exclude compilation. `scripts/benchmark-qr-cooperation.py` reproduces these
square, full-rank inputs. The foreground variant runs another task with 20
handoffs and checks that it finishes while QR remains unfinished.

| Size / execution | Elapsed | User + system CPU | Peak RSS |
| --- | --- | --- | --- |
| 65 / synchronous | 0.0316 s | 0.0314 s | 22,336 KiB |
| 65 / cooperative | 0.0776 s | 0.0773 s | 22,200 KiB |
| 65 / foreground + QR | 0.0827 s | 0.0823 s | 22,504 KiB |
| 129 / synchronous | 0.0571 s | 0.0568 s | 22,352 KiB |
| 129 / cooperative | 0.5423 s | 0.5417 s | 22,256 KiB |
| 129 / foreground + QR | 0.5509 s | 0.5506 s | 22,628 KiB |

Cooperative execution takes about 2.5x and 9.5x the elapsed time of synchronous
execution in these workloads. Persistent page access, private step versions
and VM scheduling have a measurable cost even with grouped digest refreshes.
Use synchronous QR for throughput when task responsiveness is unnecessary.
These measurements do not establish Windows timings, large-matrix performance,
long-run memory stability or a bound on individual scheduler latency. RSS is
mostly common executable/runtime overhead at these sizes; no significant memory
reduction is claimed. Larger kernels and runtime/page-access overhead remain
performance work toward v2.
