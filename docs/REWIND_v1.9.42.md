# REWIND v1.9.42 — Cooperative least squares

`std.numericAsync.leastSquares(matrix,right,tolerance)` adds a pure task for
full-column-rank rectangular least squares. Inputs are FloatArray, m >= n,
and a rank-one rhs of length m. Tolerance is finite and nonnegative. The inner
result is `Result<FloatArray,StdError>`; awaiting adds the outer task result.
Rank deficiency returns NumericSingular; fatal execution and memory limits are
runtime errors, not numeric results.

The kernel first validates rhs values, then reuses bounded Householder QR,
computes compensated Q-transpose times rhs, performs upper-triangular back
substitution, and applies the pivot permutation. Each step processes at most
4096 scalar work units. It preserves synchronous arithmetic and error order,
including rhs nonfinite errors preceding invalid tolerance. It does not create
another LU factorization or materialize entire rhs/Q/R arrays. Work and numeric
pages remain immutable between steps; snapshots share previous pages. Cancellation
and failed steps expose no partial answer.

Native work fees and scratch admission are conservative, include QR progress and
copy-on-write numeric pages, and remain fatal when limits are exceeded. The work
record is opaque in language 1.9.42 and later. Older selected language versions
keep user-defined names and cannot call the new primitives. Compiler artifact and
record versions remain strict; recompile source or replay with its original compiler.

## Publication acceptance

Check bit agreement with synchronous rectangular, pivoted, empty and strided
inputs, independent residuals, checkpointed partial projection, typed shape/rank/
nonfinite/domain failures, cancellation and fatal budgets. Check language-level
source-free debug/compact recording and replay, opaque/versioned work records,
large shared input under bounded memory, standard API regeneration and the
extracted signed SDK on both Linux and Windows.

The existing integration sample remains part of SDK acceptance. General memory
and sensitive-value accounting, cooperative autodiff, remaining GUI capabilities,
capacity measurements and sustained performance acceptance still require work.
This patch does not establish completion of the v2 plan.

## Optimized Linux measurements

Serial source-free runs; one warm-up and three measured repetitions. Startup and shared array setup are included; compilation is excluded. The foreground variant completes 20 handoffs while the factorization remains unfinished. These measurements do not establish Windows performance, GUI latency or large-input scalability.

| Size / execution | Elapsed median | CPU median | Peak RSS median |
| --- | --- | --- | --- |
| 65 / synchronous | 0.0347 s | 0.0345 s | 23,360 KiB |
| 65 / cooperative | 0.0610 s | 0.0607 s | 23,408 KiB |
| 65 / foreground | 0.0646 s | 0.0641 s | 23,556 KiB |
| 129 / synchronous | 0.0614 s | 0.0610 s | 23,352 KiB |
| 129 / cooperative | 0.3170 s | 0.3167 s | 23,404 KiB |
| 129 / foreground | 0.3305 s | 0.3300 s | 23,396 KiB |

The cooperative path has scheduler and immutable-work overhead; it is not a general speedup over the synchronous kernel. Reproduce these inputs with `scripts/benchmark-least-squares-cooperation.py`.

Use `scripts/benchmark-least-squares-cancellation.py` to reproduce the following cancellation measurements. A separate optimized run repeatedly cancelled an unfinished task after one yield, using a shared 65-by-65 zero matrix and zero rhs. At 128, 512 and 2,048 cancellations, retained numeric storage was 32,016 bytes and the task quota table held two entries; completed GC cycles were 2, 8 and 32. Peak process RSS stayed within 22,880–23,052 KiB across the zero-count baseline and these runs. This is a small, short cancellation workload; it does not prove general payload accounting or long-running application memory stability.
