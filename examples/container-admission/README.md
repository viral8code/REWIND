# Shared container admission

Run `rewind run main.rw`. Expected output is `31` followed by `0`.
To demonstrate bounded admission use `--history-memory 8MiB --steps 20000000
--native-work 100000000`. The list retains 32 changed checkpoint roots while
sharing unchanged pages and values. Checkpoints still keep their own changes.
