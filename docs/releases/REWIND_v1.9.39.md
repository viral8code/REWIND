# REWIND v1.9.39 — Numeric storage digest caching

Numeric page/tree nodes cache their existing SHA-256 content identity on demand.
Writes invalidate the changed private path and update storage/ledger metadata;
unchanged immutable pages and checkpoint versions retain their caches. A COW
clone copies an already computed digest without forcing a cold digest. Debug
state, buffer equality and autodiff structural keys request the same byte-for-byte
storage digest as before. No numeric arithmetic, wire format or external
observation schema is changed.

The cache is embedded in each node and uses synchronized initialization for
immutable readers. It holds no additional storage owners or payload copies.
Storage admission counts materialized tree nodes and odd-level empty leaves
using the target OS node layout instead of a fixed per-page metadata constant.
Runtime accounting still charges each reachable shared node once, includes node
metadata, charges COW pages and releases them when their last owner disappears.
QR/eigen now update cheap metadata and invalidate caches directly during writes,
removing the redundant dirty-path HashSet and deferred refresh pass. No partial
step escapes. Existing conservative scratch admission remains; no exact peak
allocator-byte bound is claimed.

Cold structural identity takes O(backing storage); subsequent identity takes
O(rank) with cached page hashes. A small view can retain a large backing array.
Starting with language 1.9.39, TensorKey admission conservatively charges the
backing element count, independently of cache warmth. Recording may warm caches,
so budget admission must never depend on a cache's current state. Programs selecting an older language
retain their existing fixed key fee. Artifact/record compiler-version checks
remain strict: rebuild artifacts when upgrading and replay existing records
with the compiler version that created them. Fatal native work/memory limits
remain separate from numeric/autodiff Result errors. Large tapes/views may need
an explicit native work budget. Hashing itself remains synchronous; cooperative
autodiff and larger per-step bounds remain separate work.

Validation must cover an independent eager reference digest, cached/uncached
COW and range edits, zero-page sharing, views/wire identity, simultaneous
immutable readers, shared ledger lifetime, source-free debug/compact checkpoint
replay and cold/warm backing-storage admission. Execution/memory measurements
and the complete release checks are required before publication.

Cooperative least squares/autodiff, remaining GUI features, general snapshot and
secret accounting, sustained performance and integrated v2 acceptance remain
in progress. This patch does not establish v2 completion.

## Serial execution measurements

Linux optimized v1.9.38 and v1.9.39 binaries, one warm-up and three measured
source-free runs per variant. Common array setup/startup are included;
compilation is excluded. `scripts/benchmark-qr-cooperation.py` and
`scripts/benchmark-eigen-cooperation.py` reproduce the square inputs. Foreground
variants complete 20 handoffs while decomposition remains unfinished.

| Kernel / size / execution | v1.9.38 elapsed | v1.9.39 elapsed | v1.9.39 CPU | v1.9.39 peak RSS |
| --- | --- | --- | --- | --- |
| QR / 65 / synchronous | 0.0393 s | 0.0375 s | 0.0372 s | 22,576 KiB |
| QR / 65 / cooperative | 0.0829 s | 0.0635 s | 0.0633 s | 22,740 KiB |
| QR / 65 / foreground | 0.0778 s | 0.0621 s | 0.0618 s | 22,916 KiB |
| QR / 129 / synchronous | 0.0621 s | 0.0623 s | 0.0620 s | 22,420 KiB |
| QR / 129 / cooperative | 0.5454 s | 0.3178 s | 0.3175 s | 22,556 KiB |
| QR / 129 / foreground | 0.5841 s | 0.3456 s | 0.3453 s | 22,752 KiB |
| eigen / 65 / synchronous | 0.0368 s | 0.0361 s | 0.0346 s | 22,560 KiB |
| eigen / 65 / cooperative | 0.4100 s | 0.2133 s | 0.2128 s | 22,556 KiB |
| eigen / 65 / foreground | 0.4260 s | 0.2034 s | 0.2031 s | 22,888 KiB |
| eigen / 129 / synchronous | 0.0592 s | 0.0520 s | 0.0516 s | 22,588 KiB |
| eigen / 129 / cooperative | 4.5285 s | 1.7565 s | 1.7557 s | 22,716 KiB |
| eigen / 129 / foreground | 4.3357 s | 1.7411 s | 1.7398 s | 22,764 KiB |

Digest caching and removal of dirty-path tracking reduce cooperative decomposition
cost for these inputs. Synchronous timings include startup/setup and vary between
runs; no general synchronous speedup is claimed. Cooperative execution remains
slower than synchronous execution, especially for strided large-matrix updates.
Further page-access work is required. RSS is mostly common runtime/executable
overhead at these sizes and does not prove storage savings or long-run memory
stability. These measurements do not establish Windows timings, large-input
performance, GC pause bounds or individual scheduler latency.

## Cancellation retention regression

The sparse cancellation regression measures the live input baseline separately
from allocations awaiting the next automatic collection. The collection trigger
is 4 MiB of allocations since the previous collection, not a 4 MiB cap on all
reachable numeric data. The test retains the 8 MiB history limit, checks task and
quota metadata together, and runs 0, 128, 512 and 2,048 cancellations. The zero-run
baseline plus the collection threshold bounds retained numeric pages; collection
must occur for each nonzero case. The work budget permits the longer run without
changing the production collection policy.
