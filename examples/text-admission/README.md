# Shared text admission

Run `rewind run main.rw --history-memory 8MiB --steps 20000000 --native-work 100000000`.
Expected output: `32`, then `0`. Thirty-two references to the same one-MiB text share
its buffer. A checkpoint restores the empty list without copying the shared text.
Use `rewind profile main.rw` with the same budgets to inspect `shared_payloads`.
Independent texts and edited copies remain separate allocations.
