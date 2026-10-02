# decimal-postgres

Requires REWIND 1.8.3 or later. See [contracts](../../docs/REWIND_v1.8.3.md).

Set REWIND_PG_DSN as a secret environment value and place the trusted DER CA certificate at ca.der. Run `rewind run main.rw --allow-effects external,db,tasks,env,fileRead --secret-env REWIND_PG_DSN`. Prints 0.3 after real NUMERIC addition.
