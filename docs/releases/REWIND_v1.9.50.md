# REWIND v1.9.50 — Minibatch training helpers

`std.training` composes the existing native-array autodiff and optimizer APIs.
`meanSquaredError` calculates the mean of squared differences over all cells;
`gradients` runs one backward pass and returns only selected named gradients.
Intermediate gradients become collectible after the local result owner is dropped;
selected arrays retain shared native storage. Constants and unreachable nodes
produce `TrainingUnused`, rather than silently supplying zeros. Names and output
maps follow the model's 128-parameter /32 MiB contract; selected outputs receive
the existing model validation pass.

`mergeMean(left,leftSamples,right,rightSamples)` weights two batch-mean maps by
positive sample counts. Names and shapes must match; the exact integer total is
bounded by 2^53-1. This computes a sample mean when each sample contributes the
same number of loss cells. It is not a sum-gradient accumulator or an automatic
weighting policy for variable-length samples.

`clipGlobalNorm` computes scaled per-array Euclidean norms and combines them
without squaring large magnitudes. It shares unchanged arrays and constructs
new native arrays only when clipping is needed. The maximum must be positive
and finite. A nonrepresentable individual array norm returns the existing numeric
overflow error; a combined norm larger than Float64 can still be clipped when its
individual array norms are representable. Inputs are unchanged on success and
typed failure; fatal execution-budget recovery follows the checkpoint contract.

These helpers are synchronous. They do not promise cancellation or GUI handoff
inside existing autodiff kernels. Successful tape operations preceding a failed
`meanSquaredError` call remain on the tape; the tape is not implicitly reverted.

`examples/nonlinear-training` trains a sigmoid model using unequal batches,
clipping and Adam, checks loss and known parameters, and verifies model persistence.
Independent derivative, weighted-mean, overflow, invalid-node and checkpoint
checks and extracted SDK source-free replay are required before publication.
This increment does not establish v2.0 completion.

Full 250-update training is recorded with `--record-mode compact`. The existing
16 MiB debug-index limit can reject this longer workload. `trace-small.rw`
provides bounded named-gradient/weighted-mean/clipping/Adam checkpoint and revert
acceptance in debug mode; its short trace is not the full training run.

## Repeated-work measurement

Optimized Linux source-free runs used the same eight samples, unequal batches,
named backward gradients, clipping, Adam and model persistence. Each case had one
warm-up and three serial samples. Compilation is excluded; startup and profiling
are included. Wall/CPU/RSS are medians; deterministic GC counts and end-of-run
retained bytes are shown from a sample.

| Updates | Wall s | CPU s | Peak RSS KiB | Completed GC | Native numeric bytes | Shared payload bytes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 250 | 6.483 | 6.479 | 46416 | 63 | 16296 | 367088 |
| 1000 | 25.210 | 25.186 | 46504 | 254 | 2760 | 53936 |
| 5000 | 129.681 | 129.661 | 46320 | 1269 | 16744 | 381148 |

RSS stays approximately level across these cases, and the discarded tapes and
gradients are collected. This does not prove memory behavior for larger networks,
long-lived checkpoints or GUI latency. CPU time grows with updates. The 5000-update
case reports about 4.5 GB in the cumulative shared-storage charging counter despite
small retained storage. This counter includes conservative metadata fees; it is
not an allocator measurement or 4.5 GB of simultaneously live memory. Repeated
metadata construction and interpreter overhead remain optimization targets. GC duration alone does not explain the execution cost.
Reproduce with `scripts/benchmark-nonlinear-training.py`; reports contain bounded
metrics and omit heap contents.
