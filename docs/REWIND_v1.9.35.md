# REWIND v1.9.35 — Shared verified HTTP trust configuration

The bounded HTTP origin pool now reuses immutable Rustls configuration for the
same public-CA fingerprint. Native system roots are loaded once for an HTTP
host's lifetime. Eight custom-CA fingerprints plus default trust bound the
configuration cache; origin eviction does not reload system roots or reset the
custom-CA limit. A new VM host loads the current native roots. An existing host
does not observe subsequent OS trust-store changes; recreate the runtime to
adopt those changes.

Custom PEM roots remain additive to the native system roots. Certificate chain,
validity, server name and signature checks use Rustls's standard verifier, with
TLS 1.2/1.3 safe defaults, SNI and HTTP/1.1 ALPN. Client identity and request
credentials are not part of the shared configuration. TLS session storage is
configured with Rustls `in_memory_sessions(256)`: the pinned Rustls version
keeps at most 32 server-name entries, each with eight TLS 1.3 tickets and one
TLS 1.2 session. Separate trust configurations do not share it.
Redirect, retry, DNS, proxy, cancellation and
external recording behavior remain unchanged.

Configuration lifetime is distinct from the origin-client cache. A retained
client's existing conservative allowance covers its trust configuration. When
no retained client covers a cached configuration, its allowance is counted
separately. Consequently `runtime.http_client_reservation_bytes` may remain
nonzero when `runtime.http_retained_clients` is zero. A fixed nine-slot bitmap
avoids allocation while calculating this shared accounting. Invalid PEM/DER or
configuration failures retain neither a new configuration nor a newly loaded
system-root store. The allowance is still conservative rather than an exact
account of every native/OS allocation.

Validation includes real keepalive/eviction, pending/download/upload leases,
cancel acknowledgement and closing-stream quotas. Real HTTPS accepts the
fixture CA and rejects untrusted chains, hostname mismatch and malformed PEM;
native tests also check shared configuration identity and accounting after all
clients expire. SDK/source-free, offline replay, memory denial and GUI/HTTP with
both DB adapters remain required release checks.

`scripts/benchmark-http-clients.py` compares source-free runs across independent
loopback origins, excluding compilation and checking actual requests and peer
connections. It reports warmup-separated wall time, child CPU and peak RSS on
Linux. Release measurements must run serially without other builds or tests.
This workload measures HTTP client setup/reuse, not WAN latency or TLS throughput.

On the same Linux machine, optimized v1.9.34 and v1.9.35 binaries were run
serially with 64 origins, one discarded warmup and three measured runs. Every
run verified 66 responses and 65 physical connections. Median results were:

| Version | Wall seconds | Child CPU seconds | Peak RSS KiB |
| --- | ---: | ---: | ---: |
| v1.9.34 | 0.2187 | 0.1248 | 21,284 |
| v1.9.35 | 0.1839 | 0.0865 | 19,432 |

The measurements include VM startup and request validation. They show lower
client-setup cost and memory for this workload; they are not a throughput or
latency guarantee on other hosts or Windows. Root sets and connection latency
affect the gain.

Cooperative QR/eigen/autodiff, general snapshot/secret accounting, GUI expansion
and the remaining integrated v2 acceptance measurements are still in progress.
