# REWIND v1.9.34 — Bounded HTTP clients and copy admission

HTTP clients are cached by canonical origin (scheme, hostname and effective
port) and the public CA fingerprint. At most 16 origin/CA pairs are cached.
Repeated requests reuse the client and HTTP/1 connection; least recently used
entries are evicted. Idle entries are removed when a later request observes
30 seconds without use. Each client's underlying pool still keeps at most two
idle connections per host for at most 30 seconds. Redirects and automatic
retries remain disabled, so each client serves only its selected origin.

Eviction does not cancel an active request or stream. An Arc lease retains its
client through driver completion/cancellation acknowledgement and through
download/upload resource ownership. A weak registry counts each retained client
once, including an evicted client with a live owner, and releases its allowance
when the final owner drops. Closing download/upload slots occupy their eight-slot
limit until the worker releases them, preventing repeated cancellation from
creating an unbounded queue of retired streams. Cache, driver and stream limits consequently bound
ownership; the registry removes stale weak entries during subsequent requests.
Revert restores VM receipts, not a closed socket or the native cache.

Before physical submission, native admission includes a conservative client
configuration/pool allowance of 1 MiB plus public-CA/key/metadata space for a new
client. The allowance remains in native memory accounting while a lease lives;
request/response/stream buffers have their existing separate reservations.
Failure to admit returns `HttpMemoryLimit` with `NotSent`. This reservation is
not an exact measurement or a hard bound on all Rustls/OS allocations. Peak RSS
and further native allocator/capacity auditing remain separate v2 work. A cold
origin also incurs client/trust-store setup; existing clients avoid that setup.

The eight distinct custom-CA fingerprint limit is preserved independently of
origin eviction. A supplied PEM bundle must contain at least one certificate;
empty/non-certificate input now fails before I/O rather than silently adding
no trust. Certificate chains and hostnames remain verified, PEM roots add to
system trust, and credentials belong to individual requests rather than cached
client configuration. No new credential or external-I/O capability is added.

`rewind profile` exposes three native measurements, always including zero:
`runtime.http_cached_clients`, `runtime.http_retained_clients`, and
`runtime.http_client_reservation_bytes`. These distinguish a bounded connection
cache from VM checkpoint memory and the existing `native_resources` handle
count. They are current native state, not replayed historical resource counts.

LU input-copy admission now reserves only the bounded contiguous pages and
paths when an entire step stays in the initial copy phase. Later mixed phases
retain conservative reservation. A 4096×4096 virtual-zero input can perform a
step, checkpoint, restore and cancel under a 2 MiB history limit; filling or
solving the full matrix still requires its actual work/storage. This changes
admission, not the result, ordering, algorithm or array-size limit.

The SDK includes `http-pool`. Verification uses real independent loopback
origins to check keepalive, eviction/reconnection, current resource counts and
source-free disconnected replay in both record modes. Native tests check origin
normalization, trust isolation, idle peer closure, retained leases and metadata
bounds. Real HTTPS tests accept the fixture CA and reject an untrusted chain,
hostname mismatch and malformed PEM before the HTTP service sees a request.
Pending responses and uploads survive eviction; completion and cancellation
release their leases after worker acknowledgement. Retained closing download
and upload owners count toward the eight-slot limit, and releasing them admits
the next stream. A two-origin 2 MiB admission test confirms that the second
request returns `HttpMemoryLimit` / `NotSent` without reaching its server.
Whole-standard-library tests, all 71 API baselines and a signed SDK with the new
sample are checked. Existing TLS, streaming, cancellation and budget regressions continue.
QR/eigen/autodiff subdivision, snapshot/secret accounting, GUI expansion and
remaining v2 acceptance measurements remain in progress.
