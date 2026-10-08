# Numeric content identity and checkpoints

Run `rewind run main.rw --native-work 10000000`, or compile with
`rewind compile main.rw` and run the generated `main.rwc` with the same budget.
The example retains an original autodiff parameter, performs 128 native array
updates, verifies the new and old values, then restores and drops a checkpoint.
It prints `127` and `0`. Numeric buffers remain shared until a touched page
changes; cached content identity never replaces actual snapshot ownership.
