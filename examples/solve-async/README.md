# Cooperative linear solve

Run `rewind run main.rw --native-work 100000000` in this directory.
Expected output: `solved`, `restored`, `solve done`.

`std.numericAsync.solve(matrix,rhs,tolerance)` returns a cold
`Task<Result<FloatArray,StdError>>`. The outer `await` result reports task
failure; the inner result reports numeric failure. The matrix must be square
Float64 and the right-hand side rank one. The relative pivot tolerance must be
finite and nonnegative. This sample uses a permuted diagonally dominant system
with a row swap, allows another task to finish, checkpoints a running solve,
restores it after published output, and cancels another solve.

The solver runs on the VM thread in bounded pure steps. It shares input storage,
keeps private copy-on-write pages and exposes only completed output. Checkpoints
retain those pages until dropped. No worker or external operation is started.
See the v1.9.33 contract for budgets, numerical order and remaining limitations.
