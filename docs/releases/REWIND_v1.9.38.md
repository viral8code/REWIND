# REWIND v1.9.38 — Cooperative symmetric eigen decomposition

`numericAsync.eigenSymmetric(matrix, tolerance, maxSweeps)` returns a pure
`Task<Result<EigenResult,StdError>>`. Input copying, scaling, symmetry checks,
cyclic Jacobi scans/rotations, stable eigenvalue ordering and result copying
execute in at most 4096 scalar work units per step, with task handoff between
steps. No host worker or external observation is involved.

`EigenResult` keeps the synchronous API: native FloatArray `values` / `vectors`
and Int `sweeps`. Values are sorted ascending using Float total ordering;
equal eigenvalues preserve their original order. Eigenvectors are columns.
Rotation and arithmetic order match synchronous `numeric.eigenSymmetric`.
The input must be a finite square Float64 matrix, symmetric within the scaled
nonnegative tolerance. maxSweeps is 0..10000. Nonconvergence, nonfinite input,
shape/domain/type errors and overflow remain explicit numeric errors.

Private `EigenWork` holds COW native arrays and scalar progress, and is opaque
starting with language 1.9.38. Copy, normalization, checks, diagonalization,
merge passes and output copies all have bounded work. Sorting uses native
IntArray scratch rather than an unbounded VM List or an indivisible library
sort. QR and eigen steps share grouped private page-digest refreshes. A failed
or cancelled step exposes no partial result; checkpoints retain their reachable
work versions, and discarded versions release their native pages.

Native work admission accounts for small/empty shapes and bounds larger steps.
Fatal work/memory limits stay separate from numeric Result errors. Initialization
uses shared zero storage, and a full initial copy chunk reserves only its touched
pages. Later COW admission remains conservative; native node metadata and dirty
path scratch are included, without claiming an exact allocator-byte guarantee.
Large jobs need an appropriate `--native-work` budget; the SDK sample documents
its explicit execution budgets.

Validation covers synchronous bit/digest/sweep agreement, stable equal-value
ordering and negative zero, transposed/scaled/empty inputs, independent residual
and orthogonality, checkpointed rotations, late input/overflow failure, original
state and accounting preservation, small-budget large-input cancellation,
repeated cancellation/GC, source-free debug/compact replay and signed SDK use.
Execution and memory measurements are required before publication.

Cooperative least squares/autodiff, general snapshot/secret accounting, remaining
GUI features and integrated v2 acceptance/performance work remain in progress.

## Serial execution measurements

Linux optimized builds, one warm-up and three measured source-free runs per
variant. Medians include common matrix setup and runtime startup and exclude
compilation. `scripts/benchmark-eigen-cooperation.py` reproduces these symmetric
inputs. The foreground variant performs 20 task handoffs and verifies that it
finishes while eigen decomposition remains unfinished.

| Size / execution | Elapsed | User + system CPU | Peak RSS |
| --- | --- | --- | --- |
| 65 / synchronous | 0.0323 s | 0.0314 s | 22,616 KiB |
| 65 / cooperative | 0.4083 s | 0.4077 s | 22,640 KiB |
| 65 / foreground + eigen | 0.3972 s | 0.3968 s | 22,848 KiB |
| 129 / synchronous | 0.0518 s | 0.0515 s | 22,844 KiB |
| 129 / cooperative | 4.0690 s | 4.0664 s | 22,692 KiB |
| 129 / foreground + eigen | 4.1039 s | 4.1025 s | 22,868 KiB |

Cooperative execution takes about 12.6x and 78.5x the synchronous
elapsed time for these inputs. Persistent strided page updates, grouped digest
refreshes, private step versions and VM scheduling remain substantial costs.
Use synchronous eigen decomposition when throughput matters more than task
responsiveness. These results do not establish Windows timings, large-input
performance, long-run memory stability or individual scheduler latency bounds.
RSS is mostly common executable/runtime overhead at these sizes; no memory
reduction is claimed. Reducing page-update and digest costs remains required
performance work toward v2.
