# Shared Bytes admission

Run `rewind run main.rw --history-memory 8MiB --steps 20000000 --native-work 100000000`.
One MiB of encoded bytes referenced 32 times fits the budget. The saved list
restores to empty. Expected output is `32` followed by `0`.
Shared buffer capacity is admitted once, while container bookkeeping still counts.
