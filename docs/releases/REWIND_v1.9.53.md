# REWIND v1.9.53 — Cooperative unary transforms

`std.numericTransformAsync.mapFloat(operation,array)` and
`activation(operation,array)` create cold tasks with owned, shared-page inputs.
Each native step processes at most 4096 logical values and the task explicitly
hands execution to other ready tasks between steps. Initialization uses bounded
shared zero storage. Strided and broadcast inputs are traversed without a full
materialized copy; page-range COW output and scratch remain admitted before work.

`mapFloat` preserves all existing numeric map operations, domain checks, rounding
and nonfinite-result errors. `activation` preserves relu / reluGrad / sigmoid /
sigmoidGrad / tanhGrad / reciprocal, including stable extreme sigmoid inputs,
forward-output derivative domains and numeric overflow errors. The two operation
namespaces remain separate; invalid operations fail even on empty arrays.

Typed errors are the inner task Result. Cancellation is the outer TaskError and
does not expose a partial output. Fatal execution and memory budget errors follow
the existing execution contract. Inputs and checkpointed partial outputs retain
their storage versions. Debug/compact artifacts and replay use existing native
array wire storage; no new value or host resource is introduced.

These primitives require selected language 1.9.53. Existing numericAsync and
synchronous numeric / tensor / autodiff contracts remain available. This step
does not make `autodiff.backward` cooperative: tape traversal, reductions and
other derivative kernels require further work. See the shipped unary-async
example for task handoff, cancellation and checkpoint restore.

The shared-storage profile's cumulative allocated_bytes counter measures
conservative charged storage, including metadata fees. It is neither an exact
allocator instrumentation counter nor simultaneous live memory. The v1.9.50
training measurement wording is corrected accordingly; retained bytes and
measured OS RSS remain separate evidence.

Before publication, verify independent numeric functions, reversed-page results,
old error contracts, COW/wire roundtrip, late failure, handoff, cancellation/GC,
source-free recording/replay, and both extracted platform SDKs. This increment
does not assert v2.0 completion.

## Small source-free measurement

Optimized Linux runs mapped exp over 262144 native zero cells and checked their
sum. Each case used one warm-up and three serial samples. Compilation is
excluded; startup, profiling and the final synchronous sum are included.

| Case | Median wall s | Median CPU s | Peak RSS KiB |
| --- | ---: | ---: | ---: |
| 1.9.52 synchronous | 0.02311 | 0.02294 | 22112 |
| 1.9.53 synchronous | 0.02388 | 0.02370 | 21420 |
| 1.9.53 cooperative | 0.02908 | 0.02885 | 20696 |

This small workload does not establish a synchronous throughput improvement.
The cooperative API pays task/step overhead to allow handoff and cancellation;
these measurements do not prove GUI latency or general large-model performance.
Use scripts/benchmark-unary-transforms.py to reproduce this specific workload.
