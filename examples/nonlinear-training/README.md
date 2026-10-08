# Nonlinear minibatch training

Train a sigmoid model using two unequal batches (three and five samples),
weighted mean gradients, global-norm clipping and Adam. Verify loss reduction,
known parameters and model encode/decode/save/load. Numeric cells remain native
arrays; each iteration drops its tapes and intermediate gradients. The initial
checkpoint deliberately retains initial weights and optimizer state until the end.

Use `rewind run main.rw --allow-effects fileRead,fileWrite --steps 100000000 --history-memory 128MiB --native-work 1000000000`.
Native autodiff and training helpers are synchronous; this example does not
establish cancellation within a kernel or sustained GUI responsiveness.

An optional argument chooses 250..10000 updates (default 250), for repeated-work
measurements; pass it after `--`. This does not change the input dataset.

Full 250-update training is recorded with `--record-mode compact`. The existing
16 MiB debug-index limit can reject this longer workload. `trace-small.rw`
provides bounded named-gradient/weighted-mean/clipping/Adam checkpoint and revert
acceptance in debug mode; its short trace is not the full training run.
