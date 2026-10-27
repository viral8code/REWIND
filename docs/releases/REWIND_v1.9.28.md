# REWIND v1.9.28: bounded shared DNS admission

HTTP, TCP and PostgreSQL now share process-wide admission for OS hostname
resolution. At most eight blocking resolution jobs may be admitted, including
queued jobs and work whose caller was cancelled. Admission uses try-acquire;
exhaustion returns a typed failure rather than adding another unbounded waiter.
Numeric IPv4 / IPv6 addresses bypass DNS admission. Results contain at most
64 socket addresses; larger answers are rejected before a connection is made.
Names passed to OS resolution are bounded to 254 bytes.

A permit lives inside the blocking closure. Cancelling its async caller, dropping
its JoinHandle, reaching a deadline or reverting the VM does not release that
permit while getaddrinfo still runs. OS name resolution itself cannot be
cancelled. A stalled resolver can occupy a slot until the OS finishes; the cap
bounds outstanding work rather than promising interruption or progress. It is
shared across Runtime instances and transport adapters, not just one connection.

Both default and explicit-CA HTTP clients use this resolver. HTTP exposes
HttpBusy, HttpResolveLimit or HttpResolve with the existing NotSent phase when
resolution fails; deadlines retain HttpTimeout. Error values do not contain the
hostname or OS error detail. PostgreSQL maps admission/limit/lookup errors to
DbBusy/DbLimit/DbDisconnected; TCP retains TcpBusy/TcpResolveLimit/TcpResolve.
PostgreSQL still selects one address and makes one connect attempt. TCP and
HTTP retain their transport's pre-application address handling. This adds no
application resend, certificate-verification bypass or reconnection on revert.

The shared HTTP runtime has two async and at most two blocking workers with
explicit 1 MiB stacks. A PostgreSQL connection's runtime permits one blocking
worker with a 1 MiB stack. The existing TCP runtime remains bounded. Static
worker pools, per-connection reservations and VM budgets remain distinct;
this patch does not claim exact accounting of every allocator or OS buffer.

Successful observations, live leases, typed failure receipts, affine resource
lifetimes and source-free replay keep their existing contracts. Replay does not
resolve names or connect to the host. No new public library function or effect
is introduced; SDK module count remains 71.

Validation covers cancelled/queued jobs retaining admission, numeric addresses
with no available DNS slot, bounded result lists, typed reqwest error causes,
existing raw TCP/TLS tests and actual HTTP/PostgreSQL/TCP connections. Extracted
SDK smoke tests continue to verify TLS, both DB adapters, GUI/network examples,
source-free execution and disconnected replay on each supported OS.

Remaining v2 conditions include cooperative LU/QR/eigen/autodiff and vector
kernels, native idle-client/cache and general snapshot memory accounting,
application integration, server TLS/private request credentials, GUI facilities,
model/precision/long-run benchmarks and remaining capacity checks.
