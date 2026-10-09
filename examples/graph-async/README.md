# Cooperative directed graph traversal

Run `rewind run main.rw`. The program builds and traverses a directed chain,
checks an unreachable vertex, restores an unfinished task checkpoint and cancels
a second task. Graph endpoints are immutable shared native integer arrays.
Expected output is `true`, `true`, `graph done` on separate lines.

The vertex/edge limit is 1,048,576; native work, task and memory budgets apply.
