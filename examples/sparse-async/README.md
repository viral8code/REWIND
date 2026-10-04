# Cooperative sparse matrix-vector product

Run `rewind run main.rw --native-work 100000000`, or compile with
`rewind compile main.rw` and run `rewind run main.rwc --native-work 100000000`.

The example processes a 16384-row empty CSR matrix and a finite right vector,
checks that another task finishes first, restores the computation from a
checkpoint, and cancels a separate computation without receiving partial output.
Successful execution prints `sparse done`. No network or external DB is required.
See [contracts](../../docs/REWIND_v1.9.27.md) for numerical and budget limits.
