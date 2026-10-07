# REWIND v1.9.29 — TLS HTTP server

This patch adds `std.httpServer.tlsCredential` and `configuredTls`. Existing
plain HTTP APIs and their operation records remain compatible. These new
primitives require language 1.9.29 and `external,network,tasks` effects.

`tlsCredential(alias, certificate, key)` registers a PEM certificate chain and
a matching `Secret<String>` PEM private key inside an external boundary. Both
inputs are limited to 64KiB, with at most eight certificates, sixteen immutable
aliases and 512KiB reserved per configuration. Re-registering identical bytes
is idempotent; changing a registered configuration returns
`HttpServerTlsCredentialImmutable`. Invalid PEM or mismatched keys return
`HttpServerTlsCredential`. These failures have phase `NotSent` and status zero.
Private keys stay in native configuration outside checkpoints. They are never
serialized into listener operations; the public alias is their identifier.

`configuredTls(alias,address,port,maxBodyBytes,maxConnections,lifetimeMillis,deadlineMillis)`
has the existing configured listener limits. It uses mature Rustls TLS 1.2/1.3
with HTTP/1.1 ALPN. Every configured connection reserves an additional 4MiB for
TLS. Memory admission happens before binding. The whole-connection lifetime
starts at accept and includes handshake, request framing, body, application
reply and write. The connection permit is held during the handshake. Close
aborts stalled handshakes and requests; retired resources remain accounted
until task cancellation has completed.

Listener activation still waits for host polling. Restore of observed task
results uses the cached receipt, without rebinding or resending. Source-free
record/replay supplies the same public alias and a valid private configuration;
the matching private key must be re-injected for registration. Socket state, client receipt and
external effects cannot be restored by VM revert. Respond success means queued,
not delivery or client processing, and no automatic retry is introduced.

TLS encrypts transport; client certificate authentication and private request
authentication are not provided by this patch. Existing known-secret guards
continue to reject private request fields before exposing them to the VM.
Application authentication and safe removal of private headers from recorded
requests require a separate implementation. Certificate rotation requires a
new alias or Runtime. Global secret retention and native allocator accounting
remain part of the v2 audit.

Validation covers trusted HTTPS transport, client rejection of an untrusted server certificate, bounded
stalled handshakes, immutable private configuration, key redaction and cached
reply receipts. Distribution acceptance also requires the standard contracts,
API compatibility, both OS SDKs and source-free disconnected replay.
