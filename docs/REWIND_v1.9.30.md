# REWIND v1.9.30 — Private bearer authentication

`std.httpServer.bearerCredential(alias,token)` registers a `Secret<String>`
inside an external boundary. Aliases contain 1..128 ASCII letters, digits,
underscores or hyphens. Tokens contain 16..4096 ASCII graphic bytes. There are
at most sixteen immutable aliases with a 16KiB reservation each. Identical
registration is idempotent. Invalid, changed and excess configurations return
`HttpServerAuthCredential`, `HttpServerAuthCredentialImmutable` and
`HttpServerAuthCredentialLimit`. Budget failure does not register an alias.

`configuredTlsAuthenticated(tlsAlias,authAlias,address,port,maxBodyBytes,maxConnections,lifetimeMillis,deadlineMillis)`
requires registered TLS and bearer aliases. It retains the connection, body,
header, deadline and TLS memory limits of `configuredTls`. Unknown aliases are
denied before binding. Both new primitives require language 1.9.30 and effects
`external,network,tasks`.

The transport accepts exactly one Authorization field. Its Bearer scheme is
case insensitive and permits spaces before the token. The configured token is
matched exactly using SHA-256 and a constant-time comparison of fixed-length
digests through `subtle`. Header parsing and hashing remain bounded by the
existing header capacity; the complete request is not claimed to be constant
time. Native configuration stores the digest. Known-secret guards retain the
token classification, with its storage covered by the configuration reservation.

Missing, wrong or duplicate credentials receive an empty 401 response with
`WWW-Authenticate: Bearer` and `Connection: close`. These protocol failures do
not become VM input or consume the waiting `next` result. An accepted
Authorization field is removed before request capture. Tokens, keys and their
digests do not enter operation fingerprints, traces or artifacts; only public
aliases identify configuration. Existing guards still reject known private
values in URI, other headers and body. Other request headers retain their
normal public-input behavior.

Listener activation, physical connection cleanup and observed reply receipts
retain the previous contracts. Protocol 401 responses are immediate native
effects of the configured listener. VM revert does not undo them. Recorded
replay consumes already observed public requests without opening a listener or
validating a remote client again. A valid private configuration is re-injected;
the bearer token may differ. The TLS private key still matches the recorded
public certificate. `external live` uses the same authentication filter and
keeps observations only while owned by a task or checkpoint.

This is a static shared bearer credential, without user roles, JWT claims,
client certificates or session management. Rotation uses a new alias and
listener; close the previous listener to revoke its physical connections.
Registrations remain outside VM checkpoints for the Runtime lifetime. General
secret retention, native cache accounting, GUI/PostgreSQL integration,
cooperative numerical kernels and remaining v2 acceptance work continue.

Validation includes rejection before VM capture, duplicate headers, scheme
handling, stripping, private body rejection, immutable and capacity-limited
configuration, memory admission and source-free record/replay/live SDK runs.

The loopback authentication fixture connects to the IPv4 listener directly while
keeping `localhost` as the certificate-verified TLS hostname and SNI. This avoids
accumulated IPv6 connection-refusal delays during its consecutive denial probes
on Windows; certificate and authentication verification remain enabled.
