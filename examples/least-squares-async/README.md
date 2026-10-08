# Cooperative least squares

Run `rewind run main.rw --steps 20000000 --task-steps 2000000 --native-work 1000000000`. This deliberately computes and restores multiple factorizations, so the example supplies an explicit total work budget. The task solves a full-column-rank rectangular system while another task runs, restores an unfinished checkpoint, and cancels a separate unfinished computation. Expected output: `solved`, `restored`, `least squares done`.

`std.numericAsync.leastSquares(matrix, right, tolerance)` accepts a FloatArray matrix with m >= n and a rank-one rhs of length m. Rank-deficient systems return NumericSingular. Fatal work and memory limits remain runtime errors.
