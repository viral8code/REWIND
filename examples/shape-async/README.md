# Cooperative reshape and broadcast contraction

Run `rewind run main.rw`. The program contracts a reversed broadcast view,
restores an unfinished task, copies a logical view and cancels a second copy.
Expected output is `true`, `true`, `shape done` on separate lines.

Dimension Lists are owned task arguments: pass a constructor result, as in this
example, or use `move shape`. Numeric inputs share immutable native pages.
At most 4096 cells are processed per native step; work/memory budgets apply.
