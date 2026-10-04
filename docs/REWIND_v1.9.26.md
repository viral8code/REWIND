# REWIND v1.9.26: cooperative FFT and shared zero pages

`std.fftAsync.transform(real,imag,inverse)` is a cold pure Task returning
`Result<fft.Spectrum,StdError>`. Starting it shares the input arrays and creates
private COW scratch. Equal rank-one power-of-two Float64 vectors are required,
with 1..1048576 samples. The existing synchronous `std.fft.transform` remains
available.

Initialization builds shared zero subtrees in logarithmic time. Subsequent
native calls process at most 4096 input copies, cached twiddles, butterflies,
or inverse-scale items. Each call hands off to ready tasks. Bit-reversed copying
checks finite inputs incrementally; the butterfly stages preserve the existing
radix-2 formula and use a shared twiddle table. Adjacent small blocks are written
in one page range, avoiding a COW copy/hash for every individual butterfly.
Large blocks use two contiguous ranges. Native scratch is bounded and reserved
before each step; global and per-task work limits still apply.

Cancellation returns `TaskError::Cancelled`. A typed numerical error or
cancellation exposes no partial Spectrum. Checkpoints retain the private work
state and shared pages; restoring a running Task resumes that state. This is VM
computation, so it starts no host worker and records no external observation.
Recorded scheduler choices and source-free debug/compact replay remain available.

The implementation uses O(n) work storage and O(n log n) arithmetic. An inverse
transform scales by 1/n. Float64 results are approximate. The maximum sample
count is a shape bound, not a promise that every configured memory/work budget
can execute the transform. Large workloads can require raising `--native-work`.

Zero-filled numeric buffers now share identical immutable subtrees. Their shape
and canonical digest match materialized zero buffers; a write copies only its
path/pages and a checkpoint keeps the prior values. Numeric accounting charges
unique physical nodes and releases them when the last owner disappears. Existing
public zero-constructor work/admission estimates remain conservative; this version
does not remove all numerical capacity or allocation bounds.

`FftWork` is an opaque reserved implementation type, available from language
1.9.26. Old language programs may still use user-defined server type names that
predate the native server API; these are not treated as native handles.

Validation covers an independent DFT, inverse transforms, mid-kernel COW state,
late nonfinite errors, invalid state bounds, dense/shared-page accounting,
source-free scheduler fairness, cancellation and both record modes. The SDK
includes the cooperative FFT example and module API snapshot.

Remaining v2 work includes cooperative solve/QR/eigen, CSR and autodiff kernels,
general container/snapshot memory accounting, application integration, server TLS
and private request credentials, and the remaining GUI and capacity checks.
