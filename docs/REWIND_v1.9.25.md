# REWIND v1.9.25: HTTP server and exact routes

This version adds `std.httpServer` and `std.httpRouter`. HTTP/1.1 parsing and
framing use Hyper. The adapter binds numeric IPv4/IPv6 addresses; port zero
selects an available port. Server-side TLS is not provided in this version.

`listen(address,port,deadlineMillis)` defaults to 65536 body bytes, eight
connections and a 30000 ms whole-connection lifetime. `configured` accepts
body bytes (1..1048576), connection count (1..8), lifetime (1..120000 ms), and
an operation deadline (1..120000 ms). Up to four listeners and eight native
pending/retiring operations are admitted per runtime. Headers are limited to
128 entries and 32768 bytes, URI targets to 8192 bytes. Admission reserves
conservative buffer costs before binding or accepting application operations.
The lifetime starts at accept and includes headers, body, application waiting
and physical response writes; keep-alive does not extend it.

`next(&mut server,deadlineMillis)` permits one waiting consumer per listener.
Cancellation or deadline expiry ends that wait and leaves the listener open.
A returned affine request exposes method, target, path, query, binary headers
and body. URI strings retain percent encoding. `respond` consumes one response
slot; success means queued, not delivered or processed by the peer. Statuses
200..599 are accepted; 204/304 must have empty bodies. Framing headers
`connection`, `content-length`, `transfer-encoding` and `upgrade` are managed
by the transport and cannot be supplied by the application. Header names and
values are validated before consuming the slot. HEAD framing is handled by Hyper.

`closeRequest` releases a request without an application response; a connected
peer receives 503. Explicit server close or scope exit aborts all its physical
connections and pending waits. Completed receipts survive VM checkpoints,
but handles restored by `revert` do not reopen listeners or resend responses.
Recorded execution replays observations without binding or contacting peers.
`external live` keeps observations only while their future leases remain alive;
a request loop can use it without accumulating recorded history.

The transport automatically rejects malformed/oversized requests and closes
expired connections. These protocol actions are physical effects, outside VM
rollback. No external effect is automatically retried. Known private values in
public outgoing bodies/headers are rejected before fingerprinting. Incoming
method, URI, headers and body are checked against the current private-value
registry before being exposed or recorded. Private request authentication and
server TLS credential aliases require a separate adapter extension.

`HttpServer`, `HttpServerRequest` and `HttpServerError` are reserved native types,
available from language 1.9.25. Listener and request ownership is affine; library
calls require `external`, `network` and `tasks`. Both recorded and live submission
require an explicit external region.

`std.httpRouter.Route(method,path,handler)` is an immutable value. `find` returns
the first exact path/method match (`*` accepts any method). `allows` reports
whether a path has a route, helping distinguish 404 from 405. Routing is pure;
there is no implicit URI decoding, prefix matching or regular-expression work.

The SDK includes an echo example, reference and both module API snapshots.
The next stages remain numerical-kernel scheduling, general container/snapshot
memory accounting, application integration and the final v2 acceptance checks.
