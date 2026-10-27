# REWIND v1.9.31 — Cooperative vectors and solver steps

`std.numericAsync` adds three pure cold Tasks:

| API | Result and contract |
| --- | --- |
| `scale(array,factor)` | `Result<FloatArray,StdError>`; finite scalar multiplication, shape preserved |
| `zipFloat(operation,left,right)` | `Result<FloatArray,StdError>`; matching shapes, `add` / `sub` / `mul` / `div` |
| `norm2(array)` | `Result<Float,StdError>`; scaled Euclidean norm, empty norm is zero |

Each native step processes at most 4096 logical values, then yields if work
remains. Arbitrary array ranks, broadcast views and reverse/strided views are
supported without materializing the entire input. Values are visited in the
same logical order as the synchronous kernels. The norm retains scaled
accumulation rather than squaring large or tiny magnitudes. Synchronous APIs
remain available for callers that do not need handoff.

Only the completed output is returned. A late invalid value, division by zero,
overflow or cancellation does not expose a partial output or mutate inputs.
Private output pages and progress are VM state: a retained checkpoint restores
them, and removing the last owner releases native storage. Execution and memory
budgets remain fatal, including while a task is suspended. There is no host
worker, I/O or external observation.

`std.sparseAsync.conjugateGradient` now uses the same bounded vector steps,
scaled norms and compensated dot steps inside the solver, as well as the
previously bounded CSR validation/products. Helpers hand off the current task;
they do not create child tasks per iteration. Its signature, zero initial guess,
positive-definite input requirement, convergence criterion, actual-residual
verification and exhaustion result are preserved. The numerical work order is
unchanged.

Numeric zero arrays and cooperative output initialization reserve their shared
zero-page/tree storage rather than the full logical array byte count. Their
fixed native work charge covers bounded rank and logarithmic tree construction.
The existing 16,777,216-element limit remains. Each subsequent write reserves
new COW paths/pages and bounded scratch before execution; the Runtime's unique
physical-storage ledger still accounts for every retained version. A large
virtual zero is cheap to create, while filling it still consumes work and memory.
Dot/matmul steps charge actual remaining operations up to 4096, avoiding a full
chunk fee for tiny or final steps. These are budget admission improvements,
not a removal of limits.

The SDK includes `vector-async`, demonstrating another task's progress, a
mid-operation checkpoint, published output, views, cancellation and source-free
replay. Native tests compare chunked and synchronous results and preserve old
output on late errors. CLI tests cover both trace modes, large/tiny norms,
late division failure, low-budget virtual-zero/output admission, and repeated
cancellation with GC and quota reclamation. Existing numeric and sparse solver
regressions continue to run. Linux and Windows packages execute the example
after deleting source and compilation cache.

LU / QR / eigen / autodiff kernels still require cooperative subdivision.
General snapshot, secret and native cache accounting, GUI/PostgreSQL integration,
long-run measurements and the remaining v2 acceptance conditions continue.
