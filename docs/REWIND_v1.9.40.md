# REWIND v1.9.40 — Cooperative eigen page access

The cooperative symmetric Jacobi kernel keeps private eigenvectors in column-major
storage and reads rotation inputs through contiguous page iterators. Each bounded
rotation stages four arrays of at most 4,096 elements, writes vector columns and
matrix rows in ranges, and updates mirrored matrix columns. The scratch admission
adds 128 KiB for these four buffers. Existing conservative COW admission remains.
The public eigenvectors still use the existing row-major FloatArray layout.

The rotation formulas, pair order, sorting and scalar-unit work bound remain
unchanged. Symmetry averaging is still performed, including signed-zero cases;
writing a cell is skipped only if its resulting bits already equal its contents.
Private work is immutable between steps. Checkpoints keep their previous version,
and cancellation or a failed step cannot publish partial output.

Artifact and record compiler-version checks remain strict. Recompile source for
this compiler and replay an old record with the compiler that created it. The
private eigenvector layout is not an interchange format. Older selected language
versions retain their existing syntax and library gates.

## Acceptance before publication

Verify exact synchronous/cooperative output bits, independent residual and
orthogonality checks, stable ties, large/small scales, empty and strided inputs,
signed zero, and checkpoints across partial rotations and page boundaries.
Run source-free debug/compact replay, cancellation and fatal-budget regressions,
standard API checks and extracted signed SDK acceptance on Linux and Windows.
Measure serial optimized execution against v1.9.39; do not infer a speedup or
memory reduction from the layout change alone.

Cooperative least squares/autodiff, general snapshot and secret accounting,
integrated GUI/server/DB acceptance, remaining GUI functions and sustained
performance measurements remain part of the v2 plan. This patch does not
establish v2 completion.

## Serial optimized execution measurements

Linux optimized v1.9.39 and v1.9.40 binaries; one warm-up and three measured
source-free runs per variant. Common array setup/startup are included; compilation
is excluded. The foreground case completes 20 handoffs while decomposition is
unfinished. Use `scripts/benchmark-eigen-cooperation.py` to reproduce the inputs.

| Size / execution | v1.9.39 elapsed | v1.9.40 elapsed | v1.9.40 CPU | v1.9.40 peak RSS |
| --- | --- | --- | --- | --- |
| 65 / synchronous | 0.0309 s | 0.0311 s | 0.0309 s | 23,356 KiB |
| 65 / cooperative | 0.1912 s | 0.0949 s | 0.0947 s | 23,384 KiB |
| 65 / foreground | 0.1890 s | 0.0973 s | 0.0970 s | 23,512 KiB |
| 129 / synchronous | 0.0485 s | 0.0478 s | 0.0476 s | 23,276 KiB |
| 129 / cooperative | 1.8094 s | 0.6886 s | 0.6880 s | 23,376 KiB |
| 129 / foreground | 1.6725 s | 0.7203 s | 0.7198 s | 23,452 KiB |

Page-range rotations reduce cooperative execution cost for these inputs. The
cooperative kernel still costs more than synchronous execution. Startup/setup
affect the small synchronous timings; no general synchronous speedup is claimed.
RSS is dominated by common runtime overhead here and does not establish a storage
saving. These runs do not establish Windows speed, large-input performance,
long-run memory stability, GC pause bounds or per-handoff latency.
