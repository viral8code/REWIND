# REWIND v1.9.27: cooperative CSR matrix-vector products

`std.sparseAsync.matvec(matrix,right)` is a cold pure Task returning
`Result<FloatArray,StdError>`. It shares the CSR and right-hand array pages and
creates private COW output, without a temporary copy of every nonzero entry or
an external worker. The existing synchronous `std.sparse.matvec` remains available.
The existing `SolveResult` and `conjugateGradient` APIs are preserved.

Initialization validates ranks, lengths, types, capacities and the first offset.
It creates shared zero output in logarithmic time. Native steps first check all
right-vector values, including unused columns, then validate CSR offsets, strictly
increasing in-range columns and finite values while computing compensated sums.
Each step processes at most 4096 checks, entries or row completions and then hands
off to ready tasks. A single wide row is split across steps; empty rows also
consume work items. Incomplete sums and their compensation stay in private state.
Completed adjacent rows are written in one page range rather than copying a page
for every output cell. Bounds remain 2^20 rows, columns and entries.

`conjugateGradient` now uses these bounded CSR steps for its initial validation,
search-direction products, true residual checks and final residual. It does not
spawn child tasks for these calls. Vector arithmetic, dot products and norms in
the solver remain synchronous. Its convergence criterion, zero initial guess,
SPD precondition, iteration limit and typed exhaustion are unchanged.

Indexed page reads take logarithmic tree traversal: total work is bounded by
O(cols log(cols) + (rows + nnz) log(max(rows,cols,nnz))), plus fixed per-step
metadata overhead. Storage is shared inputs plus O(rows) output; step scratch
is bounded. Small problems are charged for their remaining items rather than
an entire 4096-item batch. Work and allocation limits still apply before native
execution. This version does not remove capacity or global history budgets.

Cancellation returns `TaskError::Cancelled`; numerical failure exposes no partial
result. A checkpoint retaining a running Task keeps its partial sums and COW
pages. Restoring it resumes computation, while dropping the final owner frees
that storage. Scheduler recording, compiled source-free execution and
both debug/compact replay remain supported. `SparseWork` is an opaque reserved
implementation type from language 1.9.27.

Validation includes dense/synchronous references, strided inputs, compensation,
wide and empty rows, checkpoint state, late invalid columns, unused nonfinite
inputs, existing solver regressions, fairness, cancellation and source-free replay.
The extracted SDK runs the cooperative sparse example on each supported OS.

Remaining v2 work includes cooperative LU/QR/eigen/autodiff and vector kernels,
general container/snapshot accounting and capacity measurements, integrated GUI /
PostgreSQL / HTTP examples, server TLS and private request credentials, and the
remaining GUI, model and release acceptance checks.
