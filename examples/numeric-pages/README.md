# Numeric output pages

Run `rewind run main.rw` with an installed SDK. This example checks reversed
views, float map and zip, integer zip, copy-on-write and checkpoint restore.
The VM memory admission contract is unchanged. The implementation builds these
results directly in native output pages, avoiding a contiguous output scratch
buffer. Synchronous calls remain synchronous.
