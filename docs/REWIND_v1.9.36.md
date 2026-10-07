# REWIND v1.9.36 — Shared immutable VM text

Long `String` values share reference-counted storage when copied by assignment,
function arguments/returns, captures and retained VM state. Concatenation and
Rust-side mutation use copy-on-write, preserving the contents of other owners.
Dropping the final owner releases its buffer. Short text keeps the previous
owned-String path to avoid an extra Arc allocation for tiny labels and scalar
conversions. The current implementation uses a 256-byte threshold; it is an
implementation choice, not a new language distinction.

String equality, ordering, hashing, Unicode contents and the serialized
`Value::Text` format remain unchanged. The serializer still emits a string,
so compiled artifacts and recorded state do not expose storage identities.
The Rust embedding API now accepts `Value::Text(text.into())`; an owned `String`
can be recovered with `text.into_owned()` or `String::from(text)`. REWIND source
does not need this conversion.

`String.byteLen()` admits the cost of copying its receiver: bounded short-text
work or constant shared-reference work for long text. The byte length itself
uses stored metadata. Methods that scan, slice, encode or produce new contents
retain their respective content work; this does not waive budgets for those
operations.

This change removes repeated large byte copies inside the VM. General history
and container accounting still uses logical payload sizes in several paths;
it does not yet count every shared scalar backing allocation once. Checkpoint
and capture owners deliberately retain their data, and file/trace/secret/native
accounting remain separate v2 work. Reference counting of string buffers does
not replace the VM's collection of heap objects and cycles.

The SDK example `text-storage` exercises a captured value, List, Map,
concatenation and checkpoint restoration with Japanese text. Verification
includes source-free debug/compact recording and replay, byte-length work
admission, actual shared ownership and copy-on-write, old string wire format,
hash/order compatibility, and existing Unicode/codec/secret/task regressions.
Optimized serial measurements must cover long copies as well as short text;
root/compiler setup is excluded from execution measurements.

Cooperative QR/eigen/autodiff, physical snapshot/secret accounting, GUI expansion
and remaining integrated v2 acceptance measurements are still in progress.

## Serial execution measurements

Linux optimized builds, one warm-up and three measured source-free runs per
variant; compilation is excluded. Values below are medians. The same program
passes text through a function, copies local values and checks byte lengths.
`scripts/benchmark-text-storage.py` reproduces the workload.

| Text / iterations | v1.9.35 elapsed / CPU | v1.9.36 elapsed / CPU | v1.9.35 / v1.9.36 peak RSS |
| --- | --- | --- | --- |
| 11 bytes / 16,384 | 0.3264 / 0.3262 s | 0.3321 / 0.3318 s | 13,076 / 12,456 KiB |
| 69,632 bytes / 2,048 | 0.0750 / 0.0748 s | 0.0472 / 0.0470 s | 14,192 / 13,212 KiB |
| Alternating 11 and 34,816 bytes / 4,096 | 0.1200 / 0.1197 s | 0.1030 / 0.1028 s | 13,900 / 12,964 KiB |

The long-copy workload reduces elapsed time by about 37% and peak RSS by about
7%; the mixed workload reduces elapsed time by about 14%. Short text has about
2% higher median elapsed/CPU time in this run, so this measurement does not
establish a short-text speedup. RSS includes executable/runtime overhead, and
these small local workloads do not establish Windows performance, allocation
counts or sustained large-application memory behavior.
