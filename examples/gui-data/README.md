# GUI data workflow

```sh
rewind run main.rw --allow-effects gui,external,network,db,tasks,env,fileRead,output -- sqlite http://127.0.0.1:8080/data "Data editor"
```

The HTTP endpoint returns JSON such as `{"text":"日本語"}`. The application
opens a main window and a saved-data window. An HTTP failure can be retried with
Retry; a successful result is saved with bound parameters in a DB transaction.
Undo restores the initial VM view and reads the already committed row back from
the database. It does not undo the DB transaction. Reload starts a new request;
Cancel remains available while it waits. Closing either window exits.

SQLite uses `state.sqlite`. For PostgreSQL, change the first argument to
`postgres`, supply `REWIND_PG_DSN` as secret environment input with
`--secret-env REWIND_PG_DSN`, and place the trusted DER certificate in `ca.der`.
The SDK PostgreSQL example explains the verified TLS configuration. Both
backends use a `rewind_gui_data` table with the window title as its unique entry
key; use a new title for a new run. Writes and physical requests are external
effects. A request cancellation does not guarantee remote cancellation.

The checkpoint is created in the scope containing the whole interaction loop.
Undo restores its VM values while the loop continues, then drops that checkpoint.
New DB/HTTP operations explicitly use `external fresh`; restored receipts never
repeat a write. A committed transaction can have an unknown outcome on connection
failure, so this example reports DB errors rather than automatically retrying writes.

Release tests use native Win32/X11 pointer input and real HTTP/SQLite/PostgreSQL:
503, Retry, JSON save, Undo, Reload and Cancel. They independently verify the
saved DB row after connections close. Source execution and compiled execution
are tested; compiled debug/compact replay works after source/cache removal,
without the HTTP endpoint, DB connection or GUI display. The source-run trace
retains its source entry until its own replay finishes.
