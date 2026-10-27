# REWIND v1.9.33 — Cooperative LU solve

`std.numericAsync.solve(matrix,right,tolerance)` creates a pure cold
`Task<Result<FloatArray,StdError>>` for a square Float64 system. Partial pivoting,
relative pivot tolerance and floating-point operation order match the existing
synchronous `std.numeric.solve`. Tolerance must be finite and nonnegative; an
empty system returns an empty vector. Singular, malformed and non-finite
systems return the existing typed numeric errors. This is a direct solver,
not an iterative approximation.

`SolveWork` is reserved as opaque native progress storage from language 1.9.33.
Programs explicitly targeting language 1.9.32 or earlier retain their existing
user-defined record name. Callers normally use the public task API and never
construct or edit progress storage.

Each native step processes at most 4096 copy items, pivot comparisons, row-swap
pairs, elimination/back-substitution products or control transitions. Input
validation/copy, pivot scanning, row swaps and final back substitution are
included in subdivision. The task yields between native steps; it does not
start a host worker. Views are traversed in logical order without materializing
the entire input in a host vector.

Inputs share immutable page roots. Private matrix, right-hand side and output
use shared zero initialization and page-range copy-on-write. Work is O(n³) and
private populated storage is O(n²). A checkpoint retains prior roots and
progress; reverting restores them. Cancellation and typed failure expose no
partial solution and do not change input arrays. Removing the last owner
reclaims native pages through the existing Runtime ledger and storage lifetime.

Native work and history-memory limits remain fatal. Admission reserves bounded
scratch, metadata and a conservative upper bound on new COW storage before each
step, capped by full private array storage. A budget failure stops the current
task instead of returning a numeric `StdError`; a child task reports it through
the outer `TaskError::BudgetExceeded(BudgetKind::NativeWork)` for a native-work
limit (and the corresponding budget kind for other limits). It may reject a tightly budgeted
operation earlier than a phase-specific estimator would. The existing
16,777,216-element array limit remains; a square matrix is consequently at most
4096 by 4096. Subdivision ensures handoff, not fast completion of a maximum-size
dense system. The synchronous API remains useful when handoff is unnecessary.

The SDK includes `solve-async` with another task's progress, pivoting,
checkpoint/restore, cancellation and source-free replay. Native verification
compares exact result bits with synchronous solve, independently checks a known
solution and residual, and exercises views, empty inputs, late invalid values,
singularity and retained work. CLI and extracted SDK verification cover both
record modes, task cancellation, reclamation and fatal budgets.

`scripts/benchmark-solve-cooperation.py` measures a source-free 257×257
cyclically permuted, diagonally dominant system with a known all-one solution.
Setup and validation of every solution component are included; compilation is
excluded. Linux x86_64 release measurements use a warmup and three serial samples
without concurrent local builds/tests. Medians in this environment were:

| Implementation | Wall time | CPU time | Peak process RSS |
| --- | ---: | ---: | ---: |
| v1.9.30 synchronous baseline | 0.0661 s | 0.0659 s | 21,372 KiB |
| v1.9.33 synchronous | 0.0726 s | 0.0723 s | 22,432 KiB |
| v1.9.33 cooperative | 0.4551 s | 0.4548 s | 22,556 KiB |
| v1.9.33 cooperative with foreground task | 0.5029 s | 0.5021 s | 22,548 KiB |

The foreground task progressed 1440 times. Bounded range reads in elimination
and back substitution avoid traversing the page tree for every cell. Cooperative
execution still adds significant state/COW/scheduler overhead for this workload;
callers can use synchronous solve when throughput matters more than handoff.
These process measurements include VM startup and metadata, not only native
array bytes. They do not establish timing guarantees, GUI latency, Windows
performance or large-system memory requirements.

QR, eigen and autodiff subdivision, broader snapshot/secret/cache accounting,
GUI features, additional integrated server workflows and the remaining v2
acceptance measurements continue. This release does not declare v2 complete.
