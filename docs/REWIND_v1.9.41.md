# REWIND v1.9.41 — GUI, HTTP server and database integration

The SDK includes `service-data`, a finite native-window service using the existing
HTTP server, SQLite/PostgreSQL adapters, channels and task readiness selection.
The application task presents/publishes the GUI while an owned service task
accepts requests and awaits database operations. Worker messages are delivered
through a bounded channel. Readiness selection covers input, messages and service
completion without blocking the GUI thread or publishing from a worker.
When GUI input wins, the pending receive is cancelled and its result is still
inspected: a receive that already completed keeps its message. Simultaneous input
and notification must not silently discard a delivered channel value.

The first request inserts a row, encounters a duplicate-key failure and rolls back;
the second commits a row. Parameterized queries check the transaction's outcome.
Each cursor is explicitly closed and awaited before reusing its connection.
Dropping a cursor schedules cleanup; it does not guarantee cleanup has completed
before an immediately following operation. The existing typed `DbBusy` contract
remains. Server requests, the listener, database and native window are closed;
database cleanup is awaited at the end.

After service completion, a view checkpoint is restored and the completed task
is awaited again. The task receipt returns its existing result. Committed database
rows remain and host requests/transactions are not restarted. The sample does not
promise restoration of closed sockets or database connections.

## SDK acceptance

`scripts/smoke-service-data-sdk.py` runs SQLite and a real verified-TLS PostgreSQL
fixture, each through source execution and source-free debug/compact artifacts.
The fixtures add a bounded slow database query and click the native window while
the service is running. They verify rollback, commit and durable rows using an
independent database client, exact HTTP responses,
zero final native resources and no published window.

Replay runs after source removal for both compiled cases, with the SQLite file
or PostgreSQL CA removed, DISPLAY unavailable and a disconnected PostgreSQL DSN.
No database is recreated and no listener is opened. The private DSN must be absent
from the exported record. The source-execution case retains its source for replay.
Both extracted Linux and Windows SDK release checks run this acceptance.

This is an integration and distribution improvement built on the existing APIs.
General memory/secret accounting, larger-data capacity, remaining GUI functions,
cooperative least squares/autodiff and sustained performance acceptance remain
part of the v2 plan. This patch does not establish v2 completion.
