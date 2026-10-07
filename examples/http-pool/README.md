# Bounded HTTP connection reuse

Pass one or more URLs returning status 200 and a two-byte body:
`rewind run main.rw --allow-effects external,network,tasks -- URL URL ...`.
The example sends a fresh GET for each argument. Successful requests print
`requests done`; use `rewind profile` to see cached/retained client counts and
their memory reservation.

The SDK verification fixture uses 17 independent loopback origins. It checks
same-origin keepalive, idle connection closure after eviction, re-opening an
evicted origin and source-free disconnected replay in both record modes.
The client cache is native state and has a maximum of 16 origin/CA pairs.
An active driver or stream holds its own client lease after eviction. Revert
does not reopen a physical connection. See the v1.9.34 contract for memory
admission, trust validation and the limits of profiling.
