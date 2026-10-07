# Cooperative vector operations

Run `rewind run main.rw --native-work 100000000` in this directory.
`std.numericAsync.scale`, `zipFloat` and `norm2` yield between native steps of
at most 4096 values. Broadcast and reverse-stride inputs share their pages.
This example checks progress of another task, a checkpoint during computation,
published output before and after revert, synchronous reference results and
cancellation. Expected output is `98304`, `98304`, then `vector done`.

All operations are pure VM computations. No external worker or I/O is started.
The default numeric array size limit and fatal execution/memory budgets still
apply. See the v1.9.31 contract for stable norm accumulation and failure rules.
