# decimal-sqlite

Requires REWIND 1.8.3 or later. See [contracts](../../docs/REWIND_v1.8.3.md).

Run `rewind run main.rw --allow-effects external,db,tasks`. Uses BLOB storage in a NUMERIC affinity column and prints 12345678901234567890.12340000 exactly.
